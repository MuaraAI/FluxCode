//! The workbench: title bar + menus, activity bar, side bar (explorer, search,
//! run, OpenCode), editor tabs, bottom panel (terminal, output) and status
//! bar — the same layout as the Tauri FluxCode.

use crate::core;
use crate::files;
use crate::proc::{ProcEvent, Procs};
use crate::theme::{self, c};
use futures::StreamExt;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Editor, EditorState, InputEvent, InputState, Position, TabSize},
    menu::{DropdownMenu, PopupMenu},
    resizable::{h_resizable, resizable_panel, v_resizable},
    status_bar::StatusBar,
    tree::TreeState,
    v_flex, Disableable, Icon, IconName, Sizable, TitleBar,
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
        ExplorerDelete, ExplorerRefresh, CopyPath, RevealInExplorer, ShowSettings, About, TogglePreview, ReloadPreview
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
        KeyBinding::new("ctrl-k v", TogglePreview, None),
        KeyBinding::new("f2", ExplorerRename, Some("Explorer")),
        KeyBinding::new("delete", ExplorerDelete, Some("Explorer")),
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    Normal,
    Err,
    Cmd,
    Info,
}

pub(crate) struct Line {
    pub text: SharedString,
    pub kind: LineKind,
    /// The process may still append to this line (no newline yet).
    pub open: bool,
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
    pub(crate) docs: Vec<Doc>,
    pub(crate) active: Option<usize>,
    pub(crate) view: SideView,
    pub(crate) sidebar_open: bool,
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
    pub(crate) agent: crate::agent::AgentState,
    pub(crate) preview_server: Option<crate::preview::PreviewServer>,
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
        let term_input = cx.new(|cx| InputState::new(window, cx));
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
            docs: Vec::new(),
            active: None,
            view: SideView::Explorer,
            sidebar_open: true,
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
            agent,
            preview_server: None,
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
        cx.notify();
    }

    pub(crate) fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let tree = self.tree.clone();
        // Walk the folder off the UI thread, then hand the items to the tree.
        cx.spawn(async move |_, cx| {
            let nodes = cx.background_spawn(async move { files::scan(&root) }).await;
            _ = tree.update(cx, |state, cx| state.set_items(files::to_items(nodes), cx));
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
        let (tab, wrap) = (s.tab_size, s.word_wrap);
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(lang)
                .line_number(true)
                .indent_guides(true)
                .tab_size(TabSize { tab_size: tab, hard_tabs: false })
                .soft_wrap(wrap)
                .default_value(file.text)
        });
        crate::complete::attach(&editor, lang, cx);
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

    fn on_show_agent(&mut self, _: &ShowAgent, _: &mut Window, cx: &mut Context<Self>) {
        self.show_view(SideView::Agent, cx);
    }

    fn on_show_settings(&mut self, _: &ShowSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        cx.notify();
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
                let kind = if err { LineKind::Err } else { LineKind::Normal };
                if Some(id) == self.term_proc {
                    push_text(&mut self.term_lines, &text, kind);
                    self.term_scroll.scroll_to_bottom();
                } else if Some(id) == self.out_proc {
                    push_text(&mut self.out_lines, &text, kind);
                    self.out_scroll.scroll_to_bottom();
                } else if self.agent.owns(id) {
                    self.agent_on_output(&text, err, cx);
                }
            }
            ProcEvent::Exit { id, code } => {
                self.procs.forget(id);
                let msg = match code {
                    Some(0) => None,
                    Some(c) => Some(format!("exit code {c}")),
                    None => Some("stopped".to_string()),
                };
                if Some(id) == self.term_proc {
                    self.term_proc = None;
                    close_line(&mut self.term_lines);
                    if let Some(m) = msg {
                        push_line(&mut self.term_lines, &m, LineKind::Info);
                    }
                    self.term_scroll.scroll_to_bottom();
                } else if Some(id) == self.out_proc {
                    self.out_proc = None;
                    close_line(&mut self.out_lines);
                    push_line(&mut self.out_lines, &format!("[Done] {}", msg.as_deref().unwrap_or("exit code 0")), LineKind::Info);
                    self.out_scroll.scroll_to_bottom();
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

    fn render_title_bar(&self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let folder = self
            .root
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "FluxCode".into());
        TitleBar::new().child(
            h_flex()
                .w_full()
                .pr_2()
                .items_center()
                .gap_1()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(div().px_1().text_color(c(theme::ACCENT)).font_weight(FontWeight::BOLD).child("⚡"))
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
                .child(Self::render_menu("m-help", "Help", |m| m.menu("About FluxCode", Box::new(About))))
                .child(div().flex_1())
                .child(
                    h_flex()
                        .w(px(380.))
                        .h(px(24.))
                        .justify_center()
                        .rounded(px(6.))
                        .bg(c(theme::BG_ELEVATED))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .gap_2()
                        .child(Icon::new(IconName::Search).xsmall())
                        .child(folder),
                )
                .child(div().flex_1()),
        )
    }

    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, icon: IconName, view: SideView, this: &Self, cx: &mut Context<Self>| {
            let active = this.sidebar_open && this.view == view;
            div()
                .id(id)
                .w_full()
                .h(px(44.))
                .flex()
                .items_center()
                .justify_center()
                .border_l_2()
                .border_color(if active { c(theme::FG_STRONG) } else { transparent_black() })
                .text_color(if active { c(theme::FG_STRONG) } else { c(theme::FG_DIM) })
                .hover(|s| s.text_color(c(theme::FG_STRONG)))
                .child(Icon::new(icon).size(px(22.)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.view == view && this.sidebar_open {
                        this.sidebar_open = false;
                    } else {
                        this.view = view;
                        this.sidebar_open = true;
                    }
                    cx.notify();
                }))
        };
        v_flex()
            .w(px(48.))
            .h_full()
            .bg(c(theme::BG_TITLE))
            .border_r_1()
            .border_color(c(theme::BORDER))
            .child(item("ab-explorer", IconName::FolderOpen, SideView::Explorer, self, cx))
            .child(item("ab-search", IconName::Search, SideView::Search, self, cx))
            .child(item("ab-run", IconName::Play, SideView::Run, self, cx))
            .child(item("ab-agent", IconName::Bot, SideView::Agent, self, cx))
            .child(div().flex_1())
            .child(
                div()
                    .id("ab-settings")
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(if self.settings_open { c(theme::FG_STRONG) } else { c(theme::FG_DIM) })
                    .hover(|s| s.text_color(c(theme::FG_STRONG)))
                    .child(Icon::new(IconName::Settings).size(px(22.)))
                    .on_click(cx.listener(|this, _, w, cx| this.on_show_settings(&ShowSettings, w, cx))),
            )
    }

    fn render_side_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let title = match self.view {
            SideView::Explorer => "EXPLORER",
            SideView::Search => "SEARCH",
            SideView::Run => "RUN",
            SideView::Agent => "OPENCODE",
        };
        let header = h_flex()
            .h(px(35.))
            .px_4()
            .gap_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(theme::FG_MUTED))
            .child(div().flex_1().child(title))
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
        v_flex().size_full().bg(c(theme::BG_SIDE)).child(header).child(div().flex_1().min_h_0().child(body)).into_any_element()
    }

    fn render_run_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let file = self.active.map(|a| self.docs[a].name.to_string());
        v_flex()
            .p_3()
            .gap_3()
            .text_sm()
            .child(
                Button::new("run-file")
                    .primary()
                    .label(match &file {
                        Some(f) => format!("Run {f}"),
                        None => "Run File".into(),
                    })
                    .disabled(file.is_none())
                    .on_click(cx.listener(|this, _, w, cx| this.on_run_file(&RunFile, w, cx))),
            )
            .when(self.out_proc.is_some(), |v| {
                v.child(Button::new("stop-run").label("Stop").on_click(cx.listener(|this, _, w, cx| this.on_stop_run(&StopRun, w, cx))))
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
            .h(px(35.))
            .w_full()
            .flex_none()
            .bg(c(theme::BG_SIDE))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .overflow_x_scroll()
            .children(self.docs.iter().enumerate().map(|(ix, d)| {
                let active = self.active == Some(ix);
                let (txt, col) = files::badge(&d.name);
                h_flex()
                    .id(("tab", ix))
                    .h_full()
                    .px_3()
                    .gap_2()
                    .flex_none()
                    .text_sm()
                    .border_r_1()
                    .border_color(c(theme::BORDER))
                    .relative()
                    .when(active, |s| {
                        s.bg(c(theme::BG_EDITOR))
                            .child(div().absolute().top_0().left_0().right_0().h(px(1.)).bg(c(theme::ACCENT)))
                    })
                    .text_color(if active { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                    .child(div().text_size(px(9.)).font_weight(FontWeight::BOLD).text_color(c(col)).child(txt))
                    .child(d.name.clone())
                    .child(
                        div()
                            .id(("tab-close", ix))
                            .w(px(16.))
                            .flex()
                            .justify_center()
                            .rounded(px(3.))
                            .text_color(c(theme::FG_MUTED))
                            .hover(|s| s.bg(c(theme::BG_ELEVATED)))
                            .child(if d.dirty { "●" } else { "×" })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close(ix, window, cx);
                            })),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, window, cx)))
            }))
    }

    fn render_editor_area(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.settings_open {
            return self.render_settings(window, cx);
        }
        match self.active {
            Some(ix) => v_flex()
                .size_full()
                .child(self.render_tabs(cx))
                .child(
                    div().flex_1().min_h_0().bg(c(theme::BG_EDITOR)).child(
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
                .gap_2()
                .bg(c(theme::BG_EDITOR))
                .child(div().text_size(px(64.)).text_color(c(0x26282d)).font_weight(FontWeight::BOLD).child("FluxCode"))
                .child(shortcut("Open Folder", "Ctrl+K Ctrl+O"))
                .child(shortcut("Open File", "Ctrl+O"))
                .child(shortcut("Find in Files", "Ctrl+Shift+F"))
                .child(shortcut("Toggle Terminal", "Ctrl+`"))
                .child(shortcut("Run File", "F5"))
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
        let item = |s: SharedString| div().px_2().child(s);
        StatusBar::new()
            .left(
                div()
                    .id("sb-panel")
                    .px_2()
                    .child(if self.out_proc.is_some() { "▶ running" } else { "⌨ Terminal" })
                    .on_click(cx.listener(|this, _, w, cx| this.on_toggle_terminal(&ToggleTerminal, w, cx))),
            )
            .left(item(self.status.clone()))
            .right(item(pos.into()))
            .right(item(crlf.into()))
            .right(item(if lang.is_empty() { "".into() } else { "UTF-8".into() }))
            .right(item(lang.into()))
            .right(item("GPUI".into()))
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
        .w(px(300.))
        .text_sm()
        .child(div().flex_1().text_color(c(theme::FG_DIM)).child(label))
        .child(div().text_color(c(theme::FG_MUTED)).child(keys))
}

/// Keep at most this many lines per panel.
const MAX_LINES: usize = 10_000;

pub(crate) fn push_line(lines: &mut Vec<Line>, text: &str, kind: LineKind) {
    close_line(lines);
    lines.push(Line { text: text.to_string().into(), kind, open: false });
    trim(lines);
}

pub(crate) fn close_line(lines: &mut [Line]) {
    if let Some(l) = lines.last_mut() {
        l.open = false;
    }
}

/// Append streamed output, continuing an unfinished last line.
pub(crate) fn push_text(lines: &mut Vec<Line>, text: &str, kind: LineKind) {
    let clean = crate::proc::strip_ansi(text);
    let mut parts = clean.split('\n').peekable();
    while let Some(part) = parts.next() {
        let last_piece = parts.peek().is_none();
        match lines.last_mut() {
            Some(l) if l.open && l.kind == kind => l.text = format!("{}{}", l.text, part).into(),
            _ if part.is_empty() && last_piece => break,
            _ => lines.push(Line { text: part.to_string().into(), kind, open: true }),
        }
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
        let main = h_flex().flex_1().min_h_0().child(self.render_activity_bar(cx)).child(if self.sidebar_open {
            h_resizable("workbench-split")
                .child(resizable_panel().size(px(260.)).size_range(px(160.)..px(600.)).child(self.render_side_bar(window, cx)))
                .child(resizable_panel().child(editor_and_panel))
                .into_any_element()
        } else {
            editor_and_panel
        });
        v_flex()
            .id("workbench")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .text_color(c(theme::FG))
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
