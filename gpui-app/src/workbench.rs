//! The workbench: title bar + menus, activity bar, side bar (explorer, search,
//! run, OpenCode), editor tabs, bottom panel (terminal, output) and status
//! bar — the same layout as the Tauri FluxCode.

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
    menu::{DropdownMenu, PopupMenu},
    resizable::{h_resizable, resizable_panel, v_resizable},
    tree::TreeState,
    v_flex, Icon, IconName, Sizable, TitleBar,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Files above this size open without syntax highlighting.
const LARGE_FILE_BYTES: usize = 2 * 1024 * 1024;

actions!(
    fluxcode,
    [
        Save, SaveAll, CloseTab, OpenFolder, OpenFile, Quit, ToggleSidebar, NextTab, PrevTab,
        ShowExplorer, FindInFiles, ShowRun, ShowAgent, TogglePanel, ToggleTerminal, ShowOutput,
        RunFile, StopRun, NewTerminalCommand, ExplorerNewFile, ExplorerNewFolder, ExplorerRename,
        ExplorerDelete, ExplorerRefresh, CopyPath, RevealInExplorer, ShowSettings, About, TogglePreview, ReloadPreview,
        NextTheme, TabAccept, TermHistoryPrev, TermHistoryNext, TermComplete, TermInterrupt, TermEscape,
        GoToProblem, ScmShow, ScmCommitPush, ScmCommitSync, ScmCommitAmend, ScmPull, ScmPush, ScmSync, ScmFetch,
        ScmStageAll, ScmUnstageAll, ScmCreatePr, ScmOpenGithub, QuickOpenFile, QuickClose, QuickUp, QuickDown,
        AgentUseOpenCode, AgentUseMura, AgentMentionUp, AgentMentionDown, AgentMentionAccept, AgentMentionClose,
        AgentInsertCode, RemoveFolder, AgentShowOptions, ToggleAgent, AgentAccessRead, AgentAccessWrite, AgentAccessFull
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
        KeyBinding::new("ctrl-shift-a", ShowAgent, None),
        KeyBinding::new("ctrl-l", ToggleAgent, None),
        KeyBinding::new("ctrl-shift-g", ScmShow, None),
        KeyBinding::new("ctrl-p", QuickOpenFile, None),
        KeyBinding::new("up", QuickUp, Some("QuickOpen > Input")),
        KeyBinding::new("down", QuickDown, Some("QuickOpen > Input")),
        KeyBinding::new("escape", QuickClose, Some("QuickOpen > Input")),
        // Agent input: the @file list.
        KeyBinding::new("up", AgentMentionUp, Some("AgentInput > Input")),
        KeyBinding::new("down", AgentMentionDown, Some("AgentInput > Input")),
        KeyBinding::new("tab", AgentMentionAccept, Some("AgentInput > Input")),
        KeyBinding::new("escape", AgentMentionClose, Some("AgentInput > Input")),
        KeyBinding::new("ctrl-j", TogglePanel, None),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new("ctrl-shift-u", ShowOutput, None),
        KeyBinding::new("f5", RunFile, None),
        KeyBinding::new("ctrl-f5", RunFile, None),
        KeyBinding::new("shift-f5", StopRun, None),
        KeyBinding::new("ctrl-,", ShowSettings, None),
        KeyBinding::new("ctrl-k ctrl-t", NextTheme, None),
        KeyBinding::new("ctrl-k v", TogglePreview, None),
        KeyBinding::new("f2", ExplorerRename, Some("Explorer")),
        KeyBinding::new("delete", ExplorerDelete, Some("Explorer")),
        // In the code editor Tab accepts the highlighted suggestion; with no
        // suggestion menu open it indents (on_tab_accept falls back to IndentInline).
        KeyBinding::new("tab", TabAccept, Some("Input && mode == editor")),
        // Terminal prompt: shell-like keys (bound after gpui-kit's, so they win over the input's own).
        KeyBinding::new("up", TermHistoryPrev, Some("Terminal > Input")),
        KeyBinding::new("down", TermHistoryNext, Some("Terminal > Input")),
        KeyBinding::new("tab", TermComplete, Some("Terminal > Input")),
        KeyBinding::new("ctrl-c", TermInterrupt, Some("Terminal > Input")),
        KeyBinding::new("escape", TermEscape, Some("Terminal > Input")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideView {
    Explorer,
    Search,
    Run,
    Scm,
    Agent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelTab {
    Terminal,
    Output,
}

pub(crate) use crate::term::LineKind;

pub(crate) struct Line {
    pub text: SharedString,
    pub kind: LineKind,
    /// The process may still append to this line (no newline yet).
    pub open: bool,
    /// Came from stderr (or, on a status line: the command failed).
    pub err: bool,
    /// Second piece of text: the prompt's folder, a status line's time, a card's file:line.
    pub meta: SharedString,
    /// Click target: (path as printed, 1-based line).
    pub link: Option<(SharedString, u32)>,
    /// Command a card's fix button runs.
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
    /// Highlighter token ("python", "kt", "Dockerfile"); "image" for an image tab.
    #[allow(dead_code)]
    pub lang: &'static str,
    /// Status-bar name of the language or view ("Python", "Hex", "PDF").
    pub label: &'static str,
    /// A view of a file the editor can't hold as text (hex, PDF text, ...): never saved.
    pub read_only: bool,
    /// One line above the editor: what a view shows, or that the file is empty.
    pub banner: Option<SharedString>,
    pub crlf: bool,
    pub editor: Entity<EditorState>,
    pub dirty: bool,
    /// Bumped on every edit; auto save only fires for the latest one.
    pub gen: u64,
    pub _sub: Subscription,
    /// Set for an image tab (the editor stays empty and is never shown).
    pub image: Option<ImageDoc>,
}

pub(crate) struct ImageDoc {
    /// Pixel size from the file header (None for SVG and unreadable headers).
    pub size: Option<(u32, u32)>,
    pub bytes: u64,
    /// Fit the view (default) or show at 100% with scrolling.
    pub actual: bool,
}

/// What the explorer's inline name box is for.
#[derive(Clone)]
pub(crate) enum PendingOp {
    NewFile(PathBuf),
    NewFolder(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
}

pub struct Workbench {
    pub(crate) root: Option<PathBuf>,
    /// Folders added next to `root`, and folders dropped on the window.
    pub(crate) workspace: crate::workspace::Workspace,
    /// Focus of the "add or switch" question after a folder drop.
    pub(crate) drop_focus: FocusHandle,
    /// A mouse-wheel glide in progress (smooth scrolling of the code editor).
    pub(crate) smooth: Option<SmoothScroll>,
    /// The status text last shown; a new one clears itself after a few seconds.
    pub(crate) status_seen: SharedString,
    pub(crate) tree: Entity<TreeState>,
    /// Every folder in the tree (by path), including empty and skipped ones.
    pub(crate) dirs: Rc<std::collections::HashSet<String>>,
    pub(crate) docs: Vec<Doc>,
    pub(crate) active: Option<usize>,
    pub(crate) view: SideView,
    pub(crate) sidebar_open: bool,
    /// The OpenCode chat docked on the right is showing (settings.agent_right).
    pub(crate) agent_open: bool,
    /// Widths of the side bar and that dock (logical px), and which edge is being
    /// dragged: 0 none, 1 side bar, 2 dock.
    pub(crate) agent_width: f32,
    pub(crate) sidebar_width: f32,
    pub(crate) resizing: u8,
    pub(crate) focus: FocusHandle,
    pub(crate) status: SharedString,
    pub(crate) settings: crate::settings::Settings,
    pub(crate) settings_open: bool,
    // explorer
    pub(crate) ctx_target: Rc<RefCell<Option<PathBuf>>>,
    pub(crate) selected: Option<PathBuf>,
    pub(crate) pending: Option<PendingOp>,
    pub(crate) name_input: Entity<InputState>,
    // search
    pub(crate) search_input: Entity<InputState>,
    pub(crate) match_case: bool,
    pub(crate) whole_word: bool,
    pub(crate) search: core::SearchResult,
    pub(crate) searching: bool,
    pub(crate) search_rows: Vec<(usize, Option<usize>)>,
    // panel (terminal + output)
    pub(crate) procs: Procs,
    pub(crate) panel_open: bool,
    pub(crate) panel_tab: PanelTab,
    pub(crate) term_lines: Vec<Line>,
    pub(crate) term_cwd: PathBuf,
    pub(crate) term_input: Entity<InputState>,
    pub(crate) term_proc: Option<u32>,
    pub(crate) term_history: Vec<String>,
    pub(crate) term_scroll: UniformListScrollHandle,
    pub(crate) out_lines: Vec<Line>,
    pub(crate) out_proc: Option<u32>,
    pub(crate) out_scroll: UniformListScrollHandle,
    pub(crate) term_run: Option<crate::panel::RunInfo>,
    /// Wrapped rows of the terminal and output panels, and the panel's measured width.
    pub(crate) term_wrap: crate::panel::WrapCache,
    pub(crate) out_wrap: crate::panel::WrapCache,
    pub(crate) panel_width: Rc<std::cell::Cell<f32>>,
    /// Size of the image viewer area (measured last frame), to fit pictures exactly.
    pub(crate) image_box: Rc<std::cell::Cell<(f32, f32)>>,
    pub(crate) out_run: Option<crate::panel::RunInfo>,
    /// Position while walking the history with Up/Down.
    pub(crate) term_hist_ix: Option<usize>,
    /// Python errors from the last runs, underlined in the editor.
    pub(crate) problems: Vec<crate::panel::Problem>,
    /// "Python 3.15.0" for the terminal header (detected in the background).
    pub(crate) python_version: Option<SharedString>,
    pub(crate) agent: crate::agent::AgentState,
    pub(crate) scm: crate::scm::Scm,
    pub(crate) quick: crate::quick::QuickOpen,
    /// Every file of the open folder (from the explorer's scan), for Go to File.
    pub(crate) all_files: Rc<Vec<String>>,
    /// The same paths relative to the folder and lower-cased, for fast matching.
    pub(crate) all_lower: Rc<Vec<String>>,
    pub(crate) preview_server: Option<crate::preview::PreviewServer>,
    pub(crate) github: Option<crate::github::Account>,
    pub(crate) github_input: Entity<InputState>,
    #[cfg(feature = "embedded-preview")]
    pub(crate) preview: Option<(Entity<gpui_webview::WebView>, String)>,
    #[cfg(not(feature = "embedded-preview"))]
    pub(crate) preview: Option<((), String)>,
    _subs: Vec<Subscription>,
}

impl Workbench {
    pub fn new(root: Option<PathBuf>, open: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);

        let name_input = cx.new(|cx| InputState::new(window, cx));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Search")));
        let term_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Type a command (Tab completes, ↑ for history)")));
        let github_input = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(tr("Personal access token")));
        let mut subs = vec![
            cx.subscribe_in(&name_input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.commit_pending(window, cx);
                }
            }),
            cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.run_search(cx);
                }
            }),
            cx.subscribe_in(&term_input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.terminal_enter(window, cx);
                }
            }),
        ];

        let (procs, rx) = Procs::new();
        // Drain process output on the UI thread.
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
            this.procs.kill_all();
            async {}
        })
        .detach();

        let settings = crate::settings::Settings::load();
        crate::i18n::set(&settings.language);
        crate::agent::set_choice(&settings.agent_cli);
        let term_cwd = root.clone().or_else(|| std::env::current_dir().ok()).unwrap_or_default();
        let agent = crate::agent::AgentState::new(window, cx);
        let (scm, scm_sub) = crate::scm::Scm::new(window, cx);
        subs.push(scm_sub);
        let (quick, quick_sub) = crate::quick::QuickOpen::new(window, cx);
        subs.push(quick_sub);
        // Like VS Code: re-read git state when the window comes back to the front.
        subs.push(cx.observe_window_activation(window, |this, window, cx| {
            // Alt-Tab back and forth should not start a git process each time.
            let recent = this.scm.last_refresh.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(2));
            if window.is_window_active() && !recent {
                this.scm_refresh(cx);
            }
        }));
        let mut this = Self {
            root: None,
            workspace: Default::default(),
            drop_focus: cx.focus_handle(),
            smooth: None,
            status_seen: SharedString::default(),
            tree,
            dirs: Rc::default(),
            docs: Vec::new(),
            active: None,
            view: SideView::Explorer,
            sidebar_open: true,
            agent_open: false,
            agent_width: 380.,
            sidebar_width: 260.,
            resizing: 0,
            focus,
            status: SharedString::default(),
            settings,
            settings_open: false,
            ctx_target: Rc::new(RefCell::new(None)),
            selected: None,
            pending: None,
            name_input,
            search_input,
            match_case: false,
            whole_word: false,
            search: core::SearchResult::default(),
            searching: false,
            search_rows: Vec::new(),
            procs,
            panel_open: false,
            panel_tab: PanelTab::Terminal,
            term_lines: Vec::new(),
            term_cwd,
            term_input,
            term_proc: None,
            term_history: Vec::new(),
            term_scroll: UniformListScrollHandle::new(),
            out_lines: Vec::new(),
            out_proc: None,
            out_scroll: UniformListScrollHandle::new(),
            term_run: None,
            term_wrap: Default::default(),
            out_wrap: Default::default(),
            panel_width: Rc::default(),
            image_box: Rc::default(),
            out_run: None,
            term_hist_ix: None,
            problems: Vec::new(),
            python_version: None,
            agent,
            scm,
            quick,
            all_files: Rc::default(),
            all_lower: Rc::default(),
            preview_server: None,
            github: crate::github::load(),
            github_input,
            preview: None,
            _subs: subs,
        };
        let restore = root.is_none() && open.is_empty();
        if let Some(root) = root {
            this.open_folder(root, cx);
        }
        for path in open {
            this.open_file(path, window, cx);
        }
        if restore {
            this.restore_session(window, cx);
        }
        this
    }

    pub(crate) fn open_folder(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(root.clone());
        if !self.term_proc.is_some_and(|id| self.procs.is_running(id)) {
            self.term_cwd = root.clone();
        }
        self.refresh_tree(cx);
        self.save_session(cx);
        self.detect_python(cx);
        self.scm_refresh(cx);
        cx.notify();
    }

    /// Find the workspace's Python version off the UI thread, for the terminal header.
    pub(crate) fn detect_python(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        cx.spawn(async move |this, cx| {
            let v = cx.background_spawn(async move { crate::panel::detect_python(root.as_deref()) }).await;
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
        let tree = self.tree.clone();
        // Walk the folder off the UI thread, then hand the items to the tree.
        // More than one folder: each is a top-level node, open to start with.
        cx.spawn(async move |this, cx| {
            let multi = roots.len() > 1;
            let nodes = cx.background_spawn(async move { if multi { files::scan_roots(&roots) } else { files::scan(&root) } }).await;
            let mut flat = Vec::new();
            files::flatten(&nodes, &mut flat);
            let mut dirs = std::collections::HashSet::new();
            let mut items = files::to_items(nodes, &mut dirs);
            if multi {
                items = items.into_iter().map(|i| i.expanded(true)).collect();
            }
            let all = crate::quick::file_list(flat, &dirs);
            _ = tree.update(cx, |state, cx| state.set_items(items, cx));
            _ = this.update(cx, |this, cx| {
                this.all_lower = Rc::new(all.iter().map(|p| this.rel_display(p).to_lowercase()).collect());
                this.all_files = Rc::new(all);
                this.dirs = Rc::new(dirs);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.docs.iter().position(|d| d.path == path) {
            self.activate(ix, window, cx);
            return;
        }
        if files::is_image(&path) {
            self.open_image(path, window, cx);
            return;
        }
        let bytes = match read_head(&path) {
            Ok(b) => b,
            Err(e) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                // os error 32 / 33: another program (a running server) holds the file.
                self.status = if e.contains("os error 32") || e.contains("os error 33") {
                    crate::trf!("{name} is in use by another program (a running server, for example), so it can't be opened now"; name => name)
                } else {
                    crate::trf!("Couldn't open {name}: {e}"; name => name, e => e)
                }
                .into();
                cx.notify();
                return;
            }
        };
        // Files a text editor can't show (binary, Office, PDF, SQLite, ZIP) open
        // as a read-only view instead of an error.
        let view = crate::viewer::view(&path, &bytes);
        let read_only = view.is_some();
        let (text, crlf, mut lang, mut label, banner) = match view {
            Some(v) => (v.text, false, v.lang, v.label, Some(v.banner)),
            None => {
                let file = core::decode_text(&bytes);
                let first = file.text.lines().next().unwrap_or("");
                let (lang, label) = files::language_for(&path, first);
                let banner = file.text.is_empty().then(|| crate::i18n::tr("This file is empty. Anything you type is saved to it.").to_string());
                (file.text, file.crlf, lang, label, banner)
            }
        };
        // Very large files open as plain text, like VS Code's large-file mode:
        // highlighting keeps styles for the whole document.
        let large = text.len() > LARGE_FILE_BYTES;
        if large && !read_only {
            lang = "text";
            label = "Plain Text";
            self.status = crate::i18n::tr("Large file: syntax colors are off").into();
        }
        let file = core::FileText { text, crlf };
        let s = &self.settings;
        let (tab, wrap, dots) = (s.tab_size, s.word_wrap, s.show_whitespace);
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(lang)
                .line_number(true)
                // Vertical guides show how far each block reaches; spaces show as faint dots.
                .indent_guides(!read_only)
                .show_whitespaces(dots && !read_only)
                .tab_size(TabSize { tab_size: tab, hard_tabs: false })
                .soft_wrap(wrap)
                .default_value(file.text)
        });
        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if read_only {
            editor.update(cx, |e, cx| e.set_readonly(true, cx));
        } else {
            crate::complete::attach(&editor, lang, &file_name, cx);
        }
        let sub = cx.subscribe(&editor, |this, editor, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let auto = this.settings.auto_save;
                if let Some(d) = this.docs.iter_mut().find(|d| d.editor == editor && !d.read_only) {
                    d.dirty = true;
                    // The "empty file" note goes once there is text.
                    d.banner = None;
                    d.gen += 1;
                    let gen = d.gen;
                    if auto {
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(std::time::Duration::from_millis(1000)).await;
                            _ = this.update(cx, |this, cx| {
                                if let Some(ix) = this.docs.iter().position(|d| d.editor == editor && d.gen == gen && d.dirty) {
                                    this.save_doc(ix, cx);
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                    }
                }
            }
            cx.notify();
        });
        let name: SharedString = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
        let banner = banner.map(SharedString::from);
        self.docs.push(Doc { path, name, lang, label, read_only, banner, crlf: file.crlf, editor, dirty: false, gen: 0, _sub: sub, image: None });
        self.activate(self.docs.len() - 1, window, cx);
        self.save_session(cx);
    }

    /// Open a picture in its own tab (no text editor, nothing to save).
    fn open_image(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let size = image::image_dimensions(&path).ok();
        let editor = cx.new(|cx| EditorState::new(window, cx));
        let sub = cx.subscribe(&editor, |_, _, _: &InputEvent, _| {});
        let name: SharedString = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
        self.docs.push(Doc {
            path,
            name,
            lang: "image",
            label: "Image",
            read_only: true,
            banner: None,
            crlf: false,
            editor,
            dirty: false,
            gen: 0,
            _sub: sub,
            image: Some(ImageDoc { size, bytes, actual: false }),
        });
        self.activate(self.docs.len() - 1, window, cx);
        self.save_session(cx);
    }

    fn render_image(&self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let d = &self.docs[ix];
        let Some(im) = &d.image else { return div().into_any_element() };
        let path = d.path.clone();
        // 100% means one image pixel per screen pixel.
        let scale = window.scale_factor().max(0.5);
        let picture = img(path).id(("image", ix));
        // Fit: never larger than 100%, never larger than the area (measured by the probe
        // below), aspect kept. Sizes are explicit so the picture sits exactly centered.
        let (bw, bh) = self.image_box.get();
        let (aw, ah) = ((bw - 32.).max(16.), (bh - 32.).max(16.));
        let view: AnyElement = match (im.actual, im.size) {
            (true, Some((w, h))) => div()
                .id(("image-scroll", ix))
                .size_full()
                .overflow_scroll()
                .child(div().p_4().child(picture.w(px(w as f32 / scale)).h(px(h as f32 / scale)).object_fit(ObjectFit::Fill)))
                .into_any_element(),
            (_, Some((w, h))) => {
                let (lw, lh) = (w as f32 / scale, h as f32 / scale);
                let k = (aw / lw).min(ah / lh).min(1.);
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(picture.w(px((lw * k).floor())).h(px((lh * k).floor())).object_fit(ObjectFit::Contain))
                    .into_any_element()
            }
            // SVG or an unreadable header: give it the whole area, keep its shape.
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(picture.w(px(aw)).h(px(ah)).object_fit(ObjectFit::Contain))
                .into_any_element(),
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
        let kind = d.path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
        let size = im.size.map(|(w, h)| format!("{w} × {h}  ·  ")).unwrap_or_default();
        let info = format!("{size}{}  ·  {kind}", human_size(im.bytes));
        let can_zoom = im.size.is_some();
        v_flex()
            .size_full()
            .child(
                div()
                    .id(("image-view", ix))
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
                    .child(probe)
                    .when(can_zoom, |d| d.cursor_pointer())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(im) = this.docs.get_mut(ix).and_then(|d| d.image.as_mut()) {
                            if im.size.is_some() {
                                im.actual = !im.actual;
                                cx.notify();
                            }
                        }
                    }))
                    .child(view),
            )
            .child(
                h_flex()
                    .h(theme::u(26.))
                    .flex_none()
                    .justify_center()
                    .gap_3()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .border_t_1()
                    .border_color(c(theme::BORDER))
                    .child(info)
                    .when(can_zoom, |h| h.child(if im.actual { "Click to fit" } else { "Click for 100%" })),
            )
            .into_any_element()
    }

    pub(crate) fn go_to_line(&mut self, line: u32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else { return };
        self.docs[ix].editor.update(cx, |ed, cx| {
            ed.set_cursor_position(Position::new(line.saturating_sub(1), 0), window, cx);
        });
    }

    pub(crate) fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = Some(ix);
        let handle = self.docs[ix].editor.focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn close(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        self.docs.remove(ix);
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
        // Image tabs and read-only views have no text to write (writing it would wipe the file).
        if self.docs[ix].image.is_some() || self.docs[ix].read_only {
            return true;
        }
        let doc = &mut self.docs[ix];
        let text = doc.editor.read(cx).value();
        match core::write_text(&doc.path, &text, doc.crlf) {
            Ok(()) => {
                doc.dirty = false;
                self.status = crate::trf!("Saved {0}"; 0 => doc.name).into();
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

    // ----------------------------------------------------------- session

    pub(crate) fn save_session(&self, _: &mut Context<Self>) {
        crate::settings::Session {
            root: self.root.clone(),
            extra: self.workspace.extra.clone(),
            files: self.docs.iter().map(|d| d.path.clone()).collect(),
            active: self.active,
        }
        .save();
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = crate::settings::Session::load();
        self.workspace.extra = s.extra.into_iter().filter(|p| p.is_dir()).collect();
        if let Some(root) = s.root.filter(|r| r.is_dir()) {
            self.open_folder(root, cx);
        }
        for f in s.files.into_iter().filter(|f| f.is_file()) {
            self.open_file(f, window, cx);
        }
        if let Some(a) = s.active.filter(|a| *a < self.docs.len()) {
            self.activate(a, window, cx);
        }
    }

    // ----------------------------------------------------------- actions

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

    fn on_quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        cx.notify();
    }

    fn show_view(&mut self, view: SideView, cx: &mut Context<Self>) {
        self.view = view;
        self.sidebar_open = true;
        self.settings_open = false;
        cx.notify();
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

    fn on_show_agent(&mut self, _: &ShowAgent, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.agent_right {
            self.agent_open = true;
            self.settings_open = false;
            let h = self.agent_input_focus(cx);
            h.focus(window, cx);
            cx.notify();
        } else {
            self.show_view(SideView::Agent, cx);
        }
    }

    /// Ctrl+L: open the agent panel with the caret in its input, or close it.
    fn on_toggle_agent(&mut self, _: &ToggleAgent, window: &mut Window, cx: &mut Context<Self>) {
        let open = if self.settings.agent_right { self.agent_open } else { self.sidebar_open && self.view == SideView::Agent };
        if !open {
            return self.on_show_agent(&ShowAgent, window, cx);
        }
        if self.settings.agent_right {
            self.agent_open = false;
        } else {
            self.view = SideView::Explorer;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn on_show_settings(&mut self, _: &ShowSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        cx.notify();
    }

    /// Ctrl+K Ctrl+T: the next color theme, FluxDark → FluxCream → FluxMono → FluxRed.
    fn on_next_theme(&mut self, _: &NextTheme, window: &mut Window, cx: &mut Context<Self>) {
        let ix = theme::PALETTES.iter().position(|p| p.id == theme::palette().id).unwrap_or(0);
        let next = theme::PALETTES[(ix + 1) % theme::PALETTES.len()].id;
        self.set_theme(next, window, cx);
    }

    /// Tab in the code editor: accept the highlighted suggestion, else indent.
    fn on_tab_accept(&mut self, _: &TabAccept, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{Enter, IndentInline};
        let editor = self.active.and_then(|ix| self.docs.get(ix)).map(|d| d.editor.clone());
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
        self.status = crate::trf!("FluxCode {0} (GPUI edition)"; 0 => env!("CARGO_PKG_VERSION")).into();
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
                if Some(id) == self.term_proc {
                    push_text(&mut self.term_lines, &text, err);
                    self.term_scroll.scroll_to_bottom();
                } else if Some(id) == self.out_proc {
                    push_text(&mut self.out_lines, &text, err);
                    self.out_scroll.scroll_to_bottom();
                } else if self.agent.owns(id) {
                    self.agent_on_output(&text, err, cx);
                }
            }
            ProcEvent::Exit { id, code } => {
                self.procs.forget(id);
                if Some(id) == self.term_proc {
                    self.term_proc = None;
                    self.finish_run(true, code, cx);
                } else if Some(id) == self.out_proc {
                    self.out_proc = None;
                    self.finish_run(false, code, cx);
                } else if self.agent.owns(id) {
                    self.agent_on_exit(cx);
                }
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------ render

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
        // Empty parts of the bar move the window (double-click maximizes). See wintitle.rs
        // for why this is done by hand on Windows.
        let drag = |id: &'static str| {
            div().id(id).flex_1().h_full().min_w(px(8.)).on_mouse_down(MouseButton::Left, |ev: &MouseDownEvent, window, _| {
                if cfg!(windows) {
                    if ev.click_count >= 2 {
                        window.zoom_window();
                    } else {
                        crate::wintitle::begin_move();
                    }
                }
            })
        };
        // The search box in the middle: Go to File, like VS Code's command center.
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
            .on_click(cx.listener(|this, _, w, cx| this.on_quick_open(&QuickOpenFile, w, cx)))
            .child(Icon::new(IconName::Search).xsmall().text_color(c(theme::FG_MUTED)))
            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_MUTED)).child(label))
            .child(div().flex_none().text_color(c(theme::FG_DIM)).child("Ctrl+P"));
        let row = h_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .pl_3()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(c(theme::FG_MUTED))
            .child(div().mr_2().flex_none().text_sm().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::SEMIBOLD).child(tr("FluxCode")))
            .child(Self::render_menu("m-file", tr("File"), |m| {
                m.menu(tr("Go to File..."), Box::new(QuickOpenFile))
                    .menu(tr("Open File..."), Box::new(OpenFile))
                    .menu(tr("Open Folder..."), Box::new(OpenFolder))
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
                    .separator()
                    .menu(tr("Toggle Sidebar"), Box::new(ToggleSidebar))
                    .menu(tr("Toggle Panel"), Box::new(TogglePanel))
                    .menu(tr("Output"), Box::new(ShowOutput))
            }))
            .child(Self::render_menu("m-run", tr("Run"), |m| {
                m.menu(tr("Run File"), Box::new(RunFile))
                    .menu(tr("Stop"), Box::new(StopRun))
                    .separator()
                    .menu(tr("Live Preview"), Box::new(TogglePreview))
            }))
            .child(Self::render_menu("m-term", tr("Terminal"), |m| m.menu(tr("Toggle Terminal"), Box::new(ToggleTerminal))))
            .child(Self::render_menu("m-help", tr("Help"), |m| {
                m.menu(tr("Next Color Theme"), Box::new(NextTheme)).separator().menu(tr("About FluxCode"), Box::new(About))
            }))
            .child(drag("tb-drag"));
        // Too narrow for a centered box: Ctrl+P still opens Go to File.
        let show_center = window.viewport_size().width > px(900.);
        if !cfg!(windows) {
            return TitleBar::new().child(row.when(show_center, |r| r.child(center).child(div().flex_1()))).into_any_element();
        }
        // Centered in the window, not in the space after the menus, so it lines up with
        // the Go to File box. The overlay has no hitbox: clicks pass to the bar below.
        let center_layer = div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().when(show_center, |d| d.child(center));
        // Windows: our own min / max / close, as plain buttons (always clickable).
        let maximized = window.is_maximized();
        let ctl = |id: &'static str, icon: IconName, close: bool| {
            div()
                .id(id)
                .w(px(46.))
                .h_full()
                .flex()
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
                .child(Icon::new(icon).small())
        };
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
            .child(ctl("tb-min", IconName::WindowMinimize, false).on_click(|_, w, _| w.minimize_window()))
            .child(
                ctl("tb-max", if maximized { IconName::WindowRestore } else { IconName::WindowMaximize }, false)
                    .on_click(|_, w, _| w.zoom_window()),
            )
            .child(ctl("tb-close", IconName::WindowClose, true).on_click(|_, w, cx| w.dispatch_action(Box::new(Quit), cx)))
            .child(center_layer)
            .into_any_element()
    }

    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // VS Code's pattern: the active view gets a 2px accent line on the left and full-strength icon.
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
            // OpenCode docked right: its icon shows and hides that dock instead.
            if view == SideView::Agent && this.settings.agent_right {
                let active = this.agent_open && !this.settings_open;
                return div().id((id, 0usize)).child(item(id, icon, active, this, cx)).on_click(cx.listener(move |this, _, _, cx| {
                    this.agent_open = !(this.agent_open && !this.settings_open);
                    this.settings_open = false;
                    cx.notify();
                }));
            }
            let active = this.sidebar_open && this.view == view && !this.settings_open;
            div().id((id, 0usize)).child(item(id, icon, active, this, cx)).on_click(cx.listener(move |this, _, _, cx| {
                if this.view == view && this.sidebar_open && !this.settings_open {
                    this.sidebar_open = false;
                } else {
                    this.view = view;
                    this.sidebar_open = true;
                    this.settings_open = false;
                }
                cx.notify();
            }))
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
                div()
                    .relative()
                    .child(view_item("ab-scm", Icon::new(gpui_kit::assets::IconName::GitBranch), SideView::Scm, self, cx))
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
            .child(view_item("ab-agent", Icon::new(IconName::Bot), SideView::Agent, self, cx))
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
        // Small uppercase view titles follow VS Code, whose layout this workbench mirrors.
        let title = match self.view {
            SideView::Explorer => tr("EXPLORER"),
            SideView::Search => tr("SEARCH"),
            SideView::Run => tr("RUN"),
            SideView::Scm => tr("SOURCE CONTROL"),
            SideView::Agent => match crate::agent::agent_name() {
                "Mura" => "MURA",
                _ => "OPENCODE",
            },
        };
        // With a folder open the Explorer header names it, like VS Code's root row.
        let title: SharedString = match (self.view, self.root.as_deref().and_then(Path::file_name)) {
            (SideView::Explorer, Some(_)) if !self.workspace.extra.is_empty() => tr("WORKSPACE").into(),
            (SideView::Explorer, Some(name)) => name.to_string_lossy().to_uppercase().into(),
            _ => title.into(),
        };
        let header = h_flex()
            .h(theme::u(35.))
            .pl_5()
            .pr_2()
            .gap_2()
            .flex_none()
            .when(self.view != SideView::Agent, |h| {
                h.child(div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_MUTED)).child(title))
            })
            .when(self.view == SideView::Agent, |h| h.child(self.render_agent_picker()).child(self.render_agent_switch(cx)).child(div().flex_1()))
            .when(self.view == SideView::Scm && self.scm.repo.is_some(), |h| h.child(self.render_scm_header(cx)))
            .when(self.view == SideView::Agent, |h| h.child(self.render_agent_tools(cx)))
            .when(self.view == SideView::Explorer && self.root.is_some(), |h| {
                h.child(tool_button("x-newfile", IconName::Plus, tr("New File"), cx, |this, w, cx| this.on_explorer_new_file(&ExplorerNewFile, w, cx)))
                    .child(tool_button("x-newdir", IconName::FolderClosed, tr("New Folder"), cx, |this, w, cx| this.on_explorer_new_folder(&ExplorerNewFolder, w, cx)))
                    .child(tool_button("x-refresh", IconName::RefreshCw, tr("Refresh"), cx, |this, w, cx| this.on_explorer_refresh(&ExplorerRefresh, w, cx)))
            });
        let body: AnyElement = match self.view {
            SideView::Explorer => self.render_explorer(window, cx),
            SideView::Search => self.render_search(cx),
            SideView::Run => self.render_run_view(cx),
            SideView::Scm => self.render_scm(cx),
            SideView::Agent => self.render_agent(window, cx),
        };
        v_flex()
            .size_full()
            .bg(c(theme::BG_SIDE))
            .border_r_1()
            .border_color(c(theme::BORDER))
            .child(header)
            .child(div().flex_1().min_h_0().size_full().child(body))
            .into_any_element()
    }

    fn render_run_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let file = self.active.map(|a| self.docs[a].name.to_string());
        v_flex()
            .p_3()
            .gap_3()
            .text_sm()
            .child(match &file {
                Some(f) => self
                    .action_button("run-file", crate::trf!("Run {f}"; f => f), ui::Kind::Primary, cx, |this, w, cx| this.on_run_file(&RunFile, w, cx))
                    .into_any_element(),
                None => div().text_xs().text_color(c(theme::FG_DIM)).child(tr("Open a file to run it.")).into_any_element(),
            })
            .when(self.out_proc.is_some(), |v| {
                v.child(self.action_button("stop-run", tr("Stop"), ui::Kind::Danger, cx, |this, w, cx| this.on_stop_run(&StopRun, w, cx)))
            })
            .child(
                div()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child("F5 runs the active file (Python, Node, PowerShell, batch, Rust via cargo). Output appears in the Output panel. Breakpoint debugging is not in the GPUI edition yet."),
            )
            .into_any_element()
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("tabs")
            .h(theme::u(35.))
            .w_full()
            .flex_none()
            .bg(c(theme::BG_SIDE))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .overflow_x_scroll()
            .children(self.docs.iter().enumerate().map(|(ix, d)| {
                let active = self.active == Some(ix);
                let (icon, tint) = files::icon_for(&d.name);
                h_flex()
                    .id(("tab", ix))
                    .h_full()
                    .pl_3()
                    .pr_1()
                    .gap_2()
                    .flex_none()
                    .text_sm()
                    .border_r_1()
                    .border_color(c(theme::BORDER))
                    .relative()
                    .cursor_pointer()
                    .when(active, |s| {
                        s.bg(c(theme::BG_EDITOR))
                            .child(div().absolute().top_0().left_0().right_0().h(px(1.)).bg(c(theme::ACCENT)))
                    })
                    .when(!active, |s| s.hover(|s| s.bg(c(theme::BG_HOVER))))
                    .text_color(if active { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                    .child(Icon::new(icon).size(theme::u(16.)).text_color(theme::tint(tint)))
                    .child(d.name.clone())
                    .child(
                        div()
                            .id(("tab-close", ix))
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
                    .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, window, cx)))
            }))
    }

    /// The active file's path below the tabs: folder › folder › file.
    fn render_breadcrumbs(&self, ix: usize) -> impl IntoElement {
        let d = &self.docs[ix];
        // Outside the workspace (a diff, a file opened on its own): just the name.
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
        match self.active {
            Some(ix) => v_flex()
                .size_full()
                .bg(theme::editor_wash())
                .child(self.render_tabs(cx))
                .child(self.render_breadcrumbs(ix))
                .when(self.docs[ix].image.is_some(), |v| v.child(div().flex_1().min_h_0().child(self.render_image(ix, window, cx))))
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
                            .child(Icon::new(if self.docs[ix].read_only { IconName::Eye } else { IconName::Info }).small().text_color(c(theme::FG_DIM)))
                            .child(div().min_w_0().truncate().child(b)),
                    )
                })
                .when(self.docs[ix].image.is_none(), |v| v.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .child(
                            Editor::new(&self.docs[ix].editor)
                                .bordered(false)
                                .p_0()
                                .h_full()
                                .font_family(self.settings.font_family.clone())
                                .text_size(px(self.settings.font_size)),
                        )
                        .child(self.smooth_scroll_layer(ScrollTarget::Editor(self.docs[ix].editor.clone()), cx)),
                ))
                .into_any_element(),
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_5()
                .bg(theme::editor_wash())
                .child(
                    v_flex()
                        .items_center()
                        .gap_1()
                        .child(div().text_size(theme::u(40.)).font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("FluxCode")))
                        .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(tr("Lightweight. Native. One process."))),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(self.action_button("w-open-folder", tr("Open folder"), ui::Kind::Primary, cx, |this, w, cx| this.on_open_folder(&OpenFolder, w, cx)))
                        .child(self.action_button("w-open-file", tr("Open file"), ui::Kind::Plain, cx, |this, w, cx| this.on_open_file(&OpenFile, w, cx))),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .pt_2()
                        .child(shortcut(tr("Find in files"), "Ctrl+Shift+F"))
                        .child(shortcut(tr("Toggle terminal"), "Ctrl+`"))
                        .child(shortcut(tr("Run file"), "F5"))
                        .child(shortcut(tr("OpenCode"), "Ctrl+Shift+A"))
                        .child(shortcut(tr("Next color theme"), "Ctrl+K Ctrl+T"))
                        .child(shortcut(tr("Settings"), "Ctrl+,")),
                )
                .into_any_element(),
        }
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (pos, lang) = match self.active {
            Some(ix) if self.docs[ix].image.is_some() => {
                let im = self.docs[ix].image.as_ref().unwrap();
                (im.size.map(|(w, h)| format!("{w} × {h}")).unwrap_or_default(), "Image")
            }
            Some(ix) => {
                let d = &self.docs[ix];
                let p = d.editor.read(cx).cursor_position();
                (crate::trf!("Ln {0}, Col {1}"; 0 => p.line + 1, 1 => p.character + 1), d.label)
            }
            None => (String::new(), ""),
        };
        // Images and read-only views have no line endings or encoding to show.
        let is_image = self.active.is_some_and(|a| self.docs[a].image.is_some() || self.docs[a].read_only);
        let crlf = self.active.filter(|_| !is_image).map(|a| if self.docs[a].crlf { "CRLF" } else { "LF" }).unwrap_or("");
        let running = self.out_proc.is_some() || self.term_proc.is_some();
        let seg = |s: SharedString| div().h_full().px_2().flex().items_center().child(s);
        let folder = self.root.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
        h_flex()
            .h(theme::u(24.))
            .w_full()
            .flex_none()
            .bg(c(theme::BG_TITLE))
            .border_t_1()
            .border_color(c(theme::BORDER))
            .text_xs()
            .text_color(c(theme::FG_MUTED))
            .when_some(folder, |h, f| h.child(seg(f.into()).pl_3().text_color(c(theme::FG))))
            .when_some(self.scm.status.branch.clone().filter(|_| self.scm.repo.is_some()), |h, b| {
                let st = &self.scm.status;
                let dirty = if st.count() > 0 { "*" } else { "" };
                let sync = match &st.upstream {
                    None => "Publish".to_string(),
                    Some(_) => format!("{}↓ {}↑", st.behind, st.ahead),
                };
                h.child(
                    h_flex()
                        .id("sb-branch")
                        .h_full()
                        .px_2()
                        .gap_1()
                        .cursor_pointer()
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(Icon::new(gpui_kit::assets::IconName::GitBranch).size(theme::u(13.)))
                        .child(format!("{b}{dirty}"))
                        .on_click(cx.listener(|this, _, w, cx| this.on_scm_show(&ScmShow, w, cx))),
                )
                .child(
                    h_flex()
                        .id("sb-sync")
                        .h_full()
                        .px_2()
                        .gap_1()
                        .cursor_pointer()
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(Icon::new(if st.upstream.is_some() { gpui_kit::assets::IconName::RefreshCcw } else { gpui_kit::assets::IconName::CloudUpload }).size(theme::u(13.)))
                        .child(if self.scm.busy.is_some() { "…".to_string() } else { sync })
                        .on_click(cx.listener(|this, _, w, cx| this.on_scm_sync(&ScmSync, w, cx))),
                )
            })
            // Only a running program gets a marker: it is the one state worth noticing here.
            .when(running, |h| {
                h.child(
                    h_flex()
                        .id("sb-running")
                        .h_full()
                        .px_2()
                        .gap_1()
                        .cursor_pointer()
                        .text_color(c(theme::GREEN))
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(div().size(px(6.)).rounded_full().bg(c(theme::GREEN)))
                        .child(tr("Running"))
                        .on_click(cx.listener(|this, _, w, cx| this.on_toggle_terminal(&ToggleTerminal, w, cx))),
                )
            })
            .when(!self.problems.is_empty(), |h| {
                h.child(
                    h_flex()
                        .id("sb-problems")
                        .h_full()
                        .px_2()
                        .gap_1()
                        .cursor_pointer()
                        .text_color(c(theme::DANGER))
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(Icon::new(IconName::CircleX).size(theme::u(13.)))
                        .child(self.problems.len().to_string())
                        .on_click(cx.listener(|this, _, w, cx| this.on_go_to_problem(&GoToProblem, w, cx))),
                )
            })
            .child(div().px_2().flex_1().truncate().text_color(c(theme::FG)).child(self.status.clone()))
            .when(!pos.is_empty(), |h| {
                h.child(seg(pos.into())).when(!is_image, |h| h.child(seg(crlf.into())).child(seg("UTF-8".into()))).child(seg(lang.into()))
            })
            .child(seg(theme::palette().name.into()).pr_3())
    }
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
        .w(theme::u(320.))
        .gap_2()
        .text_sm()
        .child(div().flex_1().text_color(c(theme::FG_MUTED)).child(label))
        .children(keys.split(['+', ' ']).map(|k| {
            div()
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

/// "245 KB", "1.2 MB".
fn human_size(b: u64) -> String {
    match b {
        0..=1023 => format!("{b} B"),
        1024..=1_048_575 => format!("{:.0} KB", b as f64 / 1024.),
        _ => format!("{:.1} MB", b as f64 / 1_048_576.),
    }
}

/// A panel's content may not grow its slot: anything wider is clipped, so a long
/// tab row or prompt can't push the next panel past the window edge.
fn clip(e: AnyElement) -> AnyElement {
    div().size_full().min_w_0().min_h_0().overflow_hidden().child(e).into_any_element()
}

/// Keep at most this many lines per panel.
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

/// Append streamed output, continuing an unfinished last line, and (re)classify
/// each touched line so its color follows what it says, not which pipe it used.
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
            LineKind::Frame => crate::term::parse_frame(&l.text).map(|(p, n, _)| (p.into(), n)),
            LineKind::Warn => crate::term::parse_path_line(&l.text).map(|(p, n)| (p.into(), n)),
            _ => None,
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
        if !self.agent.reloads.is_empty() {
            self.apply_agent_reloads(window, cx);
        }
        if self.scm.clear_message {
            self.scm.clear_message = false;
            self.scm.message.update(cx, |s, cx| s.set_value("", window, cx));
        }
        // Interface size: rem-based spacing and text follow this; fixed sizes use theme::u.
        window.set_rem_size(theme::rem());
        let editor_view: AnyElement = match &self.preview {
            Some(_) => h_resizable("editor-preview-split")
                .child(resizable_panel().child(clip(self.render_editor_area(window, cx))))
                .child(resizable_panel().size(px(520.)).size_range(px(240.)..px(1400.)).child(self.render_preview(cx)))
                .into_any_element(),
            None => self.render_editor_area(window, cx),
        };
        let editor_view = div().relative().size_full().child(editor_view).child(self.render_drop_hint()).into_any_element();
        let editor_and_panel: AnyElement = if self.panel_open {
            v_resizable("editor-panel-split")
                .child(resizable_panel().child(clip(editor_view)))
                .child(resizable_panel().size(px(240.)).size_range(px(80.)..px(900.)).child(clip(self.render_panel(window, cx).into_any_element())))
                .into_any_element()
        } else {
            editor_view
        };
        // Side bar and right dock are fixed-width columns dragged by their inner edge;
        // the editor in between takes what is left and never pushes them out.
        let handle = |id: &'static str, which: u8, this: &Self, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(4.))
                .h_full()
                .flex_none()
                .cursor_col_resize()
                .hover(|s| s.bg(c(theme::ACCENT)))
                .when(this.resizing == which, |d| d.bg(c(theme::ACCENT)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                    this.resizing = which;
                    cx.notify();
                }))
        };
        let center: AnyElement = if self.sidebar_open {
            h_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(div().w(px(self.sidebar_width)).flex_none().h_full().overflow_hidden().child(self.render_side_bar(window, cx)))
                .child(handle("sidebar-resize", 1, self, cx))
                .child(div().flex_1().min_w_0().h_full().overflow_hidden().child(editor_and_panel))
                .into_any_element()
        } else {
            editor_and_panel
        };
        // The right dock is a fixed-width column (dragged by its left edge), so it always
        // fits in the window: the editor side gives up the space, never the dock.
        let center: AnyElement = if self.settings.agent_right && self.agent_open {
            h_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(div().flex_1().min_w_0().h_full().overflow_hidden().child(center))
                .child(handle("agent-resize", 2, self, cx))
                .child(div().w(px(self.agent_width)).flex_none().h_full().overflow_hidden().child(self.render_agent_dock(window, cx)))
                .into_any_element()
        } else {
            center
        };
        let main = h_flex().flex_1().min_h_0().child(self.render_activity_bar(cx)).child(center);
        // The user's picture, under every panel. Panels turn translucent while it is set
        // (theme::c), so it shows through each of them at the chosen visibility.
        let backdrop = self.settings.background().map(|p| {
            img(p.to_path_buf()).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)
        });
        v_flex()
            .id("workbench")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(gpui_kit::Hsla::from(rgb(theme::palette().swatches()[1])))
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
                    _ => return,
                }
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.resizing != 0 {
                    this.resizing = 0;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(Self::on_quick_open))
            .on_action(cx.listener(Self::on_quick_close))
            .on_action(cx.listener(Self::on_quick_up))
            .on_action(cx.listener(Self::on_quick_down))
            .on_action(cx.listener(Self::on_agent_use_opencode))
                        .on_action(cx.listener(Self::on_agent_use_mura))
            .on_action(cx.listener(Self::on_agent_mention_up))
            .on_action(cx.listener(Self::on_agent_mention_down))
            .on_action(cx.listener(Self::on_agent_mention_accept))
            .on_action(cx.listener(Self::on_agent_mention_close))
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
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_save_all))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_open_folder))
            .on_action(cx.listener(Self::on_open_file))
            .on_action(cx.listener(Self::on_quit))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .on_action(cx.listener(Self::on_show_explorer))
            .on_action(cx.listener(Self::on_find_in_files))
            .on_action(cx.listener(Self::on_show_run))
            .on_action(cx.listener(Self::on_show_agent))
            .on_action(cx.listener(Self::on_show_settings))
            .on_action(cx.listener(Self::on_about))
            .on_action(cx.listener(Self::on_toggle_panel))
            .on_action(cx.listener(Self::on_toggle_terminal))
            .on_action(cx.listener(Self::on_show_output))
            .on_action(cx.listener(Self::on_run_file))
            .on_action(cx.listener(Self::on_stop_run))
            .on_action(cx.listener(Self::on_explorer_new_file))
            .on_action(cx.listener(Self::on_explorer_new_folder))
            .on_action(cx.listener(Self::on_explorer_rename))
            .on_action(cx.listener(Self::on_explorer_delete))
            .on_action(cx.listener(Self::on_explorer_refresh))
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_reveal))
            .on_action(cx.listener(Self::on_toggle_preview))
            .on_action(cx.listener(Self::on_reload_preview))
            .on_action(cx.listener(Self::on_remove_folder))
            // Folders and files dropped anywhere (the agent panel keeps its own drops).
            .on_drop(cx.listener(|this, paths: &ExternalPaths, w, cx| this.on_drop_paths(paths, w, cx)))
            .child(self.render_title_bar(window, cx))
            .child(main)
            .child(self.render_status_bar(cx))
            .when(self.quick.open, |v| v.child(self.render_quick_open(cx)))
            .children(self.render_drop_ask(cx))
    }
}

