
use crate::i18n::tr;
use crate::core;
use crate::files;
use crate::proc::{ProcEvent, Procs};
use crate::theme::{self, c};
use crate::ui;
use futures::StreamExt;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Editor, EditorState, InputEvent, InputState, Position, TabSize},
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    resizable::{h_resizable, resizable_panel, v_resizable},
    v_flex, Icon, IconName, Sizable, TitleBar,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const LARGE_FILE_BYTES: usize = 2 * 1024 * 1024;
const READ_OFF_THREAD_BYTES: u64 = 64 * 1024;
const SAVE_OFF_THREAD_BYTES: u64 = 64 * 1024;
const MAX_AUTO_WRAP_BYTES: usize = 512 * 1024;
const MAX_COMPLEX_LINE_BYTES: usize = 8 * 1024;
const MAX_WARM_EDITORS: usize = 8;
const PANE_ANIM_MS: u64 = 185;

actions!(
    fluxcode,
    [
        Save, SaveAll, CloseTab, OpenFolder, AddFolderToWorkspace, OpenFolderInWsl, OpenFile, Quit, ToggleSidebar, NextTab, PrevTab,
        ShowExplorer, FindInFiles, ShowRun, ShowAgent, TogglePanel, ToggleTerminal, ShowOutput, ShowProblems,
        RunFile, StopRun, NewTerminal, NewTerminalCommand, ExplorerNewFile, ExplorerNewFolder, ExplorerRename,
        ExplorerDelete, ExplorerRefresh, ExplorerUp, ExplorerDown, ExplorerLeft, ExplorerRight, ExplorerOpen,
        CopyPath, CopyRelativePath, CancelPending, RevealInExplorer, ShowSettings, About, TogglePreview, ReloadPreview,
        NextTheme, TabAccept, TermHistoryPrev, TermHistoryNext, TermComplete, TermInterrupt, TermEscape,
        GoToProblem, ScmShow, ScmCommitPush, ScmCommitSync, ScmCommitAmend, ScmPull, ScmPush, ScmSync, ScmFetch,
        ScmStageAll, ScmUnstageAll, ScmCreatePr, ScmOpenGithub, ScmRemoveGit, QuickOpenFile, QuickClose, QuickUp, QuickDown,
        AgentUseOpenCode, AgentMentionUp, AgentMentionDown, AgentMentionAccept, AgentMentionClose, AgentForceSend,
        AgentMenuUp, AgentMenuDown, AgentMenuPick, AgentMenuClose,
        AgentInsertCode, RemoveFolder, AgentShowOptions, ToggleAgent, AgentAccessRead, AgentAccessWrite, AgentAccessFull,
        CheckUpdates, ToggleLineComment, MoveLineUp, MoveLineDown, CopyLineUp, CopyLineDown, DeleteLine,
        InsertLineBelow, InsertLineAbove, GoToLine, CommandPalette, ReopenClosedTab, CloseAllTabs, ToggleWordWrap,
        ZoomIn, ZoomOut, ZoomReset, ShowWhatsNew, ShowExtensions, ShowWizTree, NavBack, NavForward
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-s", Save, None),
        KeyBinding::new("ctrl-alt-s", SaveAll, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("ctrl-k ctrl-o", OpenFolder, None),
        KeyBinding::new("ctrl-o", OpenFile, None),
        KeyBinding::new("ctrl-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
        KeyBinding::new("ctrl-shift-e", ShowExplorer, None),
        KeyBinding::new("ctrl-shift-f", FindInFiles, None),
        KeyBinding::new("ctrl-shift-d", ShowRun, None),
        KeyBinding::new("ctrl-shift-x", ShowExtensions, None),
        KeyBinding::new("ctrl-shift-y", ShowWizTree, None),
        KeyBinding::new("alt-left", NavBack, None),
        KeyBinding::new("alt-right", NavForward, None),
        KeyBinding::new("ctrl-shift-a", ShowAgent, None),
        KeyBinding::new("ctrl-l", ToggleAgent, None),
        KeyBinding::new("ctrl-shift-g", ScmShow, None),
        KeyBinding::new("ctrl-p", QuickOpenFile, None),
        KeyBinding::new("up", QuickUp, Some("QuickOpen > Input")),
        KeyBinding::new("down", QuickDown, Some("QuickOpen > Input")),
        KeyBinding::new("escape", QuickClose, Some("QuickOpen > Input")),
        KeyBinding::new("up", AgentMentionUp, Some("AgentInput > Input")),
        KeyBinding::new("down", AgentMentionDown, Some("AgentInput > Input")),
        KeyBinding::new("tab", AgentMentionAccept, Some("AgentInput > Input")),
        KeyBinding::new("escape", AgentMentionClose, Some("AgentInput > Input")),
        KeyBinding::new("ctrl-enter", AgentForceSend, Some("AgentInput > Input")),
        KeyBinding::new("up", AgentMenuUp, Some("AgentMenu")),
        KeyBinding::new("down", AgentMenuDown, Some("AgentMenu")),
        KeyBinding::new("enter", AgentMenuPick, Some("AgentMenu")),
        KeyBinding::new("escape", AgentMenuClose, Some("AgentMenu")),
        KeyBinding::new("up", AgentMenuUp, Some("AgentMenu > Input")),
        KeyBinding::new("down", AgentMenuDown, Some("AgentMenu > Input")),
        KeyBinding::new("enter", AgentMenuPick, Some("AgentMenu > Input")),
        KeyBinding::new("escape", AgentMenuClose, Some("AgentMenu > Input")),
        KeyBinding::new("ctrl-j", TogglePanel, None),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new("ctrl-shift-`", NewTerminal, None),
        KeyBinding::new("ctrl-shift-u", ShowOutput, None),
        KeyBinding::new("ctrl-shift-m", ShowProblems, None),
        KeyBinding::new("f8", GoToProblem, None),
        KeyBinding::new("f5", RunFile, None),
        KeyBinding::new("ctrl-f5", RunFile, None),
        KeyBinding::new("shift-f5", StopRun, None),
        KeyBinding::new("ctrl-,", ShowSettings, None),
        KeyBinding::new("ctrl-k ctrl-t", NextTheme, None),
        KeyBinding::new("ctrl-k v", TogglePreview, None),
        KeyBinding::new("up", ExplorerUp, Some("Explorer")),
        KeyBinding::new("down", ExplorerDown, Some("Explorer")),
        KeyBinding::new("left", ExplorerLeft, Some("Explorer")),
        KeyBinding::new("right", ExplorerRight, Some("Explorer")),
        KeyBinding::new("enter", ExplorerOpen, Some("Explorer")),
        KeyBinding::new("f2", ExplorerRename, Some("Explorer")),
        KeyBinding::new("delete", ExplorerDelete, Some("Explorer")),
        KeyBinding::new("escape", CancelPending, Some("Explorer > Input")),
        KeyBinding::new("escape", CancelPending, Some("Explorer")),
        KeyBinding::new("tab", TabAccept, Some("Input && mode == editor")),
        KeyBinding::new("up", TermHistoryPrev, Some("Terminal > Input")),
        KeyBinding::new("down", TermHistoryNext, Some("Terminal > Input")),
        KeyBinding::new("tab", TermComplete, Some("Terminal > Input")),
        KeyBinding::new("ctrl-c", TermInterrupt, Some("Terminal > Input")),
        KeyBinding::new("escape", TermEscape, Some("Terminal > Input")),
        KeyBinding::new("ctrl-shift-p", CommandPalette, None),
        KeyBinding::new("f1", CommandPalette, None),
        KeyBinding::new("ctrl-g", GoToLine, None),
        KeyBinding::new("ctrl-shift-t", ReopenClosedTab, None),
        KeyBinding::new("ctrl-k ctrl-w", CloseAllTabs, None),
        KeyBinding::new("ctrl-k s", SaveAll, None),
        KeyBinding::new("ctrl-f4", CloseTab, None),
        KeyBinding::new("ctrl-pageup", PrevTab, None),
        KeyBinding::new("ctrl-pagedown", NextTab, None),
        KeyBinding::new("alt-z", ToggleWordWrap, None),
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("ctrl-shift-=", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ZoomReset, None),
        KeyBinding::new("ctrl-/", ToggleLineComment, Some("Input && mode == editor")),
        KeyBinding::new("alt-up", MoveLineUp, Some("Input && mode == editor")),
        KeyBinding::new("alt-down", MoveLineDown, Some("Input && mode == editor")),
        KeyBinding::new("shift-alt-up", CopyLineUp, Some("Input && mode == editor")),
        KeyBinding::new("shift-alt-down", CopyLineDown, Some("Input && mode == editor")),
        KeyBinding::new("ctrl-shift-k", DeleteLine, Some("Input && mode == editor")),
        KeyBinding::new("ctrl-enter", InsertLineBelow, Some("Input && mode == editor")),
        KeyBinding::new("ctrl-shift-enter", InsertLineAbove, Some("Input && mode == editor")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideView {
    Explorer,
    Search,
    Run,
    Scm,
    Agent,
    Extensions,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelTab {
    Terminal,
    Output,
    Problems,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NavTarget {
    File(PathBuf),
    WizTree,
}

fn push_nav_stack(stack: &mut Vec<NavTarget>, target: NavTarget) {
    if stack.last() == Some(&target) {
        return;
    }
    stack.push(target);
    if stack.len() > 60 {
        stack.remove(0);
    }
}

pub(crate) use crate::term::LineKind;

pub(crate) struct Line {
    pub text: SharedString,
    pub kind: LineKind,
    pub open: bool,
    pub err: bool,
    pub meta: SharedString,
    pub link: Option<(SharedString, u32, u32)>,
    pub run: Option<SharedString>,
}

impl Line {
    pub fn new(text: &str, kind: LineKind) -> Self {
        Self { text: text.to_string().into(), kind, open: false, err: false, meta: SharedString::default(), link: None, run: None }
    }
}

pub(crate) struct Doc {
    pub path: PathBuf,
    pub name: SharedString,
    pub icon: (crate::fluxicon::FluxIcon, Option<u32>),
    pub lang: &'static str,
    pub label: &'static str,
    pub read_only: bool,
    pub nbt_format: Option<crate::nbt::NbtFormat>,
    pub banner: Option<SharedString>,
    pub crlf: bool,
    pub editor: Option<Entity<EditorState>>,
    pub dirty: bool,
    pub gen: u64,
    pub _sub: Option<Subscription>,
    pub image: Option<ImageDoc>,
    pub flux: Option<crate::fluxdoc::view::FluxView>,
    pub flux_source: bool,
    pub flux_lazy: bool,
    pub diff: Option<crate::diff_view::DiffDoc>,
    pub long_lines: bool,
    pub complex: bool,
    pub wrap_override: Option<bool>,
    pub saved_cursor: Position,
    pub saved_scroll: Point<Pixels>,
    pub last_used: u64,
    pub loading: bool,
    pub diagnostics: Vec<crate::diagnostics::CodeDiag>,
    pub error_count: usize,
    pub warn_count: usize,
    pub diag_ranges: Option<gpui_kit::component::input::RangeDecorationCollection>,
    pub diag_ruler: Option<gpui_kit::component::input::OverviewRulerDecorationCollection>,
}

impl Doc {
    pub fn dormant(path: PathBuf) -> Self {
        let name: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
        let icon = files::icon_for(&name);
        let is_img = files::is_image(&path);
        let (lang, label) = if is_img { ("image", "Image") } else { files::language_for(&path, "") };
        Self {
            path,
            name,
            icon,
            lang,
            label,
            read_only: is_img,
            nbt_format: None,
            banner: None,
            crlf: false,
            editor: None,
            dirty: false,
            gen: 0,
            _sub: None,
            image: None,
            flux: None,
            flux_source: false,
            flux_lazy: false,
            diff: None,
            long_lines: false,
            complex: false,
            wrap_override: None,
            saved_cursor: Position::default(),
            saved_scroll: Point::default(),
            last_used: 0,
            loading: false,
            diagnostics: Vec::new(),
            error_count: 0,
            warn_count: 0,
            diag_ranges: None,
            diag_ruler: None,
        }
    }

    pub fn wraps(&self, setting: bool) -> bool {
        self.wrap_override.unwrap_or(setting || self.long_lines)
    }

    pub fn text(&self, cx: &App) -> String {
        match (&self.flux, self.flux_lazy, &self.editor) {
            (Some(v), true, _) => v.source(),
            (_, _, Some(ed)) => ed.read(cx).value().to_string(),
            (_, _, None) => core::read_text(&self.path)
                .map(|f| f.text)
                .or_else(|_| crate::nbt::decode_path(&self.path).map(|d| d.snbt).ok_or(()))
                .unwrap_or_default(),
        }
    }

    pub fn cursor_position(&self, cx: &App) -> Position {
        self.editor.as_ref().map(|e| e.read(cx).cursor_position()).unwrap_or(self.saved_cursor)
    }
}

pub(crate) struct ImageDoc {
    pub size: Option<(u32, u32)>,
    pub bytes: u64,
    pub zoom: Option<f32>,
}

const ZOOM_STEPS: &[f32] = &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1., 1.5, 2., 3., 5., 7., 10., 15., 20., 30., 50., 75., 100.];
const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 100.;

#[derive(Clone)]
pub(crate) enum PendingOp {
    NewFile(PathBuf),
    NewFolder(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
}

pub struct Workbench {
    pub(crate) root: Option<PathBuf>,
    pub(crate) update: crate::update::UpdateUi,
    pub(crate) title_menu_end: Rc<std::cell::Cell<f32>>,
    pub(crate) workspace: crate::workspace::Workspace,
    pub(crate) drop_focus: FocusHandle,
    pub(crate) smooth: Option<SmoothScroll>,
    welcome_w: std::rc::Rc<std::cell::Cell<f32>>,
    pub(crate) status_seen: SharedString,
    pub(crate) explorer: crate::explorer::ExplorerState,
    pub(crate) tree_nodes: Vec<crate::files::Node>,
    pub(crate) expanded: std::collections::HashSet<String>,
    pub(crate) dirs: Rc<std::collections::HashSet<String>>,
    pub(crate) docs: Vec<Doc>,
    pub(crate) active: Option<usize>,
    doc_use_clock: u64,
    last_switch: Option<std::time::Instant>,
    switch_fast: bool,
    session_save_pending: bool,
    pub(crate) view: SideView,
    pub(crate) sidebar_open: bool,
    pub(crate) agent_open: bool,
    pub(crate) agent_width: f32,
    pub(crate) agent_full: bool,
    pub(crate) pop_shut: Option<(&'static str, std::time::Instant)>,
    pub(crate) right_view: SideView,
    pub(crate) sidebar_width: f32,
    pub(crate) resizing: u8,
    pub(crate) focus: FocusHandle,
    pub(crate) status: SharedString,
    pub(crate) settings: crate::settings::Settings,
    pub(crate) settings_open: bool,
    pub(crate) settings_section: crate::settings::SettingsSection,
    pub(crate) settings_scroll: ScrollHandle,
    pub(crate) settings_search: Entity<InputState>,
    pub(crate) settings_focus: FocusHandle,
    pub(crate) settings_anim: std::cell::Cell<(bool, u64)>,
    pub(crate) settings_sec_anim: std::cell::Cell<(crate::settings::SettingsSection, u64)>,
    pub(crate) whats_new: Option<crate::whatsnew::Page>,
    pub(crate) closed_tabs: Vec<PathBuf>,
    pub(crate) nav_back: Vec<NavTarget>,
    pub(crate) nav_forward: Vec<NavTarget>,
    pub(crate) nav_replaying: bool,
    pub(crate) nav_suppress_path: Option<PathBuf>,
    pub(crate) wiztree: crate::wiztree::WizTreeState,
    pub(crate) ctx_target: Rc<RefCell<Option<PathBuf>>>,
    pub(crate) selected: Option<PathBuf>,
    pub(crate) pending: Option<PendingOp>,
    pub(crate) name_input: Entity<InputState>,
    pub(crate) pending_icon: Option<(crate::fluxicon::FluxIcon, Option<u32>)>,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) match_case: bool,
    pub(crate) whole_word: bool,
    pub(crate) search: core::SearchResult,
    pub(crate) searching: bool,
    pub(crate) search_rows: Vec<(usize, Option<usize>)>,
    pub(crate) search_unfolded: std::collections::HashSet<usize>,
    pub(crate) search_closed: std::collections::HashSet<usize>,
    pub(crate) procs: Procs,
    pub(crate) panel_open: bool,
    pub(crate) panel_tab: PanelTab,
    pub(crate) terms: Vec<crate::panel::TermTab>,
    pub(crate) term_ix: usize,
    pub(crate) wsl: Option<crate::remote::WslTarget>,
    pub(crate) term_input: Entity<InputState>,
    pub(crate) out_lines: Vec<Line>,
    pub(crate) out_proc: Option<u32>,
    pub(crate) out_scroll: UniformListScrollHandle,
    pub(crate) out_wrap: crate::panel::WrapCache,
    pub(crate) panel_size: Rc<std::cell::Cell<(f32, f32)>>,
    pub(crate) panel_left: Rc<std::cell::Cell<f32>>,
    pub(crate) panel_sel: Option<crate::panel::PanelSel>,
    pub(crate) image_box: Rc<std::cell::Cell<(f32, f32)>>,
    pub(crate) tabs_scroll: ScrollHandle,
    pub(crate) tabs_width: Rc<std::cell::Cell<f32>>,
    pub(crate) tabs_scroll_x: Rc<std::cell::Cell<f32>>,
    pub(crate) opened_tab: Option<(PathBuf, u64)>,
    pub(crate) tab_switch_seq: u64,
    pub(crate) file_jump_anim: Option<(PathBuf, u64)>,
    pub(crate) tab_hover_ix: Option<usize>,
    pub(crate) tab_hover_from: (f32, f32),
    pub(crate) tab_hover_to: (f32, f32),
    pub(crate) tab_hover_seq: u64,
    pub(crate) tab_hover_fade_in: bool,
    pub(crate) tab_hover_leave: Option<((f32, f32), u64)>,
    pub(crate) tab_hover_last_at: Option<std::time::Instant>,
    pub(crate) problems_scroll: UniformListScrollHandle,
    pub(crate) side_anim: std::cell::Cell<(bool, u64)>,
    pub(crate) full_anim: std::cell::Cell<(bool, u64)>,
    pub(crate) panel_anim: std::cell::Cell<(bool, u64)>,
    pub(crate) right_anim: std::cell::Cell<(bool, u64)>,
    pub(crate) panel_tab_anim: std::cell::Cell<(PanelTab, u64)>,
    pub(crate) side_closing: Option<u64>,
    pub(crate) right_closing: Option<u64>,
    pub(crate) panel_closing: Option<u64>,
    pub(crate) anim_seq: u64,
    pub(crate) wash: std::cell::Cell<(bool, u64)>,
    pub(crate) out_run: Option<crate::panel::RunInfo>,
    pub(crate) problems: Vec<crate::panel::Problem>,
    pub(crate) python_version: Option<SharedString>,
    pub(crate) agent: crate::agent::AgentState,
    pub(crate) scm: crate::scm::Scm,
    pub(crate) ext: crate::ext::Ext,
    pub(crate) theme_menu: bool,
    pub(crate) settings_theme_open: bool,
    pub(crate) settings_theme_focus: FocusHandle,
    pub(crate) settings_theme_sel: usize,
    pub(crate) settings_theme_anim: u64,
    pub(crate) theme_preview_origin: Option<crate::settings::ThemeSnapshot>,
    pub(crate) motion_check: Option<crate::motion_check::Check>,
    pub(crate) quick: crate::quick::QuickOpen,
    pub(crate) all_files: Arc<Vec<String>>,
    pub(crate) all_lower: Arc<Vec<String>>,
    pub(crate) preview_server: Option<crate::preview::PreviewServer>,
    pub(crate) github: Option<crate::github::Account>,
    pub(crate) github_input: Entity<InputState>,
    pub(crate) copilot: crate::copilot::Copilot,
    pub(crate) copilot_installing: bool,
    pub(crate) hidden_to_tray: bool,
    pub(crate) force_quit: bool,
    #[cfg(feature = "embedded-preview")]
    pub(crate) preview: Option<(Entity<gpui_webview::WebView>, String)>,
    #[cfg(not(feature = "embedded-preview"))]
    pub(crate) preview: Option<((), String)>,
    #[cfg(feature = "embedded-preview")]
    pub(crate) preview_ask: bool,
    #[cfg(feature = "embedded-preview")]
    pub(crate) preview_ask_hide: bool,
    #[cfg(feature = "embedded-preview")]
    pub(crate) preview_ask_focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Workbench {
    pub fn new(root: Option<PathBuf>, open: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::boot::mark("workbench new");
        let explorer = crate::explorer::ExplorerState::new(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);

        let name_input = cx.new(|cx| InputState::new(window, cx));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Search")));
        let term_input = cx.new(|cx| InputState::new(window, cx));
        let settings_search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Search settings…")));
        let settings_focus = cx.focus_handle();
        let github_input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(tr("Personal access token"))
        });
        crate::boot::mark("  inputs");
        let mut subs = vec![
            cx.subscribe_in(
                &name_input,
                window,
                |this, _, ev: &InputEvent, window, cx| {
                    match ev {
                        InputEvent::PressEnter { .. } => this.commit_pending(window, cx),
                        InputEvent::Change => {
                            if let Some(icon) = this.pending_file_icon(cx) {
                                if this.pending_icon != Some(icon) {
                                    this.pending_icon = Some(icon);
                                    cx.notify();
                                }
                            }
                        }
                        _ => {}
                    }
                },
            ),
            cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| match ev {
                InputEvent::PressEnter { .. } => this.run_search(cx),
                InputEvent::Change => this.search_soon(cx),
                _ => {}
            }),
            cx.subscribe_in(&term_input, window, |this, _, ev: &InputEvent, window, cx| {
                match ev {
                    InputEvent::PressEnter { .. } => this.terminal_enter(window, cx),
                    InputEvent::Change => {
                        let val = this.term_input.read(cx).value();
                        let typed = !val.is_empty();
                        let ix = this.term_ix;
                        if let Some(cycle) = &this.terms[ix].tab_cycle {
                            if cycle.items.get(cycle.ix).map(String::as_str) != Some(val.as_ref()) {
                                this.terms[ix].tab_cycle = None;
                            }
                        }
                        if typed != this.terms[ix].typed {
                            this.terms[ix].typed = typed;
                            cx.notify();
                        }
                    }
                    _ => {}
                }
            }),
            cx.subscribe_in(&settings_search, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.settings_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
            }),
        ];

        let (procs, rx) = Procs::new();
        cx.spawn(async move |this, cx| {
            let mut rx = rx;
            while let Some(ev) = rx.next().await {
                if this.update(cx, |this, cx| this.on_proc_event(ev, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        cx.on_app_quit(|this, _| {
            crate::wintitle::tray_hide();
            crate::instance::set_hidden(false);
            this.agent_stop_all();
            this.procs.kill_all();
            this.copilot.stop_and_wait();
            async {}
        })
        .detach();

        let close_weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            close_weak.update(cx, |this, cx| this.handle_close_request(window, cx)).unwrap_or(true)
        });

        let mut settings = crate::settings::Settings::load();
        crate::agent::worktree::prune_accounts(&mut settings);
        crate::i18n::set(&settings.language);
        crate::remote::set_helper_enabled(settings.wsl_helper);
        let copilot_enabled = settings.copilot;
        let mut settings = settings;
        if settings.agent_hint < AGENT_HINT_LAUNCHES {
            settings.agent_hint += 1;
            settings.save();
        }
        crate::agent::set_choice(&settings.agent_cli);
        let term_cwd = root.clone().or_else(|| std::env::current_dir().ok()).unwrap_or_default();
        crate::boot::mark("  settings");
        let mut agent = crate::agent::AgentState::new(window, cx);
        agent.run.agent = settings.agent_cli.clone();
        crate::boot::mark("  agent state");
        let (scm, scm_sub) = crate::scm::Scm::new(window, cx);
        let ext = crate::ext::Ext::new(window, cx);
        subs.push(scm_sub);
        crate::boot::mark("  scm");
        let (quick, quick_sub) = crate::quick::QuickOpen::new(window, cx);
        crate::boot::mark("  quick open");
        subs.push(quick_sub);
        subs.push(cx.observe_window_activation(window, |this, window, cx| {
            let recent = this.scm.last_refresh.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(2));
            if window.is_window_active() && !recent {
                this.scm_refresh(cx);
            }
            if window.is_window_active() {
                crate::wintitle::capture_main_hwnd();
                this.scm_on_return(cx);
            }
        }));
        let mut this = Self {
            root: None,
            workspace: Default::default(),
            drop_focus: cx.focus_handle(),
            smooth: None,
            welcome_w: std::rc::Rc::new(std::cell::Cell::new(f32::MAX)),
            status_seen: SharedString::default(),
            explorer,
            dirs: Rc::default(),
            docs: Vec::new(),
            active: None,
            doc_use_clock: 0,
            view: SideView::Explorer,
            sidebar_open: true,
            agent_open: false,
            agent_width: 380.,
            agent_full: false,
            pop_shut: None,
            right_view: SideView::Agent,
            sidebar_width: 260.,
            resizing: 0,
            focus,
            status: SharedString::default(),
            settings,
            settings_open: false,
            settings_section: crate::settings::SettingsSection::Appearance,
            settings_scroll: ScrollHandle::new(),
            settings_search,
            settings_focus,
            settings_anim: std::cell::Cell::new((false, 0)),
            settings_sec_anim: std::cell::Cell::new((crate::settings::SettingsSection::Appearance, 0)),
            whats_new: None,
            closed_tabs: Vec::new(),
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            nav_replaying: false,
            nav_suppress_path: None,
            last_switch: None,
            switch_fast: false,
            session_save_pending: false,
            wiztree: crate::wiztree::WizTreeState::new(window, cx),
            ctx_target: Rc::new(RefCell::new(None)),
            selected: None,
            pending: None,
            pending_icon: None,
            name_input,
            search_input,
            match_case: false,
            whole_word: false,
            search: core::SearchResult::default(),
            searching: false,
            search_rows: Vec::new(),
            search_unfolded: Default::default(),
            search_closed: Default::default(),
            procs,
            panel_open: false,
            panel_tab: PanelTab::Terminal,
            terms: vec![crate::panel::TermTab::new(term_cwd)],
            term_ix: 0,
            wsl: None,
            term_input,
            out_lines: Vec::new(),
            out_proc: None,
            out_scroll: UniformListScrollHandle::new(),
            out_wrap: Default::default(),
            panel_size: Rc::default(),
            panel_left: Rc::default(),
            panel_sel: None,
            image_box: Rc::default(),
            tabs_scroll: ScrollHandle::new(),
            tabs_width: Rc::new(std::cell::Cell::new(960.)),
            tabs_scroll_x: Rc::new(std::cell::Cell::new(0.)),
            opened_tab: None,
            tab_switch_seq: 0,
            file_jump_anim: None,
            tab_hover_ix: None,
            tab_hover_from: (0., 0.),
            tab_hover_to: (0., 0.),
            tab_hover_seq: 0,
            tab_hover_fade_in: false,
            tab_hover_leave: None,
            tab_hover_last_at: None,
            problems_scroll: UniformListScrollHandle::new(),
            side_anim: std::cell::Cell::new((true, 0)),
            full_anim: std::cell::Cell::new((false, 0)),
            panel_anim: std::cell::Cell::new((false, 0)),
            right_anim: std::cell::Cell::new((false, 0)),
            panel_tab_anim: std::cell::Cell::new((PanelTab::Terminal, 0)),
            side_closing: None,
            right_closing: None,
            panel_closing: None,
            anim_seq: 0,
            wash: std::cell::Cell::new((false, 0)),
            out_run: None,
            problems: Vec::new(),
            python_version: None,
            agent,
            scm,
            ext,
            theme_menu: false,
            settings_theme_open: false,
            settings_theme_focus: cx.focus_handle(),
            settings_theme_sel: 0,
            settings_theme_anim: 0,
            theme_preview_origin: None,
            motion_check: None,
            quick,
            tree_nodes: Vec::new(),
            expanded: std::collections::HashSet::new(),
            all_files: Arc::default(),
            all_lower: Arc::default(),
            preview_server: None,
            github: crate::github::load(),
            github_input,
            copilot: crate::copilot::Copilot::new(copilot_enabled),
            copilot_installing: false,
            hidden_to_tray: false,
            force_quit: false,
            preview: None,
            #[cfg(feature = "embedded-preview")]
            preview_ask: false,
            #[cfg(feature = "embedded-preview")]
            preview_ask_hide: false,
            #[cfg(feature = "embedded-preview")]
            preview_ask_focus: cx.focus_handle(),
            update: Default::default(),
            title_menu_end: Rc::default(),
            _subs: subs,
        };
        crate::boot::mark("  struct");
        if this.settings.last_build != crate::update::BUILD {
            let before = std::mem::replace(&mut this.settings.last_build, crate::update::BUILD.to_string());
            if crate::whatsnew::build_date(&before).is_some() {
                this.whats_new = Some(crate::whatsnew::Page::after_update(&before));
            } else if before.is_empty() && crate::settings::existed_before_start() {
                this.whats_new = Some(crate::whatsnew::Page::after_update(""));
            }
            this.settings.save();
        }
        if let Some(root) = root {
            this.open_folder(root, cx);
            for path in open {
                this.open_file(path, window, cx);
            }
        } else {
            this.restore_session(open, window, cx);
        }
        this.take_forwarded_files(window, cx);
        crate::boot::mark("  folder + session");
        this.start_update_checks(cx);
        this.start_agent_update_checks(cx);
        crate::boot::mark("  update check scheduled");
        this.start_copilot(cx);
        cx.background_spawn(async { crate::agent::shots::prune_cache() }).detach();
        crate::memtrim::watch(window, cx);
        this
    }

    fn start_copilot(&mut self, cx: &mut Context<Self>) {
        let mut changes = self.copilot.watch();
        cx.spawn(async move |this, cx| {
            use futures::StreamExt as _;
            while changes.next().await.is_some() {
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        self.kick_copilot(cx);
    }

    pub(crate) fn kick_copilot(&mut self, cx: &mut Context<Self>) {
        if !self.copilot.untried() {
            return;
        }
        let copilot = self.copilot.clone();
        let folder = self.root.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            if copilot.untried() {
                cx.background_spawn(async move { copilot.start_in(folder) }).await.ok();
            }
        })
        .detach();
    }

    pub(crate) fn open_folder(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(root.clone());
        let previous = std::mem::replace(&mut self.wsl, crate::remote::WslTarget::from_path(&root));
        if previous.as_ref().map(|t| t.distro.clone()) != self.wsl.as_ref().map(|t| t.distro.clone()) {
            crate::remote::drop_client();
        }
        if !self.terms[self.term_ix].proc.is_some_and(|id| self.procs.is_running(id)) {
            self.terms[self.term_ix].cwd = root.clone();
        }
        self.explorer.gen = self.explorer.gen.wrapping_add(1);
        self.tree_nodes.clear();
        self.expanded.clear();
        self.explorer.loading.clear();
        self.explorer.rows.clear();
        self.explorer.revealed_tab = None;
        self.explorer.hovered_ix = None;
        self.explorer.hover_leave = None;
        self.selected = None;
        self.explorer.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.explorer.bump_workspace_anim(Vec::new());
        self.refresh_tree(cx);
        self.save_session(cx);
        self.detect_python(cx);
        self.scm_refresh(cx);
        cx.notify();
    }

    pub(crate) fn detect_python(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let v = cx.background_spawn(async move { crate::panel::detect_python(root.as_deref(), wsl.as_ref()) }).await;
            _ = this.update(cx, |this, cx| {
                this.python_version = v.map(Into::into);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let roots = self.roots();
        let multi = roots.len() > 1;
        let wsl = self.wsl.clone();
        self.explorer.gen = self.explorer.gen.wrapping_add(1);
        let gen = self.explorer.gen;
        let expanded_snap: std::collections::HashSet<String> = self
            .expanded
            .iter()
            .filter(|e| {
                let p = Path::new(e.as_str());
                roots.iter().any(|r| p.starts_with(r))
            })
            .cloned()
            .collect();
        let (root2, roots2) = (root.clone(), roots.clone());
        cx.spawn(async move |this, cx| {
            let listing = cx
                .background_spawn(async move {
                    let mut nodes = if let Some(t) = &wsl {
                        remote_listing(t, &root2, multi)
                            .unwrap_or_else(|| if multi { files::scan_roots_shallow(&roots2) } else { files::scan_shallow(&root2) })
                    } else if multi {
                        files::scan_roots_shallow(&roots2)
                    } else {
                        files::scan_shallow(&root2)
                    };
                    files::hydrate_expanded(&mut nodes, &expanded_snap, &|p| {
                        wsl.as_ref()
                            .and_then(|t| remote_dir(t, p))
                            .unwrap_or_else(|| files::scan_dir(p))
                    });
                    nodes
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.explorer.gen != gen {
                    return;
                }
                let old = std::mem::take(&mut this.tree_nodes);
                this.tree_nodes = files::merge_nodes(old, listing);
                this.explorer.loading.clear();
                this.rebuild_explorer_rows(cx);
            });
        })
        .detach();
        let wsl_scan = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let (all, lower) = cx
                .background_spawn(async move {
                    let all = if let Some(paths) = wsl_scan.as_ref().and_then(|t| remote_scan(t, &root, files::MAX_ENTRIES)) {
                        paths
                    } else if multi {
                        let mut paths = Vec::new();
                        for r in &roots {
                            let rem = files::MAX_ENTRIES.saturating_sub(paths.len());
                            if rem == 0 {
                                break;
                            }
                            paths.extend(files::scan_paths(r, rem));
                        }
                        paths
                    } else {
                        files::scan_paths(&root, files::MAX_ENTRIES)
                    };
                    let lower: Vec<String> = all
                        .iter()
                        .map(|p| crate::workspace::rel_display_for(p, &roots, multi).to_lowercase())
                        .collect();
                    (all, lower)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.explorer.gen != gen {
                    return;
                }
                this.all_lower = Arc::new(lower);
                this.all_files = Arc::new(all);
                this.ext_scan(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_file_early(path.clone(), window, cx) {
            return;
        }
        if read_off_thread(&path) || self.wsl.is_some() {
            let wsl = self.wsl.clone();
            cx.spawn_in(window, async move |this, cx| {
                let p = path.clone();
                let res = cx
                    .background_spawn(async move {
                        let bytes = match &wsl {
                            Some(t) => remote_read(t, &p, core::MAX_FILE_BYTES).map(Ok).unwrap_or_else(|| read_head(&p)),
                            None => read_head(&p),
                        }?;
                        Ok(prepare_file(&p, bytes))
                    })
                    .await;
                this.update_in(cx, |this, window, cx| this.finish_open_prepared(path, res, window, cx)).ok()
            })
            .detach();
            return;
        }
        let res = read_head(&path).map(|bytes| prepare_file(&path, bytes));
        self.finish_open_prepared(path, res, window, cx);
    }

    pub(crate) fn open_file_now(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_file_early(path.clone(), window, cx) {
            return;
        }
        let res = match &self.wsl {
            Some(t) => remote_read(t, &path, core::MAX_FILE_BYTES).map(Ok).unwrap_or_else(|| read_head(&path)),
            None => read_head(&path),
        }
        .map(|bytes| prepare_file(&path, bytes));
        self.finish_open_prepared(path, res, window, cx);
    }

    fn open_file_early(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.wiztree.page && !self.nav_replaying {
            self.push_nav_history(NavTarget::WizTree);
            self.nav_suppress_path = Some(path.clone());
        }
        self.ext.page = None;
        self.settings_open = false;
        self.whats_new = None;
        self.wiztree.page = false;
        if let Some(ix) = self.docs.iter().position(|d| d.path == path) {
            self.activate(ix, window, cx);
            return true;
        }
        if files::is_image(&path) {
            self.open_image(path, window, cx);
            return true;
        }
        false
    }

    fn build_doc_editor(
        &self,
        path: &PathBuf,
        prep: &mut PreparedFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> BuiltDocEditor {
        let s = &self.settings;
        let too_big_for_wrap = prep.text.len() > MAX_AUTO_WRAP_BYTES;
        let (tab, wrap, dots) = (s.tab_size, !too_big_for_wrap && (s.word_wrap || prep.long_lines), s.show_whitespace);
        let opts = crate::settings::EditorOpts::of(s);
        let lang = prep.lang;
        let read_only = prep.read_only;
        let complex = prep.complex;
        let text = std::mem::take(&mut prep.text);
        let diff = prep.is_diff.then(|| crate::diff_view::parse(&text));
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(lang)
                .line_number(opts.line_numbers)
                .indent_guides(!read_only && !complex)
                .show_whitespaces(dots && !read_only)
                .tab_size(TabSize { tab_size: tab, hard_tabs: false })
                .soft_wrap(wrap)
                .default_value(text)
        });
        let (diag_ranges, diag_ruler) = editor.update(cx, |e, cx| {
            opts.apply(e, window, cx);
            if complex || read_only {
                e.set_folding(false, window, cx);
            }
            (
                e.create_range_decorations_collection(Vec::new(), cx),
                e.create_overview_ruler_decorations_collection(Vec::new(), cx),
            )
        });
        if read_only {
            editor.update(cx, |e, cx| e.set_readonly(true, cx));
        } else {
            crate::complete::attach(&editor, lang, path, tab, self.copilot.clone(), cx);
        }
        let sub = cx.subscribe(&editor, |this, editor, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let auto = this.settings.auto_save;
                if let Some(d) = this.docs.iter_mut().find(|d| d.editor.as_ref() == Some(&editor) && !d.read_only) {
                    d.dirty = true;
                    d.gen += 1;
                    let gen = d.gen;
                    let checked = editor.clone();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_millis(300)).await;
                        _ = this.update(cx, |this, cx| {
                            if let Some(ix) = this.docs.iter().position(|d| d.editor.as_ref() == Some(&checked) && d.gen == gen) {
                                this.apply_problems_to(ix, cx);
                            }
                        });
                    })
                    .detach();
                    if auto {
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(Duration::from_millis(1000)).await;
                            _ = this.update(cx, |this, cx| {
                                if let Some(ix) = this.docs.iter().position(|d| d.editor.as_ref() == Some(&editor) && d.gen == gen && d.dirty) {
                                    this.save_doc(ix, cx);
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                    }
                }
                cx.notify();
            }
        });
        let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let lang_ui = crate::fluxdoc::view::ui_lang();
        let mut rest: Option<String> = None;
        let flux = prep.drawable.is_some().then(|| {
            if prep.file_text_for_flux.len() >= crate::fluxdoc::view::PROGRESSIVE_FROM {
                let v = crate::fluxdoc::view::FluxView::first_part(&prep.file_text_for_flux, base.clone(), lang_ui);
                rest = Some(std::mem::take(&mut prep.file_text_for_flux));
                v
            } else {
                crate::fluxdoc::view::FluxView::new(std::mem::take(&mut prep.file_text_for_flux), base.clone(), lang_ui)
            }
        });
        if let Some(full) = rest {
            let at = path.to_path_buf();
            let task = cx.background_spawn(async move { crate::fluxdoc::view::prepare(full) });
            cx.spawn(async move |this, cx| {
                let p = task.await;
                _ = this.update(cx, |this, cx| {
                    if let Some(v) = this.docs.iter_mut().find(|d| d.path == at).and_then(|d| d.flux.as_mut()).filter(|v| v.pending) {
                        v.install(p, crate::fluxdoc::view::ui_lang());
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        BuiltDocEditor {
            editor,
            sub,
            diff,
            flux,
            diag_ranges,
            diag_ruler,
        }
    }

    fn report_open_error(&mut self, path: &Path, e: String, cx: &mut Context<Self>) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.status = if e.contains("os error 32") || e.contains("os error 33") {
            crate::trf!("{name} is in use by another program (a running server, for example), so it can't be opened now"; name => name)
        } else {
            crate::trf!("Couldn't open {name}: {e}"; name => name, e => e)
        }
        .into();
        cx.notify();
    }

    fn finish_open_prepared(&mut self, path: PathBuf, prep: Result<PreparedFile, String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.docs.iter().position(|d| d.path == path) {
            self.activate(ix, window, cx);
            return;
        }
        let mut prep = match prep {
            Ok(p) => p,
            Err(e) => return self.report_open_error(&path, e, cx),
        };
        if prep.large && !prep.read_only {
            self.status = crate::i18n::tr("Large file: syntax colors are off").into();
        } else if prep.complex && !prep.read_only {
            self.status = crate::i18n::tr("Long lines detected: syntax colors and folding are off").into();
        }
        let is_markdown = prep.drawable == Some(true);
        let built = self.build_doc_editor(&path, &mut prep, window, cx);
        let name: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
        let banner = prep.banner.map(SharedString::from);
        let icon = files::icon_for(&name);
        self.anim_seq = self.anim_seq.wrapping_add(1);
        self.opened_tab = Some((path.clone(), self.anim_seq));
        self.docs.push(Doc {
            path,
            name,
            icon,
            lang: prep.lang,
            label: prep.label,
            read_only: prep.read_only,
            nbt_format: prep.nbt_format,
            banner,
            crlf: prep.crlf,
            editor: Some(built.editor),
            dirty: false,
            gen: 0,
            _sub: Some(built.sub),
            image: None,
            flux: built.flux,
            flux_source: is_markdown,
            flux_lazy: prep.flux_lazy,
            diff: built.diff,
            long_lines: prep.long_lines,
            complex: prep.complex,
            wrap_override: None,
            saved_cursor: Position::default(),
            saved_scroll: Point::default(),
            last_used: 0,
            loading: false,
            diagnostics: Vec::new(),
            error_count: 0,
            warn_count: 0,
            diag_ranges: Some(built.diag_ranges),
            diag_ruler: Some(built.diag_ruler),
        });
        let ix = self.docs.len() - 1;
        self.apply_problems_to(ix, cx);
        self.activate(ix, window, cx);
        self.save_session(cx);
    }

    fn hydrate_doc(&mut self, ix: usize, sync: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.docs.get(ix) else { return };
        if doc.editor.is_some() || doc.image.is_some() {
            return;
        }
        let path = doc.path.clone();
        if files::is_image(&path) {
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let size = image::image_dimensions(&path).ok();
            let d = &mut self.docs[ix];
            d.image = Some(ImageDoc { size, bytes, zoom: None });
            d.read_only = true;
            d.loading = false;
            cx.notify();
            return;
        }
        if doc.loading && !sync {
            return;
        }
        if !sync && (read_off_thread(&path) || self.wsl.is_some()) {
            self.docs[ix].loading = true;
            cx.notify();
            let wsl = self.wsl.clone();
            cx.spawn_in(window, async move |this, cx| {
                let p = path.clone();
                let res = cx
                    .background_spawn(async move {
                        let bytes = match &wsl {
                            Some(t) => remote_read(t, &p, core::MAX_FILE_BYTES).map(Ok).unwrap_or_else(|| read_head(&p)),
                            None => read_head(&p),
                        }?;
                        Ok(prepare_file(&p, bytes))
                    })
                    .await;
                this.update_in(cx, |this, window, cx| this.install_hydrated(path, res, window, cx)).ok()
            })
            .detach();
            return;
        }
        let res = match &self.wsl {
            Some(t) => remote_read(t, &path, core::MAX_FILE_BYTES).map(Ok).unwrap_or_else(|| read_head(&path)),
            None => read_head(&path),
        }
        .map(|bytes| prepare_file(&path, bytes));
        self.install_hydrated(path, res, window, cx);
    }

    fn install_hydrated(&mut self, path: PathBuf, prep: Result<PreparedFile, String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.docs.iter().position(|d| d.path == path) else { return };
        self.docs[ix].loading = false;
        let mut prep = match prep {
            Ok(p) => p,
            Err(e) => return self.report_open_error(&path, e, cx),
        };
        if self.docs[ix].editor.is_some() {
            return;
        }
        let is_markdown = prep.drawable == Some(true);
        let built = self.build_doc_editor(&path, &mut prep, window, cx);
        let (saved_cursor, saved_scroll) = (self.docs[ix].saved_cursor, self.docs[ix].saved_scroll);
        if saved_cursor != Position::default() || saved_scroll != Point::default() {
            built.editor.update(cx, |e, cx| {
                if saved_cursor != Position::default() {
                    e.set_cursor_position(saved_cursor, window, cx);
                }
                if saved_scroll != Point::default() {
                    e.set_scroll_offset(saved_scroll, cx);
                }
            });
        }
        let editor = built.editor.clone();
        let d = &mut self.docs[ix];
        d.lang = prep.lang;
        d.label = prep.label;
        d.read_only = prep.read_only;
        d.nbt_format = prep.nbt_format;
        d.banner = prep.banner.map(SharedString::from);
        d.crlf = prep.crlf;
        d.editor = Some(built.editor);
        d._sub = Some(built.sub);
        d.flux = built.flux;
        d.flux_source = is_markdown;
        d.flux_lazy = prep.flux_lazy;
        d.diff = built.diff;
        d.long_lines = prep.long_lines;
        d.complex = prep.complex;
        d.diag_ranges = Some(built.diag_ranges);
        d.diag_ruler = Some(built.diag_ruler);
        self.apply_problems_to(ix, cx);
        if self.active == Some(ix) {
            let d = &self.docs[ix];
            let handle = if d.image.is_some() || d.diff.as_ref().is_some_and(|x| !x.source) || (d.flux.is_some() && !d.flux_source) {
                self.focus.clone()
            } else {
                editor.focus_handle(cx)
            };
            handle.focus(window, cx);
        }
        self.evict_cold_tabs(cx);
        cx.notify();
    }

    pub(crate) fn ensure_doc_editor(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<EditorState>> {
        if ix >= self.docs.len() || self.docs[ix].image.is_some() {
            return None;
        }
        if self.docs[ix].editor.is_none() {
            self.hydrate_doc(ix, true, window, cx);
        }
        self.docs.get(ix).and_then(|d| d.editor.clone())
    }

    fn evict_cold_tabs(&mut self, cx: &App) {
        let entries: Vec<(usize, bool, u64)> = self
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.editor.is_some())
            .map(|(i, d)| {
                let pinned = Some(i) == self.active
                    || d.dirty
                    || d.image.is_some()
                    || d.loading
                    || d.flux.as_ref().is_some_and(|f| f.pending);
                (i, pinned, d.last_used)
            })
            .collect();
        for ix in select_eviction_indices(&entries, MAX_WARM_EDITORS) {
            if let Some(ed) = self.docs[ix].editor.take() {
                let state = ed.read(cx);
                self.docs[ix].saved_cursor = state.cursor_position();
                self.docs[ix].saved_scroll = state.scroll_offset();
            }
            self.docs[ix]._sub = None;
            self.docs[ix].diff = None;
            self.docs[ix].flux = None;
            self.docs[ix].diag_ranges = None;
            self.docs[ix].diag_ruler = None;
        }
    }

    fn open_image(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let size = image::image_dimensions(&path).ok();
        let name: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
        let icon = files::icon_for(&name);
        self.anim_seq = self.anim_seq.wrapping_add(1);
        self.opened_tab = Some((path.clone(), self.anim_seq));
        self.docs.push(Doc {
            path,
            name,
            icon,
            lang: "image",
            label: "Image",
            read_only: true,
            nbt_format: None,
            banner: None,
            crlf: false,
            editor: None,
            dirty: false,
            gen: 0,
            _sub: None,
            image: Some(ImageDoc { size, bytes, zoom: None }),
            flux: None,
            flux_source: false,
            flux_lazy: false,
            diff: None,
            long_lines: false,
            complex: false,
            wrap_override: None,
            saved_cursor: Position::default(),
            saved_scroll: Point::default(),
            last_used: 0,
            loading: false,
            diagnostics: Vec::new(),
            error_count: 0,
            warn_count: 0,
            diag_ranges: None,
            diag_ruler: None,
        });
        self.activate(self.docs.len() - 1, window, cx);
        self.save_session(cx);
    }

    fn image_fit_scale(&self, im: &ImageDoc, scale: f32) -> Option<f32> {
        let (w, h) = im.size?;
        let (bw, bh) = self.image_box.get();
        let (aw, ah) = ((bw - 48.).max(16.), (bh - 48.).max(16.));
        let (lw, lh) = (w.max(1) as f32 / scale, h.max(1) as f32 / scale);
        Some((aw / lw).min(ah / lh))
    }

    pub(crate) fn image_zoom_step(&mut self, ix: usize, dir: i32, window: &Window, cx: &mut Context<Self>) {
        let scale = window.scale_factor().max(0.5);
        let Some(im) = self.docs.get(ix).and_then(|d| d.image.as_ref()) else { return };
        let Some(now) = im.zoom.or_else(|| self.image_fit_scale(im, scale)) else { return };
        let next = if dir > 0 {
            ZOOM_STEPS.iter().copied().find(|z| *z > now * 1.001).unwrap_or(ZOOM_MAX)
        } else {
            ZOOM_STEPS.iter().rev().copied().find(|z| *z < now * 0.999).unwrap_or(ZOOM_MIN)
        };
        if let Some(im) = self.docs.get_mut(ix).and_then(|d| d.image.as_mut()) {
            im.zoom = Some(next);
        }
        cx.notify();
    }

    pub(crate) fn image_set_zoom(&mut self, ix: usize, zoom: Option<f32>, cx: &mut Context<Self>) {
        if let Some(im) = self.docs.get_mut(ix).and_then(|d| d.image.as_mut()) {
            im.zoom = zoom.map(|z| z.clamp(ZOOM_MIN, ZOOM_MAX));
        }
        cx.notify();
    }

    fn render_image_tools(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(im) = self.docs[ix].image.as_ref() else { return div().into_any_element() };
        let info = match im.size {
            Some((w, h)) => format!("{w} × {h} · {}", human_size(im.bytes)),
            None => human_size(im.bytes),
        };
        let can_zoom = im.size.is_some();
        let label: SharedString = match im.zoom {
            None => tr("Fit").into(),
            Some(z) => format!("{}%", (z * 100.).round()).into(),
        };
        let fitted = im.zoom.is_none();
        h_flex()
            .flex_none()
            .gap_0p5()
            .child(div().pr_2().text_xs().text_color(c(theme::FG_DIM)).child(info))
            .when(can_zoom, |h| {
                h.child(
                    ui::tool_button("img-zoom-out", gpui_kit::assets::IconName::ZoomOut, false)
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Zoom out (Ctrl+click, Ctrl+scroll)")).build(w, cx))
                        .on_click(cx.listener(move |this, _, w, cx| this.image_zoom_step(ix, -1, w, cx))),
                )
                .child(
                    div()
                        .id("img-zoom-label")
                        .h(theme::u(22.))
                        .min_w(theme::u(46.))
                        .px_1p5()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(c(theme::FG))
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(if fitted { tr("Fitted to the editor. Click for 100%") } else { tr("Click to fit the image to the editor") }).build(w, cx))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| this.image_set_zoom(ix, if fitted { Some(1.) } else { None }, cx))),
                )
                .child(
                    ui::tool_button("img-zoom-in", gpui_kit::assets::IconName::ZoomIn, false)
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Zoom in (click the image, Ctrl+scroll)")).build(w, cx))
                        .on_click(cx.listener(move |this, _, w, cx| this.image_zoom_step(ix, 1, w, cx))),
                )
            })
            .into_any_element()
    }

    fn render_image(&self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let d = &self.docs[ix];
        let Some(im) = &d.image else { return div().into_any_element() };
        let path = d.path.clone();
        let scale = window.scale_factor().max(0.5);
        let picture = img(path).id(("image", ix)).object_fit(ObjectFit::Contain);
        let (bw, bh) = self.image_box.get();
        let view: AnyElement = match (im.size, im.zoom.or_else(|| self.image_fit_scale(im, scale))) {
            (Some((w, h)), Some(k)) => {
                let (pw, ph) = ((w as f32 / scale * k).round().max(1.), (h as f32 / scale * k).round().max(1.));
                div()
                    .id(("image-scroll", ix))
                    .size_full()
                    .overflow_scroll()
                    .child(
                        div()
                            .min_w(px(bw))
                            .min_h(px(bh))
                            .w(px(pw + 48.))
                            .h(px(ph + 48.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(picture.w(px(pw)).h(px(ph))),
                    )
                    .into_any_element()
            }
            _ => {
                let (aw, ah) = ((bw - 48.).max(16.), (bh - 48.).max(16.));
                div().size_full().flex().items_center().justify_center().child(picture.w(px(aw)).h(px(ah))).into_any_element()
            }
        };
        let probe = {
            let cell = self.image_box.clone();
            let id = cx.entity_id();
            canvas(
                move |b, _, cx| {
                    let s = (f32::from(b.size.width), f32::from(b.size.height));
                    let old = cell.get();
                    if (old.0 - s.0).abs() > 0.5 || (old.1 - s.1).abs() > 0.5 {
                        cell.set(s);
                        cx.defer(move |cx| cx.notify(id));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };
        let can_zoom = im.size.is_some();
        div()
            .id(("image-view", ix))
            .size_full()
            .relative()
            .overflow_hidden()
            .child(probe)
            .when(can_zoom, |d| d.cursor(CursorStyle::PointingHand))
            .on_click(cx.listener(move |this, e: &ClickEvent, w, cx| {
                let m = e.modifiers();
                this.image_zoom_step(ix, if m.control || m.alt { -1 } else { 1 }, w, cx);
            }))
            .on_scroll(cx.listener(move |this, e: &ScrollEvent, w, cx| {
                if !e.modifiers.control {
                    return;
                }
                cx.stop_propagation();
                let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
                if dy.abs() < 0.5 {
                    return;
                }
                let scale = w.scale_factor().max(0.5);
                let Some(now) = this.docs.get(ix).and_then(|d| d.image.as_ref()).and_then(|im| im.zoom.or_else(|| this.image_fit_scale(im, scale))) else { return };
                let k = if dy > 0. { 1.08 } else { 1. / 1.08 };
                this.image_set_zoom(ix, Some(now * k), cx);
            }))
            .child(view)
            .into_any_element()
    }

    fn tab_scroll_target_for(&self, ix: usize, cur: f32) -> Option<f32> {
        if ix >= self.docs.len() {
            return None;
        }
        let scale = f32::from(theme::u(1.));
        let widths: Vec<f32> = self
            .docs
            .iter()
            .map(|d| tab_width(&d.name, d.error_count > 0 || d.warn_count > 0, scale))
            .collect();
        let vw = self.tabs_width.get().max(120.);
        Some(tab_scroll_target(&widths, ix, cur, vw, scale))
    }

    fn scroll_tab_into_view(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        let gen = TAB_SCROLL_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let weak = cx.entity().downgrade();
        // Wait one frame so newly opened tabs lay out and update ScrollHandle's max_offset first.
        window.on_next_frame(move |window, cx| {
            if TAB_SCROLL_GEN.load(std::sync::atomic::Ordering::Relaxed) != gen {
                return;
            }
            let reduce = cx.reduce_motion();
            let mut next_step: Option<(f32, std::time::Instant)> = None;
            _ = weak.update(cx, |this, cx| {
                let cur = (-f32::from(this.tabs_scroll.offset().x)).max(0.);
                let Some(target) = this.tab_scroll_target_for(ix, cur) else { return };
                if (target - cur).abs() < 0.75 {
                    return;
                }
                this.tab_hover_ix = None;
                this.tab_hover_leave = None;
                if reduce {
                    this.tabs_scroll.set_offset(point(px(-target), px(0.)));
                    this.tabs_scroll_x.set(target);
                    cx.notify();
                } else {
                    next_step = Some((cur, std::time::Instant::now()));
                }
            });
            if let Some((start, started)) = next_step {
                step_tab_scroll(gen, weak, ix, start, started, window, cx);
            }
        });
    }

    pub(crate) fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        let suppressed = self
            .nav_suppress_path
            .as_ref()
            .is_some_and(|p| *p == self.docs[ix].path);
        if suppressed {
            self.nav_suppress_path = None;
        }
        if let Some(prev) = self.active.filter(|&p| p < self.docs.len() && p != ix) {
            if !self.nav_replaying && !suppressed {
                let prev_path = self.docs[prev].path.clone();
                self.push_nav_history(NavTarget::File(prev_path));
            }
            if let Some(ed) = &self.docs[prev].editor {
                let state = ed.read(cx);
                self.docs[prev].saved_cursor = state.cursor_position();
                self.docs[prev].saved_scroll = state.scroll_offset();
            }
        }
        self.doc_use_clock = self.doc_use_clock.wrapping_add(1);
        self.docs[ix].last_used = self.doc_use_clock;
        self.scroll_tab_into_view(ix, window, cx);
        let now = std::time::Instant::now();
        self.switch_fast = self
            .last_switch
            .is_some_and(|t| now.duration_since(t) < Duration::from_millis(90));
        self.last_switch = Some(now);
        if self.active != Some(ix) {
            self.tab_switch_seq = self.tab_switch_seq.wrapping_add(1);
        }
        self.active = Some(ix);
        if self.docs[ix].editor.is_none() && self.docs[ix].image.is_none() {
            self.hydrate_doc(ix, false, window, cx);
        }
        let d = &self.docs[ix];
        let handle = if d.image.is_some()
            || d.diff.as_ref().is_some_and(|x| !x.source)
            || (d.flux.is_some() && !d.flux_source)
        {
            self.focus.clone()
        } else {
            d.editor
                .as_ref()
                .map(|e| e.focus_handle(cx))
                .unwrap_or_else(|| self.focus.clone())
        };
        handle.focus(window, cx);
        self.reveal_active_in_explorer(cx);
        self.evict_cold_tabs(cx);
        cx.notify();
    }

    /// Jumps from an agent chip or reference to a file with a brief arrival highlight.
    pub(crate) fn jump_to_file_smooth(
        &mut self,
        path: PathBuf,
        line: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.agent.user_nav = Some(std::time::Instant::now());
        self.agent_full = false;
        self.settings_open = false;
        self.ext.page = None;
        self.whats_new = None;
        self.wiztree.page = false;
        if let Some(ln) = line {
            self.open_location(path.clone(), ln, 0, window, cx);
        } else if let Some(ix) = self.docs.iter().position(|d| d.path == path) {
            self.activate(ix, window, cx);
        } else if path.is_file() {
            self.open_file(path.clone(), window, cx);
        }
        self.selected = Some(path.clone());
        self.explorer.revealed_tab = None;
        self.reveal_active_in_explorer(cx);
        self.switch_fast = false;
        self.tab_switch_seq = self.tab_switch_seq.wrapping_add(1);
        let seq = self.tab_switch_seq;
        if !cx.reduce_motion() {
            self.file_jump_anim = Some((path, seq));
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(460))
                    .await;
                _ = this.update(cx, |this, cx| {
                    if this.file_jump_anim.as_ref().is_some_and(|(_, s)| *s == seq) {
                        this.file_jump_anim = None;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub(crate) fn active_doc_path(&self) -> Option<PathBuf> {
        self.active
            .and_then(|ix| self.docs.get(ix))
            .map(|d| d.path.clone())
    }

    pub(crate) fn push_nav_history(&mut self, target: NavTarget) {
        if self.nav_replaying {
            return;
        }
        push_nav_stack(&mut self.nav_back, target);
        self.nav_forward.clear();
    }

    fn nav_target_valid(&self, target: &NavTarget) -> bool {
        match target {
            NavTarget::WizTree => true,
            NavTarget::File(p) => self.docs.iter().any(|d| d.path == *p) || p.is_file(),
        }
    }

    fn nav_current_target(&self) -> Option<NavTarget> {
        if self.wiztree.page && !self.settings_open {
            Some(NavTarget::WizTree)
        } else {
            self.active_doc_path().map(NavTarget::File)
        }
    }

    fn nav_apply_target(
        &mut self,
        target: NavTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.nav_replaying = true;
        match target {
            NavTarget::WizTree => {
                self.settings_open = false;
                self.ext.page = None;
                self.whats_new = None;
                self.agent_full = false;
                self.wiztree.page = true;
                self.wiztree.open_seq = self.wiztree.open_seq.wrapping_add(1);
                if self.wiztree.tree.is_none() && self.wiztree.scanning.is_none() {
                    if let Some(dir) = self.wiztree.target.clone().or_else(|| self.root.clone()) {
                        self.wiztree_scan(dir, window, cx);
                    }
                }
                cx.notify();
            }
            NavTarget::File(path) => {
                self.nav_suppress_path = Some(path.clone());
                self.jump_to_file_smooth(path, None, window, cx);
            }
        }
        self.nav_replaying = false;
    }

    pub(crate) fn workbench_nav_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.wiztree.page && !self.settings_open && self.wiztree_back(cx) {
            return;
        }
        let current = self.nav_current_target();
        let mut next = None;
        while let Some(cand) = self.nav_back.pop() {
            if current.as_ref() != Some(&cand) && self.nav_target_valid(&cand) {
                next = Some(cand);
                break;
            }
        }
        let Some(target) = next else { return };
        if let Some(cur) = current {
            push_nav_stack(&mut self.nav_forward, cur);
        }
        self.nav_apply_target(target, window, cx);
    }

    pub(crate) fn workbench_nav_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.wiztree.page && !self.settings_open && self.wiztree_forward(cx) {
            return;
        }
        let current = self.nav_current_target();
        let mut next = None;
        while let Some(cand) = self.nav_forward.pop() {
            if current.as_ref() != Some(&cand) && self.nav_target_valid(&cand) {
                next = Some(cand);
                break;
            }
        }
        let Some(target) = next else { return };
        if let Some(cur) = current {
            push_nav_stack(&mut self.nav_back, cur);
        }
        self.nav_apply_target(target, window, cx);
    }

    pub(crate) fn close(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        let closed = self.docs.remove(ix);
        if !closed.read_only {
            self.copilot.did_close(&closed.path);
            self.closed_tabs.retain(|p| *p != closed.path);
            self.closed_tabs.push(closed.path);
            if self.closed_tabs.len() > 30 {
                self.closed_tabs.remove(0);
            }
        }
        self.active = match self.active {
            _ if self.docs.is_empty() => None,
            Some(a) if a > ix => Some(a - 1),
            Some(a) => Some(a.min(self.docs.len() - 1)),
            None => None,
        };
        match self.active {
            Some(a) => self.activate(a, window, cx),
            None => {
                self.focus.focus(window, cx);
                cx.notify();
            }
        }
        self.save_session(cx);
    }

    pub(crate) fn save_doc(&mut self, ix: usize, cx: &mut Context<Self>) -> bool {
        if self.docs[ix].image.is_some() || self.docs[ix].read_only || self.docs[ix].flux_lazy {
            return true;
        }
        let Some(ed) = self.docs[ix].editor.as_ref() else {
            return true;
        };
        let (text, path, crlf, gen, name, nbt_format) = {
            let doc = &self.docs[ix];
            (
                ed.read(cx).value(),
                doc.path.clone(),
                doc.crlf,
                doc.gen,
                doc.name.clone(),
                doc.nbt_format,
            )
        };
        if let Some(fmt) = nbt_format {
            let r = crate::nbt::encode_file(&text, fmt).and_then(|bytes| match &self.wsl {
                Some(t) => match remote_write(t, &path, &bytes, false) {
                    Some(()) => Ok(()),
                    None => core::write_bytes(&path, &bytes),
                },
                None => core::write_bytes(&path, &bytes),
            });
            return self.finish_save(&path, gen, name, r, cx);
        }
        if text.len() as u64 > SAVE_OFF_THREAD_BYTES {
            let wsl = self.wsl.clone();
            cx.spawn(async move |this, cx| {
                let p = path.clone();
                let r = cx
                    .background_spawn(async move {
                        if let Some(t) = &wsl {
                            let bytes = text.as_bytes();
                            if let Some(()) = remote_write(t, &p, bytes, crlf) {
                                return Ok(());
                            }
                        }
                        core::write_text(&p, &text, crlf)
                    })
                    .await;
                _ = this.update(cx, |this, cx| {
                    this.finish_save(&path, gen, name, r, cx);
                });
            })
            .detach();
            return true;
        }
        let r = match &self.wsl {
            Some(t) => match remote_write(t, &path, text.as_bytes(), crlf) {
                Some(()) => Ok(()),
                None => core::write_text(&path, &text, crlf),
            },
            None => core::write_text(&path, &text, crlf),
        };
        self.finish_save(&path, gen, name, r, cx)
    }

    fn finish_save(&mut self, path: &Path, gen: u64, name: SharedString, r: Result<(), String>, cx: &mut Context<Self>) -> bool {
        match r {
            Ok(()) => {
                if let Some(d) = self.docs.iter_mut().find(|d| d.path.as_path() == path) {
                    if d.gen == gen {
                        d.dirty = false;
                    }
                }
                self.status = crate::trf!("Saved {0}"; 0 => name).into();
                if let Some(s) = &self.preview_server {
                    s.reload();
                }
                self.scm_refresh(cx);
                true
            }
            Err(e) => {
                self.status = crate::trf!("Save failed: {e}"; e => e).into();
                false
            }
        }
    }


    pub(crate) fn save_session(&mut self, cx: &mut Context<Self>) {
        if self.session_save_pending {
            return;
        }
        self.session_save_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            _ = this.update(cx, |this, _cx| {
                this.session_save_pending = false;
                this.save_session_now();
            });
        })
        .detach();
    }

    pub(crate) fn save_session_now(&self) {
        crate::settings::Session {
            root: self.root.clone(),
            extra: self.workspace.extra.clone(),
            files: self.docs.iter().map(|d| d.path.clone()).collect(),
            active: self.active,
        }
        .save();
    }

    fn take_forwarded_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use futures::StreamExt;
        let Some(mut rx) = crate::instance::listen() else { return };
        cx.spawn_in(window, async move |this, cx| {
            while let Some(paths) = rx.next().await {
                let alive = this.update_in(cx, |this, window, cx| {
                    this.restore_from_background(window, cx);
                    for p in paths.into_iter().filter(|p| p.is_file()) {
                        this.open_file(p, window, cx);
                    }
                    window.activate_window();
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn restore_session(&mut self, then: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let s = crate::settings::Session::load();
        self.workspace.extra = s.extra.into_iter().filter(|p| p.is_dir()).collect();
        let saved_root = s.root.filter(|r| match crate::remote::WslTarget::from_path(r) {
            Some(t) => remote_is_dir(&t, r).unwrap_or_else(|| r.is_dir()),
            None => r.is_dir(),
        });
        if let Some(root) = saved_root {
            self.open_folder(root, cx);
        }
        let wsl = self.wsl.clone();
        let raw_files = s.files;
        let active = s.active;
        if raw_files.is_empty() {
            for p in then {
                self.open_file(p, window, cx);
            }
            return;
        }
        // Only check existence off-thread; background tabs stay dormant until clicked.
        cx.spawn_in(window, async move |this, cx| {
            let existing: Vec<PathBuf> = cx
                .background_spawn(async move {
                    raw_files
                        .into_iter()
                        .filter(|f| match &wsl {
                            Some(t) => remote_is_file(t, f).unwrap_or_else(|| f.is_file()),
                            None => f.is_file(),
                        })
                        .collect()
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                for p in existing {
                    if !this.docs.iter().any(|d| d.path == p) {
                        this.docs.push(Doc::dormant(p));
                    }
                }
                if then.is_empty() {
                    if let Some(a) = active.filter(|a| *a < this.docs.len()).or((!this.docs.is_empty()).then_some(0)) {
                        this.activate(a, window, cx);
                    }
                }
                for p in then {
                    this.open_file(p, window, cx);
                }
                cx.notify();
            })
            .ok()
        })
        .detach();
    }


    fn on_save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.save_doc(ix, cx);
            cx.notify();
        }
    }

    fn on_save_all(&mut self, _: &SaveAll, _: &mut Window, cx: &mut Context<Self>) {
        for ix in 0..self.docs.len() {
            if self.docs[ix].dirty {
                self.save_doc(ix, cx);
            }
        }
        cx.notify();
    }

    fn on_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.active {
            self.close(ix, window, cx);
        }
    }

    fn on_open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = paths.await.ok()?.ok()??.into_iter().next()?;
            this.update(cx, |this, cx| {
                this.workspace.extra.clear();
                this.open_folder(path, cx)
            })
            .ok()
        })
        .detach();
    }

    fn on_open_folder_in_wsl(&mut self, _: &OpenFolderInWsl, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::remote::available() {
            self.status = tr("WSL is only available on Windows.").into();
            cx.notify();
            return;
        }
        self.quick.wsl_browse = None;
        self.quick.wsl_dirs.clear();
        self.quick.wsl_rows.clear();
        self.quick_open_with("wsl:", window, cx);
    }

    pub(crate) fn on_remove_folder(&mut self, _: &RemoveFolder, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.ctx_target.borrow_mut().take();
        if let Some(p) = target {
            self.remove_folder(&p, cx);
        }
    }

    fn on_open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open File".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let paths = paths.await.ok()?.ok()??;
            this.update_in(cx, |this, window, cx| {
                for p in paths {
                    this.open_file(p, window, cx);
                }
            })
            .ok()
        })
        .detach();
    }

    pub(crate) fn handle_close_request(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.save_session_now();
        self.agent_save();
        if !self.force_quit && self.any_agent_working() && cfg!(windows) {
            if !self.hidden_to_tray && crate::wintitle::hide_main_window() {
                self.hidden_to_tray = true;
                crate::instance::set_hidden(true);
                if let Some(mut rx) = crate::wintitle::tray_show(
                    tr("FluxCode - AI Agent running (click to open)"),
                    Some(tr("FluxCode AI Agent")),
                    Some(tr("Still running in background. Click the tray icon or reopen FluxCode to return.")),
                    tr("Open FluxCode"),
                    tr("Stop AI Agent & Exit"),
                ) {
                    cx.spawn_in(window, async move |this, cx| {
                        use futures::StreamExt;
                        while let Some(act) = rx.next().await {
                            let keep = this
                                .update_in(cx, |this, window, cx| match act {
                                    crate::wintitle::TrayAction::Show => {
                                        this.restore_from_background(window, cx);
                                        false
                                    }
                                    crate::wintitle::TrayAction::StopAndQuit => {
                                        this.quit_full(cx);
                                        false
                                    }
                                })
                                .unwrap_or(false);
                            if !keep {
                                break;
                            }
                        }
                    })
                    .detach();
                }
            }
            return false;
        }
        self.quit_full(cx);
        true
    }

    pub(crate) fn restore_from_background(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.hidden_to_tray {
            return;
        }
        self.hidden_to_tray = false;
        crate::instance::set_hidden(false);
        crate::wintitle::tray_hide();
        crate::wintitle::restore_main_window();
        window.activate_window();
        cx.notify();
    }

    pub(crate) fn sync_tray_agent_state(&self) {
        if self.hidden_to_tray && !self.any_agent_working() {
            crate::wintitle::tray_update(
                tr("FluxCode - AI Agent finished (click to open)"),
                Some(tr("FluxCode AI Agent")),
                Some(tr("AI Agent finished its task. Click the tray icon or reopen FluxCode to view the result.")),
            );
        }
    }

    pub(crate) fn quit_full(&mut self, cx: &mut Context<Self>) {
        self.force_quit = true;
        self.hidden_to_tray = false;
        self.save_session_now();
        self.agent_stop_all();
        self.procs.kill_all();
        self.copilot.stop_and_wait();
        crate::wintitle::tray_hide();
        crate::instance::set_hidden(false);
        cx.quit();
    }

    fn on_quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.handle_close_request(window, cx);
    }

    pub(crate) fn set_sidebar_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open {
            self.sidebar_open = true;
            self.side_closing = None;
        } else if self.sidebar_open {
            self.sidebar_open = false;
            if cx.reduce_motion() {
                self.side_closing = None;
            } else {
                self.anim_seq = self.anim_seq.wrapping_add(1);
                let seq = self.anim_seq;
                self.side_closing = Some(seq);
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(PANE_ANIM_MS)).await;
                    _ = this.update(cx, |this, cx| {
                        if this.side_closing == Some(seq) {
                            this.side_closing = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub(crate) fn set_right_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open {
            self.agent_open = true;
            self.right_closing = None;
        } else if self.agent_open {
            self.agent_open = false;
            if cx.reduce_motion() {
                self.right_closing = None;
            } else {
                self.anim_seq = self.anim_seq.wrapping_add(1);
                let seq = self.anim_seq;
                self.right_closing = Some(seq);
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(PANE_ANIM_MS)).await;
                    _ = this.update(cx, |this, cx| {
                        if this.right_closing == Some(seq) {
                            this.right_closing = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub(crate) fn set_panel_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open {
            self.panel_open = true;
            self.panel_closing = None;
        } else if self.panel_open {
            self.panel_open = false;
            if cx.reduce_motion() {
                self.panel_closing = None;
            } else {
                self.anim_seq = self.anim_seq.wrapping_add(1);
                let seq = self.anim_seq;
                self.panel_closing = Some(seq);
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(PANE_ANIM_MS)).await;
                    _ = this.update(cx, |this, cx| {
                        if this.panel_closing == Some(seq) {
                            this.panel_closing = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sidebar_open(!self.sidebar_open, cx);
    }

    fn show_view(&mut self, view: SideView, cx: &mut Context<Self>) {
        self.reveal_view(view, cx);
    }

    fn on_show_explorer(&mut self, _: &ShowExplorer, _: &mut Window, cx: &mut Context<Self>) {
        self.show_view(SideView::Explorer, cx);
    }

    fn on_find_in_files(&mut self, _: &FindInFiles, window: &mut Window, cx: &mut Context<Self>) {
        self.show_view(SideView::Search, cx);
        let h = self.search_input.focus_handle(cx);
        h.focus(window, cx);
    }

    fn on_show_run(&mut self, _: &ShowRun, _: &mut Window, cx: &mut Context<Self>) {
        self.show_view(SideView::Run, cx);
    }

    fn on_show_extensions(&mut self, _: &ShowExtensions, window: &mut Window, cx: &mut Context<Self>) {
        self.show_view(SideView::Extensions, cx);
        let h = self.ext.query.focus_handle(cx);
        h.focus(window, cx);
    }

    pub(crate) fn on_show_agent(&mut self, _: &ShowAgent, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.agent_right {
            self.right_view = SideView::Agent;
            self.settings_open = false;
            self.set_right_open(true, cx);
            let h = self.agent_input_focus(cx);
            h.focus(window, cx);
        } else {
            self.show_view(SideView::Agent, cx);
        }
    }

    fn on_toggle_agent(&mut self, _: &ToggleAgent, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.agent_hint < AGENT_HINT_DONE {
            self.settings.agent_hint = AGENT_HINT_DONE;
            self.settings.save();
        }
        let open = self.view_shown(SideView::Agent);
        if !open {
            return self.on_show_agent(&ShowAgent, window, cx);
        }
        if self.settings.agent_right {
            self.set_right_open(false, cx);
        } else {
            self.view = SideView::Explorer;
            cx.notify();
        }
        self.focus.focus(window, cx);
    }

    fn on_show_settings(&mut self, _: &ShowSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        if self.settings_open {
            self.settings_scroll.set_offset(point(px(0.), px(0.)));
            self.settings_focus.focus(window, cx);
        } else {
            self.settings_theme_open = false;
            self.cancel_theme_preview(window, cx);
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn on_next_theme(&mut self, _: &NextTheme, window: &mut Window, cx: &mut Context<Self>) {
        let all = theme::all();
        let ix = all.iter().position(|p| p.id == theme::palette().id).unwrap_or(0);
        let next = all[(ix + 1) % all.len()].id;
        self.set_theme(next, window, cx);
    }

    fn on_tab_accept(&mut self, _: &TabAccept, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{Enter, IndentInline};
        let editor = self.active.and_then(|ix| self.docs.get(ix)).and_then(|d| d.editor.clone());
        let accepted = editor.is_some_and(|editor| {
            editor.update(cx, |state, cx| {
                state.completion_menu_state().open
                    && state.route_overlay_action(Box::new(Enter { secondary: false, shift: false }), window, cx)
            })
        });
        if !accepted {
            window.dispatch_action(Box::new(IndentInline), cx);
        }
    }

    fn on_about(&mut self, _: &About, _: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(crate::update::BUILD.to_string()));
        self.status = crate::trf!("FluxCode Development, {0} (copied)"; 0 => crate::update::BUILD).into();
        cx.notify();
    }

    fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = self.active {
            self.activate((a + 1) % self.docs.len(), window, cx);
        }
    }

    fn on_prev_tab(&mut self, _: &PrevTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = self.active {
            self.activate((a + self.docs.len() - 1) % self.docs.len(), window, cx);
        }
    }

    pub(crate) fn on_proc_event(&mut self, ev: ProcEvent, cx: &mut Context<Self>) {
        match ev {
            ProcEvent::Output { id, text, err } => {
                if let Some(ix) = self.terms.iter().position(|t| t.proc == Some(id)) {
                    push_text(&mut self.terms[ix].lines, &text, err);
                    self.terms[ix].scroll.scroll_to_bottom();
                } else if Some(id) == self.out_proc {
                    push_text(&mut self.out_lines, &text, err);
                    self.ext_fold();
                    self.out_scroll.scroll_to_bottom();
                } else if self.agent.owns(id) {
                    self.agent_on_output(&text, err, cx);
                } else if let Some(key) = self.agent.run_of(id) {
                    self.agent_with_run(key, cx, |this, cx| this.agent_on_output(&text, err, cx));
                }
            }
            ProcEvent::Exit { id, code } => {
                self.procs.forget(id);
                if let Some(ix) = self.terms.iter().position(|t| t.proc == Some(id)) {
                    self.terms[ix].proc = None;
                    self.finish_run(Some(ix), code, cx);
                } else if Some(id) == self.out_proc {
                    self.out_proc = None;
                    self.finish_run(None, code, cx);
                } else if self.agent.owns(id) {
                    self.agent_on_exit(cx);
                } else if let Some(key) = self.agent.run_of(id) {
                    self.agent_with_run(key, cx, |this, cx| this.agent_on_exit(cx));
                }
            }
        }
        cx.notify();
    }


    fn render_menu(id: &'static str, label: &'static str, items: fn(PopupMenu) -> PopupMenu) -> impl IntoElement {
        Button::new(id)
            .label(label)
            .ghost()
            .xsmall()
            .dropdown_menu(move |menu, _, _| items(menu.min_w(px(220.))))
    }

    fn render_title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let folder = self
            .root
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned());
        let drag = |id: &'static str| {
            div().id(id).flex_1().h_full().min_w(px(8.)).on_mouse_down(MouseButton::Left, |ev: &MouseDownEvent, window, _| {
                if ev.click_count >= 2 {
                    window.zoom_window();
                } else if cfg!(windows) {
                    crate::wintitle::begin_move();
                } else {
                    window.start_window_move();
                }
            })
        };
        let label = match &folder {
            Some(_) if !self.workspace.extra.is_empty() => tr("Search files in the workspace").to_string(),
            Some(f) => crate::trf!("Search files in {f}"; f => f),
            None => "Open a folder to search its files".to_string(),
        };
        let center = h_flex()
            .id("cmd-center")
            .text_xs()
            .flex_shrink_1()
            .min_w(px(0.))
            .w(theme::u(420.))
            .max_w(relative(0.3))
            .h(theme::u(24.))
            .px_2()
            .gap_2()
            .rounded(px(6.))
            .cursor_pointer()
            .bg(c(theme::BG_SIDE))
            .border_1()
            .border_color(c(theme::BORDER))
            .hover(|s| s.bg(c(theme::BG_HOVER)).border_color(c(theme::BORDER_STRONG)))
            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Go to File (Ctrl+P)")).build(w, cx))
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| {
                cx.stop_propagation();
                this.on_quick_open(&QuickOpenFile, w, cx);
            }))
            .child(Icon::new(IconName::Search).xsmall().text_color(c(theme::FG_MUTED)))
            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_MUTED)).child(label))
            .child(div().flex_none().text_color(c(theme::FG_DIM)).child("Ctrl+P"));
        let row = h_flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .h_full()
            .pl_3()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(c(theme::FG_MUTED))
            .child(div().flex_none().text_sm().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::SEMIBOLD).child(tr("FluxCode")))
            .child(
                div()
                    .id("build-mark")
                    .mr_2()
                    .flex_none()
                    .px_1()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG))
                    .text_size(theme::u(10.))
                    .text_color(c(theme::FG_MUTED))
                    .child("DEV")
                    .tooltip(|w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(crate::trf!("Development build {0}"; 0 => crate::update::BUILD)).build(w, cx)
                    }),
            )
            .child(Self::render_menu("m-file", tr("File"), |m| {
                m.menu(tr("Go to File..."), Box::new(QuickOpenFile))
                    .menu(tr("Open File..."), Box::new(OpenFile))
                    .menu(tr("Open Folder..."), Box::new(OpenFolder))
                    .menu(tr("Add Folder to Workspace..."), Box::new(AddFolderToWorkspace))
                    .menu(tr("Open Folder in WSL..."), Box::new(OpenFolderInWsl))
                    .separator()
                    .menu(tr("New File..."), Box::new(ExplorerNewFile))
                    .menu(tr("New Folder..."), Box::new(ExplorerNewFolder))
                    .separator()
                    .menu(tr("Save"), Box::new(Save))
                    .menu(tr("Save All"), Box::new(SaveAll))
                    .separator()
                    .menu(tr("Settings"), Box::new(ShowSettings))
                    .separator()
                    .menu(tr("Close Editor"), Box::new(CloseTab))
                    .menu(tr("Exit"), Box::new(Quit))
            }))
            .child(Self::render_menu("m-edit", tr("Edit"), |m| {
                m.menu(tr("Undo"), Box::new(gpui_kit::component::input::Undo))
                    .menu(tr("Redo"), Box::new(gpui_kit::component::input::Redo))
                    .separator()
                    .menu(tr("Cut"), Box::new(gpui_kit::component::input::Cut))
                    .menu(tr("Copy"), Box::new(gpui_kit::component::input::Copy))
                    .menu(tr("Paste"), Box::new(gpui_kit::component::input::Paste))
                    .separator()
                    .menu(tr("Find"), Box::new(gpui_kit::component::input::Search))
                    .menu(tr("Find in Files"), Box::new(FindInFiles))
            }))
            .child(Self::render_menu("m-view", tr("View"), |m| {
                m.menu(tr("Explorer"), Box::new(ShowExplorer))
                    .menu(tr("Search"), Box::new(FindInFiles))
                    .menu(tr("Source Control"), Box::new(ScmShow))
                    .menu(tr("Run"), Box::new(ShowRun))
                    .menu(tr("AI Agent"), Box::new(ShowAgent))
                    .menu(tr("Disk Usage (WizTree)"), Box::new(ShowWizTree))
                    .separator()
                    .menu(tr("Toggle Sidebar"), Box::new(ToggleSidebar))
                    .menu(tr("Toggle Panel"), Box::new(TogglePanel))
                    .menu(tr("Problems"), Box::new(ShowProblems))
                    .menu(tr("Output"), Box::new(ShowOutput))
            }))
            .child(Self::render_menu("m-run", tr("Run"), |m| {
                m.menu(tr("Run File"), Box::new(RunFile))
                    .menu(tr("Stop"), Box::new(StopRun))
                    .separator()
                    .menu(tr("Live Preview"), Box::new(TogglePreview))
            }))
            .child(Self::render_menu("m-term", tr("Terminal"), |m| {
                m.menu(tr("Toggle Terminal"), Box::new(ToggleTerminal)).menu(tr("New Terminal"), Box::new(NewTerminal))
            }))
            .child(Self::render_menu("m-help", tr("Help"), |m| {
                m.menu(tr("Command Palette..."), Box::new(CommandPalette))
                    .menu(tr("Next Color Theme"), Box::new(NextTheme))
                    .separator()
                    .menu(tr("What's New"), Box::new(ShowWhatsNew))
                    .menu(tr("Check for Updates"), Box::new(CheckUpdates))
                    .menu(tr("About FluxCode"), Box::new(About))
            }))
            .child(self.render_title_nav(cx))
            .child({
                let cell = self.title_menu_end.clone();
                let id = cx.entity_id();
                canvas(
                    move |b, _, cx| {
                        let x = f32::from(b.origin.x);
                        if (cell.get() - x).abs() > 0.5 {
                            cell.set(x);
                            cx.defer(move |cx| cx.notify(id));
                        }
                    },
                    |_, _, _, _| {},
                )
                .w(px(0.))
                .h_full()
            })
            .child(drag("tb-drag"))
            .children(self.update_waiting().map(|r| {
                let tip: SharedString = crate::trf!("Update ready: {0}"; 0 => crate::update::build_time(&r.tag).unwrap_or_else(|| r.tag.clone())).into();
                h_flex()
                    .id("tb-update")
                    .flex_none()
                    .mr_2()
                    .h(theme::u(22.))
                    .px_2()
                    .gap_1()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(c(theme::ACCENT).opacity(0.5))
                    .text_color(c(theme::ACCENT))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.update.hidden = None;
                        cx.notify();
                    }))
                    .child(Icon::new(IconName::ArrowDown).size(theme::u(12.)))
                    .child(tr("Update"))
            }));
        let show_center = window.viewport_size().width > px(900.);
        if !cfg!(windows) {
            return TitleBar::new().child(row.when(show_center, |r| r.child(center).child(div().flex_1()))).into_any_element();
        }
        let win_w = f32::from(window.viewport_size().width);
        let box_w = f32::from(theme::u(420.)).min(win_w * 0.3);
        let menus = self.title_menu_end.get() + 12.;
        let left = ((win_w - box_w) / 2.).max(menus);
        let ctl_w = theme::u(44.);
        // Reserve space for Run + Min + Max + Close and optional Update badge.
        let right_reserve = f32::from(ctl_w) * 4.
            + if self.update_waiting().is_some() {
                96.
            } else {
                0.
            };
        let w = box_w.min(win_w - right_reserve - 12. - left);
        let center_layer = div()
            .absolute()
            .top_0()
            .left(px(left))
            .w(px(w.max(0.)))
            .h_full()
            .flex()
            .items_center()
            .when(show_center && w >= 180., |d| d.child(center.w_full().max_w(relative(1.))));
        let maximized = window.is_maximized();
        // Centered 24x24 minimize bar (gpui-kit-assets window-minimize.svg sits above y=12).
        const MINIMIZE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect x="4" y="11.2" width="16" height="1.6" rx="0.8" fill="#000000"/></svg>"##;
        let ctl = |id: &'static str, icon: Icon, close: bool| {
            h_flex()
                .id(id)
                .w(ctl_w)
                .h_full()
                .flex_none()
                .items_center()
                .justify_center()
                .text_color(c(theme::FG_MUTED))
                .hover(move |s| {
                    if close {
                        s.bg(gpui_kit::Hsla::from(rgb(0xe81123))).text_color(gpui_kit::white())
                    } else {
                        s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))
                    }
                })
                .child(icon)
        };
        let start_btn = h_flex()
            .id("tb-start")
            .w(ctl_w)
            .h_full()
            .flex_none()
            .items_center()
            .justify_center()
            .text_color(c(theme::FG_MUTED))
            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            .child(Icon::new(IconName::Play).size(theme::u(12.)))
            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Run File (F5)")).build(w, cx))
            .on_click(cx.listener(|this, _, w, cx| this.on_run_file(&RunFile, w, cx)));

        h_flex()
            .id("title-bar")
            .relative()
            .w_full()
            .h(theme::u(34.))
            .flex_none()
            .bg(c(theme::BG_TITLE))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(row)
            .child(start_btn)
            .child(ctl("tb-min", Icon::default().data(MINIMIZE_SVG).size(theme::u(14.)), false).on_click(|_, w, _| w.minimize_window()))
            .child(
                ctl(
                    "tb-max",
                    Icon::new(if maximized { IconName::WindowRestore } else { IconName::WindowMaximize }).size(theme::u(14.)),
                    false,
                )
                .on_click(|_, w, _| w.zoom_window()),
            )
            .child(ctl("tb-close", Icon::new(IconName::WindowClose).size(theme::u(14.)), true).on_click(|_, w, cx| w.dispatch_action(Box::new(Quit), cx)))
            .child(center_layer)
            .into_any_element()
    }

    fn render_title_nav(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_back = !self.nav_back.is_empty()
            || (self.wiztree.page && !self.wiztree.history.back.is_empty());
        let can_fwd = !self.nav_forward.is_empty()
            || (self.wiztree.page && !self.wiztree.history.forward.is_empty());
        let nav_btn = |id: &'static str, icon: IconName, enabled: bool, tip: &'static str| {
            div()
                .id(id)
                .flex_none()
                .size(theme::u(22.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .occlude()
                .when(enabled, |d| {
                    d.cursor_pointer()
                        .text_color(c(theme::FG_MUTED))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                })
                .when(!enabled, |d| d.text_color(c(theme::FG_DIM)).opacity(0.38))
                .tooltip(move |w, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tr(tip)).build(w, cx)
                })
                .child(Icon::new(icon).size(theme::u(13.)))
        };
        h_flex()
            .flex_none()
            .items_center()
            .gap_0p5()
            .child(div().w(px(1.)).h(theme::u(14.)).mx_1().bg(c(theme::BORDER)))
            .child(
                nav_btn(
                    "tb-nav-back",
                    IconName::ArrowLeft,
                    can_back,
                    "Back (Mouse Back / Alt+Left)",
                )
                .when(can_back, |d| {
                    d.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, w, cx| {
                            cx.stop_propagation();
                            this.workbench_nav_back(w, cx);
                        }),
                    )
                }),
            )
            .child(
                nav_btn(
                    "tb-nav-fwd",
                    IconName::ArrowRight,
                    can_fwd,
                    "Forward (Mouse Forward / Alt+Right)",
                )
                .when(can_fwd, |d| {
                    d.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, w, cx| {
                            cx.stop_propagation();
                            this.workbench_nav_forward(w, cx);
                        }),
                    )
                }),
            )
    }

    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, icon: Icon, active: bool, _this: &Self, _cx: &mut Context<Self>| {
            div()
                .id(id)
                .relative()
                .size(theme::u(40.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(c(if active { theme::FG_STRONG } else { theme::FG_DIM }))
                .when(!active, |d| d.hover(|s| s.text_color(c(theme::FG))))
                .when(active, |d| d.child(div().absolute().left_0().top(px(8.)).bottom(px(8.)).w(px(2.)).bg(c(theme::ACCENT))))
                .child(icon.size(theme::u(17.)))
        };
        let view_item = |id: &'static str, icon: Icon, view: SideView, this: &Self, cx: &mut Context<Self>| {
            let active = this.view_shown(view);
            let weak = cx.entity().downgrade();
            div()
                .id((id, 0usize))
                .child(item(id, icon, active, this, cx))
                .on_click(cx.listener(move |this, _, w, cx| {
                    this.toggle_view(view, cx);
                    if view == SideView::Search && this.view_shown(view) {
                        let h = this.search_input.focus_handle(cx);
                        h.focus(w, cx);
                    }
                    if view == SideView::Extensions && this.view_shown(view) {
                        let h = this.ext.query.focus_handle(cx);
                        h.focus(w, cx);
                    }
                }))
                .context_menu(move |menu, _, cx| match weak.upgrade() {
                    Some(wb) => wb.update(cx, |this, cx| this.view_side_menu(view, menu, cx)),
                    None => menu,
                })
        };
        v_flex()
            .w(theme::u(40.))
            .h_full()
            .items_center()
            .bg(c(theme::BG_TITLE))
            .border_r_1()
            .border_color(c(theme::BORDER))
            .child(view_item("ab-explorer", Icon::new(IconName::FolderOpen), SideView::Explorer, self, cx))
            .child(view_item("ab-search", Icon::new(IconName::Search), SideView::Search, self, cx))
            .child({
                let n = self.scm.status.count();
                let incoming = self.scm_has_incoming();
                div()
                    .relative()
                    .child(view_item("ab-scm", Icon::new(gpui_kit::assets::IconName::GitBranch), SideView::Scm, self, cx))
                    .when(incoming, |d| {
                        d.child(
                            div()
                                .absolute()
                                .right(theme::u(6.))
                                .top(theme::u(6.))
                                .size(theme::u(7.))
                                .rounded_full()
                                .bg(c(theme::BLUE))
                                .border_1()
                                .border_color(c(theme::BG_TITLE)),
                        )
                    })
                    .when(n > 0, |d| {
                        d.child(
                            div()
                                .absolute()
                                .right(theme::u(4.))
                                .bottom(theme::u(5.))
                                .min_w(theme::u(15.))
                                .h(theme::u(15.))
                                .px_1()
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(c(theme::ACCENT))
                                .text_color(c(theme::ON_ACCENT))
                                .text_size(theme::u(9.5))
                                .font_weight(FontWeight::BOLD)
                                .child(if n > 99 { "99+".to_string() } else { n.to_string() }),
                        )
                    })
            })
            .child(view_item("ab-run", Icon::new(IconName::Play), SideView::Run, self, cx))
            .child(view_item("ab-ext", Icon::new(gpui_kit::assets::IconName::Blocks), SideView::Extensions, self, cx))
            .child(view_item("ab-agent", Icon::new(gpui_kit::assets::IconName::MessageSquareCode), SideView::Agent, self, cx))
            .child({
                let active = self.wiztree.page && !self.settings_open;
                div()
                    .id("ab-wiztree-page")
                    .relative()
                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Disk Usage (WizTree)")).build(w, cx))
                    .child(item("ab-wiztree", Icon::new(gpui_kit::assets::IconName::HardDrive), active, self, cx))
                    .on_click(cx.listener(|this, _, w, cx| this.toggle_wiztree(w, cx)))
            })
            .child(div().flex_1())
            .child(
                div()
                    .id(("ab-settings", 0usize))
                    .pb_1()
                    .child(item("ab-settings", Icon::new(IconName::Settings), self.settings_open, self, cx))
                    .on_click(cx.listener(|this, _, w, cx| this.on_show_settings(&ShowSettings, w, cx))),
            )
    }

    fn render_side_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.render_view_panel(self.view, false, window, cx)
    }

    pub(crate) fn render_view_panel(&self, view: SideView, right: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let title = match view {
            SideView::Explorer => tr("EXPLORER"),
            SideView::Search => tr("SEARCH"),
            SideView::Run => tr("RUN"),
            SideView::Extensions => tr("EXTENSIONS"),
            SideView::Scm => tr("SOURCE CONTROL"),
            SideView::Agent => crate::agent::agent_name(),
        };
        let title: SharedString = match (view, self.root.as_deref().and_then(Path::file_name)) {
            (SideView::Explorer, Some(_)) if !self.workspace.extra.is_empty() => tr("WORKSPACE").into(),
            (SideView::Explorer, Some(name)) => name.to_string_lossy().to_uppercase().into(),
            _ => title.into(),
        };
        let title_el: AnyElement = {
            let label = div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(theme::FG_MUTED))
                .child(title);
            let ws_anim = self.explorer.workspace_anim;
            if view == SideView::Explorer && ws_anim > 0 && !cx.reduce_motion() {
                label
                    .with_animation(
                        SharedString::from(format!("ws-title-{ws_anim}")),
                        Animation::new(Duration::from_millis(200)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(0.25 + 0.75 * t).ml(px((1. - t) * 6.)),
                    )
                    .into_any_element()
            } else {
                label.into_any_element()
            }
        };
        let header = h_flex()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .h(theme::u(35.))
            .pl_5()
            .pr_2()
            .gap_2()
            .flex_none()
            .when(view != SideView::Agent, |h| h.child(title_el))
            .when(view == SideView::Agent, |h| {
                h.child(
                    div()
                        .min_w_0()
                        .flex_shrink(1.)
                        .overflow_hidden()
                        .child(self.render_agent_picker(cx)),
                )
                .child(div().flex_none().child(self.render_agent_switch(cx)))
                .child(div().flex_1())
            })
            .when(view == SideView::Scm && self.scm.repo.is_some(), |h| {
                h.child(div().flex_none().child(self.render_scm_header(cx)))
            })
            .when(view == SideView::Agent, |h| {
                h.child(div().flex_none().child(self.render_agent_tools(cx)))
            })
            .when(view == SideView::Explorer && self.root.is_some(), |h| {
                h.child(
                    h_flex()
                        .flex_none()
                        .gap_0p5()
                        .child(tool_button(
                            "x-newfile",
                            IconName::Plus,
                            tr("New File"),
                            cx,
                            |this, w, cx| this.on_explorer_new_file(&ExplorerNewFile, w, cx),
                        ))
                        .child(tool_button(
                            "x-newdir",
                            IconName::FolderClosed,
                            tr("New Folder"),
                            cx,
                            |this, w, cx| this.on_explorer_new_folder(&ExplorerNewFolder, w, cx),
                        ))
                        .child(tool_button(
                            "x-addws",
                            IconName::FolderOpen,
                            tr("Add Folder to Workspace"),
                            cx,
                            |this, w, cx| {
                                this.on_add_folder_to_workspace(&AddFolderToWorkspace, w, cx)
                            },
                        ))
                        .child(tool_button(
                            "x-refresh",
                            IconName::RefreshCw,
                            tr("Refresh"),
                            cx,
                            |this, w, cx| this.on_explorer_refresh(&ExplorerRefresh, w, cx),
                        )),
                )
            })
            .when(right, |h| {
                h.children(self.render_right_tabs(cx))
                    .child(div().flex_none().child(self.right_close_button(cx)))
            });
        let body: AnyElement = match view {
            SideView::Explorer => self.render_explorer(window, cx),
            SideView::Search => self.render_search(cx),
            SideView::Run => self.render_run_view(cx),
            SideView::Extensions => self.render_ext_view(cx),
            SideView::Scm => self.render_scm(cx),
            SideView::Agent => self.render_agent(window, cx),
        };
        v_flex()
            .size_full()
            .bg(c(theme::BG_SIDE))
            .when(right, |d| d.border_l_1())
            .when(!right, |d| d.border_r_1())
            .border_color(c(theme::BORDER))
            .child(header)
            .child(div().flex_1().min_h_0().size_full().child(body))
            .into_any_element()
    }

    fn render_run_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let doc = self.active.map(|a| &self.docs[a]);
        let runnable = doc.filter(|d| crate::panel::can_run(&d.path));
        let (lead, holds_file) = self.ext_lead();
        let file_leads = runnable.is_some() && !holds_file;
        let projects = self.render_ext_run(if file_leads { None } else { lead }, cx);
        let has_projects = projects.is_some();
        let file_running = self.out_proc.is_some() && self.ext.running.is_none();
        let mut v = v_flex().id("run-view").size_full().overflow_y_scroll().px_3().pt_1().pb_4().gap(theme::u(20.)).when_some(projects, |v, p| v.child(p));
        if let Some(d) = runnable {
            let label: SharedString = crate::trf!("Run {f}"; f => d.name).into();
            let tip: SharedString = crate::trf!("Run {f} (F5)"; f => d.name).into();
            let run = |this: &mut Self, w: &mut Window, cx: &mut Context<Self>| this.on_run_file(&RunFile, w, cx);
            let body = if !file_leads {
                crate::ext::view::run_row("run-file".into(), gpui_kit::assets::IconName::Play, label, file_running, tip)
                    .on_click(cx.listener(move |this, _, w, cx| run(this, w, cx)))
                    .when(file_running, |r| r.child(self.run_stop("run-file-stop", 20., cx)))
                    .when(!file_running, |r| r.child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child("F5")))
                    .into_any_element()
            } else {
                h_flex()
                    .w_full()
                    .gap_1p5()
                    .child(crate::ext::view::run_main_button("run-file".into(), gpui_kit::assets::IconName::Play, label, file_running, true, tip).on_click(cx.listener(move |this, _, w, cx| run(this, w, cx))))
                    .when(file_running, |h| h.child(self.run_stop("run-file-stop", 30., cx)))
                    .into_any_element()
            };
            v = v.child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .when(has_projects, |b| {
                        b.child(div().h(px(1.)).bg(c(theme::BORDER))).child(div().px_2().pt_3().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_MUTED)).child(tr("This file")))
                    })
                    .child(body),
            );
        } else if !has_projects {
            let (title, text): (SharedString, SharedString) = match (doc, &self.root) {
                (Some(d), _) => (
                    crate::trf!("{f} can't be run"; f => d.name).into(),
                    tr("F5 runs Python, JavaScript, TypeScript, PowerShell, batch, shell, Rust and Go files. Build tasks for a project come from its extension.").into(),
                ),
                (None, None) => (tr("Nothing to run yet").into(), tr("Open a project folder to get its build tasks here, or open a script and press F5.").into()),
                (None, Some(_)) if self.ext.scanning => (tr("Looking for projects…").into(), tr("Build tasks show up here for each project an extension knows.").into()),
                (None, Some(_)) => (tr("Nothing to run yet").into(), tr("No project here has an installed extension. Open a script and press F5, or find an extension for your project.").into()),
            };
            let button = if self.root.is_none() {
                self.action_button("run-open-folder", tr("Open folder"), ui::Kind::Plain, cx, |this, w, cx| this.on_open_folder(&OpenFolder, w, cx)).into_any_element()
            } else {
                self.action_button("run-browse-ext", tr("Browse extensions"), ui::Kind::Plain, cx, |this, _, cx| this.show_view(SideView::Extensions, cx)).into_any_element()
            };
            v = v.child(
                v_flex()
                    .w_full()
                    .px_2()
                    .pt_2()
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(title))
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(text))
                    .child(h_flex().pt_2().child(button)),
            );
        }
        if file_running && runnable.is_none() {
            let what = self.out_run.as_ref().map(|r| r.command.clone()).unwrap_or_default();
            v = v.child(
                crate::ext::view::run_row("run-file-bg".into(), gpui_kit::assets::IconName::Play, crate::trf!("Running {what}"; what => what).into(), true, tr("Show the output").into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.panel_open = true;
                        this.panel_tab = PanelTab::Output;
                        cx.notify();
                    }))
                    .child(self.run_stop("run-file-bg-stop", 20., cx)),
            );
        }
        v.into_any_element()
    }

    fn tab_hover_enter(&mut self, ix: usize, left_px: f32, width_px: f32, cx: &mut Context<Self>) {
        if self.tab_hover_ix == Some(ix) && (self.tab_hover_to.0 - left_px).abs() < 0.5 && (self.tab_hover_to.1 - width_px).abs() < 0.5 {
            return;
        }
        let gliding = self.tab_hover_ix.is_some()
            || self.tab_hover_last_at.is_some_and(|t| t.elapsed() < Duration::from_millis(140));
        self.tab_hover_from = if gliding { self.tab_hover_to } else { (left_px, width_px) };
        self.tab_hover_to = (left_px, width_px);
        self.tab_hover_ix = Some(ix);
        self.tab_hover_fade_in = !gliding;
        self.tab_hover_leave = None;
        self.tab_hover_last_at = Some(std::time::Instant::now());
        self.tab_hover_seq = self.tab_hover_seq.wrapping_add(1);
        cx.notify();
    }

    fn tab_hover_clear(&mut self, cx: &mut Context<Self>) {
        if self.tab_hover_ix.take().is_none() {
            return;
        }
        self.tab_hover_last_at = Some(std::time::Instant::now());
        if cx.reduce_motion() {
            self.tab_hover_leave = None;
            cx.notify();
            return;
        }
        self.tab_hover_seq = self.tab_hover_seq.wrapping_add(1);
        let seq = self.tab_hover_seq;
        self.tab_hover_leave = Some((self.tab_hover_to, seq));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(130)).await;
            _ = this.update(cx, |this, cx| {
                if this.tab_hover_leave.is_some_and(|(_, s)| s == seq) {
                    this.tab_hover_leave = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = f32::from(theme::u(1.));
        let widths: Vec<f32> = self
            .docs
            .iter()
            .map(|d| tab_width(&d.name, d.error_count > 0 || d.warn_count > 0, scale))
            .collect();
        let total_w: f32 = widths.iter().sum();
        let scroll_x = (-f32::from(self.tabs_scroll.offset().x)).max(0.);
        self.tabs_scroll_x.set(scroll_x);
        let viewport_w = self.tabs_width.get().max(240.);
        let (range, left_w, right_w) = visible_tab_window(&widths, scroll_x, viewport_w);
        let motion = !cx.reduce_motion();
        let opened_tab = self.opened_tab.clone();
        let switch_seq = self.tab_switch_seq;

        let mut tab_lefts: Vec<f32> = Vec::with_capacity(widths.len());
        let mut acc = 0.;
        for &w in &widths {
            tab_lefts.push(acc);
            acc += w;
        }

        let hover_pill: Option<AnyElement> = if !motion {
            None
        } else if let Some(hix) = self.tab_hover_ix.filter(|&i| i < widths.len()) {
            let live_x = tab_lefts[hix] - scroll_x;
            let live_w = widths[hix];
            let (fx, fw) = self.tab_hover_from;
            let (tx, tw) = self.tab_hover_to;
            let seq = self.tab_hover_seq;
            let fade_in = self.tab_hover_fade_in;
            let scrolled = (live_x - tx).abs() > 1.0;
            let pill = div()
                .absolute()
                .top(px(3.))
                .bottom(px(3.))
                .rounded(px(5.))
                .bg(c(theme::BG_HOVER).opacity(0.72))
                .border_1()
                .border_color(c(theme::BORDER_STRONG).opacity(0.85));
            Some(if scrolled {
                pill.left(px(live_x + 2.)).w(px((live_w - 4.).max(4.))).into_any_element()
            } else {
                pill.with_animation(
                    SharedString::from(format!("tab-hov-{seq}")),
                    Animation::new(Duration::from_millis(if fade_in { 120 } else { 150 })).with_easing(gpui_kit::ease_out_quint()),
                    move |d, t| {
                        let x = fx + (tx - fx) * t;
                        let w = fw + (tw - fw) * t;
                        let op = if fade_in { t } else { 1. };
                        d.left(px(x + 2.)).w(px((w - 4.).max(4.))).opacity(op)
                    },
                )
                .into_any_element()
            })
        } else if let Some(((lx, lw), seq)) = self.tab_hover_leave {
            Some(
                div()
                    .absolute()
                    .top(px(3.))
                    .bottom(px(3.))
                    .left(px(lx + 2.))
                    .w(px((lw - 4.).max(4.)))
                    .rounded(px(5.))
                    .bg(c(theme::BG_HOVER).opacity(0.72))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG).opacity(0.85))
                    .with_animation(
                        SharedString::from(format!("tab-hov-out-{seq}")),
                        Animation::new(Duration::from_millis(120)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(1. - t),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        let probe = {
            let w_cell = self.tabs_width.clone();
            let sx_cell = self.tabs_scroll_x.clone();
            let scroll = self.tabs_scroll.clone();
            let id = cx.entity_id();
            canvas(
                move |b, _, cx| {
                    let w = f32::from(b.size.width);
                    let sx = (-f32::from(scroll.offset().x)).max(0.);
                    let mut dirty = false;
                    if (w_cell.get() - w).abs() > 1.0 && w > 10. {
                        w_cell.set(w);
                        dirty = true;
                    }
                    if (sx_cell.get() - sx).abs() > 1.0 {
                        sx_cell.set(sx);
                        dirty = true;
                    }
                    if dirty {
                        cx.defer(move |cx| cx.notify(id));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        let visible_tabs: Vec<AnyElement> = range
            .map(|ix| {
                let d = &self.docs[ix];
                let w = widths[ix];
                let left_in_view = tab_lefts[ix] - scroll_x;
                let active = self.active == Some(ix);
                let (icon, tint) = d.icon;
                let errs = d.error_count;
                let warns = d.warn_count;
                let tab_id = SharedString::from(format!("tab-{}", d.path.display()));
                let close_id = SharedString::from(format!("tab-x-{}", d.path.display()));
                let tab = h_flex()
                    .id(tab_id)
                    .w(px(w))
                    .h_full()
                    .pl(theme::u(12.))
                    .pr(theme::u(6.))
                    .gap(theme::u(6.))
                    .flex_none()
                    .text_sm()
                    .border_r_1()
                    .border_color(c(theme::BORDER))
                    .relative()
                    .cursor_pointer()
                    .when(active, |s| {
                        let line = div().absolute().top_0().left_0().right_0().h(px(2.)).bg(c(theme::ACCENT));
                        let el = if !motion {
                            line.into_any_element()
                        } else {
                            line.with_animation(
                                SharedString::from(format!("tab-line-{switch_seq}")),
                                Animation::new(Duration::from_millis(190)).with_easing(gpui_kit::ease_out_quint()),
                                |el, t| el.left(relative(0.5 * (1. - t))).right(relative(0.5 * (1. - t))),
                            )
                            .into_any_element()
                        };
                        s.bg(c(theme::BG_EDITOR)).child(el)
                    })
                    .when(!active && !motion, |s| s.hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG))))
                    .when(!active && motion, |s| s.hover(|s| s.text_color(c(theme::FG))))
                    .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                        if *entered {
                            this.tab_hover_enter(ix, left_in_view, w, cx);
                        }
                    }))
                    .text_color(if active { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                    .child(Icon::new(icon).flex_none().size(theme::u(15.)).text_color(theme::tint(tint)))
                    .child(div().flex_1().min_w_0().truncate().whitespace_nowrap().child(d.name.clone()))
                    .when(errs > 0 || warns > 0, |s| {
                        let (badge_icon, col, count) = if errs > 0 {
                            (IconName::CircleX, theme::DANGER, errs)
                        } else {
                            (IconName::TriangleAlert, theme::WARN, warns)
                        };
                        s.child(
                            h_flex()
                                .flex_none()
                                .gap(px(2.))
                                .text_xs()
                                .text_color(c(col))
                                .child(Icon::new(badge_icon).size(theme::u(11.)))
                                .child(if count > 99 { "99+".to_string() } else { count.to_string() }),
                        )
                    })
                    .child(
                        div()
                            .id(close_id)
                            .flex_none()
                            .size(theme::u(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .text_color(if d.dirty { c(theme::ACCENT_TEXT) } else { c(theme::FG_MUTED) })
                            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                            .child(if d.dirty { "●" } else { "×" })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close(ix, window, cx);
                            })),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.agent.user_nav = Some(std::time::Instant::now());
                        this.activate(ix, window, cx)
                    }))
                    .context_menu({
                        let weak = cx.entity().downgrade();
                        let path = d.path.clone();
                        let rel = self.rel_display(&d.path.to_string_lossy());
                        move |menu, _, _| {
                            let item = |label: &'static str, f: Box<dyn Fn(&mut Workbench, &mut Window, &mut Context<Workbench>)>| {
                                let weak = weak.clone();
                                PopupMenuItem::new(tr(label)).on_click(move |_, w, cx| {
                                    _ = weak.update(cx, |this, cx| f(this, w, cx));
                                })
                            };
                            let (full, rel) = (path.to_string_lossy().into_owned(), rel.clone());
                            let reveal = path.clone();
                            menu.item(item("Close", Box::new(move |this, w, cx| this.close(ix, w, cx))))
                                .item(item("Close Others", Box::new(move |this, w, cx| this.close_tabs_where(w, cx, |i, d| i != ix && !d.dirty))))
                                .item(item("Close to the Right", Box::new(move |this, w, cx| this.close_tabs_where(w, cx, |i, d| i > ix && !d.dirty))))
                                .item(item("Close Saved", Box::new(move |this, w, cx| this.close_tabs_where(w, cx, |_, d| !d.dirty))))
                                .item(item("Close All", Box::new(move |this, w, cx| this.close_tabs_where(w, cx, |_, d| !d.dirty))))
                                .separator()
                                .item(item("Copy Path", Box::new(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(full.clone())))))
                                .item(item("Copy Relative Path", Box::new(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(rel.replace('\\', "/"))))))
                                .separator()
                                .item(item("Reveal in File Explorer", Box::new(move |_, _, cx| cx.reveal_path(&reveal))))
                                .item(item("Go to File...", Box::new(|_, w, cx| w.dispatch_action(Box::new(QuickOpenFile), cx))))
                        }
                    });
                if motion {
                    if let Some((ref op, seq)) = opened_tab {
                        if *op == d.path {
                            return tab
                                .with_animation(
                                    SharedString::from(format!("tab-open-{seq}")),
                                    Animation::new(Duration::from_millis(150)).with_easing(gpui_kit::ease_out_quint()),
                                    |el, t| el.opacity(0.2 + 0.8 * t),
                                )
                                .into_any_element();
                        }
                    }
                }
                tab.into_any_element()
            })
            .collect();

        div()
            .id("tabs-wrap")
            .relative()
            .w_full()
            .h(theme::u(35.))
            .flex_none()
            .overflow_hidden()
            .bg(c(theme::BG_SIDE))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .on_hover(cx.listener(|this, entered: &bool, _, cx| {
                if !*entered {
                    this.tab_hover_clear(cx);
                }
            }))
            .child(probe)
            .children(hover_pill)
            .child(
                h_flex()
                    .id("tabs")
                    .size_full()
                    .overflow_x_scroll()
                    .track_scroll(&self.tabs_scroll)
                    .on_scroll(cx.listener(move |this, ev: &ScrollEvent, _, cx| {
                        let d = ev.delta.pixel_delta(px(36.));
                        let delta = if d.x.abs() > px(0.5) { f32::from(d.x) } else { f32::from(d.y) };
                        if delta.abs() < 0.5 {
                            return;
                        }
                        let vw = this.tabs_width.get().max(240.);
                        let max_x = (total_w - vw).max(0.);
                        let cur = (-f32::from(this.tabs_scroll.offset().x)).max(0.);
                        let next = (cur - delta).clamp(0., max_x);
                        if (next - cur).abs() > 0.5 {
                            TAB_SCROLL_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            cx.stop_propagation();
                            this.tabs_scroll.set_offset(point(px(-next), px(0.)));
                            this.tabs_scroll_x.set(next);
                            cx.notify();
                        }
                    }))
                    .when(left_w > 0.5, |h| h.child(div().flex_none().w(px(left_w)).h_full()))
                    .children(visible_tabs)
                    .when(right_w > 0.5, |h| h.child(div().flex_none().w(px(right_w)).h_full())),
            )
    }

    pub(crate) fn close_tabs_where(&mut self, window: &mut Window, cx: &mut Context<Self>, close: impl Fn(usize, &Doc) -> bool) {
        let active = self.active.and_then(|a| self.docs.get(a)).map(|d| d.path.clone());
        let doomed: Vec<usize> = self.docs.iter().enumerate().filter(|(i, d)| close(*i, d)).map(|(i, _)| i).collect();
        for &i in doomed.iter().rev() {
            let closed = self.docs.remove(i);
            if !closed.read_only {
                self.copilot.did_close(&closed.path);
                self.closed_tabs.retain(|p| *p != closed.path);
                self.closed_tabs.push(closed.path);
            }
        }
        let n = self.closed_tabs.len();
        if n > 30 {
            self.closed_tabs.drain(..n - 30);
        }
        self.active = active.and_then(|p| self.docs.iter().position(|d| d.path == p)).or((!self.docs.is_empty()).then(|| self.docs.len() - 1));
        match self.active {
            Some(a) => self.activate(a, window, cx),
            None => {
                self.focus.focus(window, cx);
                cx.notify();
            }
        }
        self.save_session(cx);
    }

    fn render_editor_header(&self, ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let d = &self.docs[ix];
        let tools: AnyElement = if d.image.is_some() {
            self.render_image_tools(ix, cx)
        } else if d.flux.is_some() {
            h_flex()
                .gap_1()
                .when(d.flux_source, |h| h.child(self.wrap_button(ix, cx)))
                .child(self.render_flux_bar(ix, cx))
                .into_any_element()
        } else if d.diff.as_ref().is_some_and(|x| !x.source) {
            div().into_any_element()
        } else {
            self.wrap_button(ix, cx)
        };
        h_flex()
            .h(theme::u(28.))
            .flex_none()
            .pr_2()
            .gap_2()
            .child(div().flex_1().min_w_0().overflow_hidden().child(self.render_breadcrumbs(ix)))
            .child(tools)
    }

    fn wrap_button(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let on = self.docs[ix].wraps(self.settings.word_wrap);
        ui::tool_button("hdr-wrap", gpui_kit::assets::IconName::TextWrap, on)
            .tooltip(move |w, cx| {
                gpui_kit::component::tooltip::Tooltip::new(if on { tr("Word wrap is on for this file (Alt+Z)") } else { tr("Wrap long lines in this file (Alt+Z)") }).build(w, cx)
            })
            .on_click(cx.listener(|this, _, w, cx| this.toggle_wrap(w, cx)))
            .into_any_element()
    }

    fn render_breadcrumbs(&self, ix: usize) -> impl IntoElement {
        let d = &self.docs[ix];
        let shown = self.rel_display(&d.path.to_string_lossy());
        let rel = if shown == d.path.to_string_lossy() { PathBuf::from(d.name.as_ref()) } else { PathBuf::from(shown) };
        let parts: Vec<String> = rel.components().map(|p| p.as_os_str().to_string_lossy().into_owned()).collect();
        let last = parts.len().saturating_sub(1);
        h_flex()
            .h(theme::u(22.))
            .px_3()
            .gap_1()
            .flex_none()
            .text_xs()
            .text_color(c(theme::FG_MUTED))
            .children(parts.into_iter().enumerate().flat_map(move |(i, p)| {
                let sep = (i > 0).then(|| div().text_color(c(theme::FG_DIM)).child("›").into_any_element());
                let name = div().when(i == last, |d| d.text_color(c(theme::FG))).child(p).into_any_element();
                sep.into_iter().chain(std::iter::once(name))
            }))
    }

    fn render_editor_area(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.settings_open {
            return self.render_settings(window, cx);
        }
        if self.wiztree.page {
            return self.render_wiztree(window, cx);
        }
        if let Some(id) = self.ext.page.clone() {
            return self.render_ext_page(&id, cx);
        }
        if let Some(page) = &self.whats_new {
            return self.render_whats_new(page, cx);
        }
        let motion = !cx.reduce_motion();
        let backdrop = theme::has_backdrop() || crate::scene::active();
        let (had, mut gen) = self.wash.get();
        if had != self.active.is_some() {
            gen += 1;
            self.wash.set((self.active.is_some(), gen));
        }
        let wash_layer = |fade_in: bool| {
            let layer = div().absolute().top_0().left_0().size_full().bg(theme::editor_wash());
            if gen == 0 || !motion {
                return layer.when(!fade_in, |d| d.opacity(0.)).into_any_element();
            }
            layer
                .with_animation(
                    SharedString::from(format!("wash-{gen}")),
                    Animation::new(std::time::Duration::from_millis(if fade_in { 260 } else { 320 })).with_easing(gpui_kit::ease_in_out),
                    move |d, t| d.opacity(if fade_in { t } else { 1. - t }),
                )
                .into_any_element()
        };
        match self.active {
            Some(ix) => {
                let jump_seq = self
                    .file_jump_anim
                    .as_ref()
                    .filter(|(p, _)| *p == self.docs[ix].path)
                    .map(|(_, s)| *s);
                let accent = c(theme::ACCENT);
                let body: AnyElement = if self.docs[ix].image.is_some() {
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_image(ix, window, cx))
                        .into_any_element()
                } else if self.docs[ix].diff.as_ref().is_some_and(|d| !d.source) {
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_diff_doc(ix, cx))
                        .into_any_element()
                } else if self.docs[ix].flux.is_some() && !self.docs[ix].flux_source {
                    let weak = cx.entity().downgrade();
                    match self.docs[ix].flux.as_ref() {
                        Some(flux) => {
                            let view = flux.render(move |cx| {
                                _ = weak.update(cx, |_, cx| cx.notify());
                            });
                            div().flex_1().min_h_0().child(view).into_any_element()
                        }
                        None => div().flex_1().min_h_0().into_any_element(),
                    }
                } else if let Some(ed) = self.docs[ix].editor.as_ref() {
                    let mono_font = theme::resolve_mono_font(&self.settings.font_family, cx);
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .child(
                            Editor::new(ed)
                                .bordered(false)
                                .p_0()
                                .h_full()
                                .font_family(mono_font)
                                .text_size(px(self.settings.font_size)),
                        )
                        .child(self.smooth_scroll_layer(ScrollTarget::Editor(ed.clone()), cx))
                        .into_any_element()
                } else {
                    let msg = if self.docs[ix].loading {
                        tr("Loading file…")
                    } else {
                        tr("Opening file…")
                    };
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(c(theme::FG_MUTED))
                        .child(msg)
                        .into_any_element()
                };
                let animated_body: AnyElement = if self.switch_fast || !motion {
                    body
                } else {
                    let seq = self.tab_switch_seq;
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .child(body)
                        .with_animation(
                            SharedString::from(format!("doc-in-{seq}")),
                            Animation::new(Duration::from_millis(200))
                                .with_easing(gpui_kit::ease_out_quint()),
                            |el, t| el.opacity(0.32 + 0.68 * t).mt(px((1. - t) * 5.)),
                        )
                        .into_any_element()
                };
                v_flex()
                    .relative()
                    .size_full()
                    .when(!backdrop, |v| v.bg(theme::editor_wash()))
                    .when(backdrop, |v| v.child(wash_layer(true)))
                    .child(self.render_tabs(cx))
                    .child(self.render_editor_header(ix, cx))
                    .when_some(self.docs[ix].banner.clone(), |v, b| {
                        v.child(
                            h_flex()
                                .flex_none()
                                .mx_3()
                                .mb_1()
                                .px_2()
                                .py_1()
                                .gap_2()
                                .rounded(px(4.))
                                .bg(c(theme::BG_ELEVATED))
                                .text_xs()
                                .text_color(c(theme::FG_MUTED))
                                .child(
                                    Icon::new(if self.docs[ix].read_only {
                                        IconName::Eye
                                    } else {
                                        IconName::Info
                                    })
                                    .small()
                                    .text_color(c(theme::FG_DIM)),
                                )
                                .child(div().min_w_0().truncate().child(b)),
                        )
                    })
                    .child(animated_body)
                    .when_some(jump_seq.filter(|_| motion), |v, seq| {
                        v.child(
                            div()
                                .absolute()
                                .top(theme::u(35.))
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .border_2()
                                .rounded(px(2.))
                                .with_animation(
                                    SharedString::from(format!("doc-jump-{seq}")),
                                    Animation::new(Duration::from_millis(420))
                                        .with_easing(gpui_kit::ease_out_quint()),
                                    move |el, t| {
                                        el.border_color(accent.opacity((1. - t) * 0.55))
                                            .bg(accent.opacity((1. - t) * 0.06))
                                    },
                                ),
                        )
                    })
                    .into_any_element()
            }
            None if self.welcome_w.get() < 230. => div()
                .relative()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .when(!backdrop, |d| d.bg(theme::editor_wash()))
                .when(backdrop, |d| d.child(div().absolute().top_0().left_0().size_full().bg(theme::welcome_tint())).child(wash_layer(false)))
                .child(self.welcome_probe(cx))
                .when(self.welcome_w.get() >= 84., |d| d.child(ui::meander(48., c(theme::FG_STRONG), c(theme::ACCENT_TEXT))))
                .into_any_element(),
            None => v_flex()
                .id("welcome")
                .relative()
                .size_full()
                .overflow_y_scroll()
                .when(!backdrop, |d| d.bg(theme::editor_wash()))
                .when(backdrop, |d| d.child(div().absolute().top_0().left_0().size_full().bg(theme::welcome_tint())).child(wash_layer(false)))
                .child(self.welcome_probe(cx))
                .child(
                    v_flex()
                        .flex_none()
                        .w_full()
                        .min_h(relative(1.))
                        .px_4()
                        .py_6()
                        .items_center()
                        .justify_center()
                        .gap_5()
                        .child(
                            v_flex()
                                .w_full()
                                .max_w(theme::u(420.))
                                .items_center()
                                .gap_1()
                                .child(ui::meander(72., c(theme::FG_STRONG), c(theme::ACCENT_TEXT)).mb_4())
                                .child(div().max_w_full().truncate().text_size(theme::u(32.)).font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("FluxCode")))
                                .child(div().w_full().text_center().text_sm().text_color(c(theme::FG_MUTED)).child(tr("A native code editor that runs as one process."))),
                        )
                        .child(
                            h_flex()
                                .flex_wrap()
                                .justify_center()
                                .gap_2()
                                .child(self.action_button("w-open-folder", tr("Open folder"), ui::Kind::Primary, cx, |this, w, cx| this.on_open_folder(&OpenFolder, w, cx)))
                                .child(self.action_button("w-open-file", tr("Open file"), ui::Kind::Plain, cx, |this, w, cx| this.on_open_file(&OpenFile, w, cx))),
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .max_w(theme::u(320.))
                                .gap_2()
                                .pt_2()
                                .child(shortcut(tr("Find in files"), "Ctrl+Shift+F"))
                        .child(shortcut(tr("Toggle terminal"), "Ctrl+`"))
                        .child(shortcut(tr("Run file"), "F5"))
                        .child(shortcut(tr("AI agent"), "Ctrl+L"))
                        .child(shortcut(tr("Next color theme"), "Ctrl+K Ctrl+T"))
                                .child(shortcut(tr("Extensions"), "Ctrl+Shift+X"))
                                .child(shortcut(tr("Settings"), "Ctrl+,")),
                        ),
                )
                .into_any_element(),
        }
    }

    fn welcome_probe(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let cell = self.welcome_w.clone();
        let eid = cx.entity_id();
        canvas(
            move |b, _, cx| {
                let w = f32::from(b.size.width);
                let was = cell.replace(w);
                let side = |v: f32| (v < 84., v < 230.);
                if side(was) != side(w) {
                    cx.defer(move |cx| cx.notify(eid));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    fn render_status_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vp_w = f32::from(window.viewport_size().width);
        let wide = vp_w >= 820.;
        let medium = vp_w >= 640.;
        let (pos, lang) = match self.active {
            Some(ix) if self.docs[ix].image.is_some() => {
                let im = self.docs[ix].image.as_ref().unwrap();
                (im.size.map(|(w, h)| format!("{w} × {h}")).unwrap_or_default(), "Image")
            }
            Some(ix) => {
                let d = &self.docs[ix];
                let p = d.cursor_position(cx);
                (crate::trf!("Ln {0}, Col {1}"; 0 => p.line + 1, 1 => p.character + 1), d.label)
            }
            None => (String::new(), ""),
        };
        let is_image = self.active.is_some_and(|a| self.docs[a].image.is_some() || self.docs[a].read_only);
        let crlf = self.active.filter(|_| !is_image).map(|a| if self.docs[a].crlf { "CRLF" } else { "LF" }).unwrap_or("");
        let running = self.out_proc.is_some() || self.terms.iter().any(|t| t.proc.is_some());
        let code_errs: usize = self.docs.iter().map(|d| d.error_count).sum();
        let code_warns: usize = self.docs.iter().map(|d| d.warn_count).sum();
        let total_errs = self.problems.len() + code_errs;
        let seg = |s: SharedString| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .h_full()
                .px_2()
                .flex()
                .items_center()
                .child(s)
        };
        let folder = self
            .root
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned());
        h_flex()
            .h(theme::u(24.))
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .flex_none()
            .bg(c(theme::BG_TITLE))
            .border_t_1()
            .border_color(c(theme::BORDER))
            .text_xs()
            .text_color(c(theme::FG_MUTED))
            .when_some(folder.filter(|_| medium), |h, f| {
                h.child(
                    div()
                        .flex_none()
                        .max_w(theme::u(140.))
                        .truncate()
                        .h_full()
                        .pl_3()
                        .pr_2()
                        .flex()
                        .items_center()
                        .text_color(c(theme::FG))
                        .child(f),
                )
            })
            .when_some(
                self.wsl
                    .as_ref()
                    .map(|w| w.distro.clone())
                    .filter(|_| medium),
                |h, d| {
                    h.child(
                        h_flex()
                            .id("sb-wsl")
                            .flex_none()
                            .whitespace_nowrap()
                            .h_full()
                            .px_2()
                            .gap_1()
                            .text_color(c(theme::ACCENT_TEXT))
                            .child(format!("WSL: {d}")),
                    )
                },
            )
            .when_some(
                self.scm
                    .status
                    .branch
                    .clone()
                    .filter(|_| self.scm.repo.is_some()),
                |h, b| {
                    let st = &self.scm.status;
                    let dirty = if st.count() > 0 { "*" } else { "" };
                    let sync = match &st.upstream {
                        None => "Publish".to_string(),
                        Some(_) => format!("{}↓ {}↑", st.behind, st.ahead),
                    };
                    h.child(
                        h_flex()
                            .id("sb-branch")
                            .flex_none()
                            .max_w(theme::u(140.))
                            .h_full()
                            .px_2()
                            .gap_1()
                            .cursor_pointer()
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(
                                Icon::new(gpui_kit::assets::IconName::GitBranch)
                                    .flex_none()
                                    .size(theme::u(13.)),
                            )
                            .child(div().min_w_0().truncate().child(format!("{b}{dirty}")))
                            .on_click(
                                cx.listener(|this, _, w, cx| this.on_scm_show(&ScmShow, w, cx)),
                            ),
                    )
                    .when(medium, |h| {
                        h.child(
                            h_flex()
                                .id("sb-sync")
                                .flex_none()
                                .whitespace_nowrap()
                                .h_full()
                                .px_2()
                                .gap_1()
                                .cursor_pointer()
                                .hover(|s| s.bg(c(theme::BG_HOVER)))
                                .child(
                                    Icon::new(if st.upstream.is_some() {
                                        gpui_kit::assets::IconName::RefreshCcw
                                    } else {
                                        gpui_kit::assets::IconName::CloudUpload
                                    })
                                    .flex_none()
                                    .size(theme::u(13.)),
                                )
                                .child(if self.scm.busy.is_some() {
                                    "…".to_string()
                                } else {
                                    sync
                                })
                                .on_click(
                                    cx.listener(|this, _, w, cx| this.on_scm_sync(&ScmSync, w, cx)),
                                ),
                        )
                    })
                },
            )
            .when(running, |h| {
                h.child(
                    h_flex()
                        .id("sb-running")
                        .flex_none()
                        .whitespace_nowrap()
                        .h_full()
                        .px_2()
                        .gap_1()
                        .cursor_pointer()
                        .text_color(c(theme::GREEN))
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(
                            div()
                                .flex_none()
                                .size(px(6.))
                                .rounded_full()
                                .bg(c(theme::GREEN)),
                        )
                        .child(tr("Running"))
                        .on_click(cx.listener(|this, _, w, cx| this.on_toggle_terminal(&ToggleTerminal, w, cx))),
                )
            })
            .when(total_errs > 0 || code_warns > 0, |h| {
                h.child(
                    h_flex()
                        .id("sb-problems")
                        .flex_none()
                        .whitespace_nowrap()
                        .h_full()
                        .px_2()
                        .gap_1p5()
                        .cursor_pointer()
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Show Problems (Ctrl+Shift+M)")).build(w, cx))
                        .when(total_errs > 0, |r| {
                            r.child(
                                h_flex()
                                    .gap_1()
                                    .text_color(c(theme::DANGER))
                                    .child(
                                        Icon::new(IconName::CircleX)
                                            .flex_none()
                                            .size(theme::u(13.)),
                                    )
                                    .child(total_errs.to_string()),
                            )
                        })
                        .when(code_warns > 0, |r| {
                            r.child(
                                h_flex()
                                    .gap_1()
                                    .text_color(c(theme::WARN))
                                    .child(
                                        Icon::new(IconName::TriangleAlert)
                                            .flex_none()
                                            .size(theme::u(13.)),
                                    )
                                    .child(code_warns.to_string()),
                            )
                        })
                        .on_click(cx.listener(|this, _, w, cx| this.on_show_problems(&ShowProblems, w, cx))),
                )
            })
            .child(
                div()
                    .px_2()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(c(theme::FG))
                    .child(self.status.clone()),
            )
            .child(self.render_agent_hint(!medium, cx))
            .when(!pos.is_empty(), |h| {
                h.when(wide, |h| h.child(self.render_copilot_hint(cx)))
                    .child(seg(pos.into()))
                    .when(wide && !is_image, |h| {
                        h.child(seg(crlf.into())).child(seg("UTF-8".into()))
                    })
                    .when(medium, |h| h.child(seg(lang.into())))
            })
            .child(self.render_theme_button(cx))
    }
}

