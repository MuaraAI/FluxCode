use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::{AddFolderToWorkspace, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Default)]
pub(crate) struct Workspace {
    pub extra: Vec<PathBuf>,
    pub ask: Option<Vec<PathBuf>>,
    pub ask_closing: Option<(Vec<PathBuf>, u64)>,
    pub ask_seq: u64,
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

pub(crate) fn rel_display_for(full: &str, roots: &[PathBuf], multi: bool) -> String {
    for r in roots {
        let rs = r.to_string_lossy();
        if let Some(rest) = full.strip_prefix(rs.as_ref()) {
            if rest.is_empty() || rest.starts_with(['\\', '/']) {
                let rest = rest.trim_start_matches(['\\', '/']);
                return if multi { format!("{}\\{rest}", name_of(r)) } else { rest.to_string() };
            }
        }
    }
    full.to_string()
}

impl Workbench {
    pub(crate) fn roots(&self) -> Vec<PathBuf> {
        self.root.iter().cloned().chain(self.workspace.extra.iter().cloned()).collect()
    }

    pub(crate) fn rel_display(&self, full: &str) -> String {
        rel_display_for(full, &self.roots(), !self.workspace.extra.is_empty())
    }

    pub(crate) fn on_add_folder_to_workspace(&mut self, _: &AddFolderToWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Add Folder to Workspace".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let dirs = paths.await.ok()?.ok()??;
            if dirs.is_empty() {
                return None;
            }
            this.update(cx, |this, cx| {
                if this.root.is_none() {
                    let mut it = dirs.into_iter();
                    if let Some(first) = it.next() {
                        this.open_folder(first, cx);
                    }
                    this.add_folders(it.collect(), cx);
                } else {
                    this.add_folders(dirs, cx);
                }
            })
            .ok()
        })
        .detach();
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
            self.workspace.ask_closing = None;
            self.workspace.ask_seq = self.workspace.ask_seq.wrapping_add(1);
            self.workspace.ask = Some(dirs);
            self.drop_focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn add_folders(&mut self, dirs: Vec<PathBuf>, cx: &mut Context<Self>) {
        let was_single = self.workspace.extra.is_empty();
        let mut added_ids = Vec::new();
        for d in dirs {
            if self.root.as_ref() != Some(&d) && !self.workspace.extra.contains(&d) {
                let id = d.to_string_lossy().into_owned();
                self.expanded.insert(id.clone());
                added_ids.push(id);
                self.workspace.extra.push(d);
            }
        }
        let added = added_ids.len();
        if added > 0 {
            if was_single {
                if let Some(r) = &self.root {
                    self.expanded.insert(r.to_string_lossy().into_owned());
                }
            }
            self.sidebar_open = true;
            self.view = crate::workbench::SideView::Explorer;
            self.explorer.bump_workspace_anim(added_ids);
            self.status = crate::trf!(
                "Added {n} to the workspace";
                n => if added == 1 { tr("1 folder").to_string() } else { crate::trf!("{n} folders"; n => added) }
            )
            .into();
            self.refresh_tree(cx);
            self.save_session(cx);
        }
        cx.notify();
    }

    fn switch_to(&mut self, dirs: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut it = dirs.into_iter();
        let Some(first) = it.next() else { return };
        self.workspace.extra.clear();
        let extras: Vec<PathBuf> = it.collect();
        self.workspace.extra.extend(extras.clone());
        self.open_folder(first.clone(), cx);
        if !extras.is_empty() {
            self.expanded.insert(first.to_string_lossy().into_owned());
            for d in &extras {
                self.expanded.insert(d.to_string_lossy().into_owned());
            }
        }
    }

    pub(crate) fn remove_folder(&mut self, path: &Path, cx: &mut Context<Self>) {
        let before = self.workspace.extra.len();
        self.workspace.extra.retain(|p| p != path);
        if self.workspace.extra.len() != before {
            let prefix = path.to_string_lossy().into_owned();
            self.expanded.retain(|e| e != &prefix && !e.starts_with(&format!("{prefix}\\")) && !e.starts_with(&format!("{prefix}/")));
            self.explorer.bump_workspace_anim(Vec::new());
            self.status = crate::trf!("Removed {name} from the workspace"; name => name_of(path)).into();
            self.refresh_tree(cx);
            self.save_session(cx);
            cx.notify();
        }
    }

