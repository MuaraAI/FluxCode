use crate::core;
use crate::files::{self, Node};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    menu::ContextMenuExt,
    scroll::ScrollableElement,
    v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) const PENDING_ROW_ID: &str = "\u{1}pending";
const LOADING_SUFFIX: &str = "\u{1}loading";
const EMPTY_SUFFIX: &str = "\u{1}empty";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreeRow {
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
    pub loading: bool,
    pub is_pending: bool,
    pub is_placeholder: bool,
    pub parent_id: Option<String>,
}

pub(crate) struct ExplorerState {
    pub rows: Vec<TreeRow>,
    pub scroll: UniformListScrollHandle,
    pub focus: FocusHandle,
    pub gen: u64,
    pub loading: HashSet<String>,
    pub revealed_tab: Option<PathBuf>,
    pub folder_anim: Option<(String, bool, u64)>,
    pub folder_anim_seq: u64,
    pub workspace_anim: u64,
    pub added_roots: HashSet<String>,
    pub hovered_ix: Option<usize>,
    pub hover_from_y: f32,
    pub hover_to_y: f32,
    pub hover_seq: u64,
    pub hover_fade_in: bool,
    pub hover_leave: Option<(f32, u64)>,
    pub hover_last_at: Option<std::time::Instant>,
}

impl ExplorerState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            rows: Vec::new(),
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            gen: 0,
            loading: HashSet::new(),
            revealed_tab: None,
            folder_anim: None,
            folder_anim_seq: 0,
            workspace_anim: 0,
            added_roots: HashSet::new(),
            hovered_ix: None,
            hover_from_y: 0.,
            hover_to_y: 0.,
            hover_seq: 0,
            hover_fade_in: false,
            hover_leave: None,
            hover_last_at: None,
        }
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|r| !r.is_placeholder && !r.is_pending && r.id == id)
    }

    pub fn mark_folder_anim(&mut self, id: String, expanding: bool) {
        self.folder_anim_seq = self.folder_anim_seq.wrapping_add(1);
        self.folder_anim = Some((id, expanding, self.folder_anim_seq));
    }

    pub fn bump_workspace_anim(&mut self, added: Vec<String>) {
        self.workspace_anim = self.workspace_anim.wrapping_add(1);
        self.added_roots = added.into_iter().collect();
    }
}

fn stray_press_keeps_input(pending: &Option<PendingOp>, typed: &str) -> bool {
    matches!(
        pending,
        Some(PendingOp::NewFile(_)) | Some(PendingOp::NewFolder(_)) | Some(PendingOp::Rename(_))
    ) && !typed.trim().is_empty()
}

pub(crate) fn flatten_nodes(
    nodes: &[Node],
    expanded: &HashSet<String>,
    loading: &HashSet<String>,
    pending: Option<&PendingOp>,
    root: Option<&Path>,
    dirs: &mut HashSet<String>,
) -> Vec<TreeRow> {
    let pending_dir = match pending {
        Some(PendingOp::NewFile(d) | PendingOp::NewFolder(d)) => Some(d.to_string_lossy().into_owned()),
        _ => None,
    };
    let mut rows = Vec::with_capacity(nodes.len().saturating_mul(2));
    let mut inserted_pending = false;

    #[allow(clippy::too_many_arguments)]
    fn push_level(
        nodes: &[Node],
        depth: usize,
        parent_id: Option<&str>,
        expanded: &HashSet<String>,
        loading: &HashSet<String>,
        pending_dir: Option<&str>,
        inserted_pending: &mut bool,
        dirs: &mut HashSet<String>,
        out: &mut Vec<TreeRow>,
    ) {
        for n in nodes {
            let is_dir = n.children.is_some();
            if is_dir {
                dirs.insert(n.id.clone());
            }
            let is_expanded = is_dir && expanded.contains(&n.id);
            let is_loading = is_dir && (loading.contains(&n.id) || (is_expanded && !n.loaded));
            out.push(TreeRow {
                id: n.id.clone(),
                label: n.name.clone(),
                depth,
                is_dir,
                expanded: is_expanded,
                loading: is_loading,
                is_pending: false,
                is_placeholder: false,
                parent_id: parent_id.map(str::to_string),
            });
            if is_expanded {
                if pending_dir == Some(n.id.as_str()) {
                    *inserted_pending = true;
                    out.push(TreeRow {
                        id: PENDING_ROW_ID.to_string(),
                        label: String::new(),
                        depth: depth + 1,
                        is_dir: false,
                        expanded: false,
                        loading: false,
                        is_pending: true,
                        is_placeholder: false,
                        parent_id: Some(n.id.clone()),
                    });
                }
                if !n.loaded {
                    out.push(TreeRow {
                        id: format!("{}{}", n.id, LOADING_SUFFIX),
                        label: tr("Loading…").to_string(),
                        depth: depth + 1,
                        is_dir: false,
                        expanded: false,
                        loading: true,
                        is_pending: false,
                        is_placeholder: true,
                        parent_id: Some(n.id.clone()),
                    });
                } else if let Some(ch) = &n.children {
                    if ch.is_empty() && pending_dir != Some(n.id.as_str()) {
                        out.push(TreeRow {
                            id: format!("{}{}", n.id, EMPTY_SUFFIX),
                            label: tr("Empty folder").to_string(),
                            depth: depth + 1,
                            is_dir: false,
                            expanded: false,
                            loading: false,
                            is_pending: false,
                            is_placeholder: true,
                            parent_id: Some(n.id.clone()),
                        });
                    } else {
                        push_level(
                            ch,
                            depth + 1,
                            Some(&n.id),
                            expanded,
                            loading,
                            pending_dir,
                            inserted_pending,
                            dirs,
                            out,
                        );
                    }
                }
            }
        }
    }

    push_level(
        nodes,
        0,
        None,
        expanded,
        loading,
        pending_dir.as_deref(),
        &mut inserted_pending,
        dirs,
        &mut rows,
    );

    if !inserted_pending {
        if let (Some(dir), Some(r)) = (pending_dir.as_deref(), root) {
            if dir == r.to_string_lossy() {
                rows.insert(
                    0,
                    TreeRow {
                        id: PENDING_ROW_ID.to_string(),
                        label: String::new(),
                        depth: 0,
                        is_dir: false,
                        expanded: false,
                        loading: false,
                        is_pending: true,
                        is_placeholder: false,
                        parent_id: None,
                    },
                );
            }
        }
    }
    rows
}

