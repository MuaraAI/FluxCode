//! Several folders in one window, and files or folders dropped on it.
//!
//! The first folder is the main one: the terminal starts there, Source Control
//! and the agent work in it. Folders added next to it show as their own roots
//! in the explorer and are searched by Go to File, Find in Files and `@`.
//!
//! Dropping a folder on the window asks whether to add it to the workspace or
//! switch the window to it (with no folder open it just opens). Dropped files
//! open in tabs.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{button::{Button, ButtonVariants}, h_flex, v_flex, Icon, IconName, Sizable};
use gpui_kit::*;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct Workspace {
    /// Folders added next to the main one (`Workbench::root`).
    pub extra: Vec<PathBuf>,
    /// Folders dropped while one is open, waiting for "add" or "switch".
    pub ask: Option<Vec<PathBuf>>,
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

impl Workbench {
    /// Every folder of the workspace, the main one first.
    pub(crate) fn roots(&self) -> Vec<PathBuf> {
        self.root.iter().cloned().chain(self.workspace.extra.iter().cloned()).collect()
    }

    /// A path as the lists show it: relative to its folder, and with the folder's
    /// name in front when the workspace has more than one.
    pub(crate) fn rel_display(&self, full: &str) -> String {
        let multi = !self.workspace.extra.is_empty();
        for r in self.roots() {
            let rs = r.to_string_lossy();
            if let Some(rest) = full.strip_prefix(rs.as_ref()) {
                if rest.is_empty() || rest.starts_with(['\\', '/']) {
                    let rest = rest.trim_start_matches(['\\', '/']);
                    return if multi { format!("{}\\{rest}", name_of(&r)) } else { rest.to_string() };
                }
            }
        }
        full.to_string()
    }

    pub(crate) fn on_drop_paths(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let (dirs, files): (Vec<PathBuf>, Vec<PathBuf>) = paths.paths().iter().cloned().partition(|p| p.is_dir());
        for f in files.into_iter().filter(|f| f.is_file()) {
            self.open_file(f, window, cx);
        }
        if dirs.is_empty() {
            return;
        }
        if self.root.is_none() {
            let mut it = dirs.into_iter();
            if let Some(first) = it.next() {
                self.open_folder(first, cx);
            }
            self.add_folders(it.collect(), cx);
        } else {
            self.workspace.ask = Some(dirs);
            self.drop_focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn add_folders(&mut self, dirs: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut added = 0;
        for d in dirs {
            if self.root.as_ref() != Some(&d) && !self.workspace.extra.contains(&d) {
                self.workspace.extra.push(d);
                added += 1;
            }
        }
        if added > 0 {
            self.status = crate::trf!("Added {n} to the workspace"; n => if added == 1 { tr("1 folder").to_string() } else { crate::trf!("{n} folders"; n => added) }).into();
            self.refresh_tree(cx);
            self.save_session(cx);
        }
        cx.notify();
    }

    /// Switch the window to the first folder (the others join it).
    fn switch_to(&mut self, dirs: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut it = dirs.into_iter();
        let Some(first) = it.next() else { return };
        self.workspace.extra.clear();
        self.workspace.extra.extend(it);
        self.open_folder(first, cx);
    }

    pub(crate) fn remove_folder(&mut self, path: &Path, cx: &mut Context<Self>) {
        let before = self.workspace.extra.len();
        self.workspace.extra.retain(|p| p != path);
        if self.workspace.extra.len() != before {
            self.status = crate::trf!("Removed {name} from the workspace"; name => name_of(path)).into();
            self.refresh_tree(cx);
            self.save_session(cx);
            cx.notify();
        }
    }

    fn answer_drop(&mut self, add: Option<bool>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dirs) = self.workspace.ask.take() else { return };
        match add {
            Some(true) => self.add_folders(dirs, cx),
            Some(false) => self.switch_to(dirs, cx),
            None => {}
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Faint frame over the editor while files are dragged over the window.
    pub(crate) fn render_drop_hint(&self) -> impl IntoElement {
        div()
            .id("drop-hint")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .opacity(0.)
            .flex()
            .items_center()
            .justify_center()
            .border_2()
            .border_color(c(theme::ACCENT))
            .bg(c(theme::BG_EDITOR).opacity(0.85))
            .drag_over::<ExternalPaths>(|s, _, _, _| s.opacity(1.))
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .text_color(c(theme::FG_STRONG))
                    .child(Icon::new(IconName::FolderOpen).size(theme::u(28.)).text_color(c(theme::ACCENT)))
                    .child(div().text_sm().child(tr("Drop folders to open them, or files to edit them"))),
            )
    }

    /// "Add to workspace" or "switch to it", for folders dropped while one is open.
    pub(crate) fn render_drop_ask(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dirs = self.workspace.ask.as_ref()?;
        let what = match dirs.as_slice() {
            [one] => name_of(one),
            many => crate::trf!("{n} folders"; n => many.len()),
        };
        let current = self.root.as_deref().map(name_of).unwrap_or_default();
        let fade = || Animation::new(std::time::Duration::from_millis(120)).with_easing(ease_out_quint());
        let body = if dirs.len() == 1 {
            crate::trf!("Keep {current} open and add {what} next to it, or close {current} and work in {what}."; current => current, what => what)
        } else {
            crate::trf!("Keep {current} open and add them next to it, or close {current} and work in them."; current => current)
        };
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(
                    div()
                        .id("drop-scrim")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(gpui_kit::black().opacity(0.38))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.answer_drop(None, w, cx)))
                        .with_animation("drop-scrim-in", fade(), |d, t| d.opacity(t)),
                )
                .child(
                    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(
                        v_flex()
                            .id("drop-ask")
                            // Clicks inside stay here instead of reaching the backdrop (which cancels).
                            .occlude()
                            .track_focus(&self.drop_focus)
                            .w(theme::u(440.))
                            .max_w(relative(0.92))
                            .p_4()
                            .gap_3()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(c(theme::BORDER_STRONG))
                            .bg(gpui_kit::Hsla::from(rgb(theme::palette().swatches()[1])))
                            .shadow_lg()
                            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, w, cx| match ev.keystroke.key.as_str() {
                                "escape" => this.answer_drop(None, w, cx),
                                "enter" => this.answer_drop(Some(true), w, cx),
                                _ => {}
                            }))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Icon::new(IconName::FolderOpen).size(theme::u(16.)).text_color(c(theme::ACCENT)))
                                    .child(div().min_w_0().truncate().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::SEMIBOLD).child(crate::trf!("Open {what}"; what => what))),
                            )
                            .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(body))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .pt_1()
                                    .child(Button::new("drop-add").label(tr("Add to workspace")).small().primary().on_click(cx.listener(|this, _, w, cx| this.answer_drop(Some(true), w, cx))))
                                    .child(Button::new("drop-switch").label(tr("Switch to it")).small().on_click(cx.listener(|this, _, w, cx| this.answer_drop(Some(false), w, cx))))
                                    .child(div().flex_1())
                                    .child(Button::new("drop-cancel").label(tr("Cancel")).small().ghost().on_click(cx.listener(|this, _, w, cx| this.answer_drop(None, w, cx)))),
                            )
                            .with_animation("drop-ask-in", fade(), |d, t| d.opacity(t)),
                    ),
                )
                .into_any_element(),
        )
    }
}
