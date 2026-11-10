//! New commits on the remote, before you pull them: Source Control fetches quietly in
//! the background, and when the branch's upstream has commits this checkout doesn't,
//! a card at the top lists each one (title, author, when) with Pull and Not now. A
//! commit opens to the files it changed; a file opens its diff in the Output panel.
//!
//! Light: one `git fetch --quiet --prune` every five minutes while a repository with an
//! upstream is open (and when the window comes back after two minutes away), with no
//! prompts (a background fetch never asks for a password); the commit list is read only
//! when the branch is behind, and a commit's files only when it is opened.

use crate::i18n::tr;
use crate::scm::{code_color, Commit};
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::Path;
use std::time::{Duration, Instant};

/// How often the remote is checked.
const EVERY: Duration = Duration::from_secs(300);
/// Commits listed at most.
pub const MAX_INCOMING: usize = 50;

/// `git` for a background read: no prompts, no locks, no window.
fn quiet_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = crate::proc::command("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        // Git Credential Manager: never pop a sign-in window for a background fetch.
        .env("GCM_INTERACTIVE", "never")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null());
    crate::github::apply_env(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The upstream's commits this checkout doesn't have, newest first, and the upstream's head.
pub fn incoming(repo: &Path) -> (Vec<Commit>, Option<String>) {
    let head = quiet_git(repo, &["rev-parse", "@{u}"]).ok().map(|s| s.trim().to_string());
    let log = quiet_git(
        repo,
        &["-c", "i18n.logOutputEncoding=UTF-8", "log", "HEAD..@{u}", "-n", &MAX_INCOMING.to_string(), "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ar%x1f%D%x1f%P", "-z"],
    )
    .map(|s| crate::scm::parse_log(&s))
    .unwrap_or_default();
    (log, head)
}

/// `git show --name-status -z`: (letter, path) per file.
pub fn parse_name_status(out: &str) -> Vec<(char, String)> {
    let mut it = out.split('\0').filter(|s| !s.is_empty());
    let mut files = Vec::new();
    while let Some(st) = it.next() {
        let st = st.trim_start_matches('\n');
        let code = st.chars().next().unwrap_or('M');
        // Renames and copies name the old path, then the new one.
        let path = if matches!(code, 'R' | 'C') {
            it.next();
            it.next()
        } else {
            it.next()
        };
        if let Some(p) = path {
            files.push((code, p.to_string()));
        }
    }
    files
}

impl Workbench {
    /// Start checking the remote every few minutes (once per run).
    pub(crate) fn scm_start_fetching(&mut self, cx: &mut Context<Self>) {
        if self.scm.fetch_loop {
            return;
        }
        self.scm.fetch_loop = true;
        cx.spawn(async move |this, cx| {
            // A first look soon after the folder opens, then every EVERY.
            cx.background_executor().timer(Duration::from_secs(4)).await;
            loop {
                if this.update(cx, |this, cx| this.scm_fetch_quietly(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(EVERY).await;
            }
        })
        .detach();
    }

    /// Fetch in the background (no prompts), then re-read the state.
    pub(crate) fn scm_fetch_quietly(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.scm.repo.clone() else { return };
        if self.scm.fetching || self.scm.status.upstream.is_none() || self.scm.busy.is_some() {
            return;
        }
        self.scm.fetching = true;
        self.scm.last_fetch = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { quiet_git(&repo, &["fetch", "--quiet", "--prune"]) }).await;
            _ = this.update(cx, |this, cx| {
                this.scm.fetching = false;
                // A failed fetch (offline, no access) just leaves the list as it was.
                _ = res;
                this.scm_refresh(cx);
            });
        })
        .detach();
    }

    /// The window came back: check the remote if it has been a while.
    pub(crate) fn scm_on_return(&mut self, cx: &mut Context<Self>) {
        if self.scm.last_fetch.is_none_or(|t| t.elapsed() > Duration::from_secs(120)) {
            self.scm_fetch_quietly(cx);
        }
    }

    /// New commits are there and the user hasn't put them aside.
    pub(crate) fn scm_has_incoming(&self) -> bool {
        !self.scm.incoming.is_empty() && self.scm.incoming_dismissed != self.scm.incoming_head
    }

    fn scm_open_incoming(&mut self, hash: String, cx: &mut Context<Self>) {
        if self.scm.incoming_open.as_deref() == Some(hash.as_str()) {
            self.scm.incoming_open = None;
            return cx.notify();
        }
        self.scm.incoming_open = Some(hash.clone());
        cx.notify();
        if self.scm.incoming_files.contains_key(&hash) {
            return;
        }
        let Some(repo) = self.scm.repo.clone() else { return };
        cx.spawn(async move |this, cx| {
            let h = hash.clone();
            let files = cx
                .background_spawn(async move { quiet_git(&repo, &["-c", "core.quotepath=off", "show", "--format=", "--name-status", "-z", &h]).map(|s| parse_name_status(&s)).unwrap_or_default() })
                .await;
            _ = this.update(cx, |this, cx| {
                this.scm.incoming_files.insert(hash, files);
                cx.notify();
            });
        })
        .detach();
    }

    /// One file's change in an incoming commit, in the Output panel.
    fn scm_show_incoming_file(&mut self, hash: String, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.scm.repo.clone() else { return };
        cx.spawn(async move |this, cx| {
            let (h, p) = (hash.clone(), path.clone());
            let text = cx.background_spawn(async move { quiet_git(&repo, &["-c", "core.quotepath=off", "show", "--format=%h  %s%n%an, %ar%n", &h, "--", &p]).unwrap_or_else(|e| e) }).await;
            _ = this.update(cx, |this, cx| {
                this.out_lines.clear();
                let mut head = Line::new(&path, crate::term::LineKind::Cmd);
                head.meta = crate::trf!("In {hash}, not pulled yet"; hash => &hash[..hash.len().min(8)]).into();
                this.out_lines.push(head);
                for l in text.lines() {
                    let kind = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") || l.starts_with("index ") {
                        crate::term::LineKind::Dim
                    } else if l.starts_with('+') {
                        crate::term::LineKind::Success
                    } else if l.starts_with('-') {
                        crate::term::LineKind::Err
                    } else if l.starts_with("@@") {
                        crate::term::LineKind::Hunk
                    } else {
                        crate::term::LineKind::Normal
                    };
                    this.out_lines.push(Line::new(l, kind));
                }
                this.panel_open = true;
                this.panel_tab = PanelTab::Output;
                this.out_scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            });
        })
        .detach();
    }

    /// The Incoming section at the top of Source Control: what the remote has that you
    /// don't. Drawn like the Changes and Graph sections (no card), each commit like a
    /// Graph row with a hollow dot, since it isn't in this checkout yet.
    pub(crate) fn render_scm_incoming(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.scm_has_incoming() {
            return None;
        }
        let s = &self.scm;
        let n = s.status.behind as usize;
        let upstream = s.status.upstream.clone().unwrap_or_default();
        let open_list = s.incoming_shown;
        let dirty = s.status.count() > 0;
        let header = h_flex()
            .id("scm-sec-in")
            .group("scm-in-head")
            .h(theme::u(22.))
            .px_2()
            .gap_1()
            .text_xs()
            .cursor_pointer()
            .child(Icon::new(if open_list { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr("Incoming")))
            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(upstream.clone()))
            .child(div().flex_none().px_1p5().rounded_full().bg(c(theme::BG_HOVER)).text_color(c(theme::FG)).child(n.to_string()))
            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::trf!("{n} commits on {up} that this checkout doesn't have yet"; n => n, up => &upstream)).build(w, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                this.scm.incoming_shown = !this.scm.incoming_shown;
                cx.notify();
            }));
        let pull = Button::new("scm-in-pull")
            .icon(Icon::new(Ico::Download))
            .label(tr("Pull"))
            .xsmall()
            .ghost()
            .tooltip(if dirty { tr("Pull them in (your changes are set aside and put back: git pull --autostash)") } else { tr("Pull them in (git pull)") })
            .on_click(cx.listener(|this, _, w, cx| this.on_scm_pull(&ScmPull, w, cx)));
        let later = Button::new("scm-in-later").icon(Icon::new(IconName::Close)).xsmall().ghost().tooltip(tr("Not now: hide this until more commits arrive")).on_click(cx.listener(|this, _, _, cx| {
            this.scm.incoming_dismissed = this.scm.incoming_head.clone();
            cx.notify();
        }));
        let mut col = v_flex().pb_2().child(h_flex().pr_1().gap_0p5().child(header.flex_1().min_w_0()).child(pull).child(later));
        if !open_list {
            return Some(col.into_any_element());
        }
        let last = s.incoming.len().saturating_sub(1);
        for (i, cm) in s.incoming.iter().enumerate() {
            let open = s.incoming_open.as_deref() == Some(cm.hash.as_str());
            let hash = cm.hash.clone();
            // The same lane as the Graph: a line through hollow dots (not here yet).
            let lane = div()
                .relative()
                .w(theme::u(14.))
                .h_full()
                .flex_none()
                .child(
                    div()
                        .absolute()
                        .left(theme::u(6.))
                        .w(px(2.))
                        .top(if i == 0 { theme::u(11.) } else { px(0.) })
                        .bottom(if i == last && !open { theme::u(11.) } else { px(0.) })
                        .bg(c(theme::BLUE).opacity(0.45)),
                )
                .child(div().absolute().left(theme::u(3.)).top(theme::u(7.)).size(theme::u(8.)).rounded_full().border_2().border_color(c(theme::BLUE)).bg(c(theme::BG_SIDE)));
            let tip: SharedString = format!("{}\n{} · {} · {}", cm.subject, cm.short, cm.author, cm.when).into();
            col = col.child(
                h_flex()
                    .id(("scm-in", i))
                    .h(theme::u(22.))
                    .pl(theme::u(12.))
                    .pr_2()
                    .gap_1p5()
                    .text_size(theme::u(11.5))
                    .cursor_pointer()
                    .when(open, |h| h.bg(c(theme::BG_HOVER)))
                    .hover(|st| st.bg(c(theme::BG_HOVER)))
                    .child(lane)
                    .child(div().flex_1().min_w(theme::u(60.)).truncate().text_color(c(theme::FG)).child(cm.subject.clone()))
                    .child(div().flex_none().max_w(theme::u(90.)).truncate().text_color(c(theme::FG_DIM)).child(cm.author.clone()))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| this.scm_open_incoming(hash.clone(), cx))),
            );
            if !open {
                continue;
            }
            let pad = theme::u(34.);
            match s.incoming_files.get(&cm.hash) {
                None => col = col.child(div().h(theme::u(22.)).pl(pad).flex().items_center().text_xs().text_color(c(theme::FG_DIM)).child(tr("Reading the changed files…"))),
                Some(files) => {
                    for (k, (code, path)) in files.iter().enumerate().take(60) {
                        let (h, p) = (cm.hash.clone(), path.clone());
                        let name = path.rsplit('/').next().unwrap_or(path).to_string();
                        let dir = path.rsplit_once('/').map(|(d, _)| d.replace('/', "\\")).unwrap_or_default();
                        let (icon, tint) = crate::files::icon_for(&name);
                        col = col.child(
                            h_flex()
                                .id(("scm-in-file", i * 1000 + k))
                                .relative()
                                // The lane goes on past the files to the next commit.
                                .when(i < last, |h| h.child(div().absolute().left(theme::u(18.)).top_0().bottom_0().w(px(2.)).bg(c(theme::BLUE).opacity(0.45))))
                                .h(theme::u(22.))
                                .pl(pad)
                                .pr_2()
                                .gap_1p5()
                                .text_xs()
                                .cursor_pointer()
                                .hover(|st| st.bg(c(theme::BG_HOVER)))
                                .child(Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                                .child(div().flex_none().max_w(relative(0.6)).truncate().text_color(c(theme::FG)).child(name))
                                .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(dir))
                                .child(div().flex_none().w(theme::u(12.)).font_weight(FontWeight::BOLD).text_color(c(code_color(*code))).child(code.to_string()))
                                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Show what changed")).build(w, cx))
                                .on_click(cx.listener(move |this, _, _, cx| this.scm_show_incoming_file(h.clone(), p.clone(), cx))),
                        );
                    }
                    if files.len() > 60 {
                        col = col.child(div().pl(pad).py_1().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("… and {n} more files"; n => files.len() - 60)));
                    }
                }
            }
        }
        if n > s.incoming.len() {
            col = col.child(div().pl(theme::u(34.)).py_1().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("… and {n} older ones"; n => n - s.incoming.len())));
        }
        Some(col.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_name_status;

    #[test]
    fn name_status_with_renames() {
        let out = "M\0src/a.rs\0R100\0old.txt\0new.txt\0A\0docs/b.md\0";
        assert_eq!(parse_name_status(out), [('M', "src/a.rs".to_string()), ('R', "new.txt".to_string()), ('A', "docs/b.md".to_string())]);
    }
}