fn is_descendant_of(path: &str, ancestor: &str) -> bool {
    if path.len() <= ancestor.len() || !path.starts_with(ancestor) {
        return false;
    }
    let next = path.as_bytes()[ancestor.len()];
    next == b'\\' || next == b'/' || next == 1
}

impl Workbench {
    fn explorer_row_y(&self, ix: usize) -> f32 {
        let row_h = f32::from(theme::u(20.));
        let scroll_y = f32::from(self.explorer.scroll.0.borrow().base_handle.offset().y);
        ix as f32 * row_h + scroll_y
    }

    fn explorer_hover_row(&mut self, ix: usize, cx: &mut Context<Self>) {
        let target_y = self.explorer_row_y(ix);
        if self.explorer.hovered_ix == Some(ix) && (self.explorer.hover_to_y - target_y).abs() < 0.5 {
            return;
        }
        let gliding = self.explorer.hovered_ix.is_some()
            || self.explorer.hover_last_at.is_some_and(|t| t.elapsed() < Duration::from_millis(140));
        self.explorer.hover_from_y = if gliding { self.explorer.hover_to_y } else { target_y };
        self.explorer.hover_to_y = target_y;
        self.explorer.hovered_ix = Some(ix);
        self.explorer.hover_fade_in = !gliding;
        self.explorer.hover_leave = None;
        self.explorer.hover_last_at = Some(std::time::Instant::now());
        self.explorer.hover_seq = self.explorer.hover_seq.wrapping_add(1);
        cx.notify();
    }

