//! The workbench: title bar, activity bar, side bar (explorer), editor tabs
//! and status bar — the same layout as the Tauri FluxCode.

use crate::files;
use crate::theme::{self, c};
use gpui_kit::component::{
    h_flex,
    input::{Editor, EditorState, InputEvent, TabSize},
    list::ListItem,
    resizable::{h_resizable, resizable_panel},
    status_bar::StatusBar,
    tree::{tree, TreeState},
    v_flex, ActiveTheme, Icon, IconName, Sizable, TitleBar,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};

actions!(fluxcode, [Save, CloseTab, OpenFolder, ToggleSidebar, NextTab, PrevTab]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-s", Save, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("ctrl-k ctrl-o", OpenFolder, None),
        KeyBinding::new("ctrl-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SideView {
    Explorer,
    Search,
    Debug,
    Agent,
}

struct Doc {
    path: PathBuf,
    name: SharedString,
    lang: &'static str,
    editor: Entity<EditorState>,
    dirty: bool,
    _sub: Subscription,
}

pub struct Workbench {
    root: Option<PathBuf>,
    tree: Entity<TreeState>,
    docs: Vec<Doc>,
    active: Option<usize>,
    view: SideView,
    sidebar_open: bool,
    focus: FocusHandle,
    status: SharedString,
}

impl Workbench {
    pub fn new(root: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut this = Self {
            root: None,
            tree,
            docs: Vec::new(),
            active: None,
            view: SideView::Explorer,
            sidebar_open: true,
            focus,
            status: SharedString::default(),
        };
        if let Some(root) = root {
            this.open_folder(root, cx);
        }
        this
    }

    fn open_folder(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(root.clone());
        let tree = self.tree.clone();
        // Walk the folder off the UI thread, then hand the items to the tree.
        cx.spawn(async move |_, cx| {
            let nodes = cx.background_spawn(async move { files::scan(&root) }).await;
            _ = tree.update(cx, |state, cx| state.set_items(files::to_items(nodes), cx));
        })
        .detach();
        cx.notify();
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.docs.iter().position(|d| d.path == path) {
            self.activate(ix, window, cx);
            return;
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("Cannot open {}: {e}", path.display()).into();
                cx.notify();
                return;
            }
        };
        if bytes.iter().take(8000).any(|&b| b == 0) {
            self.status = format!("{} is a binary file", path.display()).into();
            cx.notify();
            return;
        }
        let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
        let lang = files::language_for(&path);
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(lang)
                .line_number(true)
                .indent_guides(true)
                .tab_size(TabSize { tab_size: 4, hard_tabs: false })
                .soft_wrap(false)
                .default_value(text)
        });
        let sub = cx.subscribe(&editor, |this, editor, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                if let Some(d) = this.docs.iter_mut().find(|d| d.editor == editor) {
                    d.dirty = true;
                }
            }
            cx.notify();
        });
        let name: SharedString = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
        self.docs.push(Doc { path, name, lang, editor, dirty: false, _sub: sub });
        self.activate(self.docs.len() - 1, window, cx);
    }

    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = Some(ix);
        let handle = self.docs[ix].editor.focus_handle(cx);
        handle.focus(window, cx);
        cx.notify();
    }

    fn close(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
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
    }

    // ----------------------------------------------------------- actions

    fn on_save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else { return };
        let doc = &mut self.docs[ix];
        let text = doc.editor.read(cx).value();
        match std::fs::write(&doc.path, text.as_bytes()) {
            Ok(()) => {
                doc.dirty = false;
                self.status = format!("Saved {}", doc.name).into();
            }
            Err(e) => self.status = format!("Save failed: {e}").into(),
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

    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
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

    // ------------------------------------------------------------ render

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
                .gap_3()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(div().text_color(c(theme::ACCENT)).font_weight(FontWeight::BOLD).child("⚡"))
                .children(["File", "Edit", "Selection", "View", "Run", "Terminal", "Help"].map(|m| div().child(m)))
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
            .child(item("ab-debug", IconName::Play, SideView::Debug, self, cx))
            .child(item("ab-agent", IconName::Bot, SideView::Agent, self, cx))
            .child(div().flex_1())
            .child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c(theme::FG_DIM))
                    .child(Icon::new(IconName::Settings).size(px(22.))),
            )
    }

    fn render_side_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = match self.view {
            SideView::Explorer => "EXPLORER",
            SideView::Search => "SEARCH",
            SideView::Debug => "RUN AND DEBUG",
            SideView::Agent => "OPENCODE",
        };
        let header = h_flex()
            .h(px(35.))
            .px_4()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(theme::FG_MUTED))
            .child(title);
        let body: AnyElement = match self.view {
            SideView::Explorer if self.root.is_some() => self.render_explorer(cx).into_any_element(),
            SideView::Explorer => v_flex()
                .p_4()
                .gap_3()
                .text_sm()
                .text_color(c(theme::FG_MUTED))
                .child("You have not yet opened a folder.")
                .child(
                    div()
                        .id("open-folder")
                        .px_3()
                        .py_1()
                        .rounded(px(4.))
                        .bg(c(theme::ACCENT))
                        .text_color(c(theme::FG_STRONG))
                        .child("Open Folder")
                        .on_click(cx.listener(|this, _, window, cx| this.on_open_folder(&OpenFolder, window, cx))),
                )
                .into_any_element(),
            _ => div()
                .p_4()
                .text_sm()
                .text_color(c(theme::FG_DIM))
                .child("Not ported to the GPUI edition yet.")
                .into_any_element(),
        };
        v_flex().size_full().bg(c(theme::BG_SIDE)).child(header).child(div().flex_1().min_h_0().child(body)).into_any_element()
    }

    fn render_explorer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let active_path = self.active.map(|a| self.docs[a].path.to_string_lossy().into_owned());
        tree(&self.tree, move |ix, entry, _selected, _window, _cx| {
            let item = entry.item();
            let is_open = active_path.as_deref() == Some(item.id.as_ref());
            let lead: AnyElement = if entry.is_folder() {
                h_flex()
                    .gap_1()
                    .text_color(c(theme::FOLDER))
                    .child(
                        Icon::new(if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight })
                            .xsmall()
                            .text_color(c(theme::FG_DIM)),
                    )
                    .child(Icon::new(if entry.is_expanded() { IconName::FolderOpen } else { IconName::Folder }).small())
                    .into_any_element()
            } else {
                let (txt, col) = files::badge(&item.label);
                div()
                    .ml(px(16.))
                    .w(px(22.))
                    .text_size(px(9.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(col))
                    .child(txt)
                    .into_any_element()
            };
            ListItem::new(ix)
                .w_full()
                .h(px(22.))
                .px_1()
                .pl(px(12.) * entry.depth() + px(6.))
                .when(is_open, |s| s.bg(c(theme::SELECT)))
                .child(h_flex().gap_1().text_color(c(theme::FG)).child(lead).child(item.label.clone()))
                .on_click({
                    let view = view.clone();
                    let item = item.clone();
                    move |_, window, cx| {
                        if !item.is_folder() {
                            view.update(cx, |this, cx| this.open_file(PathBuf::from(item.id.as_ref()), window, cx));
                        }
                    }
                })
        })
        .text_sm()
        .size_full()
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("tabs")
            .h(px(35.))
            .w_full()
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

    fn render_editor_area(&self, cx: &mut Context<Self>) -> AnyElement {
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
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(cx.theme().mono_font_size),
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
                .child(shortcut("Toggle Sidebar", "Ctrl+B"))
                .child(shortcut("Save", "Ctrl+S"))
                .child(shortcut("Close Tab", "Ctrl+W"))
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
        let item = |s: SharedString| div().px_2().child(s);
        StatusBar::new()
            .left(item(self.status.clone()))
            .right(item(pos.into()))
            .right(item(if lang.is_empty() { "".into() } else { "UTF-8".into() }))
            .right(item(lang.into()))
            .right(item("GPUI".into()))
    }
}

fn shortcut(label: &'static str, keys: &'static str) -> impl IntoElement {
    h_flex()
        .w(px(300.))
        .text_sm()
        .child(div().flex_1().text_color(c(theme::FG_DIM)).child(label))
        .child(div().text_color(c(theme::FG_MUTED)).child(keys))
}

impl Focusable for Workbench {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = h_flex().flex_1().min_h_0().child(self.render_activity_bar(cx)).child(if self.sidebar_open {
            h_resizable("workbench-split")
                .child(resizable_panel().size(px(260.)).size_range(px(160.)..px(600.)).child(self.render_side_bar(cx)))
                .child(resizable_panel().child(self.render_editor_area(cx)))
                .into_any_element()
        } else {
            self.render_editor_area(cx)
        });
        v_flex()
            .id("workbench")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .text_color(c(theme::FG))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_open_folder))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .child(self.render_title_bar(window, cx))
            .child(main)
            .child(self.render_status_bar(cx))
    }
}
