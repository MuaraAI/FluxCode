//! Explorer side bar: file tree, context menu, new/rename/delete.

use crate::i18n::tr;
use crate::core;
use crate::files;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    list::ListItem,
    menu::ContextMenuExt,
    tree::{tree, TreeItem},
    v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};

/// The id of the row a new file or folder is typed into. It starts with a
/// control character, so it can never be a real path.
pub(crate) const PENDING_ROW_ID: &str = "\u{1}pending";

/// Put the typing row first inside the folder a new file or folder goes into.
/// The workspace root itself is not a row when one folder is open, so the row
/// goes at the top of the list then. Rename and delete type in place, so they
/// need no row here.
pub(crate) fn insert_pending_row(items: &mut Vec<TreeItem>, pending: Option<&PendingOp>, root: Option<&Path>) {
    let Some(dir) = (match pending {
        Some(PendingOp::NewFile(d) | PendingOp::NewFolder(d)) => Some(d),
        _ => None,
    }) else {
        return;
    };
    fn insert_into(items: &mut [TreeItem], dir: &str) -> bool {
        for it in items.iter_mut() {
            if it.id.as_ref() == dir {
                it.children.insert(0, TreeItem::new(PENDING_ROW_ID, "").disabled(true));
                return true;
            }
            if insert_into(&mut it.children, dir) {
                return true;
            }
        }
        false
    }
    let dir_s = dir.to_string_lossy();
    if !insert_into(items, &dir_s) && Some(dir.as_path()) == root {
        items.insert(0, TreeItem::new(PENDING_ROW_ID, "").disabled(true));
    }
}