pub(crate) fn fit_columns(total: f32, side: Option<f32>, agent: Option<f32>) -> (f32, f32) {
    const BAR: f32 = 40. + 8.; // activity bar and the two drag handles
    const EDITOR: f32 = 320.;
    const SIDE_MIN: f32 = 170.;
    const AGENT_MIN: f32 = 260.;
    let room = (total - BAR - EDITOR).max(0.);
    let (mut s, mut a) = (side.unwrap_or(0.), agent.unwrap_or(0.));
    if s + a > room {
        if side.is_some() {
            s = (room - a).max(SIDE_MIN).min(s);
        }
        if s + a > room && agent.is_some() {
            a = (room - s).max(AGENT_MIN).min(a);
        }
        if s + a > room {
            let all = (total - BAR).max(0.);
            let parts = 1. + side.is_some() as u8 as f32 + agent.is_some() as u8 as f32;
            let each = all / parts;
            if side.is_some() {
                s = s.min(each);
            }
            if agent.is_some() {
                a = a.min(each);
            }
        }
    }
    (s, a)
}

pub(crate) fn tool_button(
    id: &'static str,
    icon: IconName,
    tip: &'static str,
    cx: &mut Context<Workbench>,
    f: impl Fn(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
) -> impl IntoElement {
    Button::new(id).icon(icon).ghost().xsmall().tooltip(tip).on_click(cx.listener(move |this, _, w, cx| f(this, w, cx)))
}

fn shortcut(label: &'static str, keys: &'static str) -> impl IntoElement {
    h_flex()
        .w_full()
        .gap_2()
        .text_sm()
        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_MUTED)).child(label))
        .children(keys.split(['+', ' ']).map(|k| {
            div()
                .flex_none()
                .px_2()
                .rounded(px(3.))
                .text_xs()
                .bg(c(theme::BG_ELEVATED))
                .border_1()
                .border_color(c(theme::BORDER_STRONG))
                .text_color(c(theme::FG_STRONG))
                .child(k)
        }))
}

