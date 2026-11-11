//! A `.diff` / `.patch` tab drawn like a code review, compact enough for a narrow
//! panel: added lines on green with `+`, removed ones on red with `-`, one line-number
//! column (the new file's number, the old one's for a removed line), each hunk as a
//! slim "line N" divider, and the file's name in the bar instead of `---` / `+++`.
//! Long lines wrap to the width instead of running off it. Rows go through a virtual
//! list (each row's height measured once), so a diff of any size costs only the rows
//! on screen. "Open file" opens the file the diff is of; "Edit as text" the patch.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// `--- a/x`, `+++ b/x`, `diff --git`, `index ...`.
    File,
    Hunk,
    Add,
    Del,
    Same,
}

pub struct Line {
    pub kind: Kind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: SharedString,
}

pub struct DiffDoc {
    pub lines: Rc<Vec<Line>>,
    pub added: usize,
    pub removed: usize,
    /// Showing the editor instead (to change the patch by hand).
    pub source: bool,
    pub list: ListState,
    /// The file the diff is of, as the `+++` line names it.
    pub name: SharedString,
    /// The file on disk, when known (the agent's Changes card sets it).
    pub target: Option<std::path::PathBuf>,
}

/// `@@ -12,7 +12,9 @@` → (12, 12).
fn hunk_starts(h: &str) -> Option<(u32, u32)> {
    let mut it = h.split_whitespace().skip(1);
    let num = |s: Option<&str>, sign: char| -> Option<u32> { s?.strip_prefix(sign)?.split(',').next()?.parse().ok() };
    Some((num(it.next(), '-')?, num(it.next(), '+')?))
}

pub fn parse(text: &str) -> DiffDoc {
    let (mut old, mut new) = (0u32, 0u32);
    let (mut added, mut removed) = (0, 0);
    let mut in_hunk = false;
    let lines = text
        .lines()
        .map(|l| {
            let (kind, o, n) = if l.starts_with("@@") {
                if let Some((a, b)) = hunk_starts(l) {
                    (old, new) = (a, b);
                }
                in_hunk = true;
                (Kind::Hunk, None, None)
            } else if l.starts_with("diff ") || l.starts_with("index ") || (l.starts_with("--- ") || l.starts_with("+++ ")) && !in_hunk {
                in_hunk = false;
                (Kind::File, None, None)
            } else if l.starts_with('+') {
                added += 1;
                new += 1;
                (Kind::Add, None, Some(new - 1))
            } else if l.starts_with('-') {
                removed += 1;
                old += 1;
                (Kind::Del, Some(old - 1), None)
            } else {
                old += 1;
                new += 1;
                (Kind::Same, in_hunk.then_some(old - 1), in_hunk.then_some(new - 1))
            };
            // `--- a/x` after a hunk starts the next file.
            if l.starts_with("diff ") {
                in_hunk = false;
            }
            Line { kind, old: o, new: n, text: l.to_string().into() }
        })
        // The file lines are said once, in the bar.
        .filter(|l: &Line| l.kind != Kind::File)
        .collect::<Vec<_>>();
    let name = text
        .lines()
        .find_map(|l| l.strip_prefix("+++ ").or_else(|| l.strip_prefix("--- ")))
        .map(|n| n.trim().trim_start_matches("b/").trim_start_matches("a/").to_string())
        .unwrap_or_default();
    let list = ListState::new(lines.len(), ListAlignment::Top, px(800.));
    DiffDoc { lines: Rc::new(lines), added, removed, source: false, list, name: name.into(), target: None }
}

pub fn is_diff(lang: &str) -> bool {
    lang == "diff"
}