/// The file's bytes; past the editor's limit only the first 64 KB, for the hex view.
fn read_head(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() <= core::MAX_FILE_BYTES {
        return std::fs::read(path).map_err(|e| e.to_string());
    }
    let mut buf = vec![0; 64 * 1024];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)).map_err(|e| e.to_string())?;
    buf.truncate(n);
    // NUL bytes make the viewer show it as binary even when it is huge text.
    if !buf.contains(&0) {
        return Err(crate::trf!("{0} is too large to open ({1} MB)."; 0 => path.display(), 1 => meta.len() / 1024 / 1024));
    }
    Ok(buf)
}

impl Workbench {
    /// After a language change: the text set once at creation (input placeholders).
    pub(crate) fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |s, cx| s.set_placeholder(tr("Search"), window, cx));
        self.term_input.update(cx, |s, cx| s.set_placeholder(tr("Type a command (Tab completes, ↑ for history)"), window, cx));
        self.github_input.update(cx, |s, cx| s.set_placeholder(tr("Personal access token"), window, cx));
        self.quick.input.update(cx, |s, cx| s.set_placeholder(tr("Type part of a file name"), window, cx));
        self.scm.message.update(cx, |s, cx| s.set_placeholder(tr("Message (Enter to commit)"), window, cx));
        let name = crate::agent::agent_name();
        self.agent.input.update(cx, |s, cx| s.set_placeholder(crate::trf!("Ask {name}…"; name => name), window, cx));
        self.agent.memory_input.update(cx, |s, cx| s.set_placeholder(tr("Remember something about this project…"), window, cx));
        cx.notify();
    }
}

