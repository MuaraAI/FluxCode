use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    menu::DropdownMenu,
    Icon, IconName, Selectable, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{agent_id, Item, Status};

#[derive(Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub repo: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Account {
    pub name: String,
    pub family: String,
    pub dir: PathBuf,
    #[serde(default)]
    pub flow_profile: Option<String>,
}

pub fn family(id: &str) -> Option<&'static str> {
    if id.starts_with("claude") {
        Some("claude")
    } else if crate::acp::is_opencode(id) {
        Some("opencode")
    } else if id == "antigravity-acp" || id.starts_with("antigravity") {
        Some("antigravity")
    } else {
        None
    }
}

pub fn signed_in(a: &Account) -> bool {
    let has = |p: PathBuf, any: &[&str]| {
        std::fs::read_to_string(p).is_ok_and(|s| any.iter().any(|k| s.contains(k)))
    };
    match a.family.as_str() {
        "claude" => {
            a.dir.join(".credentials.json").is_file()
                || has(
                    a.dir.join(".claude.json"),
                    &["\"oauthAccount\"", "\"primaryApiKey\""],
                )
        }
        "antigravity" => {
            let gem = a.dir.join(".gemini");
            let has_gem = gem.join("oauth_creds.json").is_file()
                || gem.join("google_accounts.json").is_file()
                || gem.join("credentials.json").is_file()
                || gem.join("antigravity_state.pbtxt").is_file()
                || gem.join("antigravity-cli").join("settings.json").is_file()
                || gem.join(".fluxcode-account-ready").is_file();
            let has_flow = a.flow_profile.as_deref().is_some_and(|fp| {
                crate::gflow::discover_profiles()
                    .into_iter()
                    .any(|p| p.name == fp && p.cookies_present)
            });
            has_gem || has_flow
        }
        _ => has(a.dir.join("opencode").join("auth.json"), &["\"type\""]),
    }
}

pub fn prune_accounts(s: &mut crate::settings::Settings) {
    let (keep, gone): (Vec<Account>, Vec<Account>) =
        s.agent_accounts.drain(..).partition(signed_in);
    s.agent_accounts = keep;
    if gone.is_empty() {
        return;
    }
    for a in &gone {
        forget_dir(a);
    }
    let accounts = s.agent_accounts.clone();
    s.agent_account
        .retain(|f, name| accounts.iter().any(|a| &a.family == f && &a.name == name));
    s.save();
}

fn forget_dir(a: &Account) {
    if a.dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|n| n == "accounts")
    {
        let _ = std::fs::remove_dir_all(&a.dir);
    }
}

pub fn account_envs(a: &Account) -> Vec<(&'static str, PathBuf)> {
    match a.family.as_str() {
        "claude" => vec![("CLAUDE_CONFIG_DIR", a.dir.clone())],
        "antigravity" => {
            let mut vars = vec![
                ("USERPROFILE", a.dir.clone()),
                ("HOME", a.dir.clone()),
                ("GEMINI_CLI_HOME", a.dir.join(".gemini")),
                ("AGY_HOME", a.dir.join(".gemini").join("antigravity-cli")),
                ("XDG_CONFIG_HOME", a.dir.join(".config")),
                ("XDG_DATA_HOME", a.dir.join(".local").join("share")),
            ];
            if let Some(fp) = &a.flow_profile {
                if !fp.is_empty() {
                    vars.push(("GFLOW_CLI_PROFILE", PathBuf::from(fp)));
                }
            }
            vars
        }
        _ => vec![("XDG_DATA_HOME", a.dir.clone())],
    }
}

pub fn account_env(a: &Account) -> (&'static str, PathBuf) {
    account_envs(a)
        .into_iter()
        .next()
        .unwrap_or(("XDG_DATA_HOME", a.dir.clone()))
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(["-c", "core.longpaths=true"])
        .args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() {
        Ok(text.trim().to_string())
    } else {
        Err(text.trim().to_string())
    }
}

