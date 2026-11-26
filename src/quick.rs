
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    h_flex,
    input::{InputEvent, InputState},
    v_flex, Icon,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;

const MAX_HITS: usize = 60;
const OFF_THREAD_QUICK: usize = 8_000;
static QUICK_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
pub fn fuzzy(query: &str, path: &str) -> Option<i32> {
    let q: Vec<u8> = query.to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    fuzzy_lower(&q, path.to_lowercase().as_bytes())
}

pub fn fuzzy_lower(q: &[u8], p: &[u8]) -> Option<i32> {
    crate::accel::fuzzy_score(q, p)
}

fn score_top_k(qb: &[u8], lower: &crate::memtrim::FlatStrings, k: usize) -> Vec<usize> {
    let mut scored: Vec<(i32, usize)> = lower
        .iter()
        .enumerate()
        .filter_map(|(i, p)| fuzzy_lower(qb, p.as_bytes()).map(|s| (s, i)))
        .collect();
    if scored.len() > k {
        scored.select_nth_unstable_by(k, |a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored.truncate(k);
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Files,
    Commands,
    Line,
    Wsl,
    Themes,
    IconThemes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Picker {
    Standard,
    Themes,
    IconThemes,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WslRow {
    Up,
    Open,
    OpenPath(String),
    Dir(String),
}

pub fn placeholder() -> &'static str {
    tr("Type part of a file name ( > commands, : a line, wsl: a distro )")
}

pub struct QuickOpen {
    pub open: bool,
    pub picker: Picker,
    pub input: Entity<InputState>,
    pub hits: Vec<usize>,
    pub sel: usize,
    pub mode: Mode,
    pub wsl: Vec<String>,
    pub wsl_loading: bool,
    pub wsl_browse: Option<(String, String)>,
    pub wsl_dirs: Vec<String>,
    pub wsl_home: String,
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
        (
            Self {
                open: false,
                picker: Picker::Standard,
                input,
                hits: Vec::new(),
                sel: 0,
                mode: Mode::Files,
                wsl: Vec::new(),
                wsl_loading: false,
                wsl_browse: None,
                wsl_dirs: Vec::new(),
                wsl_home: String::new(),
                wsl_rows: Vec::new(),
            },
            sub,
        )
    }
}

impl Workbench {
    pub(crate) fn quick_filter(&mut self, cx: &mut Context<Self>) {
        let q = self.quick.input.read(cx).value().to_string();
        self.quick.mode = match self.quick.picker {
            Picker::Themes => Mode::Themes,
            Picker::IconThemes => Mode::IconThemes,
            Picker::Standard => mode_of(&q),
        };
        match self.quick.mode {
            Mode::Themes => {
                let query: Vec<u8> = q.to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
                let list = theme::all();
                let mut scored: Vec<(i32, usize)> = list
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| {
                        let hay = format!("{} {}", p.name, tr(p.note)).to_lowercase();
                        fuzzy_lower(&query, hay.as_bytes()).map(|s| (s, i))
                    })
                    .collect();
                if !query.is_empty() {
                    scored.sort_by_key(|a| std::cmp::Reverse(a.0));
                }
                let hits: Vec<usize> = scored.into_iter().map(|(_, i)| i).collect();
                let active_id = self.committed_theme_id();
                self.quick.sel = if query.is_empty() {
                    hits.iter().position(|&ix| list[ix].id == active_id).unwrap_or(0)
                } else {
                    0
                };
                self.quick.hits = hits;
                cx.notify();
                return;
            }
            Mode::IconThemes => {
                let query: Vec<u8> = q.to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
                let list = crate::ext::vsx_icons::list();
                let mut scored: Vec<(i32, usize)> = Vec::with_capacity(list.len() + 1);
                let builtin_hay = format!("{} {}", tr("FluxCode (Built-in)"), tr("Built-in file and folder icons")).to_lowercase();
                if let Some(s) = fuzzy_lower(&query, builtin_hay.as_bytes()) {
                    scored.push((s, 0));
                }
                for (i, m) in list.iter().enumerate() {
                    let hay = format!("{} {}", m.label, m.ext_display).to_lowercase();
                    if let Some(s) = fuzzy_lower(&query, hay.as_bytes()) {
                        scored.push((s, i + 1));
                    }
                }
                if !query.is_empty() {
                    scored.sort_by_key(|a| std::cmp::Reverse(a.0));
                }
                let hits: Vec<usize> = scored.into_iter().map(|(_, i)| i).collect();
                let active = crate::ext::vsx_icons::active_id();
                self.quick.sel = if query.is_empty() {
                    let target = list.iter().position(|m| m.id == active).map_or(0, |i| i + 1);
                    hits.iter().position(|&ix| ix == target).unwrap_or(0)
                } else {
                    0
                };
                self.quick.hits = hits;
                cx.notify();
                return;
            }
            Mode::Commands => {
                let query: Vec<u8> = q.trim_start_matches('>').to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
                let list = crate::commands::commands();
                let mut scored: Vec<(i32, usize)> = list
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
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
        let gen = QUICK_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if q.trim().is_empty() {
            let open: Vec<String> = self.docs.iter().map(|d| d.path.to_string_lossy().into_owned()).collect();
            let mut hits = Vec::with_capacity(MAX_HITS.min(files.len()));
            for (i, f) in files.iter().enumerate() {
                if open.contains(f) {
                    hits.push(i);
                    if hits.len() >= MAX_HITS {
                        break;
                    }
                }
            }
            if hits.len() < MAX_HITS {
                for (i, f) in files.iter().enumerate() {
                    if !open.contains(f) {
                        hits.push(i);
                        if hits.len() >= MAX_HITS {
                            break;
                        }
                    }
                }
            }
            self.quick.hits = hits;
            self.quick.sel = 0;
            cx.notify();
            return;
        }
        let qb: Vec<u8> = q.to_lowercase().bytes().filter(|c| !c.is_ascii_whitespace()).collect();
        if lower.len() > OFF_THREAD_QUICK {
            cx.spawn(async move |this, cx| {
                let hits = cx.background_spawn(async move { score_top_k(&qb, &lower, MAX_HITS) }).await;
                if QUICK_GEN.load(std::sync::atomic::Ordering::Relaxed) != gen {
                    return;
                }
                _ = this.update(cx, |this, cx| {
                    if this.quick.open && this.quick.mode == Mode::Files {
                        this.quick.hits = hits;
                        this.quick.sel = 0;
                        cx.notify();
                    }
                });
            })
            .detach();
            return;
        }
        self.quick.hits = score_top_k(&qb, &lower, MAX_HITS);
        self.quick.sel = 0;
        cx.notify();
    }

    pub(crate) fn on_quick_open(&mut self, _: &QuickOpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.root.is_none() {
            self.on_open_folder_from_quick(window, cx);
            return;
        }
        self.quick.open = true;
        self.quick.picker = Picker::Standard;
        self.quick.input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder(placeholder(), window, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    pub(crate) fn quick_open_with(&mut self, prefix: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.open = true;
        self.quick.picker = Picker::Standard;
        let p = prefix.to_string();
        self.quick.input.update(cx, |s, cx| {
            s.set_value(p.clone(), window, cx);
            s.set_placeholder(placeholder(), window, cx);
            let end = p.len();
            s.set_selected_range(end..end, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.open = true;
        self.quick.picker = Picker::Themes;
        self.begin_theme_preview();
        self.quick.input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder(tr("Select Color Theme (type to filter)"), window, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    pub(crate) fn open_icon_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.open = true;
        self.quick.picker = Picker::IconThemes;
        self.quick.input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder(tr("Select File Icon Theme (type to filter)"), window, cx);
        });
        let h = self.quick.input.focus_handle(cx);
        h.focus(window, cx);
        self.quick_filter(cx);
    }

    pub(crate) fn on_choose_theme(&mut self, _: &ChooseTheme, window: &mut Window, cx: &mut Context<Self>) {
        self.open_theme_picker(window, cx);
    }

    pub(crate) fn on_choose_icon_theme(&mut self, _: &ChooseIconTheme, window: &mut Window, cx: &mut Context<Self>) {
        self.open_icon_theme_picker(window, cx);
    }

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
        self.quick.picker = Picker::Standard;
        self.cancel_theme_preview(window, cx);
        if let Some(ed) = self.active.and_then(|ix| self.docs.get(ix)).and_then(|d| d.editor.as_ref()) {
            let h = ed.focus_handle(cx);
            h.focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_quick_close(&mut self, _: &QuickClose, window: &mut Window, cx: &mut Context<Self>) {
        self.quick_close(window, cx);
    }

    pub(crate) fn on_quick_up(&mut self, _: &QuickUp, window: &mut Window, cx: &mut Context<Self>) {
        self.quick.sel = self.quick.sel.saturating_sub(1);
        if self.quick.mode == Mode::Themes {
            if let Some(&ix) = self.quick.hits.get(self.quick.sel) {
                if let Some(p) = theme::all().get(ix) {
                    self.preview_theme(p.id, window, cx);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn on_quick_down(&mut self, _: &QuickDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.quick.sel + 1 < self.quick.hits.len() {
            self.quick.sel += 1;
        }
        if self.quick.mode == Mode::Themes {
            if let Some(&ix) = self.quick.hits.get(self.quick.sel) {
                if let Some(p) = theme::all().get(ix) {
                    self.preview_theme(p.id, window, cx);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn quick_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.quick.mode {
            Mode::Themes => {
                let Some(&ix) = self.quick.hits.get(self.quick.sel) else { return };
                let list = theme::all();
                self.quick.open = false;
                self.quick.picker = Picker::Standard;
                if let Some(p) = list.get(ix) {
                    self.commit_theme(p.id, window, cx);
                }
                if let Some(ed) = self.active.and_then(|i| self.docs.get(i)).and_then(|d| d.editor.as_ref()) {
                    ed.focus_handle(cx).focus(window, cx);
                } else {
                    self.focus.focus(window, cx);
                }
                return;
            }
            Mode::IconThemes => {
                let Some(&ix) = self.quick.hits.get(self.quick.sel) else { return };
                let list = crate::ext::vsx_icons::list();
                let tid = if ix == 0 {
                    String::new()
                } else {
                    list.get(ix - 1).map(|m| m.id.clone()).unwrap_or_default()
                };
                self.quick_close(window, cx);
                self.set_icon_theme(&tid, cx);
                return;
            }
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
        self.agent_full = false;
        self.open_file(path, window, cx);
        cx.notify();
    }

    pub(crate) fn render_quick_open(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let q = &self.quick;
        let motion = !cx.reduce_motion();
        let view_id = window.current_view();
        let liq = crate::ui::liquid::keyed_state("quick-row-liq", window, cx);
        let row_cross = 500.0f32;
        let origin_probe = crate::ui::liquid::keyed_origin_probe(liq.clone(), view_id);

        let wrap_rows = |rows_box: Div, count: usize, sel: Option<usize>| -> Stateful<Div> {
            let (frame, _) = crate::ui::liquid::keyed_sync_and_sample(&liq, true, count, sel, row_cross, 165, motion);
            let layer = frame.map(|fr| {
                let st = crate::ui::liquid::LiquidStyle {
                    inset_a: 4.0,
                    inset_b: 4.0,
                    cross_base: row_cross,
                    max_squeeze: 3.5,
                    radius: 6.0,
                    capillary: 26.0,
                    active_bg: c(theme::ACCENT).opacity(0.20),
                    active_border: c(theme::ACCENT).opacity(0.55),
                    hover_bg: c(theme::BG_HOVER).opacity(0.75),
                    hover_border: c(theme::BORDER_STRONG).opacity(0.78),
                    union_border: c(theme::ACCENT).opacity(0.58),
                    bridge_bg: c(theme::ACCENT).opacity(0.18),
                };
                crate::ui::liquid::render_coalesced_layer("quick-liq-l", true, st, fr)
            });
            let inner = v_flex()
                .id("quick-rows-inner")
                .relative()
                .w_full()
                .on_hover({
                    let h = liq.clone();
                    move |entered: &bool, _, cx| {
                        if !*entered && motion {
                            crate::ui::liquid::keyed_hover_leave(&h, view_id, cx);
                        }
                    }
                })
                .child(origin_probe)
                .children(layer)
                .child(rows_box);
            v_flex().id("quick-list").max_h(theme::u(420.)).overflow_y_scroll().py_1().child(inner)
        };

        let mut rows_col = v_flex().w_full();
        match q.mode {
            Mode::Themes => {
                let all = theme::all();
                let committed = self.committed_theme_id();
                if q.hits.is_empty() {
                    rows_col = rows_col.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No matching color theme.")));
                    return self.quick_frame(wrap_rows(rows_col, 0, None), cx).into_any_element();
                }
                for (row, &ix) in q.hits.iter().enumerate() {
                    let Some(p) = all.get(ix) else { continue };
                    let selected = row == q.sel;
                    let on = p.id == committed;
                    let id = p.id;
                    let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), row, view_id);
                    let h_hov = liq.clone();
                    rows_col = rows_col.child(
                        h_flex()
                            .id(("quick-theme", row))
                            .relative()
                            .mx_1()
                            .px_2()
                            .h(theme::u(32.))
                            .gap_2p5()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(selected && !motion, |d| d.bg(c(theme::SELECT)))
                            .when(!motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                            .on_hover(cx.listener(move |this, entered: &bool, w, cx| {
                                if *entered {
                                    if motion {
                                        crate::ui::liquid::keyed_hover_enter(&h_hov, row, true, row_cross, 140, view_id, cx);
                                    }
                                    this.preview_theme(id, w, cx);
                                }
                            }))
                            .child(probe)
                            .child(crate::settings::theme_swatch(p))
                            .child(div().flex_none().text_sm().font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).text_color(c(theme::FG_STRONG)).child(p.name))
                            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(tr(p.note)))
                            .when(on, |h| h.child(Icon::new(gpui_kit::assets::IconName::Check).flex_none().size(theme::u(13.)).text_color(c(theme::ACCENT_TEXT))))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.quick.open = false;
                                this.quick.picker = Picker::Standard;
                                this.commit_theme(id, w, cx);
                                if let Some(ed) = this.active.and_then(|i| this.docs.get(i)).and_then(|d| d.editor.as_ref()) {
                                    ed.focus_handle(cx).focus(w, cx);
                                } else {
                                    this.focus.focus(w, cx);
                                }
                            })),
                    );
                }
                return self.quick_frame(wrap_rows(rows_col, q.hits.len(), Some(q.sel)), cx).into_any_element();
            }
            Mode::IconThemes => {
                let installed = crate::ext::vsx_icons::list();
                let active = crate::ext::vsx_icons::active_id();
                if q.hits.is_empty() {
                    rows_col = rows_col.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No matching file icon theme.")));
                    return self.quick_frame(wrap_rows(rows_col, 0, None), cx).into_any_element();
                }
                for (row, &ix) in q.hits.iter().enumerate() {
                    let selected = row == q.sel;
                    let (label, sub, tid, on) = if ix == 0 {
                        (tr("FluxCode (Built-in)").to_string(), tr("Built-in file and folder icons").to_string(), String::new(), active.is_empty())
                    } else if let Some(m) = installed.get(ix - 1) {
                        (m.label.clone(), m.ext_display.clone(), m.id.clone(), m.id == active)
                    } else {
                        continue;
                    };
                    let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), row, view_id);
                    let h_hov = liq.clone();
                    rows_col = rows_col.child(
                        h_flex()
                            .id(("quick-icon", row))
                            .relative()
                            .mx_1()
                            .px_2()
                            .h(theme::u(32.))
                            .gap_2p5()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(selected && !motion, |d| d.bg(c(theme::SELECT)))
                            .when(!motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                            .on_hover(move |entered: &bool, _, cx| {
                                if *entered && motion {
                                    crate::ui::liquid::keyed_hover_enter(&h_hov, row, true, row_cross, 140, view_id, cx);
                                }
                            })
                            .child(probe)
                            .child(div().flex_none().text_sm().font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).text_color(c(theme::FG_STRONG)).child(label))
                            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(sub))
                            .when(on, |h| h.child(Icon::new(gpui_kit::assets::IconName::Check).flex_none().size(theme::u(13.)).text_color(c(theme::ACCENT_TEXT))))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.quick_close(w, cx);
                                this.set_icon_theme(&tid, cx);
                            })),
                    );
                }
                if installed.is_empty() {
                    rows_col = rows_col.child(
                        h_flex()
                            .id("quick-icon-browse")
                            .mx_1()
                            .mt_1()
                            .px_2()
                            .h(theme::u(28.))
                            .gap_2()
                            .rounded(px(6.))
                            .border_t_1()
                            .border_color(c(theme::BORDER))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(theme::FG_MUTED))
                            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                            .child(Icon::new(gpui_kit::assets::IconName::Search).size(theme::u(12.)))
                            .child(tr("Browse icon themes in Extensions"))
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.quick_close(w, cx);
                                this.close_pages(w, cx);
                                this.reveal_view(SideView::Extensions, cx);
                                this.ext.filter = crate::ext::Filter::Market;
                                this.ext.query.update(cx, |s, cx| s.set_value("icon theme", w, cx));
                                this.market_search(0, cx);
                            })),
                    );
                }
                return self.quick_frame(wrap_rows(rows_col, q.hits.len(), Some(q.sel)), cx).into_any_element();
            }
            Mode::Commands => {
                let all = crate::commands::commands();
                if q.hits.is_empty() {
                    rows_col = rows_col.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No command matches.")));
                    return self.quick_frame(wrap_rows(rows_col, 0, None), cx).into_any_element();
                }
                let has_liq = motion && !q.hits.is_empty();
                for (row, &ix) in q.hits.iter().enumerate() {
                    let Some(cmd) = all.get(ix) else { continue };
                    let selected = row == q.sel;
                    let make = cmd.action;
                    let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), row, view_id);
                    let h_hov = liq.clone();
                    rows_col = rows_col.child(
                        h_flex()
                            .id(("quick-cmd", row))
                            .relative()
                            .mx_1()
                            .px_2()
                            .h(theme::u(28.))
                            .gap_2()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(selected && !has_liq, |d| d.bg(c(theme::SELECT)))
                            .when(!motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                            .on_hover(move |entered: &bool, _, cx| {
                                if *entered && motion {
                                    crate::ui::liquid::keyed_hover_enter(&h_hov, row, true, row_cross, 140, view_id, cx);
                                }
                            })
                            .child(probe)
                            .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG_STRONG)).child(tr(cmd.label)))
                            .when(!cmd.keys.is_empty(), |h| h.child(keys_chips(cmd.keys)))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.quick_close(w, cx);
                                w.dispatch_action(make(), cx);
                            })),
                    );
                }
                return self.quick_frame(wrap_rows(rows_col, q.hits.len(), Some(q.sel)), cx).into_any_element();
            }
            Mode::Line => {
                let qv = q.input.read(cx).value().to_string();
                let (cur, total) = self
                    .active
                    .and_then(|ix| self.docs.get(ix))
                    .and_then(|d| d.editor.as_ref())
                    .map(|ed| {
                        let e = ed.read(cx);
                        (e.cursor_position().line + 1, e.value().lines().count().max(1))
                    })
                    .unwrap_or((1, 1));
                let text = match crate::commands::parse_line_query(&qv) {
                    Some((l, 1)) => crate::trf!("Go to line {l}."; l => l),
                    Some((l, col)) => crate::trf!("Go to line {l}, column {col}."; l => l, col => col),
                    None => crate::trf!("Current line: {cur} of {total}. Type a line number to go there."; cur => cur, total => total),
                };
                rows_col = rows_col.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_MUTED)).child(text));
                return self.quick_frame(v_flex().id("quick-list").max_h(theme::u(420.)).overflow_y_scroll().py_1().child(rows_col), cx).into_any_element();
            }
            Mode::Wsl => {
                if let Some((_, path)) = &q.wsl_browse {
                    rows_col = rows_col.child(div().px_3().py_1().text_xs().text_color(c(theme::FG_DIM)).child(path.clone()));
                    let mut n = 0usize;
                    for (row, item) in q.wsl_rows.iter().enumerate() {
                        n += 1;
                        let selected = row == q.sel;
                        let icon_sz = theme::u(14.);
                        let (label, icon) = match item {
                            WslRow::Up => ("..".to_string(), Icon::new(gpui_kit::assets::IconName::ArrowUp).size(icon_sz).text_color(c(theme::FOLDER)).into_any_element()),
                            WslRow::Open => (tr("Open this folder").to_string(), crate::files::folder_icon_el("", true, false, icon_sz)),
                            WslRow::OpenPath(path) => (crate::trf!("Open {path}"; path => path), crate::files::folder_icon_el("", true, false, icon_sz)),
                            WslRow::Dir(name) => (name.clone(), crate::files::folder_icon_el(name, false, false, icon_sz)),
                        };
                        let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), row, view_id);
                        let h_hov = liq.clone();
                        rows_col = rows_col.child(
                            h_flex()
                                .id(("quick-wsl-row", row))
                                .relative()
                                .mx_1()
                                .px_2()
                                .h(theme::u(28.))
                                .gap_2()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .when(selected && !motion, |d| d.bg(c(theme::SELECT)))
                                .when(!motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                                .on_hover(move |entered: &bool, _, cx| {
                                    if *entered && motion {
                                        crate::ui::liquid::keyed_hover_enter(&h_hov, row, true, row_cross, 140, view_id, cx);
                                    }
                                })
                                .child(probe)
                                .child(icon)
                                .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG_STRONG)).child(label))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.quick.sel = row;
                                    this.quick_accept(w, cx);
                                })),
                        );
                    }
                    return self.quick_frame(wrap_rows(rows_col, n, (n > 0).then_some(q.sel)), cx).into_any_element();
                }
                if q.hits.is_empty() {
                    rows_col = rows_col.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No WSL distro found.")));
                    return self.quick_frame(wrap_rows(rows_col, 0, None), cx).into_any_element();
                }
                for (row, &ix) in q.hits.iter().enumerate() {
                    let Some(distro) = q.wsl.get(ix) else { continue };
                    let selected = row == q.sel;
                    let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), row, view_id);
                    let h_hov = liq.clone();
                    rows_col = rows_col.child(
                        h_flex()
                            .id(("quick-wsl", row))
                            .relative()
                            .mx_1()
                            .px_2()
                            .h(theme::u(28.))
                            .gap_2()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(selected && !motion, |d| d.bg(c(theme::SELECT)))
                            .when(!motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                            .on_hover(move |entered: &bool, _, cx| {
                                if *entered && motion {
                                    crate::ui::liquid::keyed_hover_enter(&h_hov, row, true, row_cross, 140, view_id, cx);
                                }
                            })
                            .child(probe)
                            .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG_STRONG)).child(distro.clone()))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.quick.sel = row;
                                this.quick_accept(w, cx);
                            })),
                    );
                }
                return self.quick_frame(wrap_rows(rows_col, q.hits.len(), Some(q.sel)), cx).into_any_element();
            }
            Mode::Files => {}
        }
        if q.hits.is_empty() {
            rows_col = rows_col.child(div().px_3().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No file matches. Try fewer letters.")));
            return self.quick_frame(wrap_rows(rows_col, 0, None), cx).into_any_element();
        }
        for (row, &ix) in q.hits.iter().enumerate() {
            let full = &self.all_files[ix];
            let rel = self.rel_display(full);
            let (dir, name) = match rel.rfind(['\\', '/']) {
                Some(i) => (&rel[..i], &rel[i + 1..]),
                None => ("", rel.as_str()),
            };
            let selected = row == q.sel;
            let path = PathBuf::from(full);
            let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), row, view_id);
            let h_hov = liq.clone();
            rows_col = rows_col.child(
                h_flex()
                    .id(("quick", row))
                    .relative()
                    .mx_1()
                    .px_2()
                    .h(theme::u(26.))
                    .gap_2()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(selected && !motion, |d| d.bg(c(theme::SELECT)))
                    .when(!motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                    .on_hover(move |entered: &bool, _, cx| {
                        if *entered && motion {
                            crate::ui::liquid::keyed_hover_enter(&h_hov, row, true, row_cross, 140, view_id, cx);
                        }
                    })
                    .child(probe)
                    .child(crate::files::file_icon_el(name, theme::u(14.)))
                    .child(div().flex_none().text_sm().text_color(c(theme::FG_STRONG)).child(name.to_string()))
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(dir.to_string()))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.quick.open = false;
                        this.open_file(path.clone(), w, cx);
                        cx.notify();
                    })),
            );
        }
        self.quick_frame(wrap_rows(rows_col, q.hits.len(), Some(q.sel)), cx).into_any_element()
    }

    fn quick_frame(&self, list: Stateful<Div>, cx: &mut Context<Self>) -> impl IntoElement {
        let q = &self.quick;
        let motion = !cx.reduce_motion();
        let fade = || Animation::new(std::time::Duration::from_millis(120)).with_easing(ease_out_quint());
        let scrim = div()
            .id("quick-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui_kit::black().opacity(0.38))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.quick_close(w, cx)));
        let scrim_el = if motion {
            scrim.with_animation("quick-scrim-in", fade(), |d, t| d.opacity(t)).into_any_element()
        } else {
            scrim.into_any_element()
        };
        let panel = v_flex()
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
            .child(div().p_2().key_context("QuickOpen").child(crate::ui::McInput::new(&q.input).small()))
            .child(list);
        let panel_el = if motion {
            panel.with_animation("quick-open-in", fade(), |d, t| d.opacity(t).mt(px((1. - t) * 4.))).into_any_element()
        } else {
            panel.into_any_element()
        };
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(scrim_el)
            .child(div().absolute().top(theme::u(38.)).left_0().right_0().flex().justify_center().child(panel_el))
    }
}

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
        assert!(fuzzy("rea", "readme.md").unwrap() > fuzzy("rea", "promo-video\\out\\fix1\\a.png").unwrap());
        assert!(fuzzy("rea", "docs\\area.md").unwrap() > fuzzy("rea", "promo-video\\out\\fix1\\a.png").unwrap());
    }

    #[test]
    fn score_top_k_returns_best_matches_in_order() {
        let raw: Vec<String> = (0..100).map(|i| format!("src/other/file_{i}.rs")).chain(["src/main.rs".to_string(), "docs/main_guide.md".to_string()]).collect();
        let paths = crate::memtrim::FlatStrings::from_iter(raw.iter().map(String::as_str));
        let hits = super::score_top_k(b"main", &paths, 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(&paths[hits[0]], "src/main.rs");
        assert_eq!(&paths[hits[1]], "docs/main_guide.md");
    }

    #[test]
    fn command_palette_includes_theme_and_icon_theme_pickers() {
        let cmds = crate::commands::commands();
        assert!(cmds.iter().any(|c| c.label == "Preferences: Color Theme" && c.keys == "Ctrl+K Ctrl+T"));
        assert!(cmds.iter().any(|c| c.label == "Preferences: File Icon Theme"));
    }
}