fn human_size(b: u64) -> String {
    match b {
        0..=1023 => format!("{b} B"),
        1024..=1_048_575 => format!("{:.0} KB", b as f64 / 1024.),
        _ => format!("{:.1} MB", b as f64 / 1_048_576.),
    }
}

fn clip(e: AnyElement) -> AnyElement {
    div().size_full().min_w_0().min_h_0().overflow_hidden().child(e).into_any_element()
}

const MAX_LINES: usize = 10_000;

pub(crate) fn push_line(lines: &mut Vec<Line>, text: &str, kind: LineKind) {
    close_line(lines);
    lines.push(Line::new(text, kind));
    trim(lines);
}

pub(crate) fn close_line(lines: &mut [Line]) {
    if let Some(l) = lines.last_mut() {
        l.open = false;
    }
}

pub(crate) fn push_text(lines: &mut Vec<Line>, text: &str, err: bool) {
    let clean = crate::proc::strip_ansi(text);
    let mut parts = clean.split('\n').peekable();
    while let Some(part) = parts.next() {
        let last_piece = parts.peek().is_none();
        match lines.last_mut() {
            Some(l) if l.open && l.err == err => l.text = format!("{}{}", l.text, part).into(),
            _ if part.is_empty() && last_piece => break,
            _ => {
                let mut l = Line::new(part, LineKind::Normal);
                l.open = true;
                l.err = err;
                lines.push(l);
            }
        }
        let n = lines.len();
        let prev = (n >= 2).then(|| lines[n - 2].kind);
        let l = &mut lines[n - 1];
        l.kind = crate::term::classify(&l.text, l.err, prev);
        l.link = match l.kind {
            LineKind::Frame => crate::term::parse_frame(&l.text)
                .map(|(p, n, _)| (p.into(), n, 0))
                .or_else(|| crate::term::parse_path_loc(&l.text).map(|(p, n, c)| (p.into(), n, c))),
            LineKind::Blank | LineKind::Cmd | LineKind::Status | LineKind::Fixed => None,
            _ => crate::term::parse_path_loc(&l.text).map(|(p, n, c)| (p.into(), n, c)),
        };
        if !last_piece {
            close_line(lines);
        }
    }
    trim(lines);
}