fn create(repo: &Path) -> Result<Worktree, String> {
    let top = PathBuf::from(git(repo, &["rev-parse", "--show-toplevel"])?);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let branch = format!("fluxcode/agent-{stamp}");
    let path = top.join(".fluxcode").join("worktrees").join(&stamp);
    if let Ok(common) = git(&top, &["rev-parse", "--git-common-dir"]) {
        let common = if Path::new(&common).is_absolute() {
            PathBuf::from(common)
        } else {
            top.join(common)
        };
        let exclude = common.join("info").join("exclude");
        let old = std::fs::read_to_string(&exclude).unwrap_or_default();
        if !old.lines().any(|l| l.trim() == "/.fluxcode/") {
            let _ = std::fs::create_dir_all(exclude.parent().unwrap_or(&common));
            let _ = std::fs::write(
                &exclude,
                format!(
                    "{old}{}/.fluxcode/\n",
                    if old.is_empty() || old.ends_with('\n') {
                        ""
                    } else {
                        "\n"
                    }
                ),
            );
        }
    }
    let p = path.to_string_lossy().into_owned();
    git(&top, &["worktree", "add", "-b", &branch, &p, "HEAD"])?;
    if let Ok(rd) = std::fs::read_dir(&top) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with(".env") && e.path().is_file() {
                let _ = std::fs::copy(e.path(), path.join(&n));
            }
        }
    }
    Ok(Worktree {
        path,
        branch,
        repo: top,
    })
}

fn merge(w: &Worktree, title: &str) -> Result<String, String> {
    let wt = &w.path;
    git(wt, &["add", "-A"])?;
    if !git(wt, &["status", "--porcelain"])?.is_empty() {
        let msg = format!(
            "FluxCode agent: {}",
            if title.is_empty() { "changes" } else { title }
        );
        git(wt, &["commit", "-m", &msg])?;
    }
    let out = git(&w.repo, &["merge", "--no-ff", "--no-edit", &w.branch])?;
    Ok(out.lines().last().unwrap_or("").to_string())
}

fn remove(w: &Worktree) -> Result<(), String> {
    let p = w.path.to_string_lossy().into_owned();
    git(&w.repo, &["worktree", "remove", "--force", &p])?;
    let _ = git(&w.repo, &["branch", "-D", &w.branch]);
    let _ = git(&w.repo, &["worktree", "prune"]);
    Ok(())
}

impl Workbench {
    pub(crate) fn agent_cwd(&self) -> PathBuf {
        match &self.agent.worktree {
            Some(w) => w.path.clone(),
            None => self
                .agent
                .folder
                .clone()
                .or_else(|| self.root.clone())
                .unwrap_or_else(|| self.terms[self.term_ix].cwd.clone()),
        }
    }

    pub(crate) fn agent_use_folder(&mut self, folder: Option<PathBuf>, cx: &mut Context<Self>) {
        let folder = folder.or_else(|| self.root.clone());
        if folder == self.agent.folder {
            return;
        }
        if self.agent.worktree.is_some() {
            self.agent.items.push(Item::Note(
                tr("This chat works in its own worktree: remove it first to pick another folder.")
                    .into(),
                true,
            ));
            return cx.notify();
        }
        self.agent.folder = folder.clone();
        if self.agent.items.iter().any(|i| matches!(i, Item::User(_))) {
            let name = folder
                .as_deref()
                .map(|f| super::chats::folder_name(&f.display().to_string()))
                .unwrap_or_default();
            self.agent.items.push(Item::Branch(
                crate::trf!("Now working in {folder}."; folder => name),
            ));
        }
        if self.agent.live() {
            self.agent_restart_here(cx);
        }
        cx.notify();
    }

