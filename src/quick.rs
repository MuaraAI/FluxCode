//! Go to File (Ctrl+P, or a click on the search box in the title bar), like
//! VS Code's Quick Open: type part of a name, Enter opens the file.
//!
//! The file list comes from the explorer's scan (no extra disk walk), and the
//! match is a small subsequence score, so it stays instant on large folders.

use crate::i18n::tr;
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

/// What the box is doing, from the first character typed (as in VS Code).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Files,
    /// ">": the Command Palette.
    Commands,
    /// ":": Go to Line.
    Line,
    /// "wsl:": pick a WSL2 distro to open.
    Wsl,
}

pub fn mode_of(q: &str) -> Mode {
    if q.starts_with("wsl:") {
        return Mode::Wsl;
    }
    match q.chars().next() {
        Some('>') => Mode::Commands,
        Some(':') => Mode::Line,
        _ => Mode::Files,
    }
}

/// One row of the `wsl:` folder browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WslRow {
    Up,
    Open,
    /// A path typed into the box, opened as it is.
    OpenPath(String),
    Dir(String),
}

/// The quick box's placeholder, in one place so the WSL browser can put it back.
pub fn placeholder() -> &'static str {
    tr("Type part of a file name ( > commands, : a line, wsl: a distro )")
}

pub struct QuickOpen {
    pub open: bool,
    pub input: Entity<InputState>,
    /// Indices into `Workbench::all_files` (files) or `commands::commands()`, best first.
    pub hits: Vec<usize>,
    pub sel: usize,
    pub mode: Mode,
    /// The distros the `wsl:` mode lists, read once.
    pub wsl: Vec<String>,
    /// A distro list is being read off the UI thread.
    pub wsl_loading: bool,
    /// The distro and folder the `wsl:` mode is browsing, once a distro is picked.
    pub wsl_browse: Option<(String, String)>,
    /// The folders of the folder being browsed.
    pub wsl_dirs: Vec<String>,
    /// The distro's home folder, so `~` can be typed.
    pub wsl_home: String,
    /// The rows the browse mode shows: up, open this folder, then the folders.
    pub wsl_rows: Vec<WslRow>,
}

impl QuickOpen {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> (Self, Subscription) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder()));
        let sub = cx.subscribe_in(&input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => this.quick_filter(cx),
            InputEvent::PressEnter { .. } => this.quick_accept(window, cx),
            _ => {}
        });
        (Self { open: false, input, hits: Vec::new(), sel: 0, mode: Mode::Files, wsl: Vec::new(), wsl_loading: false, wsl_browse: None, wsl_dirs: Vec::new(), wsl_home: String::new(), wsl_rows: Vec::new() }, sub)
    }
}

