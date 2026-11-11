//! Live Preview panel (right of the editor).

use crate::i18n::tr;
use crate::preview::PreviewServer;
#[allow(unused_imports)]
use crate::theme::{self, c};
use crate::workbench::*;
#[allow(unused_imports)]
use gpui_kit::component::{h_flex, v_flex, IconName};
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
        let file = self.active.map(|a| self.docs[a].path.clone());
        let Some(root) = self.root.clone().or_else(|| file.as_ref().and_then(|f| f.parent().map(|p| p.to_path_buf()))) else {
            self.status = tr("Open a folder or an HTML file to preview it").into();
            cx.notify();
            return;
        };
        if self.preview_server.is_none() {
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
        server.set_root(&root);
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
        // Default build: no WebView in the process at all. The page opens in the
        // system browser and still reloads itself after every save.
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
