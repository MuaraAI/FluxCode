//! Go to File (Ctrl+P, or a click on the search box in the title bar), like
//! VS Code's Quick Open: type part of a name, Enter opens the file.
//!
//! The file list comes from the explorer's scan (no extra disk walk), and the
//! match is a small subsequence score, so it stays instant on large folders.

use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;

const MAX_HITS: usize = 60;

/// Score `path` for `query` (both compared lower-case): every query character
/// must appear in order. Matches in the file name, at word starts and in a row
/// score higher; shorter paths win ties.
#[cfg(test)]
pub fn fuzzy(query: &str, path: &str) -> Option<i32> {
    let q: Vec<u8> = query.to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    fuzzy_lower(&q, path.to_lowercase().as_bytes())
}

/// `fuzzy` on an already lower-cased query and path (bytes, so a keystroke over
/// thousands of files allocates nothing). Non-ASCII bytes still match exactly.
pub fn fuzzy_lower(q: &[u8], p: &[u8]) -> Option<i32> {
    if q.is_empty() {
        return Some(0);
    }
    let name_start = p.iter().rposition(|c| *c == b'\\' || *c == b'/').map_or(0, |i| i + 1);
    let mut score = 0;
    let mut at = 0usize;
    let mut prev: Option<usize> = None;
    for &qc in q {
        let pos = at + p[at..].iter().position(|&b| b == qc)?;
        score += 1;
        if prev.is_some_and(|pv| pv + 1 == pos) {
            score += 5;
        }
        if pos >= name_start {
            score += 3;
        }
        if pos == 0 || matches!(p[pos - 1], b'\\' | b'/' | b'_' | b'-' | b'.' | b' ') {
            score += 4;
        }
        prev = Some(pos);
        at = pos + 1;
    }
    // Typed text found whole beats letters scattered along the path: name starts
    // with it, name contains it, path contains it.
    let name = &p[name_start..];
    let contains = |hay: &[u8]| hay.windows(q.len()).any(|w| w == q);
    if name.starts_with(q) {
        score += 100;
    } else if contains(name) {
        score += 60;
    } else if contains(p) {
        score += 25;
    }
    Some(score * 10 - p.len() as i32)
}

pub struct QuickOpen {
    pub open: bool,
    pub input: Entity<InputState>,
    /// Indices into `Workbench::all_files`, best first.
    pub hits: Vec<usize>,
    pub sel: usize,
}

impl QuickOpen {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> (Self, Subscription) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type part of a file name"));
        let sub = cx.subscribe_in(&input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => this.quick_filter(cx),
            InputEvent::PressEnter { .. } => this.quick_accept(window, cx),
            _ => {}
        });
        (Self { open: false, input, hits: Vec::new(), sel: 0 }, sub)
    }
}