    fn answer_drop(&mut self, add: Option<bool>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dirs) = self.workspace.ask.take() else { return };
        if !cx.reduce_motion() {
            self.workspace.ask_seq = self.workspace.ask_seq.wrapping_add(1);
            let seq = self.workspace.ask_seq;
            self.workspace.ask_closing = Some((dirs.clone(), seq));
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(145)).await;
                _ = this.update(cx, |this, cx| {
                    if this.workspace.ask_closing.as_ref().is_some_and(|(_, s)| *s == seq) {
                        this.workspace.ask_closing = None;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        match add {
            Some(true) => self.add_folders(dirs, cx),
            Some(false) => self.switch_to(dirs, cx),
            None => {}
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

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

    pub(crate) fn render_drop_ask(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (dirs, closing, seq) = match (&self.workspace.ask, &self.workspace.ask_closing) {
            (Some(d), _) => (d, false, self.workspace.ask_seq),
            (None, Some((d, s))) => (d, true, *s),
            (None, None) => return None,
        };
        let what = match dirs.as_slice() {
            [one] => name_of(one),
            many => crate::trf!("{n} folders"; n => many.len()),
        };
        let current = self.root.as_deref().map(name_of).unwrap_or_default();
        let motion = !cx.reduce_motion();
        let dur = Duration::from_millis(if closing { 140 } else { 180 });
        let body = if dirs.len() == 1 {
            crate::trf!(
                "Keep {current} open and add {what} next to it, or close {current} and work in {what}.";
                current => current,
                what => what
            )
        } else {
            crate::trf!(
                "Keep {current} open and add them next to it, or close {current} and work in them.";
                current => current
            )
        };
        let scrim = div()
            .id("drop-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui_kit::black().opacity(0.38))
            .when(!closing, |d| {
                d.on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.answer_drop(None, w, cx)))
            });
        let scrim_el = if motion {
            scrim
                .with_animation(
                    SharedString::from(format!("drop-scrim-{closing}-{seq}")),
                    Animation::new(dur).with_easing(ease_out_quint()),
                    move |d, t| d.opacity(if closing { 1. - t } else { t }),
                )
                .into_any_element()
        } else {
            scrim.into_any_element()
        };

        let card = v_flex()
            .id("drop-ask")
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
            .when(!closing, |d| {
                d.on_key_down(cx.listener(|this, ev: &KeyDownEvent, w, cx| match ev.keystroke.key.as_str() {
                    "escape" => this.answer_drop(None, w, cx),
                    "enter" => this.answer_drop(Some(true), w, cx),
                    _ => {}
                }))
            })
            .child(
                h_flex()
                    .gap_2()
                    .child(Icon::new(IconName::FolderOpen).size(theme::u(16.)).text_color(c(theme::ACCENT)))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(c(theme::FG_STRONG))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(crate::trf!("Open {what}"; what => what)),
                    ),
            )
            .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(body))
            .child(
                h_flex()
                    .gap_2()
                    .pt_1()
                    .child(
                        Button::new("drop-add")
                            .label(tr("Add to workspace"))
                            .small()
                            .primary()
                            .on_click(cx.listener(|this, _, w, cx| this.answer_drop(Some(true), w, cx))),
                    )
                    .child(
                        Button::new("drop-switch")
                            .label(tr("Switch to it"))
                            .small()
                            .on_click(cx.listener(|this, _, w, cx| this.answer_drop(Some(false), w, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("drop-cancel")
                            .label(tr("Cancel"))
                            .small()
                            .ghost()
                            .on_click(cx.listener(|this, _, w, cx| this.answer_drop(None, w, cx))),
                    ),
            );

        let card_el = if motion {
            card.with_animation(
                SharedString::from(format!("drop-ask-{closing}-{seq}")),
                Animation::new(dur).with_easing(ease_out_quint()),
                move |d, t| {
                    let a = if closing { 1. - t } else { t };
                    let y = if closing { t * 8. } else { (1. - t) * 10. };
                    d.opacity(a).mt(px(y))
                },
            )
            .into_any_element()
        } else {
            card.into_any_element()
        };

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(scrim_el)
                .child(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(card_el))
                .into_any_element(),
        )
    }
}