fn trim(lines: &mut Vec<Line>) {
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
}

impl Focusable for Workbench {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if crate::boot::pending() {
            crate::boot::mark("workbench render");
            window.on_next_frame(|_, _| {
                crate::boot::mark("first frame drawn");
                crate::boot::flush();
            });
        }
        if self.status != self.status_seen {
            self.status_seen = self.status.clone();
            if !self.status.is_empty() {
                let shown = self.status.clone();
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_secs(6)).await;
                    _ = this.update(cx, |this, cx| {
                        if this.status == shown {
                            this.status = SharedString::default();
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        }
        if std::mem::take(&mut self.agent.placeholder_dirty) {
            let name = crate::agent::agent_name();
            self.agent.input.update(cx, |s, cx| s.set_placeholder(crate::trf!("Ask {name}…"; name => name), window, cx));
        }
        if !self.agent.follow_q.is_empty() {
            self.apply_agent_follow(window, cx);
        }
        if !self.agent.reloads.is_empty() {
            self.apply_agent_reloads(window, cx);
        }
        if !self.agent.ask_clear.is_empty() {
            self.apply_ask_clear(window, cx);
        }
        if self.scm.clear_message {
            self.scm.clear_message = false;
            self.scm.message.update(cx, |s, cx| s.set_value("", window, cx));
        }
        window.set_rem_size(theme::rem());
        apply_glass(window);
        let editor_view: AnyElement = match &self.preview {
            Some(_) => h_resizable("editor-preview-split")
                .child(resizable_panel().child(clip(self.render_editor_area(window, cx))))
                .child(resizable_panel().size(px(520.)).size_range(px(240.)..px(1400.)).child(self.render_preview(cx)))
                .into_any_element(),
            None => self.render_editor_area(window, cx),
        };
        let editor_view = div().relative().size_full().child(editor_view).child(self.render_drop_hint()).into_any_element();
        if self.sidebar_open {
            self.side_closing = None;
        }
        if self.right_dock_on() {
            self.right_closing = None;
        }
        if self.panel_open {
            self.panel_closing = None;
        }
        let side_visible = self.sidebar_open || self.side_closing.is_some();
        let right_visible = self.right_dock_on() || self.right_closing.is_some();
        let (side_w, agent_w) = fit_columns(
            f32::from(window.viewport_size().width),
            side_visible.then_some(self.sidebar_width),
            (right_visible && !self.agent_full).then_some(self.agent_width),
        );
        let motion = !cx.reduce_motion();
        let (was_panel, mut panel_gen) = self.panel_anim.get();
        if was_panel != self.panel_open {
            if self.panel_open {
                panel_gen += 1;
            }
            self.panel_anim.set((self.panel_open, panel_gen));
        }
        let editor_and_panel: AnyElement = if self.panel_open {
            let panel_body: AnyElement = if panel_gen > 0 && motion {
                div()
                    .size_full()
                    .child(self.render_panel(window, cx))
                    .with_animation(
                        SharedString::from(format!("panel-in-{panel_gen}")),
                        Animation::new(Duration::from_millis(PANE_ANIM_MS)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(0.3 + 0.7 * t).mt(px((1. - t) * 10.)),
                    )
                    .into_any_element()
            } else {
                self.render_panel(window, cx).into_any_element()
            };
            v_resizable("editor-panel-split")
                .child(resizable_panel().child(clip(editor_view)))
                .child(resizable_panel().size(px(240.)).size_range(px(80.)..px(900.)).child(clip(panel_body)))
                .into_any_element()
        } else if let Some(seq) = self.panel_closing.filter(|_| motion) {
            let closing_panel = div()
                .flex_none()
                .w_full()
                .overflow_hidden()
                .child(self.render_panel(window, cx))
                .with_animation(
                    SharedString::from(format!("panel-out-{seq}")),
                    Animation::new(Duration::from_millis(PANE_ANIM_MS)).with_easing(gpui_kit::ease_out_quint()),
                    |d, t| d.h(px((220. * (1. - t)).max(0.))).opacity(1. - t),
                );
            v_flex()
                .size_full()
                .child(div().flex_1().min_h_0().child(clip(editor_view)))
                .child(closing_panel)
                .into_any_element()
        } else {
            editor_view
        };
        let handle = |id: &'static str, which: u8, this: &Self, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(4.))
                .h_full()
                .flex_none()
                .cursor_col_resize()
                .bg(c(theme::BG_SIDE))
                .hover(|s| s.bg(c(theme::ACCENT)))
                .when(this.resizing == which, |d| d.bg(c(theme::ACCENT)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    this.resizing = which;
                    cx.notify();
                }))
        };
        let (was_open, mut side_gen) = self.side_anim.get();
        if was_open != self.sidebar_open {
            if self.sidebar_open {
                side_gen += 1;
            }
            self.side_anim.set((self.sidebar_open, side_gen));
        }
        let side_column = |this: &Self, window: &mut Window, cx: &mut Context<Self>| -> AnyElement {
            let col = div().w(px(side_w)).flex_none().h_full().overflow_hidden().child(this.render_side_bar(window, cx));
            if let Some(seq) = this.side_closing.filter(|_| !this.sidebar_open && motion) {
                return col
                    .with_animation(
                        SharedString::from(format!("side-out-{seq}")),
                        Animation::new(Duration::from_millis(PANE_ANIM_MS)).with_easing(gpui_kit::ease_out_quint()),
                        move |d, t| d.w(px((side_w * (1. - t)).max(0.))).opacity(1. - t),
                    )
                    .into_any_element();
            }
            if side_gen == 0 || !motion {
                return col.into_any_element();
            }
            col.with_animation(
                SharedString::from(format!("side-in-{side_gen}")),
                Animation::new(Duration::from_millis(200)).with_easing(gpui_kit::ease_out_quint()),
                move |d, t| d.w(px((side_w * t).max(1.))).opacity(0.3 + 0.7 * t),
            )
            .into_any_element()
        };
        let full = self.agent_docked_visible() && self.agent_full && !self.settings_open;
        let (was_full, mut full_gen) = self.full_anim.get();
        if was_full != full {
            full_gen += 1;
            self.full_anim.set((full, full_gen));
        }
        let (was_settings, mut settings_gen) = self.settings_anim.get();
        if was_settings != self.settings_open {
            settings_gen += 1;
            self.settings_anim.set((self.settings_open, settings_gen));
        }
        let settings_open = self.settings_open;
        let fade = move |el: Div, key: &'static str| -> AnyElement {
            if !motion {
                return el.into_any_element();
            }
            if full_gen > 0 {
                return el
                    .with_animation(
                        SharedString::from(format!("{key}-{full_gen}")),
                        Animation::new(Duration::from_millis(200)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(0.35 + 0.65 * t),
                    )
                    .into_any_element();
            }
            if settings_gen > 0 && !settings_open {
                return el
                    .with_animation(
                        SharedString::from(format!("settings-ret-{settings_gen}")),
                        Animation::new(Duration::from_millis(160)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(0.35 + 0.65 * t),
                    )
                    .into_any_element();
            }
            el.into_any_element()
        };
        let center: AnyElement = if self.settings_open {
            let box_el = div().flex_1().min_w_0().h_full().overflow_hidden().child(self.render_settings(window, cx));
            if settings_gen > 0 && motion {
                box_el
                    .with_animation(
                        SharedString::from(format!("settings-open-{settings_gen}")),
                        Animation::new(Duration::from_millis(170)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(0.2 + 0.8 * t).mt(px((1. - t) * 8.)),
                    )
                    .into_any_element()
            } else {
                box_el.into_any_element()
            }
        } else if full {
            let agent = fade(div().flex_1().min_w_0().h_full().overflow_hidden().child(self.render_agent_full(window, cx)), "full-in");
            if side_visible {
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(side_column(self, window, cx))
                    .when(self.sidebar_open, |h| h.child(handle("sidebar-resize", 1, self, cx)))
                    .child(agent)
                    .into_any_element()
            } else {
                agent
            }
        } else if side_visible {
            h_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(side_column(self, window, cx))
                .when(self.sidebar_open, |h| h.child(handle("sidebar-resize", 1, self, cx)))
                .child(fade(div().flex_1().min_w_0().h_full().overflow_hidden().child(editor_and_panel), "editor-in"))
                .into_any_element()
        } else {
            fade(div().flex_1().min_w_0().h_full().overflow_hidden().child(editor_and_panel), "editor-in")
        };
        let right_on = self.right_dock_on() && !full && !self.settings_open;
        let right_close = (!right_on && !full && !self.settings_open && motion).then_some(self.right_closing).flatten();
        let (was_right, mut right_gen) = self.right_anim.get();
        if was_right != right_on {
            if right_on {
                right_gen += 1;
            }
            self.right_anim.set((right_on, right_gen));
        }
        let center: AnyElement = if !right_on && right_close.is_none() {
            center
        } else {
            let dock = div().w(px(agent_w)).flex_none().h_full().overflow_hidden().child(self.render_right_dock(window, cx));
            let dock: AnyElement = if let Some(seq) = right_close {
                dock.with_animation(
                    SharedString::from(format!("right-out-{seq}")),
                    Animation::new(Duration::from_millis(PANE_ANIM_MS)).with_easing(gpui_kit::ease_out_quint()),
                    move |d, t| d.w(px((agent_w * (1. - t)).max(0.))).opacity(1. - t),
                )
                .into_any_element()
            } else if right_gen > 0 && motion {
                dock.with_animation(
                    SharedString::from(format!("right-in-{right_gen}")),
                    Animation::new(Duration::from_millis(PANE_ANIM_MS)).with_easing(gpui_kit::ease_out_quint()),
                    move |d, t| d.w(px((agent_w * t).max(1.))).opacity(0.35 + 0.65 * t),
                )
                .into_any_element()
            } else {
                dock.into_any_element()
            };
            h_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(div().flex_1().min_w_0().h_full().overflow_hidden().child(center))
                .when(right_on, |h| h.child(handle("agent-resize", 2, self, cx)))
                .child(dock)
                .into_any_element()
        };
        let main = h_flex().flex_1().min_h_0().child(self.render_activity_bar(cx)).child(center);
        let backdrop = self.settings.background().map(|p| {
            img(p.to_path_buf()).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)
        });
        v_flex()
            .id("workbench")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(
                gpui_kit::Hsla::from(rgb(theme::live(theme::BG_SIDE))).opacity(
                    if crate::scene::active() {
                        0.
                    } else if theme::glass() && self.settings.background().is_none() {
                        0.42
                    } else {
                        1.
                    },
                ),
            )
            .text_color(c(theme::FG))
            .children(backdrop)
            .on_action(cx.listener(Self::on_next_theme))
            .on_action(cx.listener(Self::on_tab_accept))
            .on_action(cx.listener(Self::on_term_history_prev))
            .on_action(cx.listener(Self::on_term_history_next))
            .on_action(cx.listener(Self::on_term_complete))
            .on_action(cx.listener(Self::on_term_interrupt))
            .on_action(cx.listener(Self::on_term_escape))
            .on_action(cx.listener(Self::on_go_to_problem))
            .on_action(cx.listener(Self::on_scm_show))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                let total = f32::from(window.viewport_size().width);
                let x = f32::from(ev.position.x);
                match this.resizing {
                    1 => this.sidebar_width = (x - f32::from(theme::u(40.))).clamp(160., (total * 0.45).max(200.)),
                    2 => this.agent_width = (total - x).clamp(260., (total * 0.6).max(300.)),
                    3 | 4 => {
                        let Some((x0, w0)) = this.agent.full.drag else { return };
                        let w = if this.resizing == 3 { w0 + (x - x0) } else { w0 - (x - x0) };
                        let w = w.clamp(crate::agent::full_bounds().0, crate::agent::full_bounds().1);
                        if this.resizing == 3 {
                            this.settings.full_left_w = w;
                        } else {
                            this.settings.full_right_w = w;
                        }
                    }
                    _ => return,
                }
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.resizing != 0 {
                    if this.resizing >= 3 {
                        this.agent.full.drag = None;
                        this.settings.save();
                    }
                    this.resizing = 0;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(Self::on_quick_open))
            .on_action(cx.listener(Self::on_quick_close))
            .on_action(cx.listener(Self::on_quick_up))
            .on_action(cx.listener(Self::on_quick_down))
            .on_action(cx.listener(Self::on_agent_use_opencode))
            .on_action(cx.listener(Self::on_agent_mention_up))
            .on_action(cx.listener(Self::on_agent_mention_down))
            .on_action(cx.listener(Self::on_agent_mention_accept))
            .on_action(cx.listener(Self::on_agent_mention_close))
            .on_action(cx.listener(Self::on_agent_force_send))
            .on_action(cx.listener(Self::on_agent_menu_up))
            .on_action(cx.listener(Self::on_agent_menu_down))
            .on_action(cx.listener(Self::on_agent_menu_pick))
            .on_action(cx.listener(Self::on_agent_menu_close))
            .on_action(cx.listener(Self::on_agent_insert_code))
            .on_action(cx.listener(Self::on_agent_show_options))
            .on_action(cx.listener(Self::on_toggle_agent))
            .on_action(cx.listener(|this, _: &AgentAccessRead, _, cx| this.agent_set_access("read", cx)))
            .on_action(cx.listener(|this, _: &AgentAccessWrite, _, cx| this.agent_set_access("write", cx)))
            .on_action(cx.listener(|this, _: &AgentAccessFull, _, cx| this.agent_set_access("full", cx)))
            .on_action(cx.listener(Self::on_scm_commit_push))
            .on_action(cx.listener(Self::on_scm_commit_sync))
            .on_action(cx.listener(Self::on_scm_commit_amend))
            .on_action(cx.listener(Self::on_scm_pull))
            .on_action(cx.listener(Self::on_scm_push))
            .on_action(cx.listener(Self::on_scm_sync))
            .on_action(cx.listener(Self::on_scm_fetch))
            .on_action(cx.listener(Self::on_scm_stage_all))
            .on_action(cx.listener(Self::on_scm_unstage_all))
            .on_action(cx.listener(Self::on_scm_create_pr))
            .on_action(cx.listener(Self::on_scm_open_github))
            .on_action(cx.listener(Self::on_scm_remove_git))
            .on_action(cx.listener(|this, _: &CheckUpdates, _, cx| this.check_updates_now(cx)))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_save_all))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_open_folder))
            .on_action(cx.listener(Self::on_add_folder_to_workspace))
            .on_action(cx.listener(Self::on_open_folder_in_wsl))
            .on_action(cx.listener(Self::on_open_file))
            .on_action(cx.listener(Self::on_quit))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .on_action(cx.listener(Self::on_show_explorer))
            .on_action(cx.listener(Self::on_find_in_files))
            .on_action(cx.listener(Self::on_show_run))
            .on_action(cx.listener(Self::on_show_extensions))
            .on_action(cx.listener(Self::on_show_agent))
            .on_action(cx.listener(Self::on_show_settings))
            .on_action(cx.listener(Self::on_about))
            .on_action(cx.listener(Self::on_toggle_panel))
            .on_action(cx.listener(Self::on_toggle_terminal))
            .on_action(cx.listener(Self::on_new_terminal))
            .on_action(cx.listener(Self::on_show_output))
            .on_action(cx.listener(Self::on_show_problems))
            .on_action(cx.listener(Self::on_run_file))
            .on_action(cx.listener(Self::on_stop_run))
            .on_action(cx.listener(Self::on_explorer_new_file))
            .on_action(cx.listener(Self::on_explorer_new_folder))
            .on_action(cx.listener(Self::on_explorer_rename))
            .on_action(cx.listener(Self::on_explorer_delete))
            .on_action(cx.listener(Self::on_explorer_refresh))
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_copy_relative_path))
            .on_action(cx.listener(Self::on_cancel_pending))
            .on_action(cx.listener(Self::on_reveal))
            .on_action(cx.listener(Self::on_toggle_preview))
            .on_action(cx.listener(Self::on_reload_preview))
            .on_action(cx.listener(Self::on_remove_folder))
            .on_action(cx.listener(Self::on_toggle_line_comment))
            .on_action(cx.listener(Self::on_move_line_up))
            .on_action(cx.listener(Self::on_move_line_down))
            .on_action(cx.listener(Self::on_copy_line_up))
            .on_action(cx.listener(Self::on_copy_line_down))
            .on_action(cx.listener(Self::on_delete_line))
            .on_action(cx.listener(Self::on_insert_line_below))
            .on_action(cx.listener(Self::on_insert_line_above))
            .on_action(cx.listener(Self::on_go_to_line))
            .on_action(cx.listener(Self::on_command_palette))
            .on_action(cx.listener(Self::on_reopen_closed_tab))
            .on_action(cx.listener(Self::on_close_all_tabs))
            .on_action(cx.listener(Self::on_toggle_word_wrap))
            .on_action(cx.listener(Self::on_zoom_in))
            .on_action(cx.listener(Self::on_zoom_out))
            .on_action(cx.listener(Self::on_zoom_reset))
            .on_action(cx.listener(|this, _: &ShowWhatsNew, _, cx| {
                this.settings_open = false;
                this.wiztree.page = false;
                this.whats_new = Some(crate::whatsnew::Page::all());
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowWizTree, w, cx| {
                this.toggle_wiztree(w, cx);
            }))
            .on_action(cx.listener(|this, _: &NavBack, w, cx| {
                this.workbench_nav_back(w, cx);
            }))
            .on_action(cx.listener(|this, _: &NavForward, w, cx| {
                this.workbench_nav_forward(w, cx);
            }))
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, w, cx| {
                    cx.stop_propagation();
                    this.workbench_nav_back(w, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, w, cx| {
                    cx.stop_propagation();
                    this.workbench_nav_forward(w, cx);
                }),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, w, cx| this.on_drop_paths(paths, w, cx)))
            .child(self.render_title_bar(window, cx))
            .child(main)
            .child(self.render_status_bar(window, cx))
            .when(self.quick.open, |v| v.child(self.render_quick_open(cx)))
            .children(self.render_drop_ask(cx))
            .children(self.render_preview_ask(cx))
            .children(self.render_update(cx))
            .children(self.render_agent_updates(window, cx))
            .children(self.render_theme_menu(cx))
            .children(self.render_agent_settings(window, cx))
            .children(self.render_lightbox(window, cx))
            .child(selection_guard())
    }
}

