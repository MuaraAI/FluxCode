//! Isolation for the agent, so agents (and the user) don't run into each other.

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
    /// The checkout it was made from (where Merge lands).
    pub repo: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Account {
    pub name: String,
    /// "claude" or "opencode".
    pub family: String,
    pub dir: PathBuf,
}

/// Which sign-in folder an agent reads, or None for agents without accounts here.
pub fn family(id: &str) -> Option<&'static str> {
    if id.starts_with("claude") {
        Some("claude")
    } else if crate::acp::is_opencode(id) {
        Some("opencode")
    } else {
        None
    }
}

/// The account's folder holds a sign-in: Claude's credentials file (or an OAuth account or API key in its config), OpenCode's auth database with at least one provider.
pub fn signed_in(a: &Account) -> bool {
    let has = |p: PathBuf, any: &[&str]| std::fs::read_to_string(p).is_ok_and(|s| any.iter().any(|k| s.contains(k)));
    match a.family.as_str() {
        "claude" => a.dir.join(".credentials.json").is_file() || has(a.dir.join(".claude.json"), &["\"oauthAccount\"", "\"primaryApiKey\""]),
        _ => has(a.dir.join("opencode").join("auth.json"), &["\"type\""]),
    }
}

/// Drop added accounts that were never signed in to (their folders too), and a choice that pointed at one.
pub fn prune_accounts(s: &mut crate::settings::Settings) {
    let (keep, gone): (Vec<Account>, Vec<Account>) = s.agent_accounts.drain(..).partition(signed_in);
    s.agent_accounts = keep;
    if gone.is_empty() {
        return;
    }
    for a in &gone {
        forget_dir(a);
    }
    let accounts = s.agent_accounts.clone();
    s.agent_account.retain(|f, name| accounts.iter().any(|a| &a.family == f && &a.name == name));
    s.save();
}

/// Delete an added account's sign-in folder: only one FluxCode made itself (under its `accounts` folder), never a folder a hand-edited setting points at.
fn forget_dir(a: &Account) {
    if a.dir.parent().and_then(Path::file_name).is_some_and(|n| n == "accounts") {
        let _ = std::fs::remove_dir_all(&a.dir);
    }
}

/// The environment that makes a run use `a`.
pub fn account_env(a: &Account) -> (&'static str, PathBuf) {
    match a.family.as_str() {
        "claude" => ("CLAUDE_CONFIG_DIR", a.dir.clone()),
        _ => ("XDG_DATA_HOME", a.dir.clone()),
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(["-c", "core.longpaths=true"]).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text.trim().to_string())
    } else {
        Err(text.trim().to_string())
    }
}

/// Make the worktree (runs off the UI thread).
fn create(repo: &Path) -> Result<Worktree, String> {
    let top = PathBuf::from(git(repo, &["rev-parse", "--show-toplevel"])?);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let branch = format!("fluxcode/agent-{stamp}");
    let path = top.join(".fluxcode").join("worktrees").join(&stamp);
    // Never shows up as a change in the user's checkout.
    if let Ok(common) = git(&top, &["rev-parse", "--git-common-dir"]) {
        let common = if Path::new(&common).is_absolute() { PathBuf::from(common) } else { top.join(common) };
        let exclude = common.join("info").join("exclude");
        let old = std::fs::read_to_string(&exclude).unwrap_or_default();
        if !old.lines().any(|l| l.trim() == "/.fluxcode/") {
            let _ = std::fs::create_dir_all(exclude.parent().unwrap_or(&common));
            let _ = std::fs::write(&exclude, format!("{old}{}/.fluxcode/\n", if old.is_empty() || old.ends_with('\n') { "" } else { "\n" }));
        }
    }
    let p = path.to_string_lossy().into_owned();
    git(&top, &["worktree", "add", "-b", &branch, &p, "HEAD"])?;
    // Ignored files the project needs to run (.env, .env.local, ...).
    if let Ok(rd) = std::fs::read_dir(&top) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with(".env") && e.path().is_file() {
                let _ = std::fs::copy(e.path(), path.join(&n));
            }
        }
    }
    Ok(Worktree { path, branch, repo: top })
}