    fn explorer_hover_clear(&mut self, cx: &mut Context<Self>) {
        if self.explorer.hovered_ix.take().is_none() {
            return;
        }
        self.explorer.hover_last_at = Some(std::time::Instant::now());
        if cx.reduce_motion() {
            self.explorer.hover_leave = None;
            cx.notify();
            return;
        }
        self.explorer.hover_seq = self.explorer.hover_seq.wrapping_add(1);
        let seq = self.explorer.hover_seq;
        let last_y = self.explorer.hover_to_y;
        self.explorer.hover_leave = Some((last_y, seq));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(130)).await;
            _ = this.update(cx, |this, cx| {
                if this.explorer.hover_leave.is_some_and(|(_, s)| s == seq) {
                    this.explorer.hover_leave = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn rebuild_explorer_rows(&mut self, cx: &mut Context<Self>) {
        let mut dirs = HashSet::new();
        self.explorer.rows = flatten_nodes(
            &self.tree_nodes,
            &self.expanded,
            &self.explorer.loading,
            self.pending.as_ref(),
            self.root.as_deref(),
            &mut dirs,
        );
        if let Some(h) = self.explorer.hovered_ix {
            if h >= self.explorer.rows.len() {
                self.explorer.hovered_ix = None;
            }
        }
        self.dirs = std::rc::Rc::new(dirs);
        cx.notify();
    }

    pub(crate) fn toggle_folder(&mut self, id: String, cx: &mut Context<Self>) {
        if self.expanded.contains(&id) {
            self.expanded.remove(&id);
            self.explorer.loading.remove(&id);
            self.explorer.mark_folder_anim(id, false);
            self.rebuild_explorer_rows(cx);
        } else {
            self.expand_folder(id, true, cx);
        }
    }

    pub(crate) fn expand_folder(&mut self, id: String, animate: bool, cx: &mut Context<Self>) {
        self.expanded.insert(id.clone());
        if animate {
            self.explorer.mark_folder_anim(id.clone(), true);
        }
        let needs_load = files::find_node(&mut self.tree_nodes, &id).is_some_and(|n| !n.loaded);
        if !needs_load {
            self.rebuild_explorer_rows(cx);
            return;
        }
        if !self.explorer.loading.insert(id.clone()) {
            self.rebuild_explorer_rows(cx);
            return;
        }
        self.rebuild_explorer_rows(cx);
        let gen = self.explorer.gen;
        let wsl = self.wsl.clone();
        let path = PathBuf::from(&id);
        cx.spawn(async move |this, cx| {
            let children = cx
                .background_spawn(async move {
                    if let Some(t) = &wsl {
                        if let Some(nodes) = crate::workbench::remote_dir_pub(t, &path) {
                            return nodes;
                        }
                    }
                    files::scan_dir(&path)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.explorer.loading.remove(&id);
                if this.explorer.gen != gen {
                    return;
                }
                if let Some(node) = files::find_node(&mut this.tree_nodes, &id) {
                    node.children = Some(children);
                    node.loaded = true;
                }
                if this.expanded.contains(&id) {
                    this.explorer.mark_folder_anim(id, true);
                }
                this.rebuild_explorer_rows(cx);
            });
        })
        .detach();
    }

    pub(crate) fn reveal_active_in_explorer(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.active.and_then(|a| self.docs.get(a)).map(|d| d.path.clone()) else {
            return;
        };
        if self.explorer.revealed_tab.as_ref() == Some(&path) {
            return;
        }
        let roots = self.roots();
        if !roots.iter().any(|r| path.starts_with(r)) {
            return;
        }
        self.explorer.revealed_tab = Some(path.clone());
        let id = path.to_string_lossy().into_owned();
        if let Some(ix) = self.explorer.index_of(&id) {
            self.explorer.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            return;
        }
        let ancestors = files::ancestor_dirs(&path, &roots);
        if ancestors.is_empty() {
            return;
        }
        for dir in &ancestors {
            self.expanded.insert(dir.clone());
        }
        if files::contains_node(&self.tree_nodes, &id) {
            self.rebuild_explorer_rows(cx);
            if let Some(ix) = self.explorer.index_of(&id) {
                self.explorer.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            }
            return;
        }
        let gen = self.explorer.gen;
        let wsl = self.wsl.clone();
        let to_load = ancestors.clone();
        cx.spawn(async move |this, cx| {
            let loaded: Vec<(String, Vec<Node>)> = cx
                .background_spawn(async move {
                    to_load
                        .into_iter()
                        .map(|d| {
                            let p = PathBuf::from(&d);
                            let kids = wsl
                                .as_ref()
                                .and_then(|t| crate::workbench::remote_dir_pub(t, &p))
                                .unwrap_or_else(|| files::scan_dir(&p));
                            (d, kids)
                        })
                        .collect()
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if this.explorer.gen != gen {
                    return;
                }
                for (dir, kids) in loaded {
                    if let Some(node) = files::find_node(&mut this.tree_nodes, &dir) {
                        if !node.loaded {
                            node.children = Some(kids);
                            node.loaded = true;
                        }
                    }
                }
                this.rebuild_explorer_rows(cx);
                if let Some(ix) = this.explorer.index_of(&id) {
                    this.explorer.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
                }
            });
        })
        .detach();
    }

    pub(crate) fn on_explorer_up(&mut self, _: &ExplorerUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_explorer_selection(-1, cx);
    }

    pub(crate) fn on_explorer_down(&mut self, _: &ExplorerDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_explorer_selection(1, cx);
    }

    fn move_explorer_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = &self.explorer.rows;
        if rows.is_empty() {
            return;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|p| self.explorer.index_of(&p.to_string_lossy()));
        let mut ix = match cur {
            Some(i) => (i as isize + delta).clamp(0, rows.len() as isize - 1) as usize,
            None => 0,
        };
        while rows[ix].is_placeholder || rows[ix].is_pending {
            let next = ix as isize + delta;
            if next < 0 || next >= rows.len() as isize {
                return;
            }
            ix = next as usize;
        }
        self.selected = Some(PathBuf::from(&rows[ix].id));
        self.explorer.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        cx.notify();
    }

    pub(crate) fn on_explorer_left(&mut self, _: &ExplorerLeft, _: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned()) else { return };
        let Some(ix) = self.explorer.index_of(&sel) else { return };
        let row = self.explorer.rows[ix].clone();
        if row.is_dir && row.expanded {
            self.toggle_folder(row.id, cx);
        } else if let Some(parent) = row.parent_id {
            self.selected = Some(PathBuf::from(&parent));
            if let Some(pix) = self.explorer.index_of(&parent) {
                self.explorer.scroll.scroll_to_item(pix, ScrollStrategy::Nearest);
            }
            cx.notify();
        }
    }

    pub(crate) fn on_explorer_right(&mut self, _: &ExplorerRight, _: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned()) else { return };
        let Some(ix) = self.explorer.index_of(&sel) else { return };
        let row = self.explorer.rows[ix].clone();
        if row.is_dir && !row.expanded {
            self.expand_folder(row.id, true, cx);
        } else if row.is_dir && ix + 1 < self.explorer.rows.len() {
            let next = &self.explorer.rows[ix + 1];
            if !next.is_placeholder && !next.is_pending {
                self.selected = Some(PathBuf::from(&next.id));
                self.explorer.scroll.scroll_to_item(ix + 1, ScrollStrategy::Nearest);
                cx.notify();
            }
        }
    }

