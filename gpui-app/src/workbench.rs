//! The workbench: title bar + menus, activity bar, side bar (explorer, search,
//! run, OpenCode), editor tabs, bottom panel (terminal, output) and status
//! bar — the same layout as the Tauri FluxCode.

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
        GoToProblem
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
    pub lang: &'static str,
    pub crlf: bool,
    pub editor: Entity<EditorState>,
    pub dirty: bool,
    /// Bumped on every edit; auto save only fires for the latest one.
    pub gen: u64,
    pub _sub: Subscription,
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
    pub(crate) tree: Entity<TreeState>,
    /// Every folder in the tree (by path), including empty and skipped ones.
    pub(crate) dirs: Rc<std::collections::HashSet<String>>,
    pub(crate) docs: Vec<Doc>,
    pub(crate) active: Option<usize>,
    pub(crate) view: SideView,
    pub(crate) sidebar_open: bool,
    /// The OpenCode chat docked on the right is showing (settings.agent_right).
    pub(crate) agent_open: bool,
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
    pub(crate) out_run: Option<crate::panel::RunInfo>,
    /// Position while walking the history with Up/Down.
    pub(crate) term_hist_ix: Option<usize>,
    /// Python errors from the last runs, underlined in the editor.
    pub(crate) problems: Vec<crate::panel::Problem>,
    /// "Python 3.15.0" for the terminal header (detected in the background).
    pub(crate) python_version: Option<SharedString>,
    pub(crate) agent: crate::agent::AgentState,
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
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let term_input = cx.new(|cx| InputState::new(window, cx).placeholder("Type a command (Tab completes, ↑ for history)"));
        let github_input = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Personal access token"));
        let subs = vec![
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
        let term_cwd = root.clone().or_else(|| std::env::current_dir().ok()).unwrap_or_default();
        let agent = crate::agent::AgentState::new(window, cx);
        let mut this = Self {
            root: None,
            tree,
            dirs: Rc::default(),
            docs: Vec::new(),
            active: None,
            view: SideView::Explorer,
            sidebar_open: true,
            agent_open: false,
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
            out_run: None,
            term_hist_ix: None,
            problems: Vec::new(),
            python_version: None,
            agent,
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
        let tree = self.tree.clone();
        // Walk the folder off the UI thread, then hand the items to the tree.
        cx.spawn(async move |this, cx| {
            let nodes = cx.background_spawn(async move { files::scan(&root) }).await;
            let mut dirs = std::collections::HashSet::new();
            let items = files::to_items(nodes, &mut dirs);
            _ = tree.update(cx, |state, cx| state.set_items(items, cx));
            _ = this.update(cx, |this, cx| {
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
        let file = match core::read_text(&path) {
            Ok(f) => f,
            Err(e) => {
                self.status = format!("{}: {e}", path.display()).into();
                cx.notify();
                return;
            }
        };
        // gpui-component's tree-sitter highlighter keeps styles for the whole
        // document (~+170 MB for a 3.5 MB / 60k-line JS file), so very large
        // files open as plain text, like VS Code's large-file mode.
        let large = file.text.len() > LARGE_FILE_BYTES;
        let lang = if large { "text" } else { files::language_for(&path) };
        if large {
            self.status = "Large file: syntax highlighting is off".into();
        }
        let s = &self.settings;
        let (tab, wrap, dots) = (s.tab_size, s.word_wrap, s.show_whitespace);
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(lang)
                .line_number(true)
                // Vertical guides show how far each block reaches; spaces show as faint dots.
                .indent_guides(true)
                .show_whitespaces(dots)
                .tab_size(TabSize { tab_size: tab, hard_tabs: false })
                .soft_wrap(wrap)
                .default_value(file.text)
        });
        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        crate::complete::attach(&editor, lang, &file_name, cx);
        let sub = cx.subscribe(&editor, |this, editor, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let auto = this.settings.auto_save;
                if let Some(d) = this.docs.iter_mut().find(|d| d.editor == editor) {
                    d.dirty = true;
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
        self.docs.push(Doc { path, name, lang, crlf: file.crlf, editor, dirty: false, gen: 0, _sub: sub });
        self.activate(self.docs.len() - 1, window, cx);
        self.save_session(cx);
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

    fn save_doc(&mut self, ix: usize, cx: &mut Context<Self>) -> bool {
        let doc = &mut self.docs[ix];
        let text = doc.editor.read(cx).value();
        match core::write_text(&doc.path, &text, doc.crlf) {
            Ok(()) => {
                doc.dirty = false;
                self.status = format!("Saved {}", doc.name).into();
                if let Some(s) = &self.preview_server {
                    s.reload();
                }
                true
            }
            Err(e) => {
                self.status = format!("Save failed: {e}").into();
                false
            }
        }
    }

    // ----------------------------------------------------------- session

    fn save_session(&self, _: &mut Context<Self>) {
        crate::settings::Session {
            root: self.root.clone(),
            files: self.docs.iter().map(|d| d.path.clone()).collect(),
            active: self.active,
        }
        .save();
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = crate::settings::Session::load();
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
            this.update(cx, |this, cx| this.open_folder(path, cx)).ok()
        })
        .detach();
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
        self.status = format!("FluxCode {} (GPUI edition)", env!("CARGO_PKG_VERSION")).into();
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

    fn render_title_bar(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let folder = self
            .root
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "no folder open".into());
        TitleBar::new().child(
            h_flex()
                .w_full()
                .pr_2()
                .items_center()
                .gap_1()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(div().mr_2().text_sm().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::SEMIBOLD).child("FluxCode"))
                .child(Self::render_menu("m-file", "File", |m| {
                    m.menu("Open File...", Box::new(OpenFile))
                        .menu("Open Folder...", Box::new(OpenFolder))
                        .separator()
                        .menu("New File...", Box::new(ExplorerNewFile))
                        .menu("New Folder...", Box::new(ExplorerNewFolder))
                        .separator()
                        .menu("Save", Box::new(Save))
                        .menu("Save All", Box::new(SaveAll))
                        .separator()
                        .menu("Settings", Box::new(ShowSettings))
                        .separator()
                        .menu("Close Editor", Box::new(CloseTab))
                        .menu("Exit", Box::new(Quit))
                }))
                .child(Self::render_menu("m-edit", "Edit", |m| {
                    m.menu("Undo", Box::new(gpui_kit::component::input::Undo))
                        .menu("Redo", Box::new(gpui_kit::component::input::Redo))
                        .separator()
                        .menu("Cut", Box::new(gpui_kit::component::input::Cut))
                        .menu("Copy", Box::new(gpui_kit::component::input::Copy))
                        .menu("Paste", Box::new(gpui_kit::component::input::Paste))
                        .separator()
                        .menu("Find", Box::new(gpui_kit::component::input::Search))
                        .menu("Find in Files", Box::new(FindInFiles))
                }))
                .child(Self::render_menu("m-view", "View", |m| {
                    m.menu("Explorer", Box::new(ShowExplorer))
                        .menu("Search", Box::new(FindInFiles))
                        .menu("Run", Box::new(ShowRun))
                        .menu("OpenCode", Box::new(ShowAgent))
                        .separator()
                        .menu("Toggle Sidebar", Box::new(ToggleSidebar))
                        .menu("Toggle Panel", Box::new(TogglePanel))
                        .menu("Output", Box::new(ShowOutput))
                }))
                .child(Self::render_menu("m-run", "Run", |m| {
                    m.menu("Run File", Box::new(RunFile))
                        .menu("Stop", Box::new(StopRun))
                        .separator()
                        .menu("Live Preview", Box::new(TogglePreview))
                }))
                .child(Self::render_menu("m-term", "Terminal", |m| {
                    m.menu("Toggle Terminal", Box::new(ToggleTerminal))
                }))
                .child(Self::render_menu("m-help", "Help", |m| {
                    m.menu("Next Color Theme", Box::new(NextTheme)).separator().menu("About FluxCode", Box::new(About))
                }))
                .child(div().flex_1())
                .child(
                    h_flex()
                        .id("cmd-center")
                        .w(theme::u(380.))
                        .h(theme::u(24.))
                        .px_2()
                        .gap_2()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(c(theme::BG_SIDE))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).border_color(c(theme::BORDER_STRONG)))
                        .on_click(cx.listener(|this, _, w, cx| this.on_find_in_files(&FindInFiles, w, cx)))
                        .child(Icon::new(IconName::Search).xsmall().text_color(c(theme::FG_MUTED)))
                        .child(div().flex_1().text_color(c(theme::FG)).child(folder))
                        .child(div().text_color(c(theme::FG_DIM)).child("Ctrl+Shift+F")),
                )
                .child(div().flex_1()),
        )
    }

    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // VS Code's pattern: the active view gets a 2px accent line on the left and full-strength icon.
        let item = |id: &'static str, icon: IconName, active: bool, _this: &Self, _cx: &mut Context<Self>| {
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
                .child(Icon::new(icon).size(theme::u(17.)))
        };
        let view_item = |id: &'static str, icon: IconName, view: SideView, this: &Self, cx: &mut Context<Self>| {
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
            .child(view_item("ab-explorer", IconName::FolderOpen, SideView::Explorer, self, cx))
            .child(view_item("ab-search", IconName::Search, SideView::Search, self, cx))
            .child(view_item("ab-run", IconName::Play, SideView::Run, self, cx))
            .child(view_item("ab-agent", IconName::Bot, SideView::Agent, self, cx))
            .child(div().flex_1())
            .child(
                div()
                    .id(("ab-settings", 0usize))
                    .pb_1()
                    .child(item("ab-settings", IconName::Settings, self.settings_open, self, cx))
                    .on_click(cx.listener(|this, _, w, cx| this.on_show_settings(&ShowSettings, w, cx))),
            )
    }

    fn render_side_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Small uppercase view titles follow VS Code, whose layout this workbench mirrors.
        let title = match self.view {
            SideView::Explorer => "EXPLORER",
            SideView::Search => "SEARCH",
            SideView::Run => "RUN",
            SideView::Agent => "OPENCODE",
        };
        let header = h_flex()
            .h(theme::u(35.))
            .pl_5()
            .pr_2()
            .gap_2()
            .flex_none()
            .child(div().flex_1().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_MUTED)).child(title))
            .when(self.view == SideView::Explorer && self.root.is_some(), |h| {
                h.child(tool_button("x-newfile", IconName::Plus, "New File", cx, |this, w, cx| this.on_explorer_new_file(&ExplorerNewFile, w, cx)))
                    .child(tool_button("x-newdir", IconName::FolderClosed, "New Folder", cx, |this, w, cx| this.on_explorer_new_folder(&ExplorerNewFolder, w, cx)))
                    .child(tool_button("x-refresh", IconName::RefreshCw, "Refresh", cx, |this, w, cx| this.on_explorer_refresh(&ExplorerRefresh, w, cx)))
            });
        let body: AnyElement = match self.view {
            SideView::Explorer => self.render_explorer(window, cx),
            SideView::Search => self.render_search(cx),
            SideView::Run => self.render_run_view(cx),
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
                    .action_button("run-file", format!("Run {f}"), ui::Kind::Primary, cx, |this, w, cx| this.on_run_file(&RunFile, w, cx))
                    .into_any_element(),
                None => div().text_xs().text_color(c(theme::FG_DIM)).child("Open a file to run it.").into_any_element(),
            })
            .when(self.out_proc.is_some(), |v| {
                v.child(self.action_button("stop-run", "Stop", ui::Kind::Danger, cx, |this, w, cx| this.on_stop_run(&StopRun, w, cx)))
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
        let rel = self.root.as_deref().and_then(|r| d.path.strip_prefix(r).ok()).unwrap_or(&d.path);
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
                .child(
                    div().flex_1().min_h_0().child(
                        Editor::new(&self.docs[ix].editor)
                            .bordered(false)
                            .p_0()
                            .h_full()
                            .font_family(self.settings.font_family.clone())
                            .text_size(px(self.settings.font_size)),
                    ),
                )
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
                        .child(div().text_size(theme::u(40.)).font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child("FluxCode"))
                        .child(div().text_sm().text_color(c(theme::FG_MUTED)).child("Lightweight. Native. One process.")),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(self.action_button("w-open-folder", "Open folder", ui::Kind::Primary, cx, |this, w, cx| this.on_open_folder(&OpenFolder, w, cx)))
                        .child(self.action_button("w-open-file", "Open file", ui::Kind::Plain, cx, |this, w, cx| this.on_open_file(&OpenFile, w, cx))),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .pt_2()
                        .child(shortcut("Find in files", "Ctrl+Shift+F"))
                        .child(shortcut("Toggle terminal", "Ctrl+`"))
                        .child(shortcut("Run file", "F5"))
                        .child(shortcut("OpenCode", "Ctrl+Shift+A"))
                        .child(shortcut("Next color theme", "Ctrl+K Ctrl+T"))
                        .child(shortcut("Settings", "Ctrl+,")),
                )
                .into_any_element(),
        }
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (pos, lang) = match self.active {
            Some(ix) => {
                let d = &self.docs[ix];
                let p = d.editor.read(cx).cursor_position();
                (format!("Ln {}, Col {}", p.line + 1, p.character + 1), files::language_label(d.lang))
            }
            None => (String::new(), ""),
        };
        let crlf = self.active.map(|a| if self.docs[a].crlf { "CRLF" } else { "LF" }).unwrap_or("");
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
                        .child("Running")
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
                h.child(seg(pos.into())).child(seg(crlf.into())).child(seg("UTF-8".into())).child(seg(lang.into()))
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
        if !self.agent.reloads.is_empty() {
            self.apply_agent_reloads(window, cx);
        }
        // Interface size: rem-based spacing and text follow this; fixed sizes use theme::u.
        window.set_rem_size(theme::rem());
        let editor_view: AnyElement = match &self.preview {
            Some(_) => h_resizable("editor-preview-split")
                .child(resizable_panel().child(self.render_editor_area(window, cx)))
                .child(resizable_panel().size(px(520.)).size_range(px(240.)..px(1400.)).child(self.render_preview(cx)))
                .into_any_element(),
            None => self.render_editor_area(window, cx),
        };
        let editor_and_panel: AnyElement = if self.panel_open {
            v_resizable("editor-panel-split")
                .child(resizable_panel().child(editor_view))
                .child(resizable_panel().size(px(240.)).size_range(px(80.)..px(900.)).child(self.render_panel(window, cx)))
                .into_any_element()
        } else {
            editor_view
        };
        let center: AnyElement = if self.sidebar_open {
            h_resizable("workbench-split")
                .child(resizable_panel().size(px(260.)).size_range(px(160.)..px(600.)).child(self.render_side_bar(window, cx)))
                .child(resizable_panel().child(editor_and_panel))
                .into_any_element()
        } else {
            editor_and_panel
        };
        let center: AnyElement = if self.settings.agent_right && self.agent_open {
            h_resizable("agent-split")
                .child(resizable_panel().child(center))
                .child(resizable_panel().size(px(380.)).size_range(px(260.)..px(900.)).child(self.render_agent_dock(window, cx)))
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
            .child(self.render_title_bar(window, cx))
            .child(main)
            .child(self.render_status_bar(cx))
    }
}
