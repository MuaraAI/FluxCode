//! A `.diff` / `.patch` tab drawn like a code review: added lines on green, removed
//! lines on red, hunk headers in blue, each line with its old and new line number.
//! The text editor's syntax colors have no names for inserted / deleted lines, so a
//! diff opened as text was all one color. Rows go through a virtual list: a diff of
//! any size costs only the rows on screen. "Edit as text" switches to the editor.

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
    pub scroll: UniformListScrollHandle,
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
        .collect();
    DiffDoc { lines: Rc::new(lines), added, removed, source: false, scroll: UniformListScrollHandle::new() }
}

pub fn is_diff(lang: &str) -> bool {
    lang == "diff"
}

impl Workbench {
    pub(crate) fn render_diff_doc(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.docs.get(ix).and_then(|d| d.diff.as_ref()) else { return div().into_any_element() };
        let lines = d.lines.clone();
        let size = px(self.settings.font_size);
        let row_h = px(self.settings.font_size * 1.6);
        let digits = lines.iter().filter_map(|l| l.new.max(l.old)).max().unwrap_or(1).to_string().len().max(3) as f32;
        let num_w = px(self.settings.font_size * 0.62 * digits + 12.);
        let green = c(theme::GREEN);
        let red = c(theme::DANGER);
        let blue = c(theme::BLUE);
        let list = uniform_list(
            ("diff-lines", ix),
            lines.len(),
            cx.processor(move |_this, range: std::ops::Range<usize>, _, _cx| {
                range
                    .map(|i| {
                        let l = &lines[i];
                        let (bg, fg, mark) = match l.kind {
                            Kind::Add => (Some(green.opacity(0.14)), c(theme::FG_STRONG), Some(green)),
                            Kind::Del => (Some(red.opacity(0.14)), c(theme::FG_STRONG), Some(red)),
                            Kind::Hunk => (Some(blue.opacity(0.10)), blue, None),
                            Kind::File => (None, c(theme::FG_MUTED), None),
                            Kind::Same => (None, c(theme::FG), None),
                        };
                        let num = |n: Option<u32>| div().flex_none().w(num_w).pr_2().text_right().text_color(c(theme::FG_DIM)).child(n.map(|n| n.to_string()).unwrap_or_default());
                        h_flex()
                            .id(("dl", i))
                            .h(row_h)
                            .w_full()
                            .when_some(bg, |d, bg| d.bg(bg))
                            .child(div().flex_none().w(px(3.)).h_full().when_some(mark, |d, m| d.bg(m)))
                            .child(num(l.old))
                            .child(num(l.new))
                            .child(div().flex_1().min_w_0().pl_2().whitespace_nowrap().text_color(fg).when(l.kind == Kind::File, |d| d.font_weight(FontWeight::SEMIBOLD)).child(l.text.clone()))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&d.scroll)
        .size_full()
        .font_family(self.settings.font_family.clone())
        .text_size(size);
        let bar = h_flex()
            .flex_none()
            .h(theme::u(28.))
            .px_3()
            .gap_2()
            .text_xs()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(Icon::new(IconName::FileDiff).size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
            .child(div().text_color(green).child(format!("+{}", d.added)))
            .child(div().text_color(red).child(format!("-{}", d.removed)))
            .child(div().flex_1())
            .child(
                div()
                    .id(("diff-source", ix))
                    .px_2()
                    .py_0p5()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(c(theme::FG_MUTED))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .child(tr("Edit as text"))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if let Some(d) = this.docs.get_mut(ix).and_then(|d| d.diff.as_mut()) {
                            d.source = true;
                        }
                        this.activate(ix, w, cx);
                    })),
            );
        v_flex().size_full().child(bar).child(div().flex_1().min_h_0().child(list)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Kind};

    #[test]
    fn numbers_follow_the_hunks() {
        let d = parse("--- a/f.rs\n+++ b/f.rs\n@@ -3,3 +3,4 @@\n same\n-old\n+new\n+more\n tail\n");
        let k: Vec<Kind> = d.lines.iter().map(|l| l.kind).collect();
        assert_eq!(k, vec![Kind::File, Kind::File, Kind::Hunk, Kind::Same, Kind::Del, Kind::Add, Kind::Add, Kind::Same]);
        assert_eq!((d.lines[3].old, d.lines[3].new), (Some(3), Some(3)));
        assert_eq!(d.lines[4].old, Some(4));
        assert_eq!(d.lines[5].new, Some(4));
        assert_eq!((d.lines[7].old, d.lines[7].new), (Some(5), Some(6)));
        assert_eq!((d.added, d.removed), (2, 1));
    }
}