    pub(crate) fn on_explorer_open(&mut self, _: &ExplorerOpen, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sel) = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned()) else { return };
        let Some(ix) = self.explorer.index_of(&sel) else { return };
        let row = self.explorer.rows[ix].clone();
        if row.is_dir {
            self.toggle_folder(row.id, cx);
        } else {
            self.agent.user_nav = Some(std::time::Instant::now());
            self.agent_full = false;
            self.open_file(PathBuf::from(row.id), window, cx);
        }
    }

    pub(crate) fn render_explorer(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.root.is_none() {
            return v_flex()
                .p_4()
                .gap_3()
                .text_sm()
                .text_color(c(theme::FG_MUTED))
                .child(tr("You have not yet opened a folder."))
                .child(self.action_button("x-open-folder", tr("Open folder"), crate::ui::Kind::Primary, cx, |_, w, cx| {
                    w.dispatch_action(Box::new(OpenFolder), cx)
                }))
                .into_any_element();
        }
        let active_path = self.active.and_then(|a| self.docs.get(a)).map(|d| d.path.to_string_lossy().into_owned());
        let selected = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned());
        let root = self.root.clone();
        let root_target = self.ctx_target.clone();
        let focused = self.explorer.focus.is_focused(window);
        let count = self.explorer.rows.len();
        let motion = !cx.reduce_motion();
        let collapsed_row_ix = self
            .explorer
            .folder_anim
            .as_ref()
            .filter(|(_, exp, _)| !*exp)
            .and_then(|(fid, _, _)| self.explorer.index_of(fid));

        let list = uniform_list(
            "explorer-rows",
            count,
            cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                let view = cx.entity();
                let multi = !this.workspace.extra.is_empty();
                let extra = this.workspace.extra.clone();
                let pending = this.pending.clone();
                let name_input = this.name_input.clone();
                let marks = this.scm.marks.clone();
                let dirty_dirs = this.scm.dirty_dirs.clone();
                let folder_anim = this.explorer.folder_anim.clone();
                let jump_anim = this
                    .file_jump_anim
                    .as_ref()
                    .map(|(p, s)| (p.to_string_lossy().into_owned(), *s));
                let ws_anim = this.explorer.workspace_anim;
                let added_roots = this.explorer.added_roots.clone();

                range
                    .map(|ix| {
                        let row = &this.explorer.rows[ix];
                        let id = row.id.clone();

                        if row.is_pending {
                            let icon = match &pending {
                                Some(PendingOp::NewFolder(_)) => Icon::new(files::folder_icon("", true))
                                    .size(theme::u(13.))
                                    .text_color(c(theme::FOLDER)),
                                _ => {
                                    let typed = name_input.read(cx).value();
                                    if typed.is_empty() {
                                        Icon::new(IconName::File).size(theme::u(13.)).text_color(c(theme::FG_MUTED))
                                    } else {
                                        let (ic, tint) = files::icon_for(&typed);
                                        Icon::new(ic).size(theme::u(13.)).text_color(theme::tint(tint))
                                    }
                                }
                            };
                            return name_row(PENDING_ROW_ID, row.depth, icon, &name_input).into_any_element();
                        }

                        if row.is_placeholder {
                            let pl = theme::u(10.) * row.depth + theme::u(22.);
                            return h_flex()
                                .id(SharedString::from(format!("xp-{}", id)))
                                .w_full()
                                .h(theme::u(20.))
                                .pl(pl)
                                .pr_2()
                                .gap_1()
                                .items_center()
                                .text_size(theme::u(11.))
                                .text_color(c(theme::FG_DIM))
                                .child(row.label.clone())
                                .into_any_element();
                        }

                        if let Some(PendingOp::Rename(p)) = &pending {
                            if p.to_string_lossy() == id {
                                let icon = if row.is_dir {
                                    Icon::new(files::folder_icon(&row.label, row.expanded))
                                        .size(theme::u(13.))
                                        .text_color(c(theme::FOLDER))
                                } else {
                                    let typed = name_input.read(cx).value();
                                    let name: &str = if typed.is_empty() { &row.label } else { &typed };
                                    let (ic, tint) = files::icon_for(name);
                                    Icon::new(ic).size(theme::u(13.)).text_color(theme::tint(tint))
                                };
                                return name_row(&id, row.depth, icon, &name_input).into_any_element();
                            }
                        }

                        if let Some(PendingOp::Delete(p)) = &pending {
                            if p.to_string_lossy() == id {
                                return delete_row(&id, row.depth, p, &view).into_any_element();
                            }
                        }

                        let naming = pending.is_some();
                        let is_open = active_path.as_deref() == Some(id.as_str());
                        let is_sel = !naming && selected.as_deref() == Some(id.as_str());
                        let is_dir = row.is_dir;
                        let expanded = row.expanded;
                        let chevron = div().w(theme::u(12.)).flex().items_center().justify_center().when(is_dir, |d| {
                            d.child(
                                Icon::new(if expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                                    .xsmall()
                                    .text_color(c(theme::FG_DIM)),
                            )
                        });
                        let icon = if is_dir {
                            Icon::new(files::folder_icon(&row.label, expanded))
                                .size(theme::u(13.))
                                .text_color(c(theme::FOLDER))
                        } else {
                            let (ic, tint) = files::icon_for(&row.label);
                            Icon::new(ic).size(theme::u(13.)).text_color(theme::tint(tint))
                        };
                        let key = id.replace('/', "\\").to_lowercase();
                        let mark = if is_dir { None } else { marks.get(&key).copied() };
                        let dir_dirty = is_dir && dirty_dirs.contains(&key);
                        let label_color = match mark {
                            Some(m) => c(crate::scm::code_color(m)),
                            None if dir_dirty => c(theme::WARN),
                            None => c(theme::FG),
                        };
                        let base_pl = f32::from(theme::u(10.) * row.depth + theme::u(6.));
                        let target = this.ctx_target.clone();
                        let extra_ctx = extra.clone();
                        let row_id_click = id.clone();
                        let row_id_rclick = id.clone();
                        let row_id_ctx = id.clone();
                        let view_l = view.clone();
                        let view_md = view.clone();
                        let view_r = view.clone();
                        let view_hov = view.clone();
                        let focus_l = this.explorer.focus.clone();

                        let row_el = h_flex()
                            .id(SharedString::from(format!("xr-{}", id)))
                            .w_full()
                            .h(theme::u(20.))
                            .pl(px(base_pl))
                            .pr_2()
                            .gap_1()
                            .items_center()
                            .cursor_pointer()
                            .text_size(theme::u(11.5))
                            .text_color(label_color)
                            .when(is_open || is_sel, |s| s.bg(c(theme::SELECT)))
                            .when(focused && is_sel, |s| s.border_l_2().border_color(c(theme::ACCENT)))
                            .when(!motion, |s| s.hover(|s| if is_open || is_sel { s } else { s.bg(c(theme::BG_HOVER)) }))
                            .on_hover(move |entered: &bool, _, cx| {
                                if *entered {
                                    view_hov.update(cx, |this, cx| this.explorer_hover_row(ix, cx));
                                }
                            })
                            .active(|s| s.bg(c(theme::SELECT)))
                            .child(chevron)
                            .child(icon)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .when(multi && row.depth == 0, |d| {
                                        d.font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG))
                                    })
                                    .when(mark == Some('D'), |d| d.line_through())
                                    .child(row.label.clone()),
                            )
                            .when_some(mark, |h, m| h.child(div().text_xs().font_weight(FontWeight::BOLD).child(m.to_string())))
                            .when(dir_dirty, |h| h.child(div().size(px(5.)).rounded_full().bg(c(theme::WARN))))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                let keep = {
                                    let w = view_md.read(cx);
                                    stray_press_keeps_input(&w.pending, &w.name_input.read(cx).value())
                                };
                                if keep {
                                    cx.stop_propagation();
                                    return;
                                }
                                view_md.update(cx, |this, cx| {
                                    if this.pending.is_some() {
                                        this.cancel_pending(window, cx);
                                    }
                                });
                            })
                            .on_click(move |_, window, cx| {
                                focus_l.focus(window, cx);
                                let path = PathBuf::from(&row_id_click);
                                view_l.update(cx, |this, cx| {
                                    this.selected = Some(path.clone());
                                    if is_dir {
                                        this.toggle_folder(row_id_click.clone(), cx);
                                    } else {
                                        this.agent.user_nav = Some(std::time::Instant::now());
                                        this.agent_full = false;
                                        this.open_file(path, window, cx);
                                    }
                                    cx.notify();
                                });
                            })
                            .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                                let keep = {
                                    let w = view_r.read(cx);
                                    stray_press_keeps_input(&w.pending, &w.name_input.read(cx).value())
                                };
                                if keep {
                                    cx.stop_propagation();
                                    return;
                                }
                                if view_r.read(cx).pending.is_some() {
                                    view_r.update(cx, |this, cx| this.cancel_pending(window, cx));
                                    cx.stop_propagation();
                                    return;
                                }
                                let path = PathBuf::from(&row_id_rclick);
                                view_r.update(cx, |this, cx| {
                                    this.selected = Some(path);
                                    cx.notify();
                                });
                            })
                            .context_menu(move |menu, _window, _cx| {
                                let path = PathBuf::from(&row_id_ctx);
                                let added = extra_ctx.contains(&path);
                                *target.borrow_mut() = Some(path);
                                let menu = if added {
                                    menu.menu(tr("Remove Folder from Workspace"), Box::new(RemoveFolder)).separator()
                                } else {
                                    menu
                                };
                                menu.menu(tr("New File..."), Box::new(ExplorerNewFile))
                                    .menu(tr("New Folder..."), Box::new(ExplorerNewFolder))
                                    .separator()
                                    .menu(tr("Rename..."), Box::new(ExplorerRename))
                                    .menu(tr("Delete"), Box::new(ExplorerDelete))
                                    .separator()
                                    .menu(tr("Copy Path"), Box::new(CopyPath))
                                    .menu(tr("Copy Relative Path"), Box::new(CopyRelativePath))
                                    .menu(tr("Reveal in File Explorer"), Box::new(RevealInExplorer))
                            });

                        if !motion {
                            return row_el.into_any_element();
                        }

                        if let Some((jump_id, seq)) = &jump_anim {
                            if *jump_id == id {
                                let accent = c(theme::ACCENT);
                                return row_el
                                    .with_animation(
                                        SharedString::from(format!("xr-jump-{seq}-{id}")),
                                        Animation::new(Duration::from_millis(420))
                                            .with_easing(gpui_kit::ease_out_quint()),
                                        move |d, t| d.bg(accent.opacity((1. - t) * 0.28)),
                                    )
                                    .into_any_element();
                            }
                        }

                        if ws_anim > 0 && row.depth == 0 && added_roots.contains(&id) {
                            let accent = c(theme::ACCENT);
                            return row_el
                                .with_animation(
                                    SharedString::from(format!("xr-ws-{ws_anim}-{id}")),
                                    Animation::new(Duration::from_millis(240)).with_easing(gpui_kit::ease_out_quint()),
                                    move |d, t| {
                                        d.opacity(0.15 + 0.85 * t)
                                            .pl(px(base_pl - (1. - t) * 10.))
                                            .bg(accent.opacity((1. - t) * 0.22))
                                    },
                                )
                                .into_any_element();
                        }

                        if let Some((anim_dir, expanding, seq)) = &folder_anim {
                            if *expanding && is_descendant_of(&id, anim_dir) {
                                return row_el
                                    .with_animation(
                                        SharedString::from(format!("xr-open-{seq}-{id}")),
                                        Animation::new(Duration::from_millis(165)).with_easing(gpui_kit::ease_out_quint()),
                                        move |d, t| d.opacity(0.08 + 0.92 * t).pl(px(base_pl - (1. - t) * 7.)),
                                    )
                                    .into_any_element();
                            }
                            if !*expanding && id == *anim_dir {
                                let hover = c(theme::BG_HOVER);
                                return row_el
                                    .with_animation(
                                        SharedString::from(format!("xr-shut-{seq}-{id}")),
                                        Animation::new(Duration::from_millis(150)).with_easing(gpui_kit::ease_out_quint()),
                                        move |d, t| d.bg(hover.opacity((1. - t) * 0.5)),
                                    )
                                    .into_any_element();
                            }
                            if !*expanding {
                                if let Some(cix) = collapsed_row_ix {
                                    if ix > cix && ix <= cix + 8 {
                                        return row_el
                                            .with_animation(
                                                SharedString::from(format!("xr-pull-{seq}-{id}")),
                                                Animation::new(Duration::from_millis(140)).with_easing(gpui_kit::ease_out_quint()),
                                                |d, t| d.mt(px((1. - t) * 4.)),
                                            )
                                            .into_any_element();
                                    }
                                }
                            }
                        }

                        row_el.into_any_element()
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.explorer.scroll)
        .size_full();

        let row_h = f32::from(theme::u(20.));
        let hover_outline: Option<AnyElement> = if !motion {
            None
        } else if let Some(hix) = self.explorer.hovered_ix.filter(|&i| i < self.explorer.rows.len()) {
            let live_y = self.explorer_row_y(hix);
            let from_y = self.explorer.hover_from_y;
            let to_y = self.explorer.hover_to_y;
            let seq = self.explorer.hover_seq;
            let fade_in = self.explorer.hover_fade_in;
            let scrolled = (live_y - to_y).abs() > 1.0;
            let pill = div()
                .absolute()
                .left(px(4.))
                .right(px(6.))
                .h(px(row_h))
                .rounded(px(5.))
                .bg(c(theme::BG_HOVER).opacity(0.65))
                .border_1()
                .border_color(c(theme::BORDER_STRONG).opacity(0.85));
            Some(if scrolled {
                pill.top(px(live_y)).into_any_element()
            } else {
                pill.with_animation(
                    SharedString::from(format!("xr-hov-{seq}")),
                    Animation::new(Duration::from_millis(if fade_in { 120 } else { 145 })).with_easing(gpui_kit::ease_out_quint()),
                    move |d, t| {
                        let y = from_y + (to_y - from_y) * t;
                        let op = if fade_in { t } else { 1. };
                        d.top(px(y)).opacity(op)
                    },
                )
                .into_any_element()
            })
        } else if let Some((ly, seq)) = self.explorer.hover_leave {
            Some(
                div()
                    .absolute()
                    .left(px(4.))
                    .right(px(6.))
                    .top(px(ly))
                    .h(px(row_h))
                    .rounded(px(5.))
                    .bg(c(theme::BG_HOVER).opacity(0.65))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG).opacity(0.85))
                    .with_animation(
                        SharedString::from(format!("xr-hov-out-{seq}")),
                        Animation::new(Duration::from_millis(120)).with_easing(gpui_kit::ease_out_quint()),
                        |d, t| d.opacity(1. - t),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        let body = div()
            .id("explorer-body")
            .relative()
            .flex_1()
            .min_h_0()
            .size_full()
            .overflow_hidden()
            .on_hover(cx.listener(|this, entered: &bool, _, cx| {
                if !*entered {
                    this.explorer_hover_clear(cx);
                }
            }))
            .children(hover_outline)
            .child(list)
            .vertical_scrollbar(&self.explorer.scroll);

        v_flex()
            .size_full()
            .key_context("Explorer")
            .track_focus(&self.explorer.focus)
            .on_action(cx.listener(Self::on_explorer_up))
            .on_action(cx.listener(Self::on_explorer_down))
            .on_action(cx.listener(Self::on_explorer_left))
            .on_action(cx.listener(Self::on_explorer_right))
            .on_action(cx.listener(Self::on_explorer_open))
            .context_menu(move |menu, _window, _cx| {
                if let Some(root) = &root {
                    *root_target.borrow_mut() = Some(root.clone());
                }
                menu.menu(tr("New File..."), Box::new(ExplorerNewFile))
                    .menu(tr("New Folder..."), Box::new(ExplorerNewFolder))
                    .separator()
                    .menu(tr("Add Folder to Workspace..."), Box::new(AddFolderToWorkspace))
                    .menu(tr("Refresh"), Box::new(ExplorerRefresh))
                    .separator()
                    .menu(tr("Copy Path"), Box::new(CopyPath))
                    .menu(tr("Reveal in File Explorer"), Box::new(RevealInExplorer))
            })
            .child(body)
            .into_any_element()
    }

    fn target_dir(&self) -> Option<PathBuf> {
        let t = self.ctx_target.borrow_mut().take().or_else(|| self.selected.clone());
        match t {
            Some(p) if p.is_dir() => Some(p),
            Some(p) => p.parent().map(Path::to_path_buf),
            None => self.root.clone(),
        }
    }

    fn target_item(&self) -> Option<PathBuf> {
        self.ctx_target.borrow_mut().take().or_else(|| self.selected.clone())
    }

    pub(crate) fn pending_file_icon(&self, cx: &App) -> Option<(crate::fluxicon::FluxIcon, Option<u32>)> {
        match &self.pending {
            Some(PendingOp::NewFile(_)) => {
                let typed = self.name_input.read(cx).value();
                Some(if typed.is_empty() {
                    (crate::fluxicon::FILE, None)
                } else {
                    files::icon_for(&typed)
                })
            }
            Some(PendingOp::Rename(p)) => {
                let typed = self.name_input.read(cx).value();
                if typed.is_empty() {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    Some(files::icon_for(&name))
                } else {
                    Some(files::icon_for(&typed))
                }
            }
            _ => None,
        }
    }

    fn start_pending(&mut self, op: PendingOp, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.view = SideView::Explorer;
        self.sidebar_open = true;
        if let PendingOp::NewFile(d) | PendingOp::NewFolder(d) = &op {
            self.expand_folder(d.to_string_lossy().into_owned(), false, cx);
        }
        self.pending = Some(op);
        self.rebuild_explorer_rows(cx);
        let placeholder: &'static str = match &self.pending {
            Some(PendingOp::NewFile(_)) => tr("File name"),
            Some(PendingOp::NewFolder(_)) => tr("Folder name"),
            Some(PendingOp::Rename(p)) if p.is_dir() => tr("Folder name"),
            _ => tr("File name"),
        };
        let value = value.to_string();
        self.name_input.update(cx, |s, cx| {
            s.set_value(value, window, cx);
            s.set_placeholder(placeholder, window, cx);
            s.focus(window, cx);
        });
        self.pending_icon = self.pending_file_icon(cx);
        cx.notify();
    }

    pub(crate) fn cancel_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.take().is_some() {
            self.rebuild_explorer_rows(cx);
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn on_cancel_pending(&mut self, _: &CancelPending, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pending(window, cx);
    }

    pub(crate) fn on_explorer_new_file(&mut self, _: &ExplorerNewFile, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = self.target_dir() {
            self.start_pending(PendingOp::NewFile(d), "", window, cx);
        }
    }

    pub(crate) fn on_explorer_new_folder(&mut self, _: &ExplorerNewFolder, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = self.target_dir() {
            self.start_pending(PendingOp::NewFolder(d), "", window, cx);
        }
    }

    pub(crate) fn on_explorer_rename(&mut self, _: &ExplorerRename, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.target_item().filter(|p| Some(p) != self.root.as_ref()) {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            self.start_pending(PendingOp::Rename(p), &name, window, cx);
        }
    }

    pub(crate) fn on_explorer_delete(&mut self, _: &ExplorerDelete, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.target_item().filter(|p| Some(p) != self.root.as_ref()) {
            self.start_pending(PendingOp::Delete(p), "", window, cx);
        }
    }

    pub(crate) fn on_explorer_refresh(&mut self, _: &ExplorerRefresh, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh_tree(cx);
    }

    pub(crate) fn on_copy_path(&mut self, _: &CopyPath, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.target_item() {
            cx.write_to_clipboard(ClipboardItem::new_string(p.to_string_lossy().into_owned()));
        }
    }

    pub(crate) fn on_copy_relative_path(&mut self, _: &CopyRelativePath, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.target_item() {
            let rel = self.rel_display(&p.to_string_lossy()).replace('\\', "/");
            cx.write_to_clipboard(ClipboardItem::new_string(rel));
        }
    }

    pub(crate) fn on_reveal(&mut self, _: &RevealInExplorer, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.target_item() {
            cx.reveal_path(&p);
        }
    }

    pub(crate) fn commit_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(op) = self.pending.take() else { return };
        let name = self.name_input.read(cx).value().trim().to_string();
        let bad_name = name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']);
        let result = match &op {
            PendingOp::NewFile(_) | PendingOp::NewFolder(_) | PendingOp::Rename(_) if bad_name => {
                Err(crate::trf!("\"{name}\" is not a valid name"; name => name))
            }
            PendingOp::NewFile(dir) => {
                let p = dir.join(&name);
                core::create_file(&p).map(|_| {
                    self.open_file(p.clone(), window, cx);
                    self.selected = Some(p);
                })
            }
            PendingOp::NewFolder(dir) => core::create_dir(&dir.join(&name)),
            PendingOp::Rename(from) => {
                let to = from.with_file_name(&name);
                core::rename_path(from, &to).map(|_| {
                    for d in self.docs.iter_mut() {
                        if let Ok(rest) = d.path.strip_prefix(from) {
                            d.path = to.join(rest);
                            d.name = d.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
                        }
                    }
                    self.selected = Some(to);
                })
            }
            PendingOp::Delete(p) => core::delete_path(p).map(|_| {
                while let Some(ix) = self.docs.iter().position(|d| d.path.starts_with(p)) {
                    self.close(ix, window, cx);
                }
                self.selected = None;
            }),
        };
        match result {
            Ok(()) => {
                self.rebuild_explorer_rows(cx);
                self.refresh_tree(cx);
            }
            Err(e) => {
                self.status = e.into();
                self.pending = Some(op);
            }
        }
        cx.notify();
    }
}