impl Workbench {
    pub(crate) fn render_explorer(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
        let view = cx.entity();
        let active_path = self.active.map(|a| self.docs[a].path.to_string_lossy().into_owned());
        // Follow the active tab: its folders open and its row scrolls into view, like
        // VS Code's "reveal active file". Runs only while the explorer is on screen (so a
        // hidden side bar costs nothing and catches up when shown), once per tab change.
        let inside = |p: &String| self.roots().iter().any(|r| Path::new(p).starts_with(r));
        if let Some(p) = active_path.clone().filter(|p| self.revealed.borrow().as_deref() != Some(p.as_str()) && inside(p)) {
            *self.revealed.borrow_mut() = Some(p.clone());
            let id = SharedString::from(p);
            self.tree.update(cx, |t, cx| {
                if t.index_of(&id).is_none() {
                    t.reveal_item(&id, ScrollStrategy::Center, cx);
                }
                if let Some(ix) = t.index_of(&id) {
                    t.scroll_to_item(ix, ScrollStrategy::Nearest);
                    t.set_selected_index(Some(ix), cx);
                }
            });
        }
        let selected = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned());
        let target = self.ctx_target.clone();
        let root_target = target.clone();
        let root = self.root.clone();
        let dirs = self.dirs.clone();
        let (marks, dirty_dirs) = (self.scm.marks.clone(), self.scm.dirty_dirs.clone());
        // In a multi-folder workspace the top rows are the folders themselves.
        let multi = !self.workspace.extra.is_empty();
        let extra = self.workspace.extra.clone();
        let pending = self.pending.clone();
        let name_input = self.name_input.clone();
        let tree_state = self.tree.clone();
        let list = tree(&self.tree, move |ix, entry, _selected, _window, cx| {
            let item = entry.item();
            let id = item.id.to_string();
            // The row a new name is typed into, and the rows being renamed or deleted:
            // they draw as an input or a confirm instead of the file's row.
            if id == PENDING_ROW_ID {
                let icon = match &pending {
                    Some(PendingOp::NewFolder(_)) => Icon::new(files::folder_icon("", true)).size(theme::u(13.)).text_color(c(theme::FOLDER)),
                    // New file: the icon follows what is typed, like VS Code, so
                    // `file.py` shows the Python icon before Enter is pressed.
                    // The workbench redraws on every change while a name is
                    // pending, so this stays live as the user types.
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
                return name_row(ix, entry.depth(), icon, &name_input);
            }
            if let Some(PendingOp::Rename(p)) = &pending {
                if p.to_string_lossy() == id {
                    let is_dir = entry.is_folder() || dirs.contains(&id);
                    let icon = if is_dir {
                        Icon::new(files::folder_icon(&item.label, entry.is_expanded()))
                            .size(theme::u(13.))
                            .text_color(c(theme::FOLDER))
                    } else {
                        // Rename: the icon follows the typed name too, falling
                        // back to the file's own icon while the field is empty.
                        let typed = name_input.read(cx).value();
                        let name: &str = if typed.is_empty() { &item.label } else { &typed };
                        let (ic, tint) = files::icon_for(name);
                        Icon::new(ic).size(theme::u(13.)).text_color(theme::tint(tint))
                    };
                    return name_row(ix, entry.depth(), icon, &name_input);
                }
            }
            if let Some(PendingOp::Delete(p)) = &pending {
                if p.to_string_lossy() == id {
                    return delete_row(ix, entry.depth(), p, &view);
                }
            }
            // While a name is being typed, no other row looks selected: the
            // input row is the only highlighted one, like VS Code. Without
            // this the file that was selected before kept its highlight next
            // to the input row, so two rows looked selected at once.
            let naming = pending.is_some();
            let is_open = !naming && active_path.as_deref() == Some(id.as_str());
            let is_sel = !naming && selected.as_deref() == Some(id.as_str());
            // Every row: a 12px chevron slot, then one 14px icon from the same outline set.
            let is_dir = entry.is_folder() || dirs.contains(&id);
            // Skipped folders (.git, node_modules, ...) are listed but never opened: dimmed, no chevron.
            let skipped = is_dir && !entry.is_folder();
            let chevron = div().w(theme::u(12.)).flex().justify_center().when(entry.is_folder(), |d| {
                d.child(
                    Icon::new(if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight })
                        .xsmall()
                        .text_color(c(theme::FG_DIM)),
                )
            });
            let icon = if is_dir {
                Icon::new(files::folder_icon(&item.label, entry.is_expanded()))
                    .size(theme::u(13.))
                    .text_color(if skipped { c(theme::FG_DIM) } else { c(theme::FOLDER) })
            } else {
                let (icon, tint) = files::icon_for(&item.label);
                Icon::new(icon).size(theme::u(13.)).text_color(theme::tint(tint))
            };
            let lead: AnyElement = h_flex().gap_1().child(chevron).child(icon).into_any_element();
            // Git decorations like VS Code: the letter on the right, the name in its color,
            // and a dot on folders that contain changes.
            let key = id.replace('/', "\\").to_lowercase();
            let mark = if is_dir { None } else { marks.get(&key).copied() };
            let dir_dirty = is_dir && dirty_dirs.contains(&key);
            let label_color = match mark {
                _ if skipped => c(theme::FG_DIM),
                Some(m) => c(crate::scm::code_color(m)),
                None if dir_dirty => c(theme::WARN),
                None => c(theme::FG),
            };
            // The row's id is its file path, not its position: GPUI tracks
            // hover by element id, so with a position id the hover tint
            // stayed on the slot and jumped to the wrong file when the
            // input row was inserted above it and every row shifted down.
            // With the path as the id the hover follows the file.
            ListItem::new(id.clone())
                .w_full()
                .h(theme::u(20.))
                .px_1()
                .pl(theme::u(10.) * entry.depth() + theme::u(6.))
                .when(is_open || is_sel, |s| s.bg(c(theme::SELECT)))
                .child(
                    h_flex()
                        .w_full()
                        .gap_1()
                        .pr_2()
                        // The tree doesn't hand its text style to rows, so size each row here
                        // (13 px at the default interface size, like VS Code).
                        .text_size(theme::u(11.5))
                        .text_color(label_color)
                        .child(lead)
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .when(multi && entry.depth() == 0, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)))
                                .when(mark == Some('D'), |d| d.line_through())
                                .child(item.label.clone()),
                        )
                        .when_some(mark, |h, m| h.child(div().text_xs().font_weight(FontWeight::BOLD).child(m.to_string())))
                        .when(dir_dirty, |h| h.child(div().size(px(5.)).rounded_full().bg(c(theme::WARN)))),
                )
                .on_click({
                    let view = view.clone();
                    let item = item.clone();
                    move |_, window, cx| {
                        let path = PathBuf::from(item.id.as_ref());
                        view.update(cx, |this, cx| {
                            this.selected = Some(path.clone());
                            if !is_dir {
                                this.agent.user_nav = Some(std::time::Instant::now());
                                this.open_file(path, window, cx);
                            }
                            cx.notify();
                        });
                    }
                })
                .on_mouse_down(MouseButton::Left, {
                    let view = view.clone();
                    move |_, window, cx| {
                        // A press elsewhere ends the name being typed, like VS
                        // Code. This runs on mouse down, before the click is
                        // made, so the input row is already gone when the
                        // click lands: the tree selects the row under the
                        // mouse with its new index, and the click still
                        // opens it. Cancelling in the click handler instead
                        // would leave the tree selecting one row too far
                        // down, and the click would not fire at all if the
                        // row still carried its old position id.
                        view.update(cx, |this, cx| {
                            if this.pending.is_some() {
                                this.cancel_pending(window, cx);
                            }
                        });
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let view = view.clone();
                    let tree_state = tree_state.clone();
                    let item = item.clone();
                    move |_, window, cx| {
                        // While a name is being typed, a right-click only
                        // closes the input. The menu stays shut: it is built
                        // from the row index captured before this press, and
                        // that index shifts when the input row goes away, so
                        // the menu would act on the wrong row. One more
                        // right-click opens it, on the row it names.
                        if view.read(cx).pending.is_some() {
                            view.update(cx, |this, cx| this.cancel_pending(window, cx));
                            cx.stop_propagation();
                            return;
                        }
                        // Right-click picks the row first, like VS Code: the
                        // highlight, the tree's own selection, and the action
                        // target all move to it. The menu opens on the same
                        // press, after this runs, so it acts on this row.
                        let path = PathBuf::from(item.id.as_ref());
                        view.update(cx, |this, cx| {
                            this.selected = Some(path);
                            cx.notify();
                        });
                        tree_state.update(cx, |state, cx| {
                            state.set_selected_index(Some(ix), cx);
                        });
                    }
                })
        })
        .context_menu(move |_ix, entry, menu, _window, _cx| {
            let path = PathBuf::from(entry.item().id.as_ref());
            let added = extra.contains(&path);
            *target.borrow_mut() = Some(path);
            let menu = if added {
                menu.menu(tr("Remove Folder from Workspace"), Box::new(crate::workbench::RemoveFolder)).separator()
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
        })
        .text_size(theme::u(11.5))
        .size_full();

        v_flex()
            .size_full()
            .key_context("Explorer")
            .context_menu(move |menu, _window, _cx| {
                // Empty space in the Explorer: the menu acts on the workspace root.
                // A row's own menu fires first and stops this one, so the two never
                // both open.
                if let Some(root) = &root {
                    *root_target.borrow_mut() = Some(root.clone());
                }
                menu.menu(tr("New File..."), Box::new(ExplorerNewFile))
                    .menu(tr("New Folder..."), Box::new(ExplorerNewFolder))
                    .separator()
                    .menu(tr("Refresh"), Box::new(ExplorerRefresh))
                    .separator()
                    .menu(tr("Copy Path"), Box::new(CopyPath))
                    .menu(tr("Reveal in File Explorer"), Box::new(RevealInExplorer))
            })
            .child(div().flex_1().min_h_0().child(list))
            .into_any_element()
    }

    /// Folder an explorer action applies to: the right-clicked / selected
    /// item (its parent when it is a file), else the workspace root.
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

    fn start_pending(&mut self, op: PendingOp, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.view = SideView::Explorer;
        self.sidebar_open = true;
        // A new file or folder types inside its folder: open the folder first, so
        // the row is on screen. A folder that was never opened loads now.
        if let PendingOp::NewFile(d) | PendingOp::NewFolder(d) = &op {
            self.tree_expanded(d.to_string_lossy().into_owned(), cx);
        }
        self.pending = Some(op);
        self.set_tree_items(cx);
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
        cx.notify();
    }

    /// Drop the name being typed, or the delete confirm, and put focus back on
    /// the Explorer.
    pub(crate) fn cancel_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.take().is_some() {
            self.set_tree_items(cx);
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
                // The typed row goes at once; the refresh brings the new listing.
                self.set_tree_items(cx);
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

/// A tree row with a text input in it, for a new file, a new folder, or a rename.
/// Exactly one tree row tall: the tree lays its rows out at one height, so a
/// taller box would paint over its neighbours. The xsmall input is exactly
/// one row tall, but its focus ring draws outside its own edge, in the ring
/// colour on top of its ring-coloured border, so a focused input looked like
/// a thick line sitting on the row below. The ring is off here: the row's
/// selected background and the input's own thin border mark the focus.
fn name_row(ix: usize, depth: usize, icon: Icon, name_input: &Entity<InputState>) -> ListItem {
    ListItem::new(ix)
        .w_full()
        .h(theme::u(20.))
        .py_0()
        .px_1()
        .pl(theme::u(10.) * depth + theme::u(6.))
        .bg(c(theme::SELECT))
        // A click here only focuses the input; without this the tree would
        // also select the row and fold its folder.
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            h_flex()
                .w_full()
                .h_full()
                .gap_1()
                .items_center()
                .child(div().w(theme::u(12.)).flex().justify_center())
                .child(icon)
                .child(div().flex_1().min_w_0().h_full().flex().items_center().child(Input::new(name_input).xsmall().focus_bordered(false).w_full())),
        )
}

/// The row of a file or folder being deleted: its name as a question, with
/// Delete and Cancel buttons in place of the row. Same height as every row.
fn delete_row(ix: usize, depth: usize, path: &Path, view: &Entity<Workbench>) -> ListItem {
    let name = short(path);
    let del = view.clone();
    let cancel = view.clone();
    ListItem::new(ix)
        .w_full()
        .h(theme::u(20.))
        .py_0()
        .px_1()
        .pl(theme::u(10.) * depth + theme::u(6.))
        .bg(c(theme::SELECT))
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            h_flex()
                .w_full()
                .h_full()
                .gap_1()
                .items_center()
                .text_size(theme::u(11.5))
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
                ),
        )
}

fn short(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}