fn selection_guard() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, window, _| {
            window.on_mouse_event(|ev: &MouseDownEvent, phase, window, cx| {
                if phase.bubble() && ev.button == MouseButton::Left && !gpui_kit::base::TextSelection::is_selectable_at(ev.position, window, cx) {
                    gpui_kit::component::GlobalState::suppress_text_selection(cx);
                }
            });
        },
    )
    .absolute()
    .size(px(0.))
}

struct BuiltDocEditor {
    editor: Entity<EditorState>,
    sub: Subscription,
    diff: Option<crate::diff_view::DiffDoc>,
    flux: Option<crate::fluxdoc::view::FluxView>,
    diag_ranges: gpui_kit::component::input::RangeDecorationCollection,
    diag_ruler: gpui_kit::component::input::OverviewRulerDecorationCollection,
}

struct PreparedFile {
    text: String,
    crlf: bool,
    lang: &'static str,
    label: &'static str,
    banner: Option<String>,
    read_only: bool,
    nbt_format: Option<crate::nbt::NbtFormat>,
    large: bool,
    complex: bool,
    long_lines: bool,
    file_text_for_flux: String,
    drawable: Option<bool>,
    flux_lazy: bool,
    is_diff: bool,
}

fn prepare_file(path: &Path, bytes: Vec<u8>) -> PreparedFile {
    let nbt = crate::nbt::decode_file(path, &bytes);
    let view = if nbt.is_none() { crate::viewer::view(path, &bytes) } else { None };
    let read_only = view.is_some();
    let nbt_format = nbt.as_ref().map(|d| d.format);
    let (text, crlf, mut lang, mut label, banner) = match (nbt, view) {
        (Some(d), _) => (d.snbt, false, "javascript", d.label, Some(d.banner)),
        (None, Some(v)) => (v.text, false, v.lang, v.label, Some(v.banner)),
        (None, None) => {
            let file = core::decode_text(&bytes);
            let first = file.text.lines().next().unwrap_or("");
            let (lang, label) = files::language_for(path, first);
            (file.text, file.crlf, lang, label, None)
        }
    };
    let stats = crate::accel::scan_file_stats(text.as_bytes(), 300);
    let large = text.len() > LARGE_FILE_BYTES;
    let very_complex = !read_only && nbt_format.is_none() && stats.max_line > MAX_COMPLEX_LINE_BYTES;
    let complex = !read_only && (large || very_complex);
    if complex {
        lang = "text";
        label = "Plain Text";
    }
    let drawable = if read_only || nbt_format.is_some() { None } else { crate::fluxdoc::view::drawable_kind(path) };
    let is_markdown = drawable == Some(true);
    let flux_lazy = drawable.is_some() && text.len() > LARGE_FILE_BYTES;
    let (file_text_for_flux, text) = match (drawable.is_some(), flux_lazy) {
        (true, true) => (if is_markdown { crate::fluxdoc::gfm::to_flux(&text) } else { text }, String::new()),
        (true, false) if is_markdown => (crate::fluxdoc::gfm::to_flux(&text), text),
        (true, false) => (text.clone(), text),
        _ => (String::new(), text),
    };
    let is_diff = crate::diff_view::is_diff(lang) && !complex;
    let long_lines = !read_only && text.len() <= MAX_AUTO_WRAP_BYTES && stats.dominated_by_long_lines();
    PreparedFile {
        text,
        crlf,
        lang,
        label,
        banner,
        read_only,
        nbt_format,
        large,
        complex,
        long_lines,
        file_text_for_flux,
        drawable,
        flux_lazy,
        is_diff,
    }
}