impl Workbench {
    pub(crate) fn quick_filter(&mut self, cx: &mut Context<Self>) {
        let q = self.quick.input.read(cx).value().to_string();
        self.quick.mode = mode_of(&q);
        match self.quick.mode {
            Mode::Commands => {
                let query: Vec<u8> = q.trim_start_matches('>').to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
                let list = crate::commands::commands();
                let mut scored: Vec<(i32, usize)> = list
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        // Both the English name and the translated one match.
                        let hay = format!("{} {}", c.label, tr(c.label)).to_lowercase();
                        fuzzy_lower(&query, hay.as_bytes()).map(|s| (s, i))
                    })
                    .collect();
                if !query.is_empty() {
                    scored.sort_by_key(|a| std::cmp::Reverse(a.0));
                }
                self.quick.hits = scored.into_iter().map(|(_, i)| i).collect();
                self.quick.sel = 0;
                cx.notify();
                return;
            }
            Mode::Line => {
                self.quick.hits.clear();
                self.quick.sel = 0;
                cx.notify();
                return;
            }
            Mode::Wsl => {
                if self.quick.wsl_browse.is_some() {
                    let typed = q.trim_start_matches("wsl:").trim();
                    // A typed path opens as it is, without walking to it.
                    if typed.starts_with('/') || typed.starts_with('~') {
                        let path = if typed == "~" {
                            self.quick.wsl_home.clone()
                        } else if let Some(rest) = typed.strip_prefix("~/") {
                            format!("{}/{}", self.quick.wsl_home.trim_end_matches('/'), rest)
                        } else {
                            typed.to_string()
                        };
                        self.quick.wsl_rows = vec![WslRow::OpenPath(path)];
                        self.quick.hits = vec![0];
                        self.quick.sel = 0;
                        cx.notify();
                        return;
                    }
                    let query = typed.to_lowercase();
                    let mut rows = vec![WslRow::Up, WslRow::Open];
                    rows.extend(
                        self.quick
                            .wsl_dirs
                            .iter()
                            .filter(|d| query.is_empty() || d.to_lowercase().contains(&query))
                            .cloned()
                            .map(WslRow::Dir),
                    );
                    self.quick.wsl_rows = rows;
                    self.quick.hits = (0..self.quick.wsl_rows.len()).collect();
                    self.quick.sel = 0;
                    cx.notify();
                    return;
                }
                // `wsl.exe -l` starts the WSL service, so ask off the UI thread.
                if self.quick.wsl.is_empty() && !self.quick.wsl_loading {
                    self.quick.wsl_loading = true;
                    cx.spawn(async move |this, cx| {
                        let list = cx.background_spawn(async move { crate::remote::distros() }).await;
                        _ = this.update(cx, |this, cx| {
                            this.quick.wsl = list;
                            this.quick.wsl_loading = false;
                            this.quick_filter(cx);
                        });
                    })
                    .detach();
                }
                let query = q.trim_start_matches("wsl:").trim().to_lowercase();
                let qb = query.as_bytes();
                let mut scored: Vec<(i32, usize)> = self
                    .quick
                    .wsl
                    .iter()
                    .enumerate()
                    .filter_map(|(i, d)| fuzzy_lower(qb, d.to_lowercase().as_bytes()).map(|s| (s, i)))
                    .collect();
                if !query.is_empty() {
                    scored.sort_by_key(|a| std::cmp::Reverse(a.0));
                }
                self.quick.hits = scored.into_iter().map(|(_, i)| i).collect();
                self.quick.sel = 0;
                cx.notify();
                return;
            }
            Mode::Files => {}
        }
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
        self.quick.input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder(placeholder(), window, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    /// Open the box with `prefix` typed (">" commands, ":" a line), caret at the end.
    pub(crate) fn quick_open_with(&mut self, prefix: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.open = true;
        let p = prefix.to_string();
        self.quick.input.update(cx, |s, cx| {
            s.set_value(p.clone(), window, cx);
            let end = p.len();
            s.set_selected_range(end..end, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    /// Show the folders of `path` in the `wsl:` box, so a folder can be picked.
    fn wsl_browse_to(&mut self, distro: String, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.open = true;
        self.quick.wsl_browse = Some((distro.clone(), path.clone()));
        self.quick.wsl_dirs.clear();
        self.quick.wsl_rows.clear();
        self.quick.input.update(cx, |s, cx| {
            s.set_value("wsl:", window, cx);
            s.set_placeholder(path.clone(), window, cx);
            s.set_selected_range(4..4, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
        let p = path.clone();
        cx.spawn(async move |this, cx| {
            let p2 = p.clone();
            let dirs = cx.background_spawn(async move { crate::remote::list_dirs(&distro, &p2) }).await;
            _ = this.update(cx, |this, cx| {
                if this.quick.wsl_browse.as_ref().is_some_and(|(_, cur)| *cur == p) {
                    this.quick.wsl_dirs = dirs;
                    this.quick_filter(cx);
                }
            });
        })
        .detach();
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
        match self.quick.mode {
            Mode::Commands => {
                let Some(&ix) = self.quick.hits.get(self.quick.sel) else { return };
                let list = crate::commands::commands();
                self.quick_close(window, cx);
                if let Some(c) = list.get(ix) {
                    window.dispatch_action((c.action)(), cx);
                }
                return;
            }
            Mode::Line => {
                let q = self.quick.input.read(cx).value().to_string();
                if let Some((line, col)) = crate::commands::parse_line_query(&q) {
                    self.quick.open = false;
                    self.go_to_line_col(line, col, window, cx);
                    cx.notify();
                }
                return;
            }
            Mode::Wsl => {
                if let Some((distro, path)) = self.quick.wsl_browse.clone() {
                    let Some(row) = self.quick.wsl_rows.get(self.quick.sel).cloned() else { return };
                    match row {
                        WslRow::Up if path == "/" => {
                            // At the root, ".." goes back to the distro list.
                            self.quick.wsl_browse = None;
                            self.quick.wsl_dirs.clear();
                            self.quick.wsl_rows.clear();
                            self.quick.input.update(cx, |s, cx| {
                                s.set_value("wsl:", window, cx);
                                s.set_placeholder(placeholder(), window, cx);
                            });
                            self.quick_filter(cx);
                        }
                        WslRow::Up => self.wsl_browse_to(distro, parent_of(&path), window, cx),
                        WslRow::OpenPath(path) => {
                            let d = distro.clone();
                            let p = path.clone();
                            cx.spawn_in(window, async move |this, cx| {
                                let (d2, p2) = (d.clone(), p.clone());
                                let ok = cx
                                    .background_spawn(async move {
                                        let t = crate::remote::WslTarget::new(&d2, "/");
                                        crate::remote::program_command(&t, "/", "sh", &["-c", r#"test -d "$1""#, "--", &p2])
                                            .output()
                                            .map(|o| o.status.success())
                                            .unwrap_or(false)
                                    })
                                    .await;
                                this.update_in(cx, |this, _window, cx| {
                                    if ok {
                                        this.quick.open = false;
                                        let target = crate::remote::WslTarget::new(&d, &p);
                                        this.open_folder(target.unc_root, cx);
                                    } else {
                                        this.status = crate::trf!("No folder named '{arg}' here."; arg => p).into();
                                        cx.notify();
                                    }
                                })
                                .ok()
                            })
                            .detach();
                        }
                        WslRow::Open => {
                            self.quick.open = false;
                            let target = crate::remote::WslTarget::new(&distro, &path);
                            self.open_folder(target.unc_root, cx);
                        }
                        WslRow::Dir(name) => {
                            let next = if path.ends_with('/') { format!("{path}{name}") } else { format!("{path}/{name}") };
                            self.wsl_browse_to(distro, next, window, cx);
                        }
                    }
                    return;
                }
                let Some(&ix) = self.quick.hits.get(self.quick.sel) else { return };
                let Some(distro) = self.quick.wsl.get(ix).cloned() else { return };
                let d = distro.clone();
                cx.spawn_in(window, async move |this, cx| {
                    let home = cx.background_spawn(async move { crate::remote::home_dir(&d) }).await;
                    this.update_in(cx, |this, window, cx| match home {
                        Some(home) => {
                            this.quick.wsl_home = home.clone();
                            this.wsl_browse_to(distro, home, window, cx)
                        }
                        None => {
                            this.status = crate::trf!("Couldn't reach {name}."; name => distro).into();
                            cx.notify();
                        }
                    })
                    .ok()
                })
                .detach();
                return;
            }
            Mode::Files => {}
        }
        let Some(&ix) = self.quick.hits.get(self.quick.sel) else { return };
        let path = PathBuf::from(&self.all_files[ix]);
        self.quick.open = false;
        self.open_file(path, window, cx);
        cx.notify();
    }

    pub(crate) fn render_quick_open(&self, cx: &mut Context<Self>) -> AnyElement {
        let q = &self.quick;
        let mut list = v_flex().id("quick-list").max_h(theme::u(420.)).overflow_y_scroll().py_1();
        match q.mode {
            Mode::Commands => {
                let all = crate::commands::commands();
                if q.hits.is_empty() {
                    list = list.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No command matches.")));
                }
                for (row, &ix) in q.hits.iter().enumerate() {
                    let Some(cmd) = all.get(ix) else { continue };
                    let selected = row == q.sel;
                    let make = cmd.action;
                    list = list.child(
                        h_flex()
                            .id(("quick-cmd", row))
                            .mx_1()
                            .px_2()
                            .h(theme::u(28.))
                            .gap_2()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .when(selected, |d| d.bg(c(theme::SELECT)))
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG_STRONG)).child(tr(cmd.label)))
                            .when(!cmd.keys.is_empty(), |h| h.child(keys_chips(cmd.keys)))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.quick_close(w, cx);
                                w.dispatch_action(make(), cx);
                            })),
                    );
                }
                return self.quick_frame(list, cx).into_any_element();
            }
            Mode::Line => {
                let qv = q.input.read(cx).value().to_string();
                let (cur, total) = self
                    .active
                    .map(|ix| {
                        let e = self.docs[ix].editor.read(cx);
                        (e.cursor_position().line + 1, e.value().lines().count().max(1))
                    })
                    .unwrap_or((1, 1));
                let text = match crate::commands::parse_line_query(&qv) {
                    Some((l, 1)) => crate::trf!("Go to line {l}."; l => l),
                    Some((l, col)) => crate::trf!("Go to line {l}, column {col}."; l => l, col => col),
                    None => crate::trf!("Current line: {cur} of {total}. Type a line number to go there."; cur => cur, total => total),
                };
                list = list.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_MUTED)).child(text));
                return self.quick_frame(list, cx).into_any_element();
            }
            Mode::Wsl => {
                if let Some((_, path)) = &q.wsl_browse {
                    list = list.child(div().px_3().py_1().text_xs().text_color(c(theme::FG_DIM)).child(path.clone()));
                    for (row, item) in q.wsl_rows.iter().enumerate() {
                        let selected = row == q.sel;
                        let (label, icon) = match item {
                            WslRow::Up => ("..".to_string(), gpui_kit::assets::IconName::ArrowUp),
                            WslRow::Open => (tr("Open this folder").to_string(), crate::files::folder_icon("", true)),
                            WslRow::OpenPath(path) => (crate::trf!("Open {path}"; path => path), crate::files::folder_icon("", true)),
                            WslRow::Dir(name) => (name.clone(), crate::files::folder_icon(name, false)),
                        };
                        list = list.child(
                            h_flex()
                                .id(("quick-wsl-row", row))
                                .mx_1()
                                .px_2()
                                .h(theme::u(28.))
                                .gap_2()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .when(selected, |d| d.bg(c(theme::SELECT)))
                                .hover(|s| s.bg(c(theme::BG_HOVER)))
                                .child(Icon::new(icon).size(theme::u(14.)).text_color(c(theme::FOLDER)))
                                .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG_STRONG)).child(label))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.quick.sel = row;
                                    this.quick_accept(w, cx);
                                })),
                        );
                    }
                    return self.quick_frame(list, cx).into_any_element();
                }
                if q.hits.is_empty() {
                    list = list.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No WSL distro found.")));
                }
                for (row, &ix) in q.hits.iter().enumerate() {
                    let Some(distro) = q.wsl.get(ix) else { continue };
                    let selected = row == q.sel;
                    list = list.child(
                        h_flex()
                            .id(("quick-wsl", row))
                            .mx_1()
                            .px_2()
                            .h(theme::u(28.))
                            .gap_2()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .when(selected, |d| d.bg(c(theme::SELECT)))
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG_STRONG)).child(distro.clone()))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.quick.sel = row;
                                this.quick_accept(w, cx);
                            })),
                    );
                }
                return self.quick_frame(list, cx).into_any_element();
            }
            Mode::Files => {}
        }
        if q.hits.is_empty() {
            list = list.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No file matches. Try fewer letters.")));
        }
        for (row, &ix) in q.hits.iter().enumerate() {
            let full = &self.all_files[ix];
            let rel = self.rel_display(full);
            let (dir, name) = match rel.rfind(['\\', '/']) {
                Some(i) => (&rel[..i], &rel[i + 1..]),
                None => ("", rel.as_str()),
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
        self.quick_frame(list, cx).into_any_element()
    }

    fn quick_frame(&self, list: Stateful<Div>, cx: &mut Context<Self>) -> impl IntoElement {
        let q = &self.quick;
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
                        .occlude()
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

/// "Ctrl+K Ctrl+O" as key caps: one small box per key, chords spaced apart.
pub fn keys_chips(keys: &'static str) -> Div {
    h_flex().flex_none().gap_1().children(keys.split(' ').enumerate().map(|(i, chord)| {
        h_flex().gap(px(2.)).when(i > 0, |h| h.ml_1()).children(chord.split('+').map(|k| {
            div()
                .px_1()
                .min_w(theme::u(16.))
                .flex()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(c(theme::BORDER_STRONG))
                .text_size(theme::u(11.))
                .text_color(c(theme::FG_MUTED))
                .child(k.to_string())
        }))
    }))
}

/// The folder above `path` inside the distro.
fn parent_of(path: &str) -> String {
    let p = path.trim_end_matches('/');
    match p.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => p[..i].to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::fuzzy;

    #[test]
    fn the_wsl_prefix_is_its_own_mode() {
        use super::{mode_of, Mode};
        assert_eq!(mode_of("wsl:"), Mode::Wsl);
        assert_eq!(mode_of("wsl:Ubu"), Mode::Wsl);
        assert_eq!(mode_of(">wsl"), Mode::Commands);
        assert_eq!(mode_of(":12"), Mode::Line);
        assert_eq!(mode_of("main.rs"), Mode::Files);
    }

    #[test]
    fn the_folder_above_a_path() {
        use super::parent_of;
        assert_eq!(parent_of("/home/viasco/Koding"), "/home/viasco");
        assert_eq!(parent_of("/home/viasco/Koding/"), "/home/viasco");
        assert_eq!(parent_of("/home"), "/");
        assert_eq!(parent_of("/"), "/");
    }

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