/// Commit what the agent did on its branch and merge the branch into the user's checkout.
fn merge(w: &Worktree, title: &str) -> Result<String, String> {
    let wt = &w.path;
    git(wt, &["add", "-A"])?;
    if !git(wt, &["status", "--porcelain"])?.is_empty() {
        let msg = format!("FluxCode agent: {}", if title.is_empty() { "changes" } else { title });
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
    /// Where the agent works: its worktree, else the open folder.
    pub(crate) fn agent_cwd(&self) -> PathBuf {
        match &self.agent.worktree {
            Some(w) => w.path.clone(),
            None => self.agent.folder.clone().or_else(|| self.root.clone()).unwrap_or_else(|| self.term_cwd.clone()),
        }
    }

    /// Work in `folder` (None: the folder open in the editor).
    pub(crate) fn agent_use_folder(&mut self, folder: Option<PathBuf>, cx: &mut Context<Self>) {
        let folder = folder.or_else(|| self.root.clone());
        if folder == self.agent.folder {
            return;
        }
        if self.agent.worktree.is_some() {
            self.agent.items.push(Item::Note(tr("This chat works in its own worktree: remove it first to pick another folder.").into(), true));
            return cx.notify();
        }
        self.agent.folder = folder.clone();
        if self.agent.items.iter().any(|i| matches!(i, Item::User(_))) {
            let name = folder.as_deref().map(|f| super::chats::folder_name(&f.display().to_string())).unwrap_or_default();
            self.agent.items.push(Item::Branch(crate::trf!("Now working in {folder}."; folder => name)));
        }
        if self.agent.live() {
            self.agent_restart_here(cx);
        }
        cx.notify();
    }

    /// Pick the chat's folder with the system dialog.
    pub(crate) fn agent_pick_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(tr("Work in this folder").into()) });
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

    /// The chat works somewhere other than the open folder.
    pub(crate) fn agent_other_folder(&self) -> Option<PathBuf> {
        self.agent.folder.clone().filter(|f| self.agent.worktree.is_none() && Some(f) != self.root.as_ref())
    }

    /// Restart the agent in a new place (or as another account); the chat goes on.
    fn agent_restart_here(&mut self, cx: &mut Context<Self>) {
        let was_on = matches!(self.agent.status, Status::Starting | Status::Ready | Status::Busy);
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
        let Some(repo) = self.agent.folder.clone().or_else(|| self.root.clone()) else { return };
        let key = self.agent.key;
        if self.wsl.is_some() {
            self.agent.items.push(Item::Note(tr("A worktree inside a WSL folder isn't supported yet.").into(), true));
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
        let Some(w) = self.agent.worktree.clone() else { return };
        let key = self.agent.key;
        let title = self.agent.items.iter().find_map(|i| if let Item::User(t) = i { Some(t.chars().take(60).collect::<String>()) } else { None }).unwrap_or_default();
        self.agent.worktree_busy = true;
        cx.notify();
        let task = cx.background_spawn(async move { merge(&w, &title) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| this.agent_with_run(key, cx, |this, cx| {
                this.agent.worktree_busy = false;
                match r {
                    Ok(_) => {
                        this.agent.items.push(Item::Note(tr("Merged the agent's branch into your checkout.").into(), false));
                        this.agent.changes.clear();
                        this.scm_refresh(cx);
                    }
                    Err(e) => this.agent.items.push(Item::Note(crate::trf!("Merge stopped: {e}"; e => e), true)),
                }
                cx.notify();
            }));
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
            _ = this.update(cx, |this, cx| this.agent_with_run(key, cx, |this, cx| {
                this.agent.worktree_busy = false;
                let note = match r {
                    Ok(()) => Item::Branch(tr("Back in your checkout; the worktree is removed.").into()),
                    Err(e) => Item::Note(crate::trf!("Couldn't remove the worktree: {e}"; e => e), true),
                };
                this.agent.items.push(note);
                cx.notify();
            }));
        })
        .detach();
    }

    /// Accounts for the running agent's family.
    fn agent_accounts(&self) -> (Option<&'static str>, Vec<Account>) {
        let fam = family(&agent_id());
        let list = fam.map(|f| self.settings.agent_accounts.iter().filter(|a| a.family == f).cloned().collect()).unwrap_or_default();
        (fam, list)
    }

    /// The account the next run uses (None: the default sign-in).
    pub(crate) fn agent_account(&self) -> Option<Account> {
        let f = family(&agent_id())?;
        // The chat's own pick first, then the one set for new chats.
        let name = match &self.agent.account {
            Some(n) if n.is_empty() => return None,
            Some(n) => n,
            None => self.settings.agent_account.get(f)?,
        };
        self.settings.agent_accounts.iter().find(|a| a.family == f && &a.name == name).cloned()
    }

    fn agent_use_account(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        // This chat signs in with it; new chats start with it too.
        self.agent.account = Some(name.clone().unwrap_or_default());
        match name {
            Some(n) => self.settings.agent_account.insert(f.to_string(), n),
            None => self.settings.agent_account.remove(f),
        };
        self.settings.save();
        let note = match self.agent_account() {
            Some(a) => crate::trf!("Now using the account {name}."; name => a.name),
            None => tr("Now using the default account.").into(),
        };
        self.agent.items.push(Item::Branch(note));
        self.agent_restart_here(cx);
        cx.notify();
    }

    /// A new sign-in folder with a console to sign in to it.
    fn agent_add_account(&mut self, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        if self.agent.account_wait.is_some() {
            return;
        }
        prune_accounts(&mut self.settings);
        let n = self.settings.agent_accounts.iter().filter(|a| a.family == f).count() + 2;
        let name = crate::trf!("Account {n}"; n => n);
        let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
        let dir = base.join("FluxCode").join("accounts").join(format!("{f}-{n}"));
        let _ = std::fs::create_dir_all(&dir);
        let acc = Account { name, family: f.into(), dir };
        open_login(&acc);
        self.agent.account_wait = Some(acc.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            // Up to 15 minutes for the browser sign-in, looked at every 2 seconds.
            for _ in 0..450 {
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                let Ok(waiting) = this.update(cx, |this, _| this.agent.account_wait.as_ref() == Some(&acc)) else { return };
                if !waiting {
                    return;
                }
                let a = acc.clone();
                if cx.background_spawn(async move { signed_in(&a) }).await {
                    _ = this.update(cx, |this, cx| {
                        this.agent.account_wait = None;
                        this.settings.agent_accounts.push(acc.clone());
                        this.agent_use_account(Some(acc.name.clone()), cx);
                        this.agent.items.push(Item::Branch(crate::trf!("Signed in: {name} added and in use."; name => acc.name)));
                    });
                    return;
                }
            }
            _ = this.update(cx, |this, cx| this.agent_cancel_account(cx));
        })
        .detach();
    }

    /// Stop waiting for a sign-in: the folder goes, the account never joins the list.
    fn agent_cancel_account(&mut self, cx: &mut Context<Self>) {
        let Some(a) = self.agent.account_wait.take() else { return };
        if signed_in(&a) {
            // Signed in after all, just now: keep it.
            self.settings.agent_accounts.push(a.clone());
            return self.agent_use_account(Some(a.name), cx);
        }
        forget_dir(&a);
        self.agent.items.push(Item::Branch(crate::trf!("{name} wasn't signed in, so it wasn't added."; name => a.name)));
        cx.notify();
    }

    /// Sign out of an added account: its sign-in folder is deleted and the default account is used again.
    fn agent_sign_out(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        let Some(pos) = self.settings.agent_accounts.iter().position(|a| a.family == f && a.name == name) else { return };
        let a = self.settings.agent_accounts.remove(pos);
        let was_current = self.settings.agent_account.get(f) == Some(&name);
        if was_current {
            self.settings.agent_account.remove(f);
        }
        self.settings.save();
        if was_current {
            self.agent.items.push(Item::Branch(crate::trf!("Signed out of {name}; using the default account."; name => name)));
            self.agent_restart_here(cx);
            // The agent that ran as it may still hold files in the folder: delete it once that process is gone.
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_secs(4));
                forget_dir(&a);
            })
            .detach();
        } else {
            forget_dir(&a);
            self.agent.items.push(Item::Branch(crate::trf!("Signed out of {name}."; name => name)));
        }
        cx.notify();
    }

    /// The header's one "more" button: the harness's mode (Claude's permission modes: ask, accept edits, bypass, plan), its own worktree, the account, the side it docks on, and its settings.
    pub(crate) fn render_agent_more(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.agent.worktree.is_some();
        let (fam, accounts) = self.agent_accounts();
        let current = self.agent_account().map(|a| a.name);
        let waiting = self.agent.account_wait.as_ref().map(|a| a.name.clone());
        let mode = self.agent_config_view().into_iter().find(|o| o.category == "mode").filter(|o| o.choices.len() > 1);
        let right = self.settings.agent_right;
        let other = self.agent_other_folder();
        let open_name = self.root.as_ref().map(|r| super::chats::folder_name(&r.display().to_string()));
        // A mode that skips the questions shows on the button, so it is never on unnoticed.
        let risky = mode.as_ref().is_some_and(|m| {
            let c = m.current.to_ascii_lowercase();
            c.contains("bypass") || c.contains("yolo") || c.contains("dontask") || c.contains("dont_ask")
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

    /// The strip under the header while the chat has a worktree: where, Merge, Remove.
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
                    .child(Icon::new(IconName::Folder).flex_none().size(theme::u(12.)).text_color(c(theme::ACCENT_TEXT)))
                    .child(div().min_w_0().truncate().text_color(c(theme::FG)).child(f.display().to_string()))
                    .child(div().flex_1())
                    .child(Button::new("ag-dir-open").label(tr("Open folder")).xsmall().ghost().on_click(move |_, _, cx| cx.reveal_path(&open)))
                    .child(Button::new("ag-dir-back").label(tr("Use the open folder")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| this.agent_use_folder(None, cx))))
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
                .child(Icon::new(gpui_kit::assets::IconName::GitFork).flex_none().size(theme::u(12.)).text_color(c(theme::ACCENT_TEXT)))
                .child(div().min_w_0().truncate().text_color(c(theme::FG)).child(w.branch.clone()))
                .child(div().flex_1())
                .child(Button::new("ag-wt-open").label(tr("Open folder")).xsmall().ghost().on_click(move |_, _, cx| cx.reveal_path(&path)))
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

/// A console window that signs `a` in (the CLIs' login flows need a real terminal).
fn open_login(a: &Account) {
    let (var, dir) = account_env(a);
    let login = if a.family == "claude" { "claude /login" } else { "opencode auth login" };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Passed as typed: cmd's quoting is not the C runtime's.
        let line = format!("/c start \"FluxCode sign-in\" cmd /k \"set \"{var}={}\" && {login}\"", dir.display());
        let _ = Command::new("cmd").raw_arg(line).spawn();
    }
    #[cfg(not(windows))]
    {
        let script = format!("{var}='{}' {login}; exec $SHELL", dir.display());
        for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
            if Command::new(term).args(["-e", "sh", "-c", &script]).spawn().is_ok() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{account_env, family, signed_in, Account};
    use std::path::PathBuf;

    #[test]
    fn an_account_counts_only_once_signed_in() {
        let dir = std::env::temp_dir().join(format!("fluxcode-acct-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("opencode")).unwrap();
        let claude = Account { name: "A".into(), family: "claude".into(), dir: dir.clone() };
        let open = Account { family: "opencode".into(), ..claude.clone() };
        assert!(!signed_in(&claude));
        assert!(!signed_in(&open));
        // Claude writes its config before the login finishes; only an account in it counts.
        std::fs::write(dir.join(".claude.json"), r#"{"numStartups":1}"#).unwrap();
        assert!(!signed_in(&claude));
        std::fs::write(dir.join(".claude.json"), r#"{"oauthAccount":{"emailAddress":"x"}}"#).unwrap();
        assert!(signed_in(&claude));
        std::fs::write(dir.join("opencode").join("auth.json"), "{}").unwrap();
        assert!(!signed_in(&open));
        std::fs::write(dir.join("opencode").join("auth.json"), r#"{"anthropic":{"type":"oauth"}}"#).unwrap();
        assert!(signed_in(&open));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn families_and_env() {
        assert_eq!(family("claude-acp"), Some("claude"));
        assert_eq!(family("opencode-v2"), Some("opencode"));
        assert_eq!(family("gemini"), None);
        let a = Account { name: "Work".into(), family: "claude".into(), dir: PathBuf::from("x") };
        assert_eq!(account_env(&a).0, "CLAUDE_CONFIG_DIR");
        let o = Account { family: "opencode".into(), ..a };
        assert_eq!(account_env(&o).0, "XDG_DATA_HOME");
    }
}