    pub(crate) fn agent_pick_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Work in this folder").into()),
        });
        let key = self.agent.key;
        cx.spawn_in(window, async move |this, cx| {
            let picked = paths.await.ok()?.ok()??.into_iter().next()?;
            this.update(cx, |this, cx| {
                this.agent_with_run(key, cx, |this, cx| this.agent_use_folder(Some(picked), cx));
            })
            .ok()
        })
        .detach();
    }

    pub(crate) fn agent_other_folder(&self) -> Option<PathBuf> {
        self.agent
            .folder
            .clone()
            .filter(|f| self.agent.worktree.is_none() && Some(f) != self.root.as_ref())
    }

    fn agent_restart_here(&mut self, cx: &mut Context<Self>) {
        let was_on = matches!(
            self.agent.status,
            Status::Starting | Status::Ready | Status::Busy
        );
        if self.agent.items.iter().any(|i| matches!(i, Item::User(_))) {
            self.agent.carry = Some(super::transcript(&self.agent.items));
        }
        self.agent_save();
        self.agent_stop(cx);
        if was_on {
            self.agent_start(cx);
        }
    }

    fn agent_worktree_toggle(&mut self, cx: &mut Context<Self>) {
        if let Some(w) = self.agent.worktree.clone() {
            self.agent_worktree_remove(w, cx);
            return;
        }
        let Some(repo) = self.agent.folder.clone().or_else(|| self.root.clone()) else {
            return;
        };
        let key = self.agent.key;
        if self.wsl.is_some() {
            self.agent.items.push(Item::Note(
                tr("A worktree inside a WSL folder isn't supported yet.").into(),
                true,
            ));
            cx.notify();
            return;
        }
        if self.scm.repo.is_none() && self.root.as_ref() == Some(&repo) {
            self.agent.items.push(Item::Note(tr("A worktree needs a git repository: this folder isn't one yet (Source Control can make it one).").into(), true));
            cx.notify();
            return;
        }
        self.agent.worktree_busy = true;
        cx.notify();
        let task = cx.background_spawn(async move { create(&repo) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| this.agent_with_run(key, cx, |this, cx| {
                this.agent.worktree_busy = false;
                match r {
                    Ok(w) => {
                        this.agent.items.push(Item::Branch(crate::trf!("Working in its own worktree on branch {b}. Your checkout stays as it is until you merge."; b => w.branch)));
                        this.agent.worktree = Some(w);
                        this.agent_restart_here(cx);
                    }
                    Err(e) => this.agent.items.push(Item::Note(crate::trf!("Couldn't make a worktree: {e}"; e => e), true)),
                }
                cx.notify();
            }));
        })
        .detach();
    }

    fn agent_worktree_merge(&mut self, cx: &mut Context<Self>) {
        let Some(w) = self.agent.worktree.clone() else {
            return;
        };
        let key = self.agent.key;
        let title = self
            .agent
            .items
            .iter()
            .find_map(|i| {
                if let Item::User(t) = i {
                    Some(t.chars().take(60).collect::<String>())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        self.agent.worktree_busy = true;
        cx.notify();
        let task = cx.background_spawn(async move { merge(&w, &title) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| {
                this.agent_with_run(key, cx, |this, cx| {
                    this.agent.worktree_busy = false;
                    match r {
                        Ok(_) => {
                            this.agent.items.push(Item::Note(
                                tr("Merged the agent's branch into your checkout.").into(),
                                false,
                            ));
                            this.agent.changes.clear();
                            this.scm_refresh(cx);
                        }
                        Err(e) => this
                            .agent
                            .items
                            .push(Item::Note(crate::trf!("Merge stopped: {e}"; e => e), true)),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn agent_worktree_remove(&mut self, w: Worktree, cx: &mut Context<Self>) {
        let key = self.agent.key;
        self.agent.worktree_busy = true;
        self.agent.worktree = None;
        self.agent.changes.clear();
        self.agent_restart_here(cx);
        let task = cx.background_spawn(async move { remove(&w) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| {
                this.agent_with_run(key, cx, |this, cx| {
                    this.agent.worktree_busy = false;
                    let note = match r {
                        Ok(()) => Item::Branch(
                            tr("Back in your checkout; the worktree is removed.").into(),
                        ),
                        Err(e) => Item::Note(
                            crate::trf!("Couldn't remove the worktree: {e}"; e => e),
                            true,
                        ),
                    };
                    this.agent.items.push(note);
                    cx.notify();
                })
            });
        })
        .detach();
    }

    pub(crate) fn agent_accounts(&self) -> (Option<&'static str>, Vec<Account>) {
        let fam = family(&agent_id());
        let list = fam
            .map(|f| {
                self.settings
                    .agent_accounts
                    .iter()
                    .filter(|a| a.family == f)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        (fam, list)
    }

    pub(crate) fn agent_account(&self) -> Option<Account> {
        let f = family(&agent_id())?;
        let name = match &self.agent.account {
            Some(n) if n.is_empty() => return None,
            Some(n) => n,
            None => self.settings.agent_account.get(f)?,
        };
        self.settings
            .agent_accounts
            .iter()
            .find(|a| a.family == f && &a.name == name)
            .cloned()
    }

    pub(crate) fn agent_use_account(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        self.agent.account = Some(name.clone().unwrap_or_default());
        match name {
            Some(n) => self.settings.agent_account.insert(f.to_string(), n),
            None => self.settings.agent_account.remove(f),
        };
        self.settings.save();
        let active = self.agent_account();
        if f == "antigravity" && self.settings.agent_gflow_sync {
            if let Some(fp) = active.as_ref().and_then(|a| a.flow_profile.clone()) {
                let st = crate::gflow::load_state();
                if st.mode == crate::gflow::RoutingMode::Sticky
                    && crate::gflow::discover_profiles()
                        .iter()
                        .any(|p| p.name == fp)
                {
                    let _ = crate::gflow::set_mode(
                        crate::gflow::RoutingMode::Sticky,
                        Some(fp),
                        None,
                        None,
                    );
                }
            }
        }
        let note = match active {
            Some(a) => crate::trf!("Now using the account {name}."; name => a.name),
            None => tr("Now using the default account.").into(),
        };
        self.agent.items.push(Item::Branch(note));
        self.agent_restart_here(cx);
        cx.notify();
    }

    pub(crate) fn agent_add_account(&mut self, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        if self.agent.account_wait.is_some() {
            return;
        }
        prune_accounts(&mut self.settings);
        let n = self
            .settings
            .agent_accounts
            .iter()
            .filter(|a| a.family == f)
            .count()
            + 2;
        let name = crate::trf!("Account {n}"; n => n);
        let base = std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let dir = base
            .join("FluxCode")
            .join("accounts")
            .join(format!("{f}-{n}"));
        let _ = std::fs::create_dir_all(&dir);
        let acc = Account {
            name,
            family: f.into(),
            dir,
            flow_profile: (f == "antigravity").then(|| format!("akun{n}")),
        };
        open_login(&acc);
        self.agent.account_wait = Some(acc.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            for _ in 0..450 {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let Ok(waiting) =
                    this.update(cx, |this, _| this.agent.account_wait.as_ref() == Some(&acc))
                else {
                    return;
                };
                if !waiting {
                    return;
                }
                let a = acc.clone();
                if cx.background_spawn(async move { signed_in(&a) }).await {
                    _ = this.update(cx, |this, cx| {
                        this.agent.account_wait = None;
                        this.settings.agent_accounts.push(acc.clone());
                        this.agent_use_account(Some(acc.name.clone()), cx);
                        this.agent.items.push(Item::Branch(
                            crate::trf!("Signed in: {name} added and in use."; name => acc.name),
                        ));
                    });
                    return;
                }
            }
            _ = this.update(cx, |this, cx| this.agent_cancel_account(cx));
        })
        .detach();
    }

    pub(crate) fn agent_cancel_account(&mut self, cx: &mut Context<Self>) {
        let Some(a) = self.agent.account_wait.take() else {
            return;
        };
        if signed_in(&a) {
            self.settings.agent_accounts.push(a.clone());
            return self.agent_use_account(Some(a.name), cx);
        }
        forget_dir(&a);
        self.agent.items.push(Item::Branch(
            crate::trf!("{name} wasn't signed in, so it wasn't added."; name => a.name),
        ));
        cx.notify();
    }

    pub(crate) fn agent_sign_out(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        let Some(pos) = self
            .settings
            .agent_accounts
            .iter()
            .position(|a| a.family == f && a.name == name)
        else {
            return;
        };
        let a = self.settings.agent_accounts.remove(pos);
        let was_current = self.settings.agent_account.get(f) == Some(&name);
        if was_current {
            self.settings.agent_account.remove(f);
        }
        self.settings.save();
        if was_current {
            self.agent.items.push(Item::Branch(
                crate::trf!("Signed out of {name}; using the default account."; name => name),
            ));
            self.agent_restart_here(cx);
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_secs(4));
                forget_dir(&a);
            })
            .detach();
        } else {
            forget_dir(&a);
            self.agent.items.push(Item::Branch(
                crate::trf!("Signed out of {name}."; name => name),
            ));
        }
        cx.notify();
    }

    pub(crate) fn render_agent_more(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.agent.worktree.is_some();
        let (fam, accounts) = self.agent_accounts();
        let current = self.agent_account().map(|a| a.name);
        let waiting = self.agent.account_wait.as_ref().map(|a| a.name.clone());
        let mode = self
            .agent_config_view()
            .into_iter()
            .find(|o| o.category == "mode")
            .filter(|o| o.choices.len() > 1);
        let right = self.settings.agent_right;
        let other = self.agent_other_folder();
        let open_name = self
            .root
            .as_ref()
            .map(|r| super::chats::folder_name(&r.display().to_string()));
        let risky = mode.as_ref().is_some_and(|m| {
            let c = m.current.to_ascii_lowercase();
            c.contains("bypass")
                || c.contains("yolo")
                || c.contains("dontask")
                || c.contains("dont_ask")
        });
        let weak = cx.entity().downgrade();
        Button::new("ag-more")
            .icon(Icon::new(IconName::Ellipsis).when(risky, |i| i.text_color(c(theme::WARN))))
            .xsmall()
            .ghost()
            .selected(on || current.is_some() || other.is_some())
            .loading(self.agent.worktree_busy)
            .tooltip(match &mode {
                Some(m) => crate::trf!("More · mode: {m}"; m => m.current_name()),
                None => tr("More").to_string(),
            })
            .dropdown_menu(move |menu, _, _| {
                let act = |label: String, checked: bool, f: Box<dyn Fn(&mut Workbench, &mut Window, &mut Context<Workbench>)>| {
                    let weak = weak.clone();
                    gpui_kit::component::menu::PopupMenuItem::new(label).checked(checked).on_click(move |_, w, cx| {
                        _ = weak.update(cx, |this, cx| f(this, w, cx));
                    })
                };
                let mut m = menu.min_w(px(240.));
                if let Some(o) = &mode {
                    m = m.label(tr("Mode"));
                    for (v, name, _) in &o.choices {
                        let (id, v) = (o.id.clone(), v.clone());
                        m = m.item(act(name.clone(), *v == o.current, Box::new(move |this, _, cx| this.agent_set_option(&id, v.clone(), true, cx))));
                    }
                    m = m.separator();
                }
                m = m.label(tr("Folder"));
                if let Some(n) = &open_name {
                    m = m.item(act(crate::trf!("The open folder ({name})"; name => n), other.is_none(), Box::new(|this, _, cx| this.agent_use_folder(None, cx))));
                }
                m = m.item(act(
                    match &other {
                        Some(f) => crate::trf!("{name} (pick another…)"; name => super::chats::folder_name(&f.display().to_string())),
                        None => tr("Another folder…").into(),
                    },
                    other.is_some(),
                    Box::new(|this, w, cx| this.agent_pick_folder(w, cx)),
                ));
                m = m.separator();
                m = m.item(act(
                    if on { tr("Own worktree (on): remove it") } else { tr("Work in its own git worktree") }.into(),
                    on,
                    Box::new(|this, _, cx| this.agent_worktree_toggle(cx)),
                ));
                if fam.is_some() {
                    m = m.separator().label(tr("Account"));
                    m = m.item(act(tr("Default account").into(), current.is_none(), Box::new(|this, _, cx| this.agent_use_account(None, cx))));
                    for a in &accounts {
                        let name = a.name.clone();
                        m = m.item(act(a.name.clone(), current.as_deref() == Some(a.name.as_str()), Box::new(move |this, _, cx| this.agent_use_account(Some(name.clone()), cx))));
                    }
                    m = match &waiting {
                        Some(w) => m.item(act(crate::trf!("Waiting for {name} to sign in… (cancel)"; name => w), false, Box::new(|this, _, cx| this.agent_cancel_account(cx)))),
                        None => m.item(act(tr("Add account and sign in…").into(), false, Box::new(|this, _, cx| this.agent_add_account(cx)))),
                    };
                    for a in &accounts {
                        let name = a.name.clone();
                        m = m.item(act(crate::trf!("Sign out of {name}"; name => a.name), false, Box::new(move |this, _, cx| this.agent_sign_out(name.clone(), cx))));
                    }
                }
                m.separator()
                    .item(act(
                        if right { tr("Move to the left side bar") } else { tr("Move to the right side") }.into(),
                        false,
                        Box::new(|this, w, cx| this.agent_move_dock(w, cx)),
                    ))
                    .item(act(
                        tr("Agent settings: model, permissions, browser").into(),
                        false,
                        Box::new(|this, w, cx| {
                            if this.agent.show_options {
                                this.agent_close_options(w, cx);
                            } else {
                                this.agent_open_options(w, cx);
                            }
                        }),
                    ))
            })
    }

    pub(crate) fn render_worktree_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(f) = self.agent_other_folder() {
            let open = f.clone();
            return Some(
                h_flex()
                    .flex_none()
                    .mx_3()
                    .mb_1()
                    .px_2()
                    .h(theme::u(26.))
                    .gap_1p5()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_ELEVATED))
                    .text_xs()
                    .child(
                        Icon::new(IconName::Folder)
                            .flex_none()
                            .size(theme::u(12.))
                            .text_color(c(theme::ACCENT_TEXT)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(c(theme::FG))
                            .child(f.display().to_string()),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("ag-dir-open")
                            .label(tr("Open folder"))
                            .xsmall()
                            .ghost()
                            .on_click(move |_, _, cx| cx.reveal_path(&open)),
                    )
                    .child(
                        Button::new("ag-dir-back")
                            .label(tr("Use the open folder"))
                            .xsmall()
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, _, cx| this.agent_use_folder(None, cx)),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let w = self.agent.worktree.as_ref()?;
        let path = w.path.clone();
        Some(
            h_flex()
                .flex_none()
                .mx_3()
                .mb_1()
                .px_2()
                .h(theme::u(26.))
                .gap_1p5()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_ELEVATED))
                .text_xs()
                .child(
                    Icon::new(gpui_kit::assets::IconName::GitFork)
                        .flex_none()
                        .size(theme::u(12.))
                        .text_color(c(theme::ACCENT_TEXT)),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(c(theme::FG))
                        .child(w.branch.clone()),
                )
                .child(div().flex_1())
                .child(
                    Button::new("ag-wt-open")
                        .label(tr("Open folder"))
                        .xsmall()
                        .ghost()
                        .on_click(move |_, _, cx| cx.reveal_path(&path)),
                )
                .child(
                    Button::new("ag-wt-merge")
                        .icon(Icon::new(gpui_kit::assets::IconName::GitMerge))
                        .label(tr("Merge"))
                        .xsmall()
                        .ghost()
                        .loading(self.agent.worktree_busy)
                        .on_click(cx.listener(|this, _, _, cx| this.agent_worktree_merge(cx))),
                )
                .into_any_element(),
        )
    }
}

fn open_login(a: &Account) {
    if a.family == "antigravity" {
        let _ = std::fs::create_dir_all(a.dir.join(".gemini").join("antigravity-cli"));
        if let Some(fp) = &a.flow_profile {
            let _ = crate::gflow::launch_browser_login(fp);
        }
        let agy = crate::acp::locate("antigravity-acp")
            .map(|l| l.cmd)
            .or_else(|| crate::acp::which("agy"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let sets: Vec<String> = account_envs(a)
                .into_iter()
                .map(|(k, v)| format!("set \"{k}={}\"", v.display()))
                .collect();
            let env_str = sets.join(" && ");
            let cmd_tail = match agy {
                Some(bin) => format!("\"{}\" acp --login", bin.display()),
                None => "echo Isolated Antigravity dir ready && echo Run: agy acp --login".into(),
            };
            let line = format!(
                "/c start \"FluxCode Antigravity sign-in\" cmd /k \"{env_str} && {cmd_tail}\""
            );
            let _ = Command::new("cmd").raw_arg(line).spawn();
        }
        #[cfg(not(windows))]
        {
            let env_str: Vec<String> = account_envs(a)
                .into_iter()
                .map(|(k, v)| format!("{k}='{}'", v.display()))
                .collect();
            let bin = agy
                .map(|b| format!("'{}' acp --login", b.display()))
                .unwrap_or_else(|| "echo 'Run agy acp --login'".into());
            let script = format!("{} {bin}; exec $SHELL", env_str.join(" "));
            for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
                if Command::new(term)
                    .args(["-e", "sh", "-c", &script])
                    .spawn()
                    .is_ok()
                {
                    break;
                }
            }
        }
        return;
    }

    let (var, dir) = account_env(a);
    let login = if a.family == "claude" {
        "claude /login"
    } else {
        "opencode auth login"
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let line = format!(
            "/c start \"FluxCode sign-in\" cmd /k \"set \"{var}={}\" && {login}\"",
            dir.display()
        );
        let _ = Command::new("cmd").raw_arg(line).spawn();
    }
    #[cfg(not(windows))]
    {
        let script = format!("{var}='{}' {login}; exec $SHELL", dir.display());
        for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
            if Command::new(term)
                .args(["-e", "sh", "-c", &script])
                .spawn()
                .is_ok()
            {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{account_env, account_envs, family, signed_in, Account};
    use std::path::PathBuf;

    #[test]
    fn an_account_counts_only_once_signed_in() {
        let dir = std::env::temp_dir().join(format!("fluxcode-acct-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("opencode")).unwrap();
        std::fs::create_dir_all(dir.join(".gemini")).unwrap();
        let claude = Account {
            name: "A".into(),
            family: "claude".into(),
            dir: dir.clone(),
            flow_profile: None,
        };
        let open = Account {
            family: "opencode".into(),
            ..claude.clone()
        };
        let agy = Account {
            family: "antigravity".into(),
            flow_profile: Some("akun2".into()),
            ..claude.clone()
        };
        assert!(!signed_in(&claude));
        assert!(!signed_in(&open));
        assert!(!signed_in(&agy));
        std::fs::write(dir.join(".claude.json"), r#"{"numStartups":1}"#).unwrap();
        assert!(!signed_in(&claude));
        std::fs::write(
            dir.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"x"}}"#,
        )
        .unwrap();
        assert!(signed_in(&claude));
        std::fs::write(dir.join("opencode").join("auth.json"), "{}").unwrap();
        assert!(!signed_in(&open));
        std::fs::write(
            dir.join("opencode").join("auth.json"),
            r#"{"anthropic":{"type":"oauth"}}"#,
        )
        .unwrap();
        assert!(signed_in(&open));
        std::fs::write(dir.join(".gemini").join("oauth_creds.json"), "{}").unwrap();
        assert!(signed_in(&agy));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn families_and_env() {
        assert_eq!(family("claude-acp"), Some("claude"));
        assert_eq!(family("opencode-v2"), Some("opencode"));
        assert_eq!(family("antigravity-acp"), Some("antigravity"));
        assert_eq!(family("gemini"), None);
        let a = Account {
            name: "Work".into(),
            family: "claude".into(),
            dir: PathBuf::from("x"),
            flow_profile: None,
        };
        assert_eq!(account_env(&a).0, "CLAUDE_CONFIG_DIR");
        let o = Account {
            family: "opencode".into(),
            ..a.clone()
        };
        assert_eq!(account_env(&o).0, "XDG_DATA_HOME");
        let agy = Account {
            family: "antigravity".into(),
            flow_profile: Some("akun2".into()),
            ..a
        };
        let envs = account_envs(&agy);
        assert!(envs.iter().any(|(k, _)| *k == "GEMINI_CLI_HOME"));
        assert!(envs.iter().any(|(k, _)| *k == "AGY_HOME"));
        assert!(envs
            .iter()
            .any(|(k, v)| *k == "GFLOW_CLI_PROFILE" && v == &PathBuf::from("akun2")));
    }
}
