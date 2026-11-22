//! Live Preview panel (right of the editor).

use crate::i18n::tr;
use crate::preview::PreviewServer;
#[allow(unused_imports)]
use crate::theme::{self, c};
use crate::workbench::*;
#[allow(unused_imports)]
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Icon, IconName, Sizable,
};
use gpui_kit::*;

impl Workbench {
    pub(crate) fn on_toggle_preview(&mut self, _: &TogglePreview, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ext_installed("html-preview") {
            self.status = tr("The preview is the HTML Preview extension: install it to use it").into();
            self.ext_open_page("html-preview", cx);
            return;
        }
        #[cfg(feature = "embedded-preview")]
        if self.preview.take().is_some() {
            cx.notify();
            return;
        }
        // The preview runs a WebView, not the native UI: say so first, and only start the preview after the user continues.
        #[cfg(feature = "embedded-preview")]
        if self.preview_ask {
            return;
        }
        #[cfg(feature = "embedded-preview")]
        if self.settings.preview_webview_notice {
            self.preview_ask = true;
            self.preview_ask_hide = false;
            cx.notify();
            return;
        }
        self.open_preview(window, cx);
    }

    /// The notice's answer: Continue starts the preview, Cancel opens nothing.
    #[cfg(feature = "embedded-preview")]
    fn answer_preview_ask(&mut self, go: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview_ask {
            return;
        }
        self.preview_ask = false;
        if self.preview_ask_hide {
            self.settings.preview_webview_notice = false;
            self.settings.save();
        }
        self.preview_ask_hide = false;
        if go {
            self.open_preview(window, cx);
        } else {
            cx.notify();
        }
    }

    /// The part of the toggle that starts the preview: the local server, then the WebView in the embedded build, the system browser otherwise.
    fn open_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let file = self.active.map(|a| self.docs[a].path.clone());
        let Some(root) = self.root.clone().or_else(|| file.as_ref().and_then(|f| f.parent().map(|p| p.to_path_buf()))) else {
            self.status = tr("Open a folder or an HTML file to preview it").into();
            cx.notify();
            return;
        };
        if self.preview_server.as_ref().is_none_or(|s| !s.is_root(&root)) {
            match PreviewServer::start(&root) {
                Ok(s) => self.preview_server = Some(s),
                Err(e) => {
                    self.status = crate::trf!("Preview server failed: {e}"; e => e).into();
                    cx.notify();
                    return;
                }
            }
        }
        let server = self.preview_server.as_ref().unwrap();
        let html = file.filter(|f| matches!(f.extension().and_then(|e| e.to_str()), Some("html" | "htm")));
        let url = server.url_for(html.as_deref());
        #[cfg(feature = "embedded-preview")]
        {
            let builder = wry::WebViewBuilder::new().with_url(&url);
            match gpui_webview::WebView::build(builder, window, cx) {
                Ok(view) => {
                    let entity = cx.new(|_| view);
                    self.preview = Some((entity, url));
                }
                Err(e) => self.status = crate::trf!("WebView2 is not available: {e}"; e => e).into(),
            }
        }
        // Default build: no WebView in the process at all.
        #[cfg(not(feature = "embedded-preview"))]
        {
            let _ = window;
            cx.open_url(&url);
            self.status = crate::trf!("Live Preview: {url} (reloads on save)"; url => url).into();
        }
        cx.notify();
    }

    pub(crate) fn on_reload_preview(&mut self, _: &ReloadPreview, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = &self.preview_server {
            s.reload();
        }
        cx.notify();
    }

    /// The WebView notice, before the preview runs: a blocking dialog, same shape as the folder-drop question (scrim, card, Escape to cancel).
    #[cfg(feature = "embedded-preview")]
    pub(crate) fn render_preview_ask(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.preview_ask {
            return None;
        }
        let hide = self.preview_ask_hide;
        let fade = || Animation::new(std::time::Duration::from_millis(120)).with_easing(ease_out_quint());
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(
                    div()
                        .id("preview-ask-scrim")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(gpui_kit::black().opacity(0.38))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.answer_preview_ask(false, w, cx)))
                        .with_animation("preview-ask-scrim-in", fade(), |d, t| d.opacity(t)),
                )
                .child(
                    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(
                        v_flex()
                            .id("preview-ask")
                            // Clicks inside stay here instead of reaching the backdrop (which cancels).
                            .occlude()
                            .track_focus(&self.preview_ask_focus)
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
                                "escape" => this.answer_preview_ask(false, w, cx),
                                "enter" => this.answer_preview_ask(true, w, cx),
                                _ => {}
                            }))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Icon::new(IconName::Globe).size(theme::u(16.)).text_color(c(theme::ACCENT)))
                                    .child(div().min_w_0().truncate().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::SEMIBOLD).child(tr("HTML preview runs a WebView"))),
                            )
                            .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(tr("This preview opens a WebView (WebView2), not FluxCode's native Rust UI. The page runs in the system's web view, so it is not part of the native editor.")))
                            .child(
                                h_flex()
                                    .id("preview-ask-hide")
                                    .gap_2()
                                    .items_center()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.preview_ask_hide = !this.preview_ask_hide;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .flex_none()
                                            .size(theme::u(18.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(px(4.))
                                            .border_1()
                                            .border_color(c(if hide { theme::ACCENT } else { theme::BORDER_STRONG }))
                                            .when(hide, |d| d.bg(c(theme::ACCENT)))
                                            .when(hide, |d| d.child(Icon::new(IconName::Check).size(theme::u(12.)).text_color(c(theme::ON_ACCENT)))),
                                    )
                                    .child(div().text_sm().text_color(c(theme::FG)).child(tr("Do not show again"))),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .pt_1()
                                    .child(div().flex_1())
                                    .child(Button::new("preview-ask-cancel").label(tr("Cancel")).small().ghost().on_click(cx.listener(|this, _, w, cx| this.answer_preview_ask(false, w, cx))))
                                    .child(Button::new("preview-ask-continue").label(tr("Continue")).small().primary().on_click(cx.listener(|this, _, w, cx| this.answer_preview_ask(true, w, cx)))),
                            )
                            .with_animation("preview-ask-in", fade(), |d, t| d.opacity(t)),
                    ),
                )
                .into_any_element(),
        )
    }

    #[cfg(not(feature = "embedded-preview"))]
    pub(crate) fn render_preview_ask(&self, _: &mut Context<Self>) -> Option<AnyElement> {
        None
    }

    #[cfg(not(feature = "embedded-preview"))]
    pub(crate) fn render_preview(&self, _: &mut Context<Self>) -> AnyElement {
        div().into_any_element()
    }

    #[cfg(feature = "embedded-preview")]
    pub(crate) fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some((view, url)) = &self.preview else { return div().into_any_element() };
        v_flex()
            .size_full()
            .border_l_1()
            .border_color(c(theme::BORDER))
            .child(
                h_flex()
                    .h(px(35.))
                    .px_2()
                    .gap_1()
                    .flex_none()
                    .bg(c(theme::BG_SIDE))
                    .text_xs()
                    .child(div().flex_1().truncate().text_color(c(theme::FG_MUTED)).child(url.clone()))
                    .child(tool_button("pv-reload", IconName::RefreshCw, "Reload", cx, |this, w, cx| this.on_reload_preview(&ReloadPreview, w, cx)))
                    .child(tool_button("pv-close", IconName::Close, "Close Preview", cx, |this, w, cx| this.on_toggle_preview(&TogglePreview, w, cx))),
            )
            .child(div().flex_1().min_h_0().bg(gpui_kit::white()).child(view.clone()))
            .into_any_element()
    }
}