fn name_row(id: &str, depth: usize, icon: Icon, name_input: &Entity<InputState>) -> Stateful<Div> {
    h_flex()
        .id(SharedString::from(format!("xr-edit-{id}")))
        .w_full()
        .h(theme::u(20.))
        .py_0()
        .px_1()
        .pl(theme::u(10.) * depth + theme::u(6.))
        .gap_1()
        .items_center()
        .bg(c(theme::SELECT))
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(div().w(theme::u(12.)).flex().justify_center())
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .child(Input::new(name_input).xsmall().focus_bordered(false).w_full()),
        )
}

fn delete_row(id: &str, depth: usize, path: &Path, view: &Entity<Workbench>) -> Stateful<Div> {
    let name = short(path);
    let del = view.clone();
    let cancel = view.clone();
    h_flex()
        .id(SharedString::from(format!("xr-del-{id}")))
        .w_full()
        .h(theme::u(20.))
        .py_0()
        .px_1()
        .pl(theme::u(10.) * depth + theme::u(6.))
        .gap_1()
        .items_center()
        .bg(c(theme::SELECT))
        .text_size(theme::u(11.5))
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(Icon::new(IconName::Delete).size(theme::u(13.)).text_color(c(theme::DANGER)))
        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(crate::trf!("Delete {name}?"; name => name)))
        .child(
            Button::new("pending-del-ok")
                .label(tr("Delete"))
                .xsmall()
                .danger()
                .on_click(move |_, window, cx| {
                    del.update(cx, |this, cx| this.commit_pending(window, cx));
                }),
        )
        .child(
            Button::new("pending-del-cancel")
                .label(tr("Cancel"))
                .xsmall()
                .ghost()
                .on_click(move |_, window, cx| {
                    cancel.update(cx, |this, cx| this.cancel_pending(window, cx));
                }),
        )
}