impl Workbench {
    fn rel<'a>(&self, path: &'a str) -> &'a str {
        let root = self.root.as_deref().map(|r| r.to_string_lossy().len()).unwrap_or(0);
        path.get(root..).map(|s| s.trim_start_matches(['\\', '/'])).unwrap_or(path)
    }

    pub(crate) fn quick_filter(&mut self, cx: &mut Context<Self>) {
        let q = self.quick.input.read(cx).value().to_string();
        let files = self.all_files.clone();
        let lower = self.all_lower.clone();
        let qb: Vec<u8> = q.to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        let mut scored: Vec<(i32, usize)> = lower.iter().enumerate().filter_map(|(i, p)| fuzzy_lower(&qb, p.as_bytes()).map(|s| (s, i))).collect();
        if q.trim().is_empty() {
            // No query: open tabs first, then the folder in its own order.
            let open: Vec<String> = self.docs.iter().map(|d| d.path.to_string_lossy().into_owned()).collect();
            scored.sort_by_key(|(_, i)| if open.contains(&files[*i]) { 0 } else { 1 });
        } else {
            scored.sort_by(|a, b| b.0.cmp(&a.0));
        }
        self.quick.hits = scored.into_iter().take(MAX_HITS).map(|(_, i)| i).collect();
        self.quick.sel = 0;
        cx.notify();
    }

    pub(crate) fn on_quick_open(&mut self, _: &QuickOpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.root.is_none() {
            self.on_open_folder_from_quick(window, cx);
            return;
        }
        self.quick.open = true;
        self.quick.input.update(cx, |s, cx| s.set_value("", window, cx));
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    fn on_open_folder_from_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(OpenFolder), cx);
    }

    pub(crate) fn quick_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.open = false;
        if let Some(ix) = self.active {
            let h = self.docs[ix].editor.focus_handle(cx);
            h.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_quick_close(&mut self, _: &QuickClose, window: &mut Window, cx: &mut Context<Self>) {
        self.quick_close(window, cx);
    }

    pub(crate) fn on_quick_up(&mut self, _: &QuickUp, _: &mut Window, cx: &mut Context<Self>) {
        self.quick.sel = self.quick.sel.saturating_sub(1);
        cx.notify();
    }

    pub(crate) fn on_quick_down(&mut self, _: &QuickDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.quick.sel + 1 < self.quick.hits.len() {
            self.quick.sel += 1;
        }
        cx.notify();
    }

    pub(crate) fn quick_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&ix) = self.quick.hits.get(self.quick.sel) else { return };
        let path = PathBuf::from(&self.all_files[ix]);
        self.quick.open = false;
        self.open_file(path, window, cx);
        cx.notify();
    }

    pub(crate) fn render_quick_open(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let q = &self.quick;
        let mut list = v_flex().id("quick-list").max_h(theme::u(420.)).overflow_y_scroll().py_1();
        if q.hits.is_empty() {
            list = list.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child("No file matches. Try fewer letters."));
        }
        for (row, &ix) in q.hits.iter().enumerate() {
            let full = &self.all_files[ix];
            let rel = self.rel(full);
            let (dir, name) = match rel.rfind(['\\', '/']) {
                Some(i) => (&rel[..i], &rel[i + 1..]),
                None => ("", rel),
            };
            let (icon, tint) = crate::files::icon_for(name);
            let selected = row == q.sel;
            let path = PathBuf::from(full);
            list = list.child(
                h_flex()
                    .id(("quick", row))
                    .mx_1()
                    .px_2()
                    .h(theme::u(26.))
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(c(theme::SELECT)))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                    .child(div().flex_none().text_sm().text_color(c(theme::FG_STRONG)).child(name.to_string()))
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(dir.to_string()))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.quick.open = false;
                        this.open_file(path.clone(), w, cx);
                        cx.notify();
                    })),
            );
        }
        // The rest of the window dims so the eye goes to the box; a click there closes it.
        // (A real blur would re-render the whole window under it every frame; a dim layer
        // costs nothing.) The box sits centered under the title bar's search box.
        let fade = || Animation::new(std::time::Duration::from_millis(120)).with_easing(ease_out_quint());
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("quick-scrim")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui_kit::black().opacity(0.38))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.quick_close(w, cx)))
                    .with_animation("quick-scrim-in", fade(), |d, t| d.opacity(t)),
            )
            .child(
                div().absolute().top(theme::u(38.)).left_0().right_0().flex().justify_center().child(
                    v_flex()
                        .id("quick-open")
                        .w(theme::u(600.))
                        .max_w(relative(0.92))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(c(theme::BORDER_STRONG))
                        .bg(gpui_kit::Hsla::from(rgb(theme::palette().swatches()[1])))
                        .shadow_lg()
                        .overflow_hidden()
                        .child(div().p_2().key_context("QuickOpen").child(Input::new(&q.input).small()))
                        .child(list)
                        .with_animation("quick-open-in", fade(), |d, t| d.opacity(t)),
                ),
            )
    }
}

/// Every file in the explorer's scan, as full paths (folders left out).
pub fn file_list(paths: Vec<String>, dirs: &std::collections::HashSet<String>) -> Vec<String> {
    paths.into_iter().filter(|p| !dirs.contains(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::fuzzy;

    #[test]
    fn ranks_file_names_first() {
        let a = fuzzy("app", "src\\app.py").unwrap();
        let b = fuzzy("app", "apple\\pie\\x.txt").unwrap();
        assert!(a > b);
        assert!(fuzzy("apy", "app.py").is_some());
        assert!(fuzzy("zz", "app.py").is_none());
        assert!(fuzzy("req", "requirements.txt").unwrap() > fuzzy("req", "src\\query\\r.txt").unwrap_or(i32::MIN));
        // whole word in the name beats letters spread over a long path
        assert!(fuzzy("rea", "readme.md").unwrap() > fuzzy("rea", "promo-video\\out\\fix1\\a.png").unwrap());
        assert!(fuzzy("rea", "docs\\area.md").unwrap() > fuzzy("rea", "promo-video\\out\\fix1\\a.png").unwrap());
    }
}