impl Workbench {
    pub(crate) fn render_diff_doc(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.docs.get(ix).and_then(|d| d.diff.as_ref()) else { return div().into_any_element() };
        let lines = d.lines.clone();
        let fs = self.settings.font_size;
        let digits = lines.iter().filter_map(|l| l.new.max(l.old)).max().unwrap_or(1).to_string().len().max(2) as f32;
        let num_w = px(fs * 0.62 * digits + 10.);
        let (green, red) = (c(theme::GREEN), c(theme::DANGER));
        // List rows don't take the list's text style: each row sets the editor's font.
        let font = self.settings.font_family.clone();
        let rows = list(d.list.clone(), move |i, _, _| {
            let Some(l) = lines.get(i) else { return div().into_any_element() };
            if l.kind == Kind::Hunk {
                // "@@ -12,7 +12,9 @@ fn main" -> a slim divider: "line 12  fn main".
                let at = l.text.split("@@").nth(1).and_then(|h| h.split_whitespace().nth(1)).and_then(|n| n.trim_start_matches('+').split(',').next()).unwrap_or("");
                let ctx = l.text.splitn(3, "@@").nth(2).unwrap_or("").trim().to_string();
                return h_flex()
                    .w_full()
                    .mt(px(if i == 0 { 0. } else { 6. }))
                    .px_2()
                    .py_0p5()
                    .gap_2()
                    .bg(c(theme::BG_ELEVATED))
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(div().flex_none().child(format!("⋯ line {at}")))
                    .child(div().min_w_0().truncate().child(ctx))
                    .into_any_element();
            }
            let (bg, sign, sign_col, num) = match l.kind {
                Kind::Add => (Some(green.opacity(0.13)), "+", green, l.new),
                Kind::Del => (Some(red.opacity(0.13)), "-", red, l.old),
                _ => (None, " ", c(theme::FG_DIM), l.new),
            };
            // The text without its +/-/space mark (the sign column says it).
            let body: SharedString = l.text.get(1..).unwrap_or("").to_string().into();
            h_flex()
                .w_full()
                .items_start()
                .font_family(font.clone())
                .text_size(px(fs))
                .line_height(relative(1.45))
                .when_some(bg, |d, bg| d.bg(bg))
                .child(div().flex_none().w(num_w).pr_1().text_right().text_color(c(theme::FG_DIM)).child(num.map(|n| n.to_string()).unwrap_or_default()))
                .child(div().flex_none().w(px(fs * 0.9)).text_center().text_color(sign_col).child(sign))
                // Wraps at the panel's width; leading spaces keep the indentation.
                .child(div().flex_1().min_w_0().pr_2().text_color(c(theme::FG)).child(body))
                .into_any_element()
        })
        .size_full();
        let target = d.target.clone();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id((id, ix))
                .flex_none()
                .px_2()
                .py_0p5()
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(c(theme::FG_MUTED))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .child(tr(label))
        };
        let bar = h_flex()
            .flex_none()
            .h(theme::u(28.))
            .px_3()
            .gap_2()
            .text_xs()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(Icon::new(IconName::FileDiff).flex_none().size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
            .child(div().min_w_0().truncate().text_color(c(theme::FG)).child(d.name.clone()))
            .child(div().flex_none().text_color(green).child(format!("+{}", d.added)))
            .child(div().flex_none().text_color(red).child(format!("-{}", d.removed)))
            .child(div().flex_1())
            .when_some(target, |h, path| {
                h.child(button("diff-open", "Open file").on_click(cx.listener(move |this, _, w, cx| {
                    this.agent.user_nav = Some(std::time::Instant::now());
                    this.open_file(path.clone(), w, cx);
                })))
            })
            .child(button("diff-source", "Edit as text").on_click(cx.listener(move |this, _, w, cx| {
                if let Some(d) = this.docs.get_mut(ix).and_then(|d| d.diff.as_mut()) {
                    d.source = true;
                }
                this.activate(ix, w, cx);
            })));
        v_flex().size_full().child(bar).child(div().flex_1().min_h_0().pt_1().child(rows)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Kind};

    #[test]
    fn numbers_follow_the_hunks() {
        let d = parse("--- a/f.rs\n+++ b/f.rs\n@@ -3,3 +3,4 @@\n same\n-old\n+new\n+more\n tail\n");
        let k: Vec<Kind> = d.lines.iter().map(|l| l.kind).collect();
        assert_eq!(k, vec![Kind::Hunk, Kind::Same, Kind::Del, Kind::Add, Kind::Add, Kind::Same]);
        assert_eq!(d.name.as_ref(), "f.rs");
        assert_eq!((d.lines[1].old, d.lines[1].new), (Some(3), Some(3)));
        assert_eq!(d.lines[2].old, Some(4));
        assert_eq!(d.lines[3].new, Some(4));
        assert_eq!((d.lines[5].old, d.lines[5].new), (Some(5), Some(6)));
        assert_eq!((d.added, d.removed), (2, 1));
    }
}