fn read_off_thread(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.len() > READ_OFF_THREAD_BYTES).unwrap_or(false)
}

pub(crate) fn remote_dir_pub(target: &crate::remote::WslTarget, path: &std::path::Path) -> Option<Vec<crate::files::Node>> {
    remote_dir(target, path)
}

fn remote_dir(target: &crate::remote::WslTarget, path: &std::path::Path) -> Option<Vec<crate::files::Node>> {
    let distro = target.distro.clone();
    let linux = target.linux_path(path)?;
    let entries = crate::remote::with_client(&distro, |c| c.list(&linux).ok())??;
    let entries = entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
    Some(crate::files::nodes_from_remote(path, entries))
}

fn remote_listing(target: &crate::remote::WslTarget, root: &std::path::Path, multi: bool) -> Option<Vec<crate::files::Node>> {
    if !multi {
        return remote_dir(target, root);
    }
    None
}

fn remote_read(target: &crate::remote::WslTarget, path: &std::path::Path, limit: u64) -> Option<Vec<u8>> {
    let distro = target.distro.clone();
    let linux = target.linux_path(path)?;
    crate::remote::with_client(&distro, |c| c.read(&linux, limit).ok()).flatten()
}

fn remote_write(target: &crate::remote::WslTarget, path: &std::path::Path, bytes: &[u8], crlf: bool) -> Option<()> {
    let distro = target.distro.clone();
    let linux = target.linux_path(path)?;
    crate::remote::with_client(&distro, |c| c.write(&linux, bytes, crlf).ok()).flatten()
}

