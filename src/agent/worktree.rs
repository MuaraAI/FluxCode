//! Isolation for the agent, so agents (and the user) don't run into each other.
//!
//! **Worktree.** One switch gives the chat its own git worktree: a second checkout
//! of the repository on its own branch, `fluxcode/agent-<time>`, kept in
//! `<repo>/.fluxcode/worktrees/<time>` (listed in `.git/info/exclude`, so it never
//! shows as a change). The agent starts there, so its edits, builds and commands
//! leave the user's checkout alone, and two FluxCode windows each with an agent never
//! touch the same files. `.env` files (ignored by git, so not in a new checkout) are
//! copied over. When the work is good, Merge commits it on the branch and merges the
//! branch into the checkout the user has open; Remove throws the worktree away. The
//! pattern is the one Claude Code (`--worktree`), Cursor, Conductor, Zed and VS Code
//! use for parallel agents (git-scm.com/docs/git-worktree).
//!
//! **Accounts.** Claude and OpenCode keep their sign-in in a folder; pointing a run at
//! another folder runs it as another account, side by side with the first:
//! `CLAUDE_CONFIG_DIR` for Claude (Anthropic documents it for exactly this) and
//! `XDG_DATA_HOME` for OpenCode (its auth database lives under it). Each account is a
//! folder under FluxCode's app data; adding one opens a console to sign in to it.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    menu::DropdownMenu,
    Icon, Selectable, Sizable,
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
            None => self.root.clone().unwrap_or_else(|| self.term_cwd.clone()),
        }
    }

    /// Restart the agent in a new place (or as another account); the chat goes on.
    fn agent_restart_here(&mut self, cx: &mut Context<Self>) {
        let was_on = matches!(self.agent.status, Status::Starting | Status::Ready | Status::Busy);
        if self.agent.items.iter().any(|i| matches!(i, Item::User(_))) {
            self.agent.carry = Some(super::transcript(&self.agent.items));
        }
        self.agent.save_chat();
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
        let Some(repo) = self.root.clone() else { return };
        if self.scm.repo.is_none() {
            self.agent.items.push(Item::Note(tr("A worktree needs a git repository: this folder isn't one yet (Source Control can make it one).").into(), true));
            cx.notify();
            return;
        }
        self.agent.worktree_busy = true;
        cx.notify();
        let task = cx.background_spawn(async move { create(&repo) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| {
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
            });
        })
        .detach();
    }

    fn agent_worktree_merge(&mut self, cx: &mut Context<Self>) {
        let Some(w) = self.agent.worktree.clone() else { return };
        let title = self.agent.items.iter().find_map(|i| if let Item::User(t) = i { Some(t.chars().take(60).collect::<String>()) } else { None }).unwrap_or_default();
        self.agent.worktree_busy = true;
        cx.notify();
        let task = cx.background_spawn(async move { merge(&w, &title) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| {
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
            });
        })
        .detach();
    }

    fn agent_worktree_remove(&mut self, w: Worktree, cx: &mut Context<Self>) {
        self.agent.worktree_busy = true;
        self.agent.worktree = None;
        self.agent.changes.clear();
        self.agent_restart_here(cx);
        let task = cx.background_spawn(async move { remove(&w) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            _ = this.update(cx, |this, cx| {
                this.agent.worktree_busy = false;
                let note = match r {
                    Ok(()) => Item::Branch(tr("Back in your checkout; the worktree is removed.").into()),
                    Err(e) => Item::Note(crate::trf!("Couldn't remove the worktree: {e}"; e => e), true),
                };
                this.agent.items.push(note);
                cx.notify();
            });
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
        let name = self.settings.agent_account.get(f)?;
        self.settings.agent_accounts.iter().find(|a| a.family == f && &a.name == name).cloned()
    }

    fn agent_use_account(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        match name {
            Some(n) => self.settings.agent_account.insert(f.to_string(), n),
            None => self.settings.agent_account.remove(f),
        };
        self.settings.save();
        self.agent.items.push(Item::Branch(match self.agent_account() {
            Some(a) => crate::trf!("Now using the account {name}."; name => a.name),
            None => tr("Now using the default account.").into(),
        }));
        self.agent_restart_here(cx);
        cx.notify();
    }

    /// A new sign-in folder, chosen at once, with a console to sign in to it.
    fn agent_add_account(&mut self, cx: &mut Context<Self>) {
        let Some(f) = family(&agent_id()) else { return };
        let n = self.settings.agent_accounts.iter().filter(|a| a.family == f).count() + 2;
        let name = crate::trf!("Account {n}"; n => n);
        let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
        let dir = base.join("FluxCode").join("accounts").join(format!("{f}-{n}"));
        let _ = std::fs::create_dir_all(&dir);
        let acc = Account { name: name.clone(), family: f.into(), dir };
        open_login(&acc);
        self.settings.agent_accounts.push(acc);
        self.agent_use_account(Some(name), cx);
    }

    pub(crate) fn render_agent_isolation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.agent.worktree.is_some();
        let busy = self.agent.worktree_busy;
        let (fam, accounts) = self.agent_accounts();
        let current = self.agent_account().map(|a| a.name);
        let weak = cx.entity().downgrade();
        h_flex()
            .gap_px()
            .child(
                Button::new("ag-worktree")
                    .icon(Icon::new(gpui_kit::assets::IconName::GitFork))
                    .xsmall()
                    .ghost()
                    .selected(on)
                    .loading(busy)
                    .tooltip(if on { tr("Working in its own worktree: click to remove it and go back to your checkout") } else { tr("Work in its own git worktree, so it doesn't touch your checkout") })
                    .on_click(cx.listener(|this, _, _, cx| this.agent_worktree_toggle(cx))),
            )
            .when(fam.is_some(), |h| {
                h.child(
                    Button::new("ag-account")
                        .icon(Icon::new(gpui_kit::assets::IconName::Users))
                        .xsmall()
                        .ghost()
                        .selected(current.is_some())
                        .tooltip(tr("Account"))
                        .dropdown_menu(move |menu, _, _| {
                            let pick = |name: Option<String>, label: String, checked: bool| {
                                let weak = weak.clone();
                                gpui_kit::component::menu::PopupMenuItem::new(label).checked(checked).on_click(move |_, _, cx| {
                                    let name = name.clone();
                                    _ = weak.update(cx, |this, cx| this.agent_use_account(name, cx));
                                })
                            };
                            let mut m = menu.min_w(px(200.)).item(pick(None, tr("Default account").into(), current.is_none()));
                            for a in &accounts {
                                m = m.item(pick(Some(a.name.clone()), a.name.clone(), current.as_deref() == Some(a.name.as_str())));
                            }
                            let weak = weak.clone();
                            m.separator().item(gpui_kit::component::menu::PopupMenuItem::new(tr("Add account and sign in…")).on_click(move |_, _, cx| {
                                _ = weak.update(cx, |this, cx| this.agent_add_account(cx));
                            }))
                        }),
                )
            })
    }

    /// The strip under the header while the chat has a worktree: where, Merge, Remove.
    pub(crate) fn render_worktree_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
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
    use super::{account_env, family, Account};
    use std::path::PathBuf;

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
