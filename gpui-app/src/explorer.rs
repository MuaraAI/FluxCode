//! Explorer side bar: file tree, context menu, new/rename/delete.

use crate::core;
use crate::files;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    list::ListItem,
    tree::tree,
    v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};

impl Workbench {
    pub(crate) fn render_explorer(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.root.is_none() {
            return v_flex()
                .p_4()
                .gap_3()
                .text_sm()
                .text_color(c(theme::FG_MUTED))
                .child("You have not yet opened a folder.")
                .child(
                    Button::new("open-folder")
                        .primary()
                        .label("Open Folder")
                        .on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(OpenFolder), cx))),
                )
                .into_any_element();
        }
        let view = cx.entity();
        let active_path = self.active.map(|a| self.docs[a].path.to_string_lossy().into_owned());
        let selected = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned());
        let target = self.ctx_target.clone();
        let list = tree(&self.tree, move |ix, entry, _selected, _window, _cx| {
            let item = entry.item();
            let id = item.id.to_string();
            let is_open = active_path.as_deref() == Some(id.as_str());
            let is_sel = selected.as_deref() == Some(id.as_str());
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
                .when(is_open || is_sel, |s| s.bg(c(theme::SELECT)))
                .child(h_flex().gap_1().text_color(c(theme::FG)).child(lead).child(item.label.clone()))
                .on_click({
                    let view = view.clone();
                    let item = item.clone();
                    move |_, window, cx| {
                        let path = PathBuf::from(item.id.as_ref());
                        view.update(cx, |this, cx| {
                            this.selected = Some(path.clone());
                            if !item.is_folder() {
                                this.open_file(path, window, cx);
                            }
                            cx.notify();
                        });
                    }
                })
        })
        .context_menu(move |_ix, entry, menu, _window, _cx| {
            *target.borrow_mut() = Some(PathBuf::from(entry.item().id.as_ref()));
            menu.menu("New File...", Box::new(ExplorerNewFile))
                .menu("New Folder...", Box::new(ExplorerNewFolder))
                .separator()
                .menu("Rename...", Box::new(ExplorerRename))
                .menu("Delete", Box::new(ExplorerDelete))
                .separator()
                .menu("Copy Path", Box::new(CopyPath))
                .menu("Reveal in File Explorer", Box::new(RevealInExplorer))
        })
        .text_sm()
        .size_full();

        v_flex()
            .size_full()
            .key_context("Explorer")
            .when_some(self.pending.clone(), |v, op| v.child(self.render_pending(op, cx)))
            .child(div().flex_1().min_h_0().child(list))
            .into_any_element()
    }

    fn render_pending(&self, op: PendingOp, cx: &mut Context<Self>) -> impl IntoElement {
        let (label, confirm) = match &op {
            PendingOp::NewFile(d) => (format!("New file in {}", short(d)), "Create"),
            PendingOp::NewFolder(d) => (format!("New folder in {}", short(d)), "Create"),
            PendingOp::Rename(p) => (format!("Rename {}", short(p)), "Rename"),
            PendingOp::Delete(p) => (format!("Move {} to the Recycle Bin?", short(p)), "Delete"),
        };
        let ask_name = !matches!(op, PendingOp::Delete(_));
        v_flex()
            .p_2()
            .gap_2()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .text_xs()
            .child(label)
            .when(ask_name, |v| v.child(Input::new(&self.name_input).small()))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("pending-ok")
                            .label(confirm)
                            .xsmall()
                            .when(matches!(op, PendingOp::Delete(_)), |b| b.danger())
                            .when(!matches!(op, PendingOp::Delete(_)), |b| b.primary())
                            .on_click(cx.listener(|this, _, w, cx| this.commit_pending(w, cx))),
                    )
                    .child(Button::new("pending-cancel").label("Cancel").xsmall().ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.pending = None;
                        cx.notify();
                    }))),
            )
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
        self.pending = Some(op);
        let value = value.to_string();
        self.name_input.update(cx, |s, cx| {
            s.set_value(value, window, cx);
            s.focus(window, cx);
        });
        cx.notify();
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
                Err(format!("\"{name}\" is not a valid name"))
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
            Ok(()) => self.refresh_tree(cx),
            Err(e) => {
                self.status = e.into();
                self.pending = Some(op);
            }
        }
        cx.notify();
    }
}

fn short(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}