fn remote_is_file(target: &crate::remote::WslTarget, path: &std::path::Path) -> Option<bool> {
    let distro = target.distro.clone();
    let linux = target.linux_path(path)?;
    crate::remote::with_client(&distro, |c| c.stat(&linux).ok()).flatten().map(|s| s.is_file)
}

fn remote_scan(target: &crate::remote::WslTarget, root: &std::path::Path, max: usize) -> Option<Vec<String>> {
    let distro = target.distro.clone();
    let linux = target.linux_path(root)?;
    let paths = crate::remote::with_client(&distro, |c| c.scan(&linux, max).ok()).flatten()?;
    Some(paths.into_iter().map(|p| target.unc_path(&p).to_string_lossy().into_owned()).collect())
}

fn remote_is_dir(target: &crate::remote::WslTarget, path: &std::path::Path) -> Option<bool> {
    let distro = target.distro.clone();
    let linux = target.linux_path(path)?;
    crate::remote::with_client(&distro, |c| c.stat(&linux).ok()).flatten().map(|s| s.is_dir)
}

static TAB_SCROLL_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

const TAB_SCROLL_MS: f32 = 170.;

fn step_tab_scroll(
    gen: u64,
    weak: WeakEntity<Workbench>,
    ix: usize,
    start: f32,
    started: std::time::Instant,
    window: &mut Window,
    _cx: &mut App,
) {
    window.on_next_frame(move |window, cx| {
        if TAB_SCROLL_GEN.load(std::sync::atomic::Ordering::Relaxed) != gen {
            return;
        }
        let elapsed_ms = started.elapsed().as_secs_f32() * 1000.;
        let t = (elapsed_ms / TAB_SCROLL_MS).clamp(0., 1.);
        let ease = 1. - (1. - t).powi(3);
        let mut keep_going = false;
        _ = weak.update(cx, |this, cx| {
            if this.active != Some(ix) {
                return;
            }
            let Some(target) = this.tab_scroll_target_for(ix, start) else { return };
            let x = if t >= 1. || (target - start).abs() < 0.75 {
                target
            } else {
                keep_going = true;
                start + (target - start) * ease
            };
            this.tabs_scroll.set_offset(point(px(-x), px(0.)));
            this.tabs_scroll_x.set(x);
            cx.notify();
        });
        if keep_going {
            step_tab_scroll(gen, weak, ix, start, started, window, cx);
        }
    });
}