fn short(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::{flatten_nodes, is_descendant_of, stray_press_keeps_input, PENDING_ROW_ID};
    use crate::files::Node;
    use crate::workbench::PendingOp;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    fn dir(id: &str, name: &str, loaded: bool, children: Vec<Node>) -> Node {
        Node { id: id.into(), name: name.into(), children: Some(children), loaded }
    }

    fn file(id: &str, name: &str) -> Node {
        Node { id: id.into(), name: name.into(), children: None, loaded: true }
    }

    #[test]
    fn toggling_two_large_sibling_folders_keeps_exact_path_identity() {
        let big_a: Vec<Node> = (0..60).map(|i| file(&format!("/bp/A/a{i}.js"), &format!("a{i}.js"))).collect();
        let big_b: Vec<Node> = (0..60).map(|i| file(&format!("/bp/B/b{i}.json"), &format!("b{i}.json"))).collect();
        let nodes = vec![dir("/bp", "behavior_packs", true, vec![
            dir("/bp/A", "KiwEssentialsVIP_BP", true, big_a),
            dir("/bp/B", "KiwHitAnimation_BP", true, big_b),
        ])];
        let mut expanded = HashSet::from(["/bp".to_string(), "/bp/B".to_string()]);
        let loading = HashSet::new();
        let mut dirs = HashSet::new();

        let rows = flatten_nodes(&nodes, &expanded, &loading, None, Some(Path::new("/bp")), &mut dirs);
        assert_eq!(rows[0].id, "/bp");
        assert_eq!(rows[1].id, "/bp/A");
        assert!(!rows[1].expanded);
        assert_eq!(rows[2].id, "/bp/B");
        assert!(rows[2].expanded);
        assert_eq!(rows.len(), 3 + 60);

        // Expand A and collapse B: rows stay strictly keyed by path.
        expanded.insert("/bp/A".to_string());
        expanded.remove("/bp/B");
        let rows2 = flatten_nodes(&nodes, &expanded, &loading, None, Some(Path::new("/bp")), &mut dirs);
        assert_eq!(rows2[1].id, "/bp/A");
        assert!(rows2[1].expanded);
        assert_eq!(rows2[2].id, "/bp/A/a0.js");
        let b_row = rows2.iter().find(|r| r.id == "/bp/B").unwrap();
        assert!(!b_row.expanded);
        assert!(!rows2.iter().any(|r| r.id.starts_with("/bp/B/")));
    }

    #[test]
    fn unloaded_and_empty_folders_show_honest_placeholders() {
        let nodes = vec![
            dir("/p/unloaded", "unloaded", false, vec![]),
            dir("/p/empty", "empty", true, vec![]),
        ];
        let expanded = HashSet::from(["/p/unloaded".to_string(), "/p/empty".to_string()]);
        let loading = HashSet::new();
        let mut dirs = HashSet::new();
        let rows = flatten_nodes(&nodes, &expanded, &loading, None, Some(Path::new("/p")), &mut dirs);
        assert_eq!(rows.len(), 4);
        assert!(rows[0].is_dir && rows[1].is_placeholder && rows[1].loading);
        assert!(rows[2].is_dir && rows[3].is_placeholder && !rows[3].loading);
    }

    #[test]
    fn pending_row_inserts_into_target_folder_or_root() {
        let nodes = vec![dir("/p/src", "src", true, vec![file("/p/src/a.rs", "a.rs")])];
        let expanded = HashSet::from(["/p/src".to_string()]);
        let loading = HashSet::new();
        let mut dirs = HashSet::new();
        let op = PendingOp::NewFile(PathBuf::from("/p/src"));
        let rows = flatten_nodes(&nodes, &expanded, &loading, Some(&op), Some(Path::new("/p")), &mut dirs);
        assert_eq!(rows[1].id, PENDING_ROW_ID);
        assert_eq!(rows[1].depth, 1);

        let root_op = PendingOp::NewFolder(PathBuf::from("/p"));
        let rows_root = flatten_nodes(&nodes, &expanded, &loading, Some(&root_op), Some(Path::new("/p")), &mut dirs);
        assert_eq!(rows_root[0].id, PENDING_ROW_ID);
        assert_eq!(rows_root[0].depth, 0);
    }

    #[test]
    fn descendant_check_respects_path_boundaries() {
        assert!(is_descendant_of("/bp/A/file.js", "/bp/A"));
        assert!(is_descendant_of(r"C:\bp\A\file.js", r"C:\bp\A"));
        assert!(!is_descendant_of("/bp/AB/file.js", "/bp/A"));
        assert!(!is_descendant_of("/bp/A", "/bp/A"));
    }

    #[test]
    fn stray_press_keeps_a_typed_name() {
        let new = Some(PendingOp::NewFile(PathBuf::from("/tmp")));
        assert!(stray_press_keeps_input(&new, "main.py"));
        assert!(!stray_press_keeps_input(&new, ""));
        assert!(!stray_press_keeps_input(&new, "   "));
        assert!(stray_press_keeps_input(&Some(PendingOp::Rename(PathBuf::from("/tmp/a.txt"))), "b"));
        assert!(!stray_press_keeps_input(&Some(PendingOp::Delete(PathBuf::from("/tmp/a.txt"))), "main.py"));
        assert!(!stray_press_keeps_input(&None, "main.py"));
    }
}
