//! Explorer side bar: file tree, context menu, new/rename/delete.

use crate::i18n::tr;
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
                .child(tr("You have not yet opened a folder."))
                .child(self.action_button("x-open-folder", tr("Open folder"), crate::ui::Kind::Primary, cx, |_, w, cx| {
                    w.dispatch_action(Box::new(OpenFolder), cx)
                }))
                .into_any_element();
        }
        let view = cx.entity();
        let active_path = self.active.map(|a| self.docs[a].path.to_string_lossy().into_owned());
        let selected = self.selected.as_ref().map(|p| p.to_string_lossy().into_owned());
        let target = self.ctx_target.clone();
        let dirs = self.dirs.clone();
        let (marks, dirty_dirs) = (self.scm.marks.clone(), self.scm.dirty_dirs.clone());
        // In a multi-folder workspace the top rows are the folders themselves.
        let multi = !self.workspace.extra.is_empty();
        let extra = self.workspace.extra.clone();
        let list = tree(&self.tree, move |ix, entry, _selected, _window, _cx| {
            let item = entry.item();
            let id = item.id.to_string();
            let is_open = active_path.as_deref() == Some(id.as_str());
            let is_sel = selected.as_deref() == Some(id.as_str());
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
            ListItem::new(ix)
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
                                this.open_file(path, window, cx);
                            }
                            cx.notify();
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
                .menu(tr("Reveal in File Explorer"), Box::new(RevealInExplorer))
        })
        .text_size(theme::u(11.5))
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
            PendingOp::NewFile(d) => (crate::trf!("New file in {0}"; 0 => short(d)), tr("Create")),
            PendingOp::NewFolder(d) => (crate::trf!("New folder in {0}"; 0 => short(d)), tr("Create")),
            PendingOp::Rename(p) => (crate::trf!("Rename {0}"; 0 => short(p)), tr("Rename")),
            PendingOp::Delete(p) => (crate::trf!("Move {0} to the Recycle Bin?"; 0 => short(p)), tr("Delete")),
        };
        let ask_name = !matches!(op, PendingOp::Delete(_));
        v_flex()
            .m_2()
            .p_2()
            .gap_2()
            .rounded(px(6.))
            .border_1()
            .border_color(c(if matches!(op, PendingOp::Delete(_)) { theme::DANGER } else { theme::BORDER_STRONG }))
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
                    .child(Button::new("pending-cancel").label(tr("Cancel")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| {
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
