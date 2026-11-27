use crate::i18n::tr;
use crate::scm::{code_color, lane_geometry, Commit, ROW_H, SCROLLBAR_PAD};
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Disableable, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::Path;
use std::time::{Duration, Instant};

const EVERY: Duration = Duration::from_secs(300);
pub const MAX_INCOMING: usize = 50;

fn quiet_git(
    dir: &Path,
    args: &[&str],
    target: Option<&crate::remote::WslTarget>,
) -> Result<String, String> {
    crate::scm_git::run_step(dir, &crate::scm_git::git_step(args), target)
}

pub fn incoming(
    repo: &Path,
    target: Option<&crate::remote::WslTarget>,
) -> (Vec<Commit>, Option<String>) {
    let head = quiet_git(repo, &["rev-parse", "@{u}"], target)
        .ok()
        .map(|s| s.trim().to_string());
    let log = quiet_git(
        repo,
        &[
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "log",
            "HEAD..@{u}",
            "-n",
            &MAX_INCOMING.to_string(),
            "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ar%x1f%D%x1f%P",
            "-z",
        ],
        target,
    )
    .map(|s| crate::scm::parse_log(&s))
    .unwrap_or_default();
    (log, head)
}

pub fn parse_name_status(out: &str) -> Vec<(char, String)> {
    let mut it = out.split('\0').filter(|s| !s.is_empty());
    let mut files = Vec::new();
    while let Some(st) = it.next() {
        let st = st.trim_start_matches('\n');
        let code = st.chars().next().unwrap_or('M');
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
    pub(crate) fn scm_start_fetching(&mut self, cx: &mut Context<Self>) {
        if self.scm.fetch_loop {
            return;
        }
        self.scm.fetch_loop = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            loop {
                if this
                    .update(cx, |this, cx| this.scm_fetch_quietly(cx))
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(EVERY).await;
            }
        })
        .detach();
    }

    pub(crate) fn scm_fetch_quietly(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.scm.repo.clone() else {
            return;
        };
        if self.scm.is_busy() || self.scm.status.upstream.is_none() {
            return;
        }
        self.scm.fetching = true;
        cx.notify();
        self.scm.last_fetch = Some(Instant::now());
        let generation = self.scm.workspace_generation;
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    crate::scm_git::run_step(
                        &repo,
                        &crate::scm_git::Step::Remote(crate::scm_git::RemoteAction::Fetch),
                        wsl.as_ref(),
                    )
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.scm.workspace_generation != generation {
                    return;
                }
                this.scm.fetching = false;
                _ = res;
                this.scm_refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn scm_on_return(&mut self, cx: &mut Context<Self>) {
        if self
            .scm
            .last_fetch
            .is_none_or(|t| t.elapsed() > Duration::from_secs(120))
        {
            self.scm_fetch_quietly(cx);
        }
    }

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
        let Some(repo) = self.scm.repo.clone() else {
            return;
        };
        let generation = self.scm.workspace_generation;
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let h = hash.clone();
            let res = cx
                .background_spawn(async move {
                    quiet_git(
                        &repo,
                        &[
                            "-c",
                            "core.quotepath=off",
                            "show",
                            "--format=",
                            "--name-status",
                            "-z",
                            &h,
                        ],
                        wsl.as_ref(),
                    )
                    .map(|s| parse_name_status(&s))
                    .map(|files| (repo, files))
                })
                .await;
            _ = this.update(cx, |this, cx| {
                match res {
                    Ok((repo, files)) => {
                        if this.scm.workspace_generation != generation
                            || this.scm.repo.as_ref() != Some(&repo)
                        {
                            return;
                        }
                        this.scm.incoming_files.insert(hash, files);
                    }
                    Err(e) => {
                        if this.scm.workspace_generation == generation {
                            this.scm.incoming_open = None;
                            this.scm.error = Some(e);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn scm_show_incoming_file(
        &mut self,
        hash: String,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.scm.repo.clone() else {
            return;
        };
        let generation = self.scm.workspace_generation;
        let wsl = self.wsl.clone();
        let short = hash[..hash.len().min(7)].to_string();
        let file_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let full_path = repo.join(&path);
        let target = if wsl.is_some() || full_path.is_file() {
            Some(full_path)
        } else {
            None
        };
        let virtual_key = std::path::PathBuf::from(format!("flux-diff://incoming/{hash}/{path}"));
        cx.spawn_in(window, async move |this, cx| {
            let (h, p, r) = (hash.clone(), path.clone(), repo.clone());
            let res = cx
                .background_spawn(async move {
                    crate::scm_git::show_commit_diff(&r, &h, Some(&p), false, wsl.as_ref())
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                if this.scm.workspace_generation != generation
                    || this.scm.repo.as_ref() != Some(&repo)
                {
                    return;
                }
                let patch = match res {
                    Ok(patch) => patch,
                    Err(e) => {
                        this.scm.error = Some(e);
                        cx.notify();
                        return;
                    }
                };
                this.open_scm_diff_tab(
                    crate::diff_view::ScmDiffSpec {
                        virtual_key,
                        tab_title: format!("{file_name} ({short})"),
                        display_name: path,
                        badge: short,
                        patch,
                        conflict_text: None,
                        target,
                        scm_stage: None,
                        multi_file: false,
                    },
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    pub(crate) fn render_scm_incoming(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.scm_has_incoming() {
            return None;
        }
        let s = &self.scm;
        let n = s.status.behind as usize;
        let upstream = s.status.upstream.clone().unwrap_or_default();
        let open_list = s.incoming_shown;
        let dirty = s.status.count() > 0;
        let header = crate::scm::scm_row()
            .id("scm-sec-in")
            .group("scm-in-head")
            .px_2()
            .gap_1()
            .items_center()
            .text_xs()
            .cursor_pointer()
            .hover(|st| st.bg(c(theme::BG_HOVER)))
            .child(Icon::new(if open_list { IconName::ChevronDown } else { IconName::ChevronRight }).flex_none().size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
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
            .disabled(s.is_busy())
            .tooltip(if dirty {
                tr("Pull them in (your changes are set aside and put back: git pull --autostash)")
            } else {
                tr("Pull them in (git pull)")
            })
            .on_click(cx.listener(|this, _, w, cx| this.on_scm_pull(&ScmPull, w, cx)));
        let later = Button::new("scm-in-later")
            .icon(Icon::new(IconName::Close))
            .xsmall()
            .ghost()
            .tooltip(tr("Not now: hide this until more commits arrive"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.scm.incoming_dismissed = this.scm.incoming_head.clone();
                cx.notify();
            }));
        let mut col = v_flex().w_full().min_w_0().flex_none().pb_2().child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_none()
                .pr(px(SCROLLBAR_PAD))
                .gap_0p5()
                .items_center()
                .child(header.flex_1().min_w_0())
                .child(pull)
                .child(later),
        );
        if !open_list {
            return Some(col.into_any_element());
        }
        let total = s.incoming.len();
        let last = total.saturating_sub(1);
        for (i, cm) in s.incoming.iter().enumerate() {
            let open = s.incoming_open.as_deref() == Some(cm.hash.as_str());
            let hash = cm.hash.clone();
            let (line_top, line_bottom, dot_top) = lane_geometry(i, total, open);
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
                        .top(theme::u(line_top))
                        .bottom(theme::u(line_bottom))
                        .bg(c(theme::BLUE).opacity(0.45)),
                )
                .child(
                    div()
                        .absolute()
                        .left(theme::u(3.))
                        .top(theme::u(dot_top))
                        .size(theme::u(8.))
                        .rounded_full()
                        .border_2()
                        .border_color(c(theme::BLUE))
                        .bg(c(theme::BG_SIDE)),
                );
            let tip: SharedString =
                format!("{}\n{} · {} · {}", cm.subject, cm.short, cm.author, cm.when).into();
            col = col.child(
                crate::scm::scm_row()
                    .id(("scm-in", i))
                    .pl(theme::u(12.))
                    .pr(px(SCROLLBAR_PAD))
                    .gap_1p5()
                    .items_center()
                    .text_size(theme::u(11.5))
                    .cursor_pointer()
                    .when(open, |h| h.bg(c(theme::BG_HOVER)))
                    .hover(|st| st.bg(c(theme::BG_HOVER)))
                    .child(lane)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(c(theme::FG))
                            .child(cm.subject.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink(1.)
                            .min_w_0()
                            .max_w(theme::u(90.))
                            .truncate()
                            .text_color(c(theme::FG_DIM))
                            .child(cm.author.clone()),
                    )
                    .tooltip(move |w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)
                    })
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.scm_open_incoming(hash.clone(), cx)),
                    ),
            );
            if !open {
                continue;
            }
            let pad = theme::u(34.);
            match s.incoming_files.get(&cm.hash) {
                None => {
                    col = col.child(
                        div()
                            .flex_none()
                            .h(theme::u(ROW_H))
                            .pl(pad)
                            .flex()
                            .items_center()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(tr("Reading the changed files…")),
                    )
                }
                Some(files) => {
                    for (k, (code, path)) in files.iter().enumerate().take(60) {
                        let (h, p) = (cm.hash.clone(), path.clone());
                        let name = path.rsplit('/').next().unwrap_or(path).to_string();
                        let dir = path
                            .rsplit_once('/')
                            .map(|(d, _)| d.replace('/', "\\"))
                            .unwrap_or_default();
                        col = col.child(
                            crate::scm::scm_row()
                                .id(("scm-in-file", i * 1000 + k))
                                .relative()
                                .when(i < last, |h| {
                                    h.child(
                                        div()
                                            .absolute()
                                            .left(theme::u(18.))
                                            .top_0()
                                            .bottom_0()
                                            .w(px(2.))
                                            .bg(c(theme::BLUE).opacity(0.45)),
                                    )
                                })
                                .pl(pad)
                                .pr(px(SCROLLBAR_PAD))
                                .gap_1p5()
                                .items_center()
                                .text_xs()
                                .cursor_pointer()
                                .hover(|st| st.bg(c(theme::BG_HOVER)))
                                .child(crate::files::file_icon_el(&name, theme::u(14.)))
                                .child(
                                    div()
                                        .flex_shrink(1.)
                                        .min_w_0()
                                        .max_w(relative(0.6))
                                        .truncate()
                                        .text_color(c(theme::FG))
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(c(theme::FG_DIM))
                                        .child(dir),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .w(theme::u(12.))
                                        .text_right()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c(code_color(*code)))
                                        .child(code.to_string()),
                                )
                                .tooltip(|w, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(tr(
                                        "Show what changed",
                                    ))
                                    .build(w, cx)
                                })
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.scm_show_incoming_file(h.clone(), p.clone(), w, cx)
                                })),
                        );
                    }
                    if files.len() > 60 {
                        col = col.child(
                            div()
                                .flex_none()
                                .pl(pad)
                                .py_1()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(crate::trf!("… and {n} more files"; n => files.len() - 60)),
                        );
                    }
                }
            }
        }
        if n > s.incoming.len() {
            col = col.child(
                div()
                    .flex_none()
                    .pl(theme::u(34.))
                    .py_1()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(crate::trf!("… and {n} older ones"; n => n - s.incoming.len())),
            );
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
        assert_eq!(
            parse_name_status(out),
            [
                ('M', "src/a.rs".to_string()),
                ('R', "new.txt".to_string()),
                ('A', "docs/b.md".to_string())
            ]
        );
    }
}