pub(crate) fn tab_scroll_target(widths: &[f32], ix: usize, cur: f32, viewport_w: f32, scale: f32) -> f32 {
    if ix >= widths.len() {
        return cur.max(0.);
    }
    let total_w: f32 = widths.iter().sum();
    let max_x = (total_w - viewport_w).max(0.);
    if max_x <= 0.5 {
        return 0.;
    }
    let left: f32 = widths[..ix].iter().sum();
    let right = left + widths[ix];
    let margin = (32. * scale).min(viewport_w * 0.2);
    let cur = cur.clamp(0., max_x);
    if left < cur + margin {
        (left - margin).clamp(0., max_x)
    } else if right > cur + viewport_w - margin {
        (right + margin - viewport_w).clamp(0., max_x)
    } else {
        cur
    }
}

pub(crate) fn tab_width(name: &str, has_diag: bool, scale: f32) -> f32 {
    let chars = name.chars().count().clamp(4, 28) as f32;
    let badge = if has_diag { 28. } else { 0. };
    ((64. + chars * 7.6 + badge) * scale).clamp(96. * scale, 240. * scale).round()
}

pub(crate) fn visible_tab_window(widths: &[f32], scroll_x: f32, viewport_w: f32) -> (std::ops::Range<usize>, f32, f32) {
    if widths.is_empty() {
        return (0..0, 0., 0.);
    }
    const OVERSCAN_PX: f32 = 160.;
    let vis_start = (scroll_x - OVERSCAN_PX).max(0.);
    let vis_end = scroll_x + viewport_w.max(1.) + OVERSCAN_PX;

    let mut x = 0.;
    let mut start = widths.len();
    let mut left_w = 0.;
    let mut end = widths.len();
    let mut end_x = 0.;
    for (i, &w) in widths.iter().enumerate() {
        let next_x = x + w;
        if start == widths.len() && next_x > vis_start {
            start = i;
            left_w = x;
        }
        if start != widths.len() && x >= vis_end {
            end = i;
            end_x = x;
            break;
        }
        x = next_x;
    }
    if start == widths.len() {
        let last = widths.len() - 1;
        let pre: f32 = widths[..last].iter().sum();
        return (last..widths.len(), pre, 0.);
    }
    let total: f32 = widths.iter().sum();
    let right_w = if end < widths.len() { (total - end_x).max(0.) } else { 0. };
    (start..end, left_w, right_w)
}

pub(crate) fn select_eviction_indices(entries: &[(usize, bool, u64)], max_warm: usize) -> Vec<usize> {
    if entries.len() <= max_warm {
        return Vec::new();
    }
    let excess = entries.len() - max_warm;
    let mut candidates: Vec<(usize, u64)> = entries
        .iter()
        .filter(|(_, pinned, _)| !*pinned)
        .map(|&(ix, _, used)| (ix, used))
        .collect();
    candidates.sort_by_key(|&(_, used)| used);
    candidates.into_iter().take(excess).map(|(ix, _)| ix).collect()
}

fn read_head(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() <= core::MAX_FILE_BYTES {
        return std::fs::read(path).map_err(|e| e.to_string());
    }
    let mut buf = vec![0; 64 * 1024];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)).map_err(|e| e.to_string())?;
    buf.truncate(n);
    if !buf.contains(&0) {
        return Err(crate::trf!("{0} is too large to open ({1} MB)."; 0 => path.display(), 1 => meta.len() / 1024 / 1024));
    }
    Ok(buf)
}

impl Workbench {
    pub(crate) fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |s, cx| s.set_placeholder(tr("Search"), window, cx));
        self.settings_search.update(cx, |s, cx| s.set_placeholder(tr("Search settings…"), window, cx));
        self.github_input.update(cx, |s, cx| s.set_placeholder(tr("Personal access token"), window, cx));
        self.quick.input.update(cx, |s, cx| s.set_placeholder(tr("Type part of a file name"), window, cx));
        self.scm.message.update(cx, |s, cx| s.set_placeholder(tr("Commit message"), window, cx));
        let name = crate::agent::agent_name();
        self.agent.input.update(cx, |s, cx| s.set_placeholder(crate::trf!("Ask {name}…"; name => name), window, cx));
        cx.notify();
    }
}

#[derive(Clone)]
pub(crate) enum ScrollTarget {
    Editor(Entity<EditorState>),
    #[allow(dead_code)]
    Handle(u64, ScrollHandle),
}

impl ScrollTarget {
    fn key(&self) -> u64 {
        match self {
            ScrollTarget::Editor(e) => e.entity_id().as_u64(),
            ScrollTarget::Handle(k, _) => *k,
        }
    }

    fn offset(&self, cx: &App) -> Point<Pixels> {
        match self {
            ScrollTarget::Editor(e) => e.read(cx).scroll_offset(),
            ScrollTarget::Handle(_, h) => h.offset(),
        }
    }

    fn line_height(&self, cx: &App) -> Pixels {
        match self {
            ScrollTarget::Editor(e) => e.read(cx).line_height().unwrap_or(px(20.)),
            ScrollTarget::Handle(..) => px(22.),
        }
    }

    fn clamp(&self, p: Point<Pixels>) -> Point<Pixels> {
        match self {
            ScrollTarget::Editor(_) => point(p.x.min(px(0.)), p.y.min(px(0.))),
            ScrollTarget::Handle(_, h) => {
                let m = h.max_offset();
                point(p.x.clamp(-m.x, px(0.)), p.y.clamp(-m.y, px(0.)))
            }
        }
    }

    fn set(&self, p: Point<Pixels>, cx: &mut App) {
        match self {
            ScrollTarget::Editor(e) => e.update(cx, |e, cx| e.set_scroll_offset(p, cx)),
            ScrollTarget::Handle(_, h) => h.set_offset(p),
        }
    }
}

pub(crate) struct SmoothScroll {
    target: ScrollTarget,
    to: Point<Pixels>,
    prev: Option<Point<Pixels>>,
    stuck: u8,
    last: std::time::Instant,
    bottom: bool,
}

impl Workbench {
    pub(crate) fn smooth_scroll_layer(&self, target: ScrollTarget, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                window.on_mouse_event(move |ev: &ScrollEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture || ev.delta.precise() || ev.modifiers.control || !hitbox.is_hovered(window) {
                        return;
                    }
                    cx.stop_propagation();
                    let (t, delta) = (target.clone(), ev.delta.clone());
                    _ = weak.update(cx, |this, cx| this.smooth_scroll(t, delta, window, cx));
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    fn smooth_scroll(&mut self, target: ScrollTarget, delta: ScrollDelta, window: &mut Window, cx: &mut Context<Self>) {
        let speed = self.settings.scroll_speed();
        let d = delta.pixel_delta(target.line_height(cx));
        let d = point(d.x * speed, d.y * speed);
        let key = target.key();
        let running = self.smooth.as_ref().is_some_and(|s| s.target.key() == key);
        let base = match self.smooth.as_ref().filter(|s| s.target.key() == key) {
            Some(s) => s.to,
            None => target.offset(cx),
        };
        let to = target.clamp(point(base.x + d.x, base.y + d.y));
        if cx.reduce_motion() {
            self.smooth = None;
            target.set(to, cx);
            cx.notify();
            return;
        }
        let bottom = match &target {
            ScrollTarget::Handle(_, h) => d.y < px(0.) && to.y <= -h.max_offset().y + px(1.),
            ScrollTarget::Editor(_) => false,
        };
        match self.smooth.as_mut().filter(|s| s.target.key() == key) {
            Some(s) => {
                s.to = to;
                s.stuck = 0;
                s.bottom = bottom;
            }
            None => self.smooth = Some(SmoothScroll { target, to, prev: None, stuck: 0, last: std::time::Instant::now(), bottom }),
        }
        if key == crate::agent::CHAT_SCROLL_KEY {
            self.agent.follow = bottom;
        }
        if !running {
            self.smooth_next_frame(window, cx);
        }
    }

    fn smooth_next_frame(&self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            _ = weak.update(cx, |this, cx| this.smooth_step(window, cx));
        });
    }

    #[allow(dead_code)]
    pub(crate) fn smooth_running(&self, key: u64) -> bool {
        self.smooth.as_ref().is_some_and(|s| s.target.key() == key)
    }

    fn smooth_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.smooth.as_mut() else { return };
        let target = s.target.clone();
        if let (true, ScrollTarget::Handle(_, h)) = (s.bottom, &target) {
            s.to.y = -h.max_offset().y;
        }
        let cur = target.offset(cx);
        let now = std::time::Instant::now();
        let dt = (now - s.last).as_secs_f32().min(0.05);
        s.last = now;
        let diff = point(s.to.x - cur.x, s.to.y - cur.y);
        if diff.x.abs() < px(0.5) && diff.y.abs() < px(0.5) {
            let to = s.to;
            self.smooth = None;
            target.set(to, cx);
            cx.notify();
            return;
        }
        if s.prev == Some(cur) {
            s.stuck += 1;
            if s.stuck > 6 {
                self.smooth = None;
                return;
            }
        } else {
            s.stuck = 0;
        }
        s.prev = Some(cur);
        let k = 1.0 - (-dt / 0.045).exp();
        target.set(point(cur.x + diff.x * k, cur.y + diff.y * k), cx);
        cx.notify();
        self.smooth_next_frame(window, cx);
    }
}

const AGENT_HINT_LAUNCHES: u8 = 3;
const AGENT_HINT_DONE: u8 = 100;

impl Workbench {
    fn render_agent_hint(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let open = self.view_shown(SideView::Agent);
        let teach = self.settings.agent_hint <= AGENT_HINT_LAUNCHES && !open;
        let chip = h_flex()
            .id("sb-agent")
            .flex_none()
            .whitespace_nowrap()
            .h_full()
            .px_2()
            .gap_1p5()
            .cursor_pointer()
            .when(open, |h| h.text_color(c(theme::FG)))
            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Open or close the AI agent (Ctrl+L)")).build(w, cx))
            .on_click(cx.listener(|this, _, w, cx| this.on_toggle_agent(&ToggleAgent, w, cx)))
            .child(
                Icon::new(gpui_kit::assets::IconName::MessageSquareCode)
                    .flex_none()
                    .size(theme::u(13.)),
            )
            .child(tr("Agent"))
            .when(!compact, |h| {
                h.child(
                    div()
                        .px_1()
                        .rounded(px(3.))
                        .border_1()
                        .border_color(c(theme::BORDER_STRONG))
                        .text_color(c(theme::FG_DIM))
                        .child("Ctrl+L"),
                )
            });
        if !teach || cx.reduce_motion() {
            return chip.into_any_element();
        }
        let accent = c(theme::ACCENT);
        chip.with_animation(
            "sb-agent-hint",
            Animation::new(std::time::Duration::from_millis(1600)).with_easing(ease_in_out),
            move |el, t| {
                let a = (t * std::f32::consts::PI).sin() * 0.35;
                el.bg(accent.opacity(a))
            },
        )
        .into_any_element()
    }

    fn render_copilot_hint(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::copilot::Status;
        let installing = self.copilot_installing;
        let label = match self.copilot.status() {
            _ if installing => tr("Copilot: installing…"),
            Status::NotInstalled if self.settings.copilot => tr("Copilot: install server"),
            Status::NotSignedIn(_) if self.settings.copilot => tr("Copilot: sign in"),
            _ => return div().into_any_element(),
        };
        h_flex()
            .id("sb-copilot")
            .h_full()
            .px_2()
            .gap_1p5()
            .cursor_pointer()
            .text_color(c(theme::FG_DIM))
            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Copilot suggestions are not set up yet. Click to open Settings.")).build(w, cx))
            .on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(ShowSettings), cx)))
            .child(if installing {
                gpui_kit::component::spinner::Spinner::new().icon(gpui_kit::assets::IconName::LoaderCircle).xsmall().into_any_element()
            } else {
                Icon::new(gpui_kit::assets::IconName::Bot).size(theme::u(13.)).into_any_element()
            })
            .child(label)
            .into_any_element()
    }
}

#[cfg(test)]
mod fit_tests {
    use super::{
        fit_columns, push_nav_stack, select_eviction_indices, tab_scroll_target, tab_width,
        visible_tab_window, NavTarget,
    };
    use std::path::PathBuf;

    #[test]
    fn columns_always_fit() {
        assert_eq!(fit_columns(1900., Some(260.), Some(380.)), (260., 380.));
        let (s, a) = fit_columns(1000., Some(300.), Some(380.));
        assert_eq!(a, 380.);
        assert!(s >= 170. && 48. + s + a + 320. <= 1000.01, "{s} {a}");
        for w in [500., 640., 800.] {
            let (s, a) = fit_columns(w, Some(260.), Some(380.));
            assert!(48. + s + a <= w, "{w}: {s} {a}");
        }
        assert_eq!(fit_columns(1200., None, None), (0., 0.));
    }

    #[test]
    fn virtual_tabs_render_only_visible_window_for_500_tabs() {
        let widths: Vec<f32> = (0..500).map(|i| tab_width(&format!("file_{i}.json"), false, 1.0)).collect();
        let total: f32 = widths.iter().sum();
        let (range, left, right) = visible_tab_window(&widths, 12_000., 800.);
        assert!(range.len() < 16, "expected <16 rendered tabs out of 500, got {}", range.len());
        assert!(left > 11_000. && right > 10_000.);
        let rendered_w: f32 = widths[range].iter().sum();
        assert!((left + rendered_w + right - total).abs() < 0.5);
    }

    #[test]
    fn cold_tab_eviction_keeps_active_dirty_and_most_recent() {
        // (index, pinned, last_used)
        let entries = vec![
            (0, false, 1),
            (1, true, 2),  // dirty or active -> never evicted
            (2, false, 3),
            (3, false, 9),
            (4, false, 4),
            (5, false, 10),
        ];
        let evicted = select_eviction_indices(&entries, 4);
        assert_eq!(evicted, vec![0, 2]);
    }

    #[test]
    fn tab_scroll_target_brings_offscreen_tabs_into_view_with_margin() {
        let widths = vec![140.; 10]; // total 1400
        let vw = 500.;
        // Opening the last tab from scroll 0 must scroll all the way to max_x (900).
        assert_eq!(tab_scroll_target(&widths, 9, 0., vw, 1.0), 900.);
        // Switching back to tab 0 from scroll 900 must return to 0.
        assert_eq!(tab_scroll_target(&widths, 0, 900., vw, 1.0), 0.);
        // Switching to a tab off the right edge leaves breathing margin.
        let mid_right = tab_scroll_target(&widths, 5, 0., vw, 1.0); // right = 840
        assert!((mid_right - (840. + 32. - 500.)).abs() < 0.5, "{mid_right}");
        // Already well inside viewport stays put.
        assert_eq!(tab_scroll_target(&widths, 2, 100., vw, 1.0), 100.);
    }

    #[test]
    fn nav_history_stack_deduplicates_and_caps_entries() {
        let mut stack = Vec::new();
        push_nav_stack(&mut stack, NavTarget::WizTree);
        push_nav_stack(&mut stack, NavTarget::WizTree);
        assert_eq!(stack, vec![NavTarget::WizTree]);
        for i in 0..70 {
            push_nav_stack(&mut stack, NavTarget::File(PathBuf::from(format!("f{i}.rs"))));
        }
        assert_eq!(stack.len(), 60);
        assert_eq!(stack.last(), Some(&NavTarget::File(PathBuf::from("f69.rs"))));
    }
}

fn apply_glass(window: &mut Window) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static APPLIED: AtomicU8 = AtomicU8::new(0);
    let want = if theme::glass() && !crate::scene::active() && !theme::has_backdrop() { 2 } else { 1 };
    if APPLIED.swap(want, Ordering::Relaxed) != want {
        window.set_background_appearance(if want == 2 { WindowBackgroundAppearance::Blurred } else { WindowBackgroundAppearance::Opaque });
    }
}
