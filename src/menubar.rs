use crate::i18n::tr;
use crate::settings::{load_json, save_json};
use crate::theme::{self, c};
use crate::ui::liquid::{
    self, fast_exp_neg, quintic_fade, CoalesceFrame, LiquidStyle, LiquidTrackState, Rect1D,
};
use crate::workbench::{
    About, AddFolderToWorkspace, CheckUpdates, ChooseIconTheme, ChooseTheme, CloseAllTabs,
    CloseFolder, CloseOtherTabs, CloseSavedTabs, CloseTab, CommandPalette, CopyLineDown,
    CopyLineUp, DeleteLine, DuplicateSelection, ExpandSelection, ExplorerNewFile,
    ExplorerNewFolder, FindInFiles, FocusMenuBar, GoToLine, GoToProblem, InsertLineAbove,
    InsertLineBelow, MoveLineDown, MoveLineUp, NavBack, NavForward, NewTerminal, NewTextFile,
    NewWindow, NextIconTheme, NextTab, NextTheme, OpenFile, OpenFolder, OpenFolderInWsl,
    PrevProblem, PrevTab, QuickOpenFile, Quit, ReloadPreview, ReopenClosedTab, RevertFile, RunFile,
    RunSelectedText, Save, SaveAll, SaveAs, ScmShow, ShowAgent, ShowExplorer, ShowExtensions,
    ShowOutput, ShowProblems, ShowRun, ShowSettings, ShowWhatsNew, ShowWizTree, ShrinkSelection,
    SideView, StopRun, TermClear, ToggleAgent, ToggleAutoSave, ToggleBlockComment,
    ToggleFullscreen, ToggleGlass, ToggleLineComment, ToggleLineNumbers, TogglePanel,
    TogglePreview, ToggleRenderWhitespace, ToggleSidebar, ToggleTerminal, ToggleWordWrap,
    Workbench, ZoomIn, ZoomOut, ZoomReset,
};
use gpui_kit::component::{h_flex, v_flex, Icon, IconName};
use gpui_kit::{linear_color_stop, linear_gradient, prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const TOP_MENU_COUNT: usize = 8;
pub const OVERFLOW_IX: usize = TOP_MENU_COUNT;
const MAX_RECENT_ENTRIES: usize = 10;
const MAX_RECENT_LABEL_CHARS: usize = 40;
const REPO_URL: &str = "https://github.com/MuaraAI/FluxCode";
const ISSUES_URL: &str = "https://github.com/MuaraAI/FluxCode/issues/new/choose";

const TOP_LABELS: [&str; TOP_MENU_COUNT] = [
    "File",
    "Edit",
    "Selection",
    "View",
    "Go",
    "Run",
    "Terminal",
    "Help",
];

// Matches VS Code's standard Windows/Linux menu mnemonics (Alt+key).
const TOP_MNEMONICS: [char; TOP_MENU_COUNT] = ['f', 'e', 's', 'v', 'g', 'r', 't', 'h'];

/// 64-byte cache-line-aligned optical sample for the liquid glass surface (Drepper 2007).
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassOptics {
    pub x: f32,
    pub width: f32,
    pub velocity_norm: f32,
    pub fresnel_rim: f32,
    pub caustic_lip: f32,
    pub depth_tint: f32,
    pub opacity: f32,
    pub lift_y: f32,
}

/// Schlick Fresnel approximation: F = F0 + (1 - F0)(1 - cos_theta)^5 in 3 multiplications (Schlick 1994).
#[inline(always)]
pub fn schlick_fresnel(cos_theta: f32, f0: f32) -> f32 {
    let u = (1.0 - cos_theta.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let u2 = u * u;
    let u5 = u2 * u2 * u;
    f0 + (1.0 - f0) * u5
}

/// Exact analytical speed of a critically-damped harmonic oscillator: |x'(t)| = omega^2 * t * e^(-omega*t) * |delta|.
/// Peaks at t = 1/omega with max value (omega / e) * |delta|, zero at t=0 and decays to 0 without overshoot.
#[inline(always)]
pub fn critically_damped_speed(t: f32, omega: f32, delta_abs: f32) -> f32 {
    let u = t.clamp(0.0, 1.0);
    let w = omega.max(0.5);
    let wu = w * u;
    (w * wu * fast_exp_neg(wu)) * delta_abs
}

/// Beer-Lambert optical attenuation T(d) = e^(-kappa * d) via Horner/minimax exp (Remez 1934).
#[inline(always)]
pub fn beer_lambert(depth: f32, kappa: f32) -> f32 {
    fast_exp_neg((depth.max(0.0)) * (kappa.max(0.0)))
}

/// Evaluate dropdown liquid glass glide kinematics & Fresnel specular optics at normalized time u in [0, 1].
#[inline(always)]
pub fn eval_dropdown_glass(
    from_x: f32,
    to_x: f32,
    from_w: f32,
    to_w: f32,
    u: f32,
    opening: bool,
    dark: bool,
) -> GlassOptics {
    let omega = 10.5f32;
    let s = liquid::critically_damped(u, omega);
    let x = from_x + (to_x - from_x) * s;
    let width = from_w + (to_w - from_w) * s;
    let delta_abs = (to_x - from_x).abs() + 0.5 * (to_w - from_w).abs();
    // Normalize speed against its theoretical peak (omega / e * 220px) so specular rim peaks cleanly mid-glide.
    let raw_v = critically_damped_speed(u, omega, delta_abs);
    let peak_ref = (omega * 0.367_879_45) * 220.0;
    let velocity_norm = (raw_v / peak_ref).clamp(0.0, 1.0);
    let cos_theta = (1.0 - 0.85 * velocity_norm).clamp(0.15, 1.0);
    let f0 = if dark { 0.14 } else { 0.09 };
    let fresnel_rim = schlick_fresnel(cos_theta, f0).clamp(f0, if dark { 0.42 } else { 0.28 });
    let caustic_lip = (0.04 + 0.14 * velocity_norm) * beer_lambert(0.35, 0.8);
    let depth_tint = 1.0 - 0.08 * beer_lambert(1.0 - velocity_norm * 0.5, 1.1);
    let open_t = if opening { quintic_fade(u) } else { 1.0 };
    GlassOptics {
        x,
        width,
        velocity_norm,
        fresnel_rim,
        caustic_lip,
        depth_tint,
        opacity: 0.15 + 0.85 * open_t,
        lift_y: (1.0 - open_t) * -5.0,
    }
}

/// O(1) stack-allocated prefix-sum overflow solver matching VS Code's `MenuBar.updateOverflowAction`.
/// Returns how many of the `widths` fit inside `avail_w`; when < N, reserves `more_w + gap` for the overflow trigger.
#[inline]
pub fn solve_overflow(
    widths: &[f32; TOP_MENU_COUNT],
    avail_w: f32,
    gap: f32,
    more_w: f32,
) -> usize {
    if avail_w <= 0.0 {
        return 0;
    }
    let mut prefix = [0.0f32; TOP_MENU_COUNT + 1];
    let mut acc = 0.0f32;
    let mut i = 0;
    while i < TOP_MENU_COUNT {
        if i > 0 {
            acc += gap;
        }
        acc += widths[i].max(0.0);
        prefix[i + 1] = acc;
        i += 1;
    }
    if prefix[TOP_MENU_COUNT] <= avail_w {
        return TOP_MENU_COUNT;
    }
    let mut k = TOP_MENU_COUNT - 1;
    loop {
        let needed = if k == 0 {
            more_w
        } else {
            prefix[k] + gap + more_w
        };
        if needed <= avail_w {
            return k;
        }
        if k == 0 {
            return 0;
        }
        k -= 1;
    }
}

/// Middle-truncate paths/labels like VS Code's `truncateMiddle` (`menubarControl.ts`).
pub fn truncate_middle(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars || max_chars < 5 {
        return s.to_string();
    }
    let head_len = (max_chars - 1) / 2;
    let tail_len = max_chars - 1 - head_len;
    let head: String = s.chars().take(head_len).collect();
    let tail: String = s.chars().skip(count - tail_len).collect();
    format!("{head}…{tail}")
}

/// Mirrors VS Code's `MenubarState` enum (`src/vs/base/browser/ui/menu/menubar.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MenubarMode {
    #[default]
    Visible,
    Focused(usize),
    Open(usize),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecentStore {
    pub folders: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
}

impl RecentStore {
    pub fn load() -> Self {
        load_json("recent.json")
    }

    pub fn save(&self) {
        save_json("recent.json", self);
    }

    pub fn push_folder(&mut self, p: PathBuf) {
        self.folders.retain(|x| x != &p);
        self.folders.insert(0, p);
        self.folders.truncate(MAX_RECENT_ENTRIES);
        self.save();
    }

    pub fn push_file(&mut self, p: PathBuf) {
        self.files.retain(|x| x != &p);
        self.files.insert(0, p);
        self.files.truncate(MAX_RECENT_ENTRIES);
        self.save();
    }

    pub fn clear(&mut self) {
        self.folders.clear();
        self.files.clear();
        self.save();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmenuId {
    OpenRecent,
    Preferences,
    Appearance,
    EditorLayout,
    SwitchEditor,
    OverflowMenu(usize),
}

#[derive(Clone)]
pub enum MenuCmd {
    Action(fn() -> Box<dyn Action>),
    EditorAction(fn() -> Box<dyn Action>),
    OpenRecentFolder(PathBuf),
    OpenRecentFile(PathBuf),
    ClearRecent,
    OpenUrl(&'static str),
}

#[derive(Clone)]
pub enum MenuEntry {
    Item {
        label: SharedString,
        shortcut: &'static str,
        checked: Option<bool>,
        enabled: bool,
        cmd: MenuCmd,
    },
    Submenu {
        id: SubmenuId,
        label: SharedString,
    },
    Separator,
}

impl MenuEntry {
    pub fn is_focusable(&self) -> bool {
        match self {
            MenuEntry::Item { enabled, .. } => *enabled,
            MenuEntry::Submenu { .. } => true,
            MenuEntry::Separator => false,
        }
    }
}

pub fn next_focusable(entries: &[MenuEntry], cur: Option<usize>, step: isize) -> Option<usize> {
    let n = entries.len();
    if n == 0 {
        return None;
    }
    let start = match cur {
        Some(i) => (i as isize + step).rem_euclid(n as isize) as usize,
        None => {
            if step >= 0 {
                0
            } else {
                n - 1
            }
        }
    };
    for off in 0..n {
        let ix = (start as isize + off as isize * step.signum()).rem_euclid(n as isize) as usize;
        if entries[ix].is_focusable() {
            return Some(ix);
        }
    }
    None
}

pub fn mnemonic_index(key: &str) -> Option<usize> {
    let ch = key.chars().next()?.to_ascii_lowercase();
    TOP_MNEMONICS.iter().position(|&m| m == ch)
}

#[derive(Clone, Copy, Debug)]
struct DropdownSlide {
    from_x: f32,
    to_x: f32,
    from_w: f32,
    to_w: f32,
    started: Instant,
    dur_ms: u32,
    opening: bool,
    seq: u64,
}

type BtnRects = Rc<RefCell<[Option<(f32, f32)>; TOP_MENU_COUNT + 1]>>;

pub struct MenubarState {
    pub mode: MenubarMode,
    pub focus: FocusHandle,
    pub mnemonics_visible: bool,
    pub item_focus: Option<usize>,
    pub sub_open: Option<(SubmenuId, usize, f32)>,
    pub sub_item_focus: Option<usize>,
    pub visible_count: Cell<usize>,
    pub bar_origin_x: Rc<Cell<f32>>,
    pub btn_rects: BtnRects,
    pub bar_liq: RefCell<LiquidTrackState>,
    pub drop_liq: RefCell<LiquidTrackState>,
    pub sub_liq: RefCell<LiquidTrackState>,
    dropdown_slide: Option<DropdownSlide>,
    pub recent: RecentStore,
    pub menu_seq: u64,
}

impl MenubarState {
    pub fn new(cx: &mut Context<Workbench>) -> Self {
        Self {
            mode: MenubarMode::Visible,
            focus: cx.focus_handle(),
            mnemonics_visible: false,
            item_focus: None,
            sub_open: None,
            sub_item_focus: None,
            visible_count: Cell::new(TOP_MENU_COUNT),
            bar_origin_x: Rc::new(Cell::new(0.0)),
            btn_rects: Rc::new(RefCell::new([None; TOP_MENU_COUNT + 1])),
            bar_liq: RefCell::new(LiquidTrackState::default()),
            drop_liq: RefCell::new(LiquidTrackState::default()),
            sub_liq: RefCell::new(LiquidTrackState::default()),
            dropdown_slide: None,
            recent: RecentStore::load(),
            menu_seq: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self.mode, MenubarMode::Open(_))
    }

    pub fn open_index(&self) -> Option<usize> {
        match self.mode {
            MenubarMode::Open(ix) => Some(ix),
            _ => None,
        }
    }

    pub fn active_slot(&self) -> Option<usize> {
        match self.mode {
            MenubarMode::Open(ix) | MenubarMode::Focused(ix) => Some(ix),
            MenubarMode::Visible => None,
        }
    }

    fn slot_count(&self) -> usize {
        let vis = self.visible_count.get().min(TOP_MENU_COUNT);
        if vis < TOP_MENU_COUNT {
            vis + 1
        } else {
            TOP_MENU_COUNT
        }
    }

    fn slot_to_menu_ix(&self, slot: usize) -> usize {
        let vis = self.visible_count.get().min(TOP_MENU_COUNT);
        if vis < TOP_MENU_COUNT && slot >= vis {
            OVERFLOW_IX
        } else {
            slot
        }
    }

    fn menu_ix_to_slot(&self, ix: usize) -> usize {
        let vis = self.visible_count.get().min(TOP_MENU_COUNT);
        if ix == OVERFLOW_IX || ix >= vis {
            vis
        } else {
            ix
        }
    }

    pub fn step_menu(&self, cur: usize, dir: isize) -> usize {
        let total = self.slot_count().max(1);
        let cur_slot = self.menu_ix_to_slot(cur).min(total - 1);
        let next_slot = (cur_slot as isize + dir).rem_euclid(total as isize) as usize;
        self.slot_to_menu_ix(next_slot)
    }

    fn dropdown_width_for(ix: usize) -> f32 {
        match ix {
            0 => 256.0, // File
            1 => 244.0, // Edit
            2 => 252.0, // Selection
            3 => 256.0, // View
            4 => 244.0, // Go
            5 => 228.0, // Run
            6 => 232.0, // Terminal
            7 => 232.0, // Help
            _ => 210.0, // Overflow
        }
    }

    fn target_x_for(&self, ix: usize) -> f32 {
        let rects = self.btn_rects.borrow();
        let fallback = self.bar_origin_x.get() + (ix.min(TOP_MENU_COUNT) as f32) * 44.0;
        rects
            .get(ix)
            .and_then(|o| *o)
            .map(|(x, _)| x)
            .unwrap_or(fallback)
    }

    fn begin_dropdown_slide(&mut self, target_ix: usize, was_open: bool, now: Instant) {
        let to_x = self.target_x_for(target_ix);
        let to_w = Self::dropdown_width_for(target_ix) * theme::scale();
        let (from_x, from_w, dur_ms, opening) = if was_open {
            if let Some(prev) = self.dropdown_slide {
                let el = now.saturating_duration_since(prev.started).as_secs_f32();
                let u = (el / (prev.dur_ms as f32 * 0.001).max(0.001)).clamp(0.0, 1.0);
                let live = eval_dropdown_glass(
                    prev.from_x,
                    prev.to_x,
                    prev.from_w,
                    prev.to_w,
                    u,
                    prev.opening,
                    true,
                );
                (live.x, live.width, 165, false)
            } else {
                (to_x, to_w, 165, false)
            }
        } else {
            (to_x, to_w, 135, true)
        };
        self.menu_seq = self.menu_seq.wrapping_add(1);
        self.dropdown_slide = Some(DropdownSlide {
            from_x,
            to_x,
            from_w,
            to_w,
            started: now,
            dur_ms,
            opening,
            seq: self.menu_seq,
        });
    }

    pub fn open_menu(&mut self, ix: usize, select_first: bool, entries: &[MenuEntry]) {
        let was_open = self.is_open();
        let effective = if ix < TOP_MENU_COUNT && ix >= self.visible_count.get() {
            OVERFLOW_IX
        } else {
            ix.min(OVERFLOW_IX)
        };
        let same = self.open_index() == Some(effective);
        self.mode = MenubarMode::Open(effective);
        self.sub_open = None;
        self.sub_item_focus = None;
        self.item_focus = if select_first {
            next_focusable(entries, None, 1)
        } else {
            None
        };
        if !same {
            *self.drop_liq.borrow_mut() = LiquidTrackState::default();
            *self.sub_liq.borrow_mut() = LiquidTrackState::default();
            self.begin_dropdown_slide(effective, was_open, Instant::now());
        }
    }

    pub fn close(&mut self) {
        self.mode = MenubarMode::Visible;
        self.mnemonics_visible = false;
        self.item_focus = None;
        self.sub_open = None;
        self.sub_item_focus = None;
        self.dropdown_slide = None;
        *self.drop_liq.borrow_mut() = LiquidTrackState::default();
        *self.sub_liq.borrow_mut() = LiquidTrackState::default();
    }
}

fn entry_height(e: &MenuEntry) -> f32 {
    let sc = theme::scale();
    match e {
        MenuEntry::Separator => (9.0 * sc).round(),
        MenuEntry::Item { .. } | MenuEntry::Submenu { .. } => (26.0 * sc).round(),
    }
}

fn row_top_offset(entries: &[MenuEntry], target_ix: usize) -> f32 {
    let mut y = 4.0 * theme::scale();
    for e in entries.iter().take(target_ix) {
        y += entry_height(e);
    }
    y
}

fn liquid_glass_style(cross: f32, radius: f32, active_strong: bool) -> LiquidStyle {
    let dark = theme::live_dark();
    LiquidStyle {
        inset_a: 2.0,
        inset_b: 2.0,
        cross_base: cross,
        max_squeeze: 2.2,
        radius,
        capillary: 20.0,
        active_bg: if active_strong {
            c(theme::ACCENT).opacity(if dark { 0.24 } else { 0.18 })
        } else {
            c(theme::BG_HOVER).opacity(if dark { 0.90 } else { 0.86 })
        },
        active_border: if active_strong {
            c(theme::ACCENT).opacity(if dark { 0.62 } else { 0.50 })
        } else {
            c(theme::BORDER_STRONG).opacity(0.80)
        },
        hover_bg: c(theme::BG_HOVER).opacity(if dark { 0.78 } else { 0.74 }),
        hover_border: if dark {
            gpui::white().opacity(0.16)
        } else {
            gpui::black().opacity(0.11)
        },
        union_border: c(theme::ACCENT).opacity(0.55),
        bridge_bg: c(theme::ACCENT).opacity(if dark { 0.20 } else { 0.14 }),
    }
}

fn glass_dropdown_fill(depth_tint: f32) -> Background {
    let base: Hsla = theme::c(theme::BG_ELEVATED);
    if !theme::glass() {
        return base.into();
    }
    let dark = theme::live_dark();
    let top = Hsla {
        l: (base.l + if dark { 0.055 } else { 0.035 }).clamp(0.0, 1.0),
        ..base
    };
    let a_top = (0.97 * depth_tint).clamp(0.88, 0.98);
    let a_bot = (0.94 * depth_tint).clamp(0.85, 0.96);
    linear_gradient(
        180.0,
        linear_color_stop(top.opacity(a_top), 0.0),
        linear_color_stop(base.opacity(a_bot), 1.0),
    )
}

fn render_mnemonic_label(label: &str, mnemonic: char, underline: bool) -> AnyElement {
    if !underline {
        return div().child(label.to_string()).into_any_element();
    }
    if let Some(pos) = label.to_ascii_lowercase().find(mnemonic) {
        let (before, rest) = label.split_at(pos);
        let mut chars = rest.chars();
        if let Some(ch) = chars.next() {
            let after = chars.as_str();
            return h_flex()
                .items_center()
                .when(!before.is_empty(), |h| h.child(before.to_string()))
                .child(div().underline().child(ch.to_string()))
                .when(!after.is_empty(), |h| h.child(after.to_string()))
                .into_any_element();
        }
    }
    div().child(label.to_string()).into_any_element()
}

impl Workbench {
    pub(crate) fn on_focus_menu_bar(
        &mut self,
        _: &FocusMenuBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.menubar.mode != MenubarMode::Visible {
            self.close_menubar(window, cx);
            return;
        }
        let target = if self.menubar.visible_count.get() > 0 {
            0
        } else {
            OVERFLOW_IX
        };
        self.menubar.mode = MenubarMode::Focused(target);
        self.menubar.mnemonics_visible = true;
        self.menubar.focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn open_menu_by_mnemonic(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.build_menu_entries(ix, window);
        self.menubar.mnemonics_visible = true;
        self.menubar.open_menu(ix, true, &entries);
        self.menubar.focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn close_menubar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menubar.mode == MenubarMode::Visible {
            return;
        }
        self.menubar.close();
        if let Some(ed) = self
            .active
            .and_then(|i| self.docs.get(i))
            .and_then(|d| d.editor.as_ref())
        {
            ed.focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn execute_menu_cmd(&mut self, cmd: MenuCmd, window: &mut Window, cx: &mut Context<Self>) {
        self.menubar.close();
        cx.notify();
        match cmd {
            MenuCmd::Action(make) => {
                self.focus.focus(window, cx);
                window.dispatch_action(make(), cx);
            }
            MenuCmd::EditorAction(make) => {
                if let Some(ed) = self
                    .active
                    .and_then(|i| self.docs.get(i))
                    .and_then(|d| d.editor.as_ref())
                {
                    ed.focus_handle(cx).focus(window, cx);
                } else {
                    self.focus.focus(window, cx);
                }
                window.dispatch_action(make(), cx);
            }
            MenuCmd::OpenRecentFolder(path) => {
                if path.is_dir() {
                    self.workspace.extra.clear();
                    self.open_folder(path, cx);
                } else {
                    self.status = tr("Folder no longer exists on disk.").into();
                    cx.notify();
                }
            }
            MenuCmd::OpenRecentFile(path) => {
                if path.is_file() {
                    self.open_file(path, window, cx);
                } else {
                    self.status = tr("File no longer exists on disk.").into();
                    cx.notify();
                }
            }
            MenuCmd::ClearRecent => {
                self.menubar.recent.clear();
                self.status = tr("Recently opened list cleared").into();
                cx.notify();
            }
            MenuCmd::OpenUrl(url) => {
                cx.open_url(url);
            }
        }
    }

    fn has_active_doc(&self) -> bool {
        self.active.is_some_and(|i| i < self.docs.len())
    }

    fn has_editable_doc(&self) -> bool {
        self.active
            .and_then(|i| self.docs.get(i))
            .is_some_and(|d| !d.read_only && d.image.is_none())
    }

    fn active_wraps(&self) -> bool {
        self.active
            .and_then(|i| self.docs.get(i))
            .map_or(self.settings.word_wrap, |d| {
                d.wraps(self.settings.word_wrap)
            })
    }

    fn has_active_selection(&self, cx: &App) -> bool {
        self.active
            .and_then(|i| self.docs.get(i))
            .and_then(|d| d.editor.as_ref())
            .is_some_and(|ed| !ed.read(cx).selected_range().is_empty())
    }

    fn build_menu_entries(&self, ix: usize, window: &Window) -> Vec<MenuEntry> {
        let item = |label: &'static str,
                    shortcut: &'static str,
                    enabled: bool,
                    make: fn() -> Box<dyn Action>| MenuEntry::Item {
            label: tr(label).into(),
            shortcut,
            checked: None,
            enabled,
            cmd: MenuCmd::Action(make),
        };
        let ed_item = |label: &'static str,
                       shortcut: &'static str,
                       enabled: bool,
                       make: fn() -> Box<dyn Action>| MenuEntry::Item {
            label: tr(label).into(),
            shortcut,
            checked: None,
            enabled,
            cmd: MenuCmd::EditorAction(make),
        };
        let check = |label: &'static str,
                     shortcut: &'static str,
                     on: bool,
                     enabled: bool,
                     make: fn() -> Box<dyn Action>| MenuEntry::Item {
            label: tr(label).into(),
            shortcut,
            checked: Some(on),
            enabled,
            cmd: MenuCmd::Action(make),
        };
        let sub = |id: SubmenuId, label: &'static str| MenuEntry::Submenu {
            id,
            label: tr(label).into(),
        };

        let has_doc = self.has_active_doc();
        let editable = self.has_editable_doc();
        let any_dirty = self.docs.iter().any(|d| d.dirty);
        let has_root = self.root.is_some();
        let can_back = !self.nav_back.is_empty()
            || (self.wiztree.page && !self.wiztree.history.back.is_empty());
        let can_fwd = !self.nav_forward.is_empty()
            || (self.wiztree.page && !self.wiztree.history.forward.is_empty());
        let has_problems =
            !self.problems.is_empty() || self.docs.iter().any(|d| !d.diagnostics.is_empty());
        let _ = window;

        match ix {
            // File menu (mirrors VS Code MenubarFileMenu)
            0 => vec![
                item("New Text File", "Ctrl+N", true, || Box::new(NewTextFile)),
                item("New File...", "", true, || Box::new(ExplorerNewFile)),
                item("New Folder...", "", true, || Box::new(ExplorerNewFolder)),
                item("New Window", "Ctrl+Shift+N", true, || Box::new(NewWindow)),
                MenuEntry::Separator,
                item("Open File...", "Ctrl+O", true, || Box::new(OpenFile)),
                item("Open Folder...", "Ctrl+K Ctrl+O", true, || {
                    Box::new(OpenFolder)
                }),
                item("Open Folder in WSL...", "", true, || {
                    Box::new(OpenFolderInWsl)
                }),
                sub(SubmenuId::OpenRecent, "Open Recent"),
                MenuEntry::Separator,
                item("Add Folder to Workspace...", "", true, || {
                    Box::new(AddFolderToWorkspace)
                }),
                MenuEntry::Separator,
                item("Save", "Ctrl+S", editable, || Box::new(Save)),
                item("Save As...", "Ctrl+Shift+S", editable, || Box::new(SaveAs)),
                item("Save All", "Ctrl+K S", any_dirty, || Box::new(SaveAll)),
                MenuEntry::Separator,
                check("Auto Save", "", self.settings.auto_save, true, || {
                    Box::new(ToggleAutoSave)
                }),
                sub(SubmenuId::Preferences, "Preferences"),
                MenuEntry::Separator,
                item("Revert File", "", editable, || Box::new(RevertFile)),
                item("Close Editor", "Ctrl+W", has_doc, || Box::new(CloseTab)),
                item("Close Folder", "Ctrl+K F", has_root, || {
                    Box::new(CloseFolder)
                }),
                MenuEntry::Separator,
                item("Exit", "Alt+F4", true, || Box::new(Quit)),
            ],
            // Edit menu (mirrors VS Code MenubarEditMenu)
            1 => vec![
                ed_item("Undo", "Ctrl+Z", editable, || {
                    Box::new(gpui_kit::component::input::Undo)
                }),
                ed_item("Redo", "Ctrl+Y", editable, || {
                    Box::new(gpui_kit::component::input::Redo)
                }),
                MenuEntry::Separator,
                ed_item("Cut", "Ctrl+X", editable, || {
                    Box::new(gpui_kit::component::input::Cut)
                }),
                ed_item("Copy", "Ctrl+C", has_doc, || {
                    Box::new(gpui_kit::component::input::Copy)
                }),
                ed_item("Paste", "Ctrl+V", editable, || {
                    Box::new(gpui_kit::component::input::Paste)
                }),
                MenuEntry::Separator,
                ed_item("Find", "Ctrl+F", has_doc, || {
                    Box::new(gpui_kit::component::input::Search)
                }),
                ed_item("Replace", "Ctrl+H", editable, || {
                    Box::new(gpui_kit::component::input::Replace)
                }),
                MenuEntry::Separator,
                item("Find in Files", "Ctrl+Shift+F", true, || {
                    Box::new(FindInFiles)
                }),
                MenuEntry::Separator,
                item("Toggle Line Comment", "Ctrl+/", editable, || {
                    Box::new(ToggleLineComment)
                }),
                item("Toggle Block Comment", "Shift+Alt+A", editable, || {
                    Box::new(ToggleBlockComment)
                }),
                MenuEntry::Separator,
                ed_item("Indent Line", "Ctrl+]", editable, || {
                    Box::new(gpui_kit::component::input::Indent)
                }),
                ed_item("Outdent Line", "Ctrl+[", editable, || {
                    Box::new(gpui_kit::component::input::Outdent)
                }),
                item("Delete Line", "Ctrl+Shift+K", editable, || {
                    Box::new(DeleteLine)
                }),
            ],
            // Selection menu (mirrors VS Code MenubarSelectionMenu)
            2 => vec![
                ed_item("Select All", "Ctrl+A", has_doc, || {
                    Box::new(gpui_kit::component::input::SelectAll)
                }),
                item("Expand Selection", "Shift+Alt+Right", has_doc, || {
                    Box::new(ExpandSelection)
                }),
                item(
                    "Shrink Selection",
                    "Shift+Alt+Left",
                    !self.sel_history.is_empty(),
                    || Box::new(ShrinkSelection),
                ),
                MenuEntry::Separator,
                item("Copy Line Up", "Shift+Alt+Up", editable, || {
                    Box::new(CopyLineUp)
                }),
                item("Copy Line Down", "Shift+Alt+Down", editable, || {
                    Box::new(CopyLineDown)
                }),
                item("Move Line Up", "Alt+Up", editable, || Box::new(MoveLineUp)),
                item("Move Line Down", "Alt+Down", editable, || {
                    Box::new(MoveLineDown)
                }),
                item("Duplicate Selection", "", editable, || {
                    Box::new(DuplicateSelection)
                }),
                MenuEntry::Separator,
                item("Insert Line Above", "Ctrl+Shift+Enter", editable, || {
                    Box::new(InsertLineAbove)
                }),
                item("Insert Line Below", "Ctrl+Enter", editable, || {
                    Box::new(InsertLineBelow)
                }),
            ],
            // View menu (mirrors VS Code MenubarViewMenu)
            3 => vec![
                item("Command Palette...", "Ctrl+Shift+P", true, || {
                    Box::new(CommandPalette)
                }),
                MenuEntry::Separator,
                sub(SubmenuId::Appearance, "Appearance"),
                sub(SubmenuId::EditorLayout, "Editor Layout"),
                MenuEntry::Separator,
                item("Explorer", "Ctrl+Shift+E", true, || Box::new(ShowExplorer)),
                item("Search", "Ctrl+Shift+F", true, || Box::new(FindInFiles)),
                item("Source Control", "Ctrl+Shift+G", true, || Box::new(ScmShow)),
                item("Run", "Ctrl+Shift+D", true, || Box::new(ShowRun)),
                item("Extensions", "Ctrl+Shift+X", true, || {
                    Box::new(ShowExtensions)
                }),
                item("AI Agent", "Ctrl+Shift+A", true, || Box::new(ShowAgent)),
                item("Disk Usage (WizTree)", "Ctrl+Shift+Y", true, || {
                    Box::new(ShowWizTree)
                }),
                MenuEntry::Separator,
                item("Problems", "Ctrl+Shift+M", true, || Box::new(ShowProblems)),
                item("Output", "Ctrl+Shift+U", true, || Box::new(ShowOutput)),
                item("Terminal", "Ctrl+`", true, || Box::new(ToggleTerminal)),
                MenuEntry::Separator,
                check("Word Wrap", "Alt+Z", self.active_wraps(), true, || {
                    Box::new(ToggleWordWrap)
                }),
                check(
                    "Render Whitespace",
                    "",
                    self.settings.show_whitespace,
                    true,
                    || Box::new(ToggleRenderWhitespace),
                ),
            ],
            // Go menu (mirrors VS Code MenubarGoMenu)
            4 => vec![
                item("Back", "Alt+Left", can_back, || Box::new(NavBack)),
                item("Forward", "Alt+Right", can_fwd, || Box::new(NavForward)),
                MenuEntry::Separator,
                sub(SubmenuId::SwitchEditor, "Switch Editor"),
                MenuEntry::Separator,
                item("Go to File...", "Ctrl+P", true, || Box::new(QuickOpenFile)),
                item("Go to Line/Column...", "Ctrl+G", has_doc, || {
                    Box::new(GoToLine)
                }),
                ed_item("Go to Definition", "F12", has_doc, || {
                    Box::new(gpui_kit::component::input::GoToDefinition)
                }),
                MenuEntry::Separator,
                item("Next Problem", "F8", has_problems, || Box::new(GoToProblem)),
                item("Previous Problem", "Shift+F8", has_problems, || {
                    Box::new(PrevProblem)
                }),
            ],
            // Run menu (mirrors VS Code MenubarDebugMenu adapted to FluxCode)
            5 => vec![
                item("Run Active File", "F5", true, || Box::new(RunFile)),
                item("Stop Running", "Shift+F5", self.out_proc.is_some(), || {
                    Box::new(StopRun)
                }),
                MenuEntry::Separator,
                item("Live Preview", "Ctrl+K V", true, || Box::new(TogglePreview)),
                item("Reload Preview", "", self.preview.is_some(), || {
                    Box::new(ReloadPreview)
                }),
                MenuEntry::Separator,
                item("Show Run Panel", "Ctrl+Shift+D", true, || Box::new(ShowRun)),
            ],
            // Terminal menu (mirrors VS Code MenubarTerminalMenu)
            6 => vec![
                item("New Terminal", "Ctrl+Shift+`", true, || {
                    Box::new(NewTerminal)
                }),
                item("Toggle Terminal", "Ctrl+`", true, || {
                    Box::new(ToggleTerminal)
                }),
                MenuEntry::Separator,
                item("Run Active File", "F5", has_doc, || Box::new(RunFile)),
                item("Run Selected Text", "", has_doc, || {
                    Box::new(RunSelectedText)
                }),
                MenuEntry::Separator,
                item("Clear Terminal", "", true, || Box::new(TermClear)),
            ],
            // Help menu (mirrors VS Code MenubarHelpMenu)
            7 => vec![
                item("Show All Commands", "Ctrl+Shift+P", true, || {
                    Box::new(CommandPalette)
                }),
                item("What's New", "", true, || Box::new(ShowWhatsNew)),
                MenuEntry::Separator,
                MenuEntry::Item {
                    label: tr("Open Repository").into(),
                    shortcut: "",
                    checked: None,
                    enabled: true,
                    cmd: MenuCmd::OpenUrl(REPO_URL),
                },
                MenuEntry::Item {
                    label: tr("Report Issue").into(),
                    shortcut: "",
                    checked: None,
                    enabled: true,
                    cmd: MenuCmd::OpenUrl(ISSUES_URL),
                },
                MenuEntry::Separator,
                item("Check for Updates...", "", true, || Box::new(CheckUpdates)),
                item("About FluxCode", "", true, || Box::new(About)),
            ],
            // Overflow menu (`•••`) holds menus that didn't fit horizontally
            _ => {
                let start = self.menubar.visible_count.get().min(TOP_MENU_COUNT);
                (start..TOP_MENU_COUNT)
                    .map(|m| MenuEntry::Submenu {
                        id: SubmenuId::OverflowMenu(m),
                        label: tr(TOP_LABELS[m]).into(),
                    })
                    .collect()
            }
        }
    }

    fn build_submenu_entries(&self, id: SubmenuId, window: &Window) -> Vec<MenuEntry> {
        let item = |label: &'static str,
                    shortcut: &'static str,
                    enabled: bool,
                    make: fn() -> Box<dyn Action>| MenuEntry::Item {
            label: tr(label).into(),
            shortcut,
            checked: None,
            enabled,
            cmd: MenuCmd::Action(make),
        };
        let check = |label: &'static str,
                     shortcut: &'static str,
                     on: bool,
                     enabled: bool,
                     make: fn() -> Box<dyn Action>| MenuEntry::Item {
            label: tr(label).into(),
            shortcut,
            checked: Some(on),
            enabled,
            cmd: MenuCmd::Action(make),
        };
        match id {
            SubmenuId::OpenRecent => {
                let mut out = Vec::new();
                out.push(item(
                    "Reopen Closed Editor",
                    "Ctrl+Shift+T",
                    !self.closed_tabs.is_empty(),
                    || Box::new(ReopenClosedTab),
                ));
                if !self.menubar.recent.folders.is_empty() {
                    out.push(MenuEntry::Separator);
                    for p in self.menubar.recent.folders.iter().take(6) {
                        let label =
                            truncate_middle(&p.display().to_string(), MAX_RECENT_LABEL_CHARS);
                        out.push(MenuEntry::Item {
                            label: label.into(),
                            shortcut: "",
                            checked: None,
                            enabled: true,
                            cmd: MenuCmd::OpenRecentFolder(p.clone()),
                        });
                    }
                }
                if !self.menubar.recent.files.is_empty() {
                    out.push(MenuEntry::Separator);
                    for p in self.menubar.recent.files.iter().take(6) {
                        let label =
                            truncate_middle(&p.display().to_string(), MAX_RECENT_LABEL_CHARS);
                        out.push(MenuEntry::Item {
                            label: label.into(),
                            shortcut: "",
                            checked: None,
                            enabled: true,
                            cmd: MenuCmd::OpenRecentFile(p.clone()),
                        });
                    }
                }
                out.push(MenuEntry::Separator);
                let has_any = !self.menubar.recent.folders.is_empty()
                    || !self.menubar.recent.files.is_empty();
                out.push(MenuEntry::Item {
                    label: tr("Clear Recently Opened").into(),
                    shortcut: "",
                    checked: None,
                    enabled: has_any,
                    cmd: MenuCmd::ClearRecent,
                });
                out
            }
            SubmenuId::Preferences => vec![
                item("Settings", "Ctrl+,", true, || Box::new(ShowSettings)),
                MenuEntry::Separator,
                item("Color Theme", "Ctrl+K Ctrl+T", true, || {
                    Box::new(ChooseTheme)
                }),
                item("File Icon Theme", "", true, || Box::new(ChooseIconTheme)),
                MenuEntry::Separator,
                item("Next Color Theme", "", true, || Box::new(NextTheme)),
                item("Next File Icon Theme", "", true, || Box::new(NextIconTheme)),
            ],
            SubmenuId::Appearance => vec![
                check("Full Screen", "F11", window.is_fullscreen(), true, || {
                    Box::new(ToggleFullscreen)
                }),
                MenuEntry::Separator,
                check(
                    "Primary Side Bar",
                    "Ctrl+B",
                    self.sidebar_open,
                    true,
                    || Box::new(ToggleSidebar),
                ),
                check(
                    "AI Agent Side Bar",
                    "Ctrl+L",
                    self.view_shown(SideView::Agent),
                    true,
                    || Box::new(ToggleAgent),
                ),
                check("Panel", "Ctrl+J", self.panel_open, true, || {
                    Box::new(TogglePanel)
                }),
                MenuEntry::Separator,
                check("Line Numbers", "", self.settings.line_numbers, true, || {
                    Box::new(ToggleLineNumbers)
                }),
                check(
                    "Liquid Glass Surface",
                    "",
                    self.settings.glass,
                    true,
                    || Box::new(ToggleGlass),
                ),
                MenuEntry::Separator,
                item("Zoom In", "Ctrl+=", true, || Box::new(ZoomIn)),
                item("Zoom Out", "Ctrl+-", true, || Box::new(ZoomOut)),
                item("Reset Zoom", "Ctrl+0", true, || Box::new(ZoomReset)),
            ],
            SubmenuId::EditorLayout => {
                let many = self.docs.len() > 1;
                let any_saved = self.docs.iter().any(|d| !d.dirty);
                vec![
                    item("Live Preview to the Side", "Ctrl+K V", true, || {
                        Box::new(TogglePreview)
                    }),
                    MenuEntry::Separator,
                    item("Close Other Editors", "", many, || Box::new(CloseOtherTabs)),
                    item("Close Saved Editors", "", any_saved, || {
                        Box::new(CloseSavedTabs)
                    }),
                    item(
                        "Close All Editors",
                        "Ctrl+K Ctrl+W",
                        !self.docs.is_empty(),
                        || Box::new(CloseAllTabs),
                    ),
                ]
            }
            SubmenuId::SwitchEditor => {
                let many = self.docs.len() > 1;
                vec![
                    item("Next Editor", "Ctrl+Tab", many, || Box::new(NextTab)),
                    item("Previous Editor", "Ctrl+Shift+Tab", many, || {
                        Box::new(PrevTab)
                    }),
                    MenuEntry::Separator,
                    item(
                        "Reopen Closed Editor",
                        "Ctrl+Shift+T",
                        !self.closed_tabs.is_empty(),
                        || Box::new(ReopenClosedTab),
                    ),
                ]
            }
            SubmenuId::OverflowMenu(ix) => self.build_menu_entries(ix, window),
        }
    }

    /// Top bar menu strip with VS Code FSM, O(1) prefix-sum overflow, and Plateau-Rayleigh liquid glass pill.
    pub(crate) fn render_menubar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let sc = theme::scale();
        let win_w = f32::from(window.viewport_size().width);
        let start_x = self.menubar.bar_origin_x.get();
        // Reserve room for nav arrows + search center (when wide) + right window controls.
        let right_budget = if win_w > 1100.0 {
            win_w * 0.34 + 300.0 * sc
        } else if win_w > 900.0 {
            win_w * 0.28 + 270.0 * sc
        } else {
            245.0 * sc
        };
        let avail_w = (win_w - start_x.max(120.0 * sc) - right_budget).max(34.0 * sc);

        let rects_snap = *self.menubar.btn_rects.borrow();
        let mut widths = [0.0f32; TOP_MENU_COUNT];
        for i in 0..TOP_MENU_COUNT {
            widths[i] = rects_snap[i]
                .map(|(_, w)| w)
                .unwrap_or_else(|| ((TOP_LABELS[i].len() as f32) * 7.2 + 16.0) * sc);
        }
        let gap = 2.0 * sc;
        let more_w = 28.0 * sc;
        let vis_count = solve_overflow(&widths, avail_w, gap, more_w);
        self.menubar.visible_count.set(vis_count);
        let has_overflow = vis_count < TOP_MENU_COUNT;

        let motion = !cx.reduce_motion();
        let bar_h = 24.0 * sc;
        let active_ix = self.menubar.active_slot().map(|ix| {
            if ix == OVERFLOW_IX || ix >= vis_count {
                OVERFLOW_IX
            } else {
                ix
            }
        });
        let origin_x = self.menubar.bar_origin_x.get();
        let rect_for = |ix: usize| -> Option<Rect1D> {
            let (bx, bw) = rects_snap.get(ix).and_then(|o| *o)?;
            if bw > 1.0 && origin_x > 0.5 {
                Some(Rect1D {
                    pos: (bx - origin_x).max(0.0),
                    len: bw,
                })
            } else {
                None
            }
        };

        let now = Instant::now();
        let frame: Option<CoalesceFrame> = if !motion {
            None
        } else {
            let mut liq = self.menubar.bar_liq.borrow_mut();
            let act_rect = active_ix.and_then(rect_for);
            liq.sync_active(active_ix, act_rect, bar_h, 195, now);
            let hov_rect = liq.hover_ix.and_then(rect_for);
            liq.sync_hover_rect(hov_rect, bar_h, now);
            liq.sample_frame(bar_h, now)
        };
        let has_active_liq = motion && active_ix.and_then(rect_for).is_some();

        let liquid_layer: Option<AnyElement> = frame.map(|fr| {
            let st = liquid_glass_style(bar_h, 6.0 * sc, self.menubar.is_open());
            liquid::render_coalesced_layer("menubar-liq", false, st, fr)
        });

        let origin_cell = self.menubar.bar_origin_x.clone();
        let view_id = cx.entity_id();
        let origin_probe = canvas(
            move |b, _, cx| {
                let x = f32::from(b.origin.x);
                if (origin_cell.get() - x).abs() > 0.5 {
                    origin_cell.set(x);
                    cx.defer(move |cx| cx.notify(view_id));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .size_full();

        let show_mnemonics = self.menubar.mnemonics_visible;
        let is_open = self.menubar.is_open();

        let make_btn = |ix: usize,
                        id: &'static str,
                        label_el: AnyElement,
                        aria: &'static str,
                        cx: &mut Context<Self>| {
            let active = active_ix == Some(ix);
            let rects_ref = self.menubar.btn_rects.clone();
            let probe = canvas(
                move |b, _, cx| {
                    let next = (f32::from(b.origin.x), f32::from(b.size.width));
                    let mut arr = rects_ref.borrow_mut();
                    let changed = arr[ix].is_none_or(|(px, pw)| {
                        (px - next.0).abs() > 0.5 || (pw - next.1).abs() > 0.5
                    });
                    if changed {
                        arr[ix] = Some(next);
                        cx.defer(move |cx| cx.notify(view_id));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
            .size_full();

            let dark = theme::live_dark();
            h_flex()
                .id(id)
                .relative()
                .flex_none()
                .h(px(bar_h))
                .px(px(8.0 * sc))
                .items_center()
                .justify_center()
                .rounded(px(6.0 * sc))
                .text_xs()
                .font_weight(if active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .cursor_pointer()
                .aria_label(tr(aria))
                .occlude()
                .text_color(c(if active {
                    theme::FG_STRONG
                } else {
                    theme::FG_MUTED
                }))
                .when(active && !has_active_liq, |d| {
                    d.bg(c(if is_open {
                        theme::BG_ELEVATED
                    } else {
                        theme::BG_HOVER
                    }))
                    .border_1()
                    .border_color(
                        c(if is_open {
                            theme::ACCENT
                        } else {
                            theme::BORDER_STRONG
                        })
                        .opacity(0.6),
                    )
                })
                .when(!active && !motion, |d| {
                    d.hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                })
                .when(!active && motion, |d| {
                    d.hover(|s| s.text_color(c(theme::FG_STRONG)))
                })
                .when(active && theme::glass(), |d| {
                    // Subtle Fresnel top highlight on active glass pill.
                    d.child(
                        div()
                            .absolute()
                            .top(px(1.0))
                            .left(px(5.0))
                            .right(px(5.0))
                            .h(px(1.0))
                            .rounded_full()
                            .bg(gpui::white().opacity(if dark { 0.22 } else { 0.45 })),
                    )
                })
                .on_hover(cx.listener(move |this, entered: &bool, window, cx| {
                    if !*entered {
                        return;
                    }
                    let now = Instant::now();
                    if let Some((bx, bw)) = this.menubar.btn_rects.borrow().get(ix).and_then(|o| *o)
                    {
                        let ox = this.menubar.bar_origin_x.get();
                        if bw > 1.0 && ox > 0.5 {
                            let rect = Rect1D {
                                pos: (bx - ox).max(0.0),
                                len: bw,
                            };
                            if this.menubar.bar_liq.borrow_mut().hover_enter(
                                ix,
                                Some(rect),
                                bar_h,
                                150,
                                now,
                            ) {
                                cx.notify();
                            }
                        }
                    }
                    // VS Code hover-to-switch: when any menu is open, moving across top-level triggers switches seamlessly.
                    if this.menubar.is_open() && this.menubar.open_index() != Some(ix) {
                        let entries = this.build_menu_entries(ix, window);
                        this.menubar.open_menu(ix, false, &entries);
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if this.menubar.open_index() == Some(ix) {
                            this.close_menubar(window, cx);
                        } else {
                            let entries = this.build_menu_entries(ix, window);
                            this.menubar.mnemonics_visible = false;
                            this.menubar.open_menu(ix, false, &entries);
                            this.menubar.focus.focus(window, cx);
                            cx.notify();
                        }
                    }),
                )
                .child(probe)
                .child(label_el)
        };

        let ids = [
            "m-file", "m-edit", "m-sel", "m-view", "m-go", "m-run", "m-term", "m-help",
        ];
        let mut strip = h_flex()
            .id("menubar-strip")
            .relative()
            .flex_none()
            .items_center()
            .gap(px(gap))
            .track_focus(&self.menubar.focus)
            .on_hover(cx.listener(|this, entered: &bool, _, cx| {
                if !*entered
                    && this
                        .menubar
                        .bar_liq
                        .borrow_mut()
                        .hover_clear(Instant::now())
                {
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_menubar_key(ev, window, cx);
            }))
            .child(origin_probe)
            .children(liquid_layer);

        for i in 0..vis_count {
            let lbl = tr(TOP_LABELS[i]);
            let el = render_mnemonic_label(lbl, TOP_MNEMONICS[i], show_mnemonics);
            strip = strip.child(make_btn(i, ids[i], el, TOP_LABELS[i], cx));
        }
        if has_overflow {
            let dots = div().px(px(2.0)).child("•••").into_any_element();
            strip = strip.child(make_btn(OVERFLOW_IX, "m-overflow", dots, "More menus", cx));
        }
        strip
    }

    fn handle_menubar_key(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.menubar.mode == MenubarMode::Visible {
            return;
        }
        let key = ev.keystroke.key.as_str();
        match self.menubar.mode {
            MenubarMode::Visible => {}
            MenubarMode::Focused(ix) => match key {
                "escape" => {
                    cx.stop_propagation();
                    self.close_menubar(window, cx);
                }
                "left" => {
                    cx.stop_propagation();
                    let next = self.menubar.step_menu(ix, -1);
                    self.menubar.mode = MenubarMode::Focused(next);
                    cx.notify();
                }
                "right" => {
                    cx.stop_propagation();
                    let next = self.menubar.step_menu(ix, 1);
                    self.menubar.mode = MenubarMode::Focused(next);
                    cx.notify();
                }
                "enter" | "space" | "down" => {
                    cx.stop_propagation();
                    let entries = self.build_menu_entries(ix, window);
                    self.menubar.open_menu(ix, true, &entries);
                    cx.notify();
                }
                other => {
                    if let Some(m_ix) = mnemonic_index(other) {
                        cx.stop_propagation();
                        let entries = self.build_menu_entries(m_ix, window);
                        self.menubar.open_menu(m_ix, true, &entries);
                        cx.notify();
                    }
                }
            },
            MenubarMode::Open(ix) => {
                let entries = self.build_menu_entries(ix, window);
                if let Some((sub_id, _, _)) = self.menubar.sub_open {
                    let sub_entries = self.build_submenu_entries(sub_id, window);
                    match key {
                        "escape" | "left" => {
                            cx.stop_propagation();
                            self.menubar.sub_open = None;
                            self.menubar.sub_item_focus = None;
                            *self.menubar.sub_liq.borrow_mut() = LiquidTrackState::default();
                            cx.notify();
                            return;
                        }
                        "down" => {
                            cx.stop_propagation();
                            self.menubar.sub_item_focus =
                                next_focusable(&sub_entries, self.menubar.sub_item_focus, 1);
                            cx.notify();
                            return;
                        }
                        "up" => {
                            cx.stop_propagation();
                            self.menubar.sub_item_focus =
                                next_focusable(&sub_entries, self.menubar.sub_item_focus, -1);
                            cx.notify();
                            return;
                        }
                        "enter" | "space" => {
                            cx.stop_propagation();
                            if let Some(MenuEntry::Item {
                                enabled: true, cmd, ..
                            }) = self
                                .menubar
                                .sub_item_focus
                                .and_then(|i| sub_entries.get(i))
                                .cloned()
                            {
                                self.execute_menu_cmd(cmd, window, cx);
                            }
                            return;
                        }
                        _ => {}
                    }
                }
                match key {
                    "escape" => {
                        cx.stop_propagation();
                        self.close_menubar(window, cx);
                    }
                    "left" => {
                        cx.stop_propagation();
                        let next = self.menubar.step_menu(ix, -1);
                        let next_entries = self.build_menu_entries(next, window);
                        self.menubar.open_menu(next, true, &next_entries);
                        cx.notify();
                    }
                    "right" => {
                        cx.stop_propagation();
                        if let Some(row_ix) = self.menubar.item_focus {
                            if let Some(MenuEntry::Submenu { id, .. }) = entries.get(row_ix) {
                                let sub_entries = self.build_submenu_entries(*id, window);
                                let y = row_top_offset(&entries, row_ix);
                                self.menubar.sub_open = Some((*id, row_ix, y));
                                self.menubar.sub_item_focus = next_focusable(&sub_entries, None, 1);
                                *self.menubar.sub_liq.borrow_mut() = LiquidTrackState::default();
                                cx.notify();
                                return;
                            }
                        }
                        let next = self.menubar.step_menu(ix, 1);
                        let next_entries = self.build_menu_entries(next, window);
                        self.menubar.open_menu(next, true, &next_entries);
                        cx.notify();
                    }
                    "down" => {
                        cx.stop_propagation();
                        self.menubar.item_focus =
                            next_focusable(&entries, self.menubar.item_focus, 1);
                        cx.notify();
                    }
                    "up" => {
                        cx.stop_propagation();
                        self.menubar.item_focus =
                            next_focusable(&entries, self.menubar.item_focus, -1);
                        cx.notify();
                    }
                    "home" => {
                        cx.stop_propagation();
                        self.menubar.item_focus = next_focusable(&entries, None, 1);
                        cx.notify();
                    }
                    "end" => {
                        cx.stop_propagation();
                        self.menubar.item_focus = next_focusable(&entries, None, -1);
                        cx.notify();
                    }
                    "enter" | "space" => {
                        cx.stop_propagation();
                        if let Some(row_ix) = self.menubar.item_focus {
                            match entries.get(row_ix).cloned() {
                                Some(MenuEntry::Item {
                                    enabled: true, cmd, ..
                                }) => {
                                    self.execute_menu_cmd(cmd, window, cx);
                                }
                                Some(MenuEntry::Submenu { id, .. }) => {
                                    let sub_entries = self.build_submenu_entries(id, window);
                                    let y = row_top_offset(&entries, row_ix);
                                    self.menubar.sub_open = Some((id, row_ix, y));
                                    self.menubar.sub_item_focus =
                                        next_focusable(&sub_entries, None, 1);
                                    cx.notify();
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Floating liquid glass dropdown & nested submenu overlay rendered at Workbench root.
    pub(crate) fn render_menubar_overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open_ix = self.menubar.open_index()?;
        let sc = theme::scale();
        let dark = theme::live_dark();
        let motion = !cx.reduce_motion();
        let win_w = f32::from(window.viewport_size().width);
        let win_h = f32::from(window.viewport_size().height);
        let top_y = f32::from(theme::u(34.0)) + 2.0;

        let mut entries = self.build_menu_entries(open_ix, window);
        // Live selection check for Terminal -> Run Selected Text.
        if open_ix == 6 {
            let has_sel = self.has_active_selection(cx);
            for e in &mut entries {
                if let MenuEntry::Item { label, enabled, .. } = e {
                    if label.as_ref() == tr("Run Selected Text") {
                        *enabled = has_sel;
                    }
                }
            }
        }

        let target_w = MenubarState::dropdown_width_for(open_ix) * sc;
        let raw_x = self.menubar.target_x_for(open_ix);
        let clamped_x = raw_x.clamp(6.0, (win_w - target_w - 8.0).max(6.0));

        let slide = self.menubar.dropdown_slide;
        let now = Instant::now();
        let anim_rem_ms = if !motion {
            0
        } else if let Some(sl) = slide {
            let el = now.saturating_duration_since(sl.started).as_millis() as u32;
            sl.dur_ms.saturating_sub(el)
        } else {
            0
        };

        let list_content = self.render_menu_rows(
            "mb-drop",
            &entries,
            target_w,
            self.menubar.item_focus,
            self.menubar.sub_open.map(|(_, row_ix, _)| row_ix),
            &self.menubar.drop_liq,
            false,
            window,
            cx,
        );

        let panel_seq = slide.map_or(self.menubar.menu_seq, |s| s.seq);
        let max_h = (win_h - top_y - 16.0).max(160.0);

        let build_glass_shell = move |panel: Stateful<Div>, opt: GlassOptics| -> Stateful<Div> {
            let rim_col = if dark {
                gpui::white().opacity(opt.fresnel_rim)
            } else {
                gpui::black().opacity(opt.fresnel_rim)
            };
            let caustic_col = c(theme::ACCENT).opacity(opt.caustic_lip);
            panel
                .left(px(opt.x.clamp(6.0, (win_w - opt.width - 8.0).max(6.0))))
                .top(px(top_y + opt.lift_y))
                .w(px(opt.width))
                .opacity(opt.opacity)
                .bg(glass_dropdown_fill(opt.depth_tint))
                .border_1()
                .border_color(rim_col)
                .child(
                    // Schlick Fresnel specular top rim highlight.
                    div()
                        .absolute()
                        .top_0()
                        .left(px(10.0))
                        .right(px(10.0))
                        .h(px(1.0))
                        .rounded_full()
                        .bg(gpui::white().opacity(if dark {
                            opt.fresnel_rim * 0.95
                        } else {
                            0.65
                        })),
                )
                .child(
                    // Bottom caustic lip tinted by theme accent during liquid motion.
                    div()
                        .absolute()
                        .bottom_0()
                        .left(px(14.0))
                        .right(px(14.0))
                        .h(px(1.0))
                        .rounded_full()
                        .bg(caustic_col),
                )
        };

        let base_panel = div()
            .id("menubar-dropdown")
            .absolute()
            .max_h(px(max_h))
            .overflow_hidden()
            .rounded(px(8.0 * sc))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                // Clicking inside the top title bar is handled by the menu buttons themselves.
                if f32::from(ev.position.y) <= f32::from(theme::u(34.0)) {
                    return;
                }
                this.close_menubar(window, cx);
            }))
            .child(list_content);

        let main_dropdown: AnyElement = if anim_rem_ms > 0 {
            let sl = slide.expect("anim_rem_ms > 0 implies slide");
            let el0 = now.saturating_duration_since(sl.started).as_secs_f32();
            let dur_s = (sl.dur_ms as f32 * 0.001).max(0.001);
            let rem_s = anim_rem_ms as f32 * 0.001;
            base_panel
                .with_animation(
                    SharedString::from(format!("mb-glass-{panel_seq}")),
                    Animation::new(Duration::from_millis(anim_rem_ms as u64)),
                    move |d, t| {
                        let u = ((el0 + t * rem_s) / dur_s).clamp(0.0, 1.0);
                        let opt = eval_dropdown_glass(
                            sl.from_x, clamped_x, sl.from_w, target_w, u, sl.opening, dark,
                        );
                        build_glass_shell(d, opt)
                    },
                )
                .into_any_element()
        } else {
            let opt =
                eval_dropdown_glass(clamped_x, clamped_x, target_w, target_w, 1.0, false, dark);
            build_glass_shell(base_panel, opt).into_any_element()
        };

        let submenu_el: Option<AnyElement> = self.menubar.sub_open.map(|(sub_id, _, row_y)| {
            let sub_entries = self.build_submenu_entries(sub_id, window);
            let sub_w = (248.0 * sc).min((win_w * 0.45).max(190.0));
            let right_x = clamped_x + target_w - 4.0;
            let sub_x = if right_x + sub_w <= win_w - 8.0 {
                right_x
            } else {
                (clamped_x - sub_w + 4.0).max(6.0)
            };
            let est_h: f32 = sub_entries.iter().map(entry_height).sum::<f32>() + 10.0 * sc;
            let sub_top = (top_y + row_y - 4.0).min((win_h - est_h - 10.0).max(top_y));
            let sub_rows = self.render_menu_rows(
                "mb-sub",
                &sub_entries,
                sub_w,
                self.menubar.sub_item_focus,
                None,
                &self.menubar.sub_liq,
                true,
                window,
                cx,
            );
            let sub_panel = div()
                .id("menubar-submenu")
                .absolute()
                .left(px(sub_x))
                .top(px(sub_top))
                .w(px(sub_w))
                .max_h(px(max_h))
                .overflow_hidden()
                .rounded(px(8.0 * sc))
                .bg(glass_dropdown_fill(0.98))
                .border_1()
                .border_color(if dark {
                    gpui::white().opacity(0.16)
                } else {
                    gpui::black().opacity(0.10)
                })
                .shadow_lg()
                .occlude()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left(px(8.0))
                        .right(px(8.0))
                        .h(px(1.0))
                        .rounded_full()
                        .bg(gpui::white().opacity(if dark { 0.20 } else { 0.60 })),
                )
                .child(sub_rows);

            if motion {
                sub_panel
                    .with_animation(
                        SharedString::from(format!(
                            "mb-sub-in-{}-{:?}",
                            self.menubar.menu_seq, sub_id
                        )),
                        Animation::new(Duration::from_millis(120)),
                        move |d, t| {
                            let s = liquid::critically_damped(t, 11.0);
                            d.opacity(0.15 + 0.85 * s).left(px(sub_x - (1.0 - s) * 6.0))
                        },
                    )
                    .into_any_element()
            } else {
                sub_panel.into_any_element()
            }
        });

        Some(
            div()
                .id("menubar-overlay-layer")
                .absolute()
                .inset_0()
                .child(main_dropdown)
                .children(submenu_el)
                .into_any_element(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_menu_rows(
        &self,
        prefix: &'static str,
        entries: &[MenuEntry],
        panel_w: f32,
        focused_ix: Option<usize>,
        active_sub_row: Option<usize>,
        liq_cell: &RefCell<LiquidTrackState>,
        is_sub: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sc = theme::scale();
        let pad_y = 4.0 * sc;
        let row_h = (26.0 * sc).round();
        let cross_w = (panel_w - 8.0 * sc).max(120.0);
        let motion = !cx.reduce_motion();
        let now = Instant::now();

        let active_row = focused_ix.or(active_sub_row);
        let rect_of_row = |ix: usize| -> Option<Rect1D> {
            let e = entries.get(ix)?;
            if matches!(e, MenuEntry::Separator) {
                return None;
            }
            Some(Rect1D {
                pos: row_top_offset(entries, ix),
                len: row_h,
            })
        };

        let row_frame = if !motion {
            None
        } else {
            let mut liq = liq_cell.borrow_mut();
            let act_r = active_row.and_then(rect_of_row);
            liq.sync_active(active_row, act_r, cross_w, 145, now);
            let hov_r = liq.hover_ix.and_then(rect_of_row);
            liq.sync_hover_rect(hov_r, cross_w, now);
            liq.sample_frame(cross_w, now)
        };

        let row_liq_layer: Option<AnyElement> = row_frame.map(|fr| {
            let st = liquid_glass_style(cross_w, 5.0 * sc, false);
            liquid::render_coalesced_layer(
                SharedString::from(format!("{prefix}-liq")),
                true,
                st,
                fr,
            )
        });

        let mut col = v_flex()
            .id(SharedString::from(format!("{prefix}-rows")))
            .relative()
            .w_full()
            .py(px(pad_y))
            .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                if !*entered {
                    let cell = if is_sub {
                        &this.menubar.sub_liq
                    } else {
                        &this.menubar.drop_liq
                    };
                    if cell.borrow_mut().hover_clear(Instant::now()) {
                        cx.notify();
                    }
                }
            }))
            .children(row_liq_layer);

        for (i, entry) in entries.iter().enumerate() {
            match entry {
                MenuEntry::Separator => {
                    col = col.child(
                        div()
                            .w_full()
                            .h(px((9.0 * sc).round()))
                            .px(px(10.0 * sc))
                            .flex()
                            .items_center()
                            .child(div().w_full().h(px(1.0)).bg(c(theme::BORDER).opacity(0.75))),
                    );
                }
                MenuEntry::Submenu { id, label } => {
                    let sub_id = *id;
                    let is_active = active_row == Some(i);
                    let y_off = row_top_offset(entries, i);
                    col = col.child(
                        h_flex()
                            .id((prefix, i))
                            .relative()
                            .w_full()
                            .h(px(row_h))
                            .px(px(10.0 * sc))
                            .gap(px(6.0 * sc))
                            .items_center()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(if is_active {
                                theme::FG_STRONG
                            } else {
                                theme::FG
                            }))
                            .when(is_active && !motion, |d| d.bg(c(theme::BG_HOVER)))
                            .when(!motion, |d| {
                                d.hover(|s| {
                                    s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))
                                })
                            })
                            .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                                if !*entered {
                                    return;
                                }
                                let cell = if is_sub {
                                    &this.menubar.sub_liq
                                } else {
                                    &this.menubar.drop_liq
                                };
                                let r = Rect1D {
                                    pos: y_off,
                                    len: row_h,
                                };
                                cell.borrow_mut().hover_enter(
                                    i,
                                    Some(r),
                                    cross_w,
                                    125,
                                    Instant::now(),
                                );
                                if !is_sub {
                                    this.menubar.item_focus = Some(i);
                                    if this.menubar.sub_open.map(|(s, _, _)| s) != Some(sub_id) {
                                        this.menubar.sub_open = Some((sub_id, i, y_off));
                                        this.menubar.sub_item_focus = None;
                                        *this.menubar.sub_liq.borrow_mut() =
                                            LiquidTrackState::default();
                                    }
                                }
                                cx.notify();
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    if !is_sub {
                                        this.menubar.sub_open = Some((sub_id, i, y_off));
                                        this.menubar.sub_item_focus = None;
                                        cx.notify();
                                    }
                                }),
                            )
                            .child(div().flex_none().w(px(14.0 * sc)))
                            .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(theme::u(12.0))
                                    .text_color(c(theme::FG_MUTED)),
                            ),
                    );
                }
                MenuEntry::Item {
                    label,
                    shortcut,
                    checked,
                    enabled,
                    cmd,
                } => {
                    let enabled = *enabled;
                    let checked = *checked;
                    let is_active = active_row == Some(i);
                    let y_off = row_top_offset(entries, i);
                    let cmd_clone = cmd.clone();
                    col = col.child(
                        h_flex()
                            .id((prefix, i))
                            .relative()
                            .w_full()
                            .h(px(row_h))
                            .px(px(10.0 * sc))
                            .gap(px(6.0 * sc))
                            .items_center()
                            .text_xs()
                            .when(enabled, |d| {
                                d.cursor_pointer()
                                    .text_color(c(if is_active {
                                        theme::FG_STRONG
                                    } else {
                                        theme::FG
                                    }))
                                    .when(is_active && !motion, |d| d.bg(c(theme::BG_HOVER)))
                                    .when(!motion, |d| {
                                        d.hover(|s| {
                                            s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))
                                        })
                                    })
                                    .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                                        if !*entered {
                                            return;
                                        }
                                        let cell = if is_sub {
                                            &this.menubar.sub_liq
                                        } else {
                                            &this.menubar.drop_liq
                                        };
                                        let r = Rect1D {
                                            pos: y_off,
                                            len: row_h,
                                        };
                                        cell.borrow_mut().hover_enter(
                                            i,
                                            Some(r),
                                            cross_w,
                                            125,
                                            Instant::now(),
                                        );
                                        if is_sub {
                                            this.menubar.sub_item_focus = Some(i);
                                        } else {
                                            this.menubar.item_focus = Some(i);
                                            if this.menubar.sub_open.is_some() {
                                                this.menubar.sub_open = None;
                                                this.menubar.sub_item_focus = None;
                                                *this.menubar.sub_liq.borrow_mut() =
                                                    LiquidTrackState::default();
                                            }
                                        }
                                        cx.notify();
                                    }))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.execute_menu_cmd(cmd_clone.clone(), window, cx);
                                        }),
                                    )
                            })
                            .when(!enabled, |d| {
                                d.cursor_not_allowed()
                                    .text_color(c(theme::FG_DIM))
                                    .opacity(0.45)
                            })
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(14.0 * sc))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(checked == Some(true), |d| {
                                        d.child(
                                            Icon::new(IconName::Check)
                                                .size(theme::u(12.0))
                                                .text_color(c(theme::ACCENT_TEXT)),
                                        )
                                    }),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                            .when(!shortcut.is_empty(), |d| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .pl(px(10.0 * sc))
                                        .text_color(c(theme::FG_DIM))
                                        .child(*shortcut),
                                )
                            }),
                    );
                }
            }
        }

        col.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        beer_lambert, eval_dropdown_glass, mnemonic_index, next_focusable, schlick_fresnel,
        solve_overflow, truncate_middle, MenuCmd, MenuEntry, SubmenuId, TOP_MENU_COUNT,
    };
    use std::time::Instant;

    #[test]
    fn schlick_fresnel_and_beer_lambert_are_monotonic_and_bounded() {
        let f0 = 0.14;
        assert!((schlick_fresnel(1.0, f0) - f0).abs() < 1e-6);
        assert!((schlick_fresnel(0.0, f0) - 1.0).abs() < 1e-6);
        let mut prev = 1.1f32;
        for i in 0..=50 {
            let c = i as f32 / 50.0;
            let f = schlick_fresnel(c, f0);
            assert!(f <= prev + 1e-6 && f >= f0 - 1e-6 && f <= 1.0 + 1e-6);
            prev = f;
        }
        assert!((beer_lambert(0.0, 1.0) - 1.0).abs() < 1e-4);
        assert!(beer_lambert(1.0, 1.0) < beer_lambert(0.2, 1.0));
    }

    #[test]
    fn dropdown_glass_kinematics_peak_mid_flight_and_settle_exactly() {
        let s0 = eval_dropdown_glass(100.0, 260.0, 240.0, 256.0, 0.0, false, true);
        let smid = eval_dropdown_glass(100.0, 260.0, 240.0, 256.0, 0.10, false, true);
        let s1 = eval_dropdown_glass(100.0, 260.0, 240.0, 256.0, 1.0, false, true);
        assert!((s0.x - 100.0).abs() < 1e-4);
        assert!((s1.x - 260.0).abs() < 1e-3 && (s1.width - 256.0).abs() < 1e-3);
        assert!(smid.velocity_norm > s0.velocity_norm && smid.velocity_norm > s1.velocity_norm);
        assert!(smid.fresnel_rim > s0.fresnel_rim);
        assert!((s1.opacity - 1.0).abs() < 1e-5 && s1.lift_y.abs() < 1e-5);
    }

    #[test]
    fn prefix_sum_overflow_solver_handles_all_widths() {
        let w = [40.0, 40.0, 68.0, 44.0, 34.0, 40.0, 62.0, 44.0];
        let total: f32 = w.iter().sum::<f32>() + 7.0 * 2.0;
        assert_eq!(solve_overflow(&w, total + 10.0, 2.0, 28.0), TOP_MENU_COUNT);
        assert_eq!(solve_overflow(&w, total - 5.0, 2.0, 28.0), 7);
        assert_eq!(solve_overflow(&w, 120.0, 2.0, 28.0), 2);
        assert_eq!(solve_overflow(&w, 30.0, 2.0, 28.0), 0);
        assert_eq!(solve_overflow(&w, 0.0, 2.0, 28.0), 0);
    }

    #[test]
    fn truncate_middle_preserves_utf8_and_bounds_length() {
        let short = "README.md";
        assert_eq!(truncate_middle(short, 40), short);
        let long = r"C:\Users\Administrator\Documents\MyGwehProject\FluxCode\src\menubar.rs";
        let out = truncate_middle(long, 32);
        assert_eq!(out.chars().count(), 32);
        assert!(out.contains('…'));
        assert!(out.ends_with("menubar.rs"));
    }

    #[test]
    fn focusable_navigation_skips_separators_and_disabled_items() {
        let entries = vec![
            MenuEntry::Item {
                label: "A".into(),
                shortcut: "",
                checked: None,
                enabled: false,
                cmd: MenuCmd::ClearRecent,
            },
            MenuEntry::Separator,
            MenuEntry::Item {
                label: "B".into(),
                shortcut: "",
                checked: None,
                enabled: true,
                cmd: MenuCmd::ClearRecent,
            },
            MenuEntry::Submenu {
                id: SubmenuId::Preferences,
                label: "C".into(),
            },
        ];
        assert_eq!(next_focusable(&entries, None, 1), Some(2));
        assert_eq!(next_focusable(&entries, Some(2), 1), Some(3));
        assert_eq!(next_focusable(&entries, Some(3), 1), Some(2));
        assert_eq!(next_focusable(&entries, Some(2), -1), Some(3));
        assert_eq!(mnemonic_index("F"), Some(0));
        assert_eq!(mnemonic_index("s"), Some(2));
        assert_eq!(mnemonic_index("g"), Some(4));
        assert_eq!(mnemonic_index("z"), None);
    }

    #[test]
    fn liquid_glass_optics_benchmark_10k_evals_under_budget() {
        let t0 = Instant::now();
        let mut acc = 0.0f32;
        for i in 0..10_000u32 {
            let u = (i % 101) as f32 * 0.01;
            let o = eval_dropdown_glass(120.0, 340.0, 240.0, 256.0, u, (i & 1) == 0, true);
            acc += o.x + o.fresnel_rim + o.caustic_lip + o.depth_tint;
        }
        let elapsed = t0.elapsed();
        std::hint::black_box(acc);
        assert!(
            elapsed.as_millis() < 15,
            "10k glass optics evals took {:?}",
            elapsed
        );
    }
}