/// What a smooth wheel scroll moves: a code editor, or a plain scroll area (the chat).
#[derive(Clone)]
pub(crate) enum ScrollTarget {
    Editor(Entity<EditorState>),
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

    /// Keep a target inside what can be scrolled (editors clamp on their own).
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
}

impl Workbench {
    /// Smooth wheel scrolling, like VS Code's: a wheel notch glides to its target
    /// in about 100 ms instead of jumping, which reads as stutter. One step per
    /// frame (paced by the display, nothing runs between glides). Touchpads send
    /// pixel deltas and keep the native scrolling; Ctrl+wheel is left alone. A
    /// transparent layer over the area catches the wheel before the area does.
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
        let d = delta.pixel_delta(target.line_height(cx));
        let key = target.key();
        let running = self.smooth.as_ref().is_some_and(|s| s.target.key() == key);
        // Notches while gliding add up from where the glide is heading.
        let base = match self.smooth.as_ref().filter(|s| s.target.key() == key) {
            Some(s) => s.to,
            None => target.offset(cx),
        };
        let to = target.clamp(point(base.x + d.x, base.y + d.y));
        match self.smooth.as_mut().filter(|s| s.target.key() == key) {
            Some(s) => {
                s.to = to;
                s.stuck = 0;
            }
            None => self.smooth = Some(SmoothScroll { target, to, prev: None, stuck: 0, last: std::time::Instant::now() }),
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
        // on_next_frame schedules the frame itself (request_animation_frame is
        // only allowed while drawing and would panic from an event handler).
    }

    fn smooth_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.smooth.as_mut() else { return };
        let target = s.target.clone();
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
        // At an edge the area clamps and nothing moves: stop there.
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
        // Exponential ease, 45 ms time constant: quick start, soft landing.
        let k = 1.0 - (-dt / 0.045).exp();
        target.set(point(cur.x + diff.x * k, cur.y + diff.y * k), cx);
        cx.notify();
        self.smooth_next_frame(window, cx);
    }
}
