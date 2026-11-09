//! Small shared building blocks: the text button used on the welcome screen,
//! in the Run view, the explorer and Settings.
//!
//! Motion is limited to hover and pressed states (no lifts, bounces or entrance
//! animations): an editor is looked at for hours, so the chrome stays still.

use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Accent fill: the one main action on a screen.
    Primary,
    /// Outlined: secondary actions.
    Plain,
    /// Outlined in the danger color: stop, delete.
    Danger,
}

impl Workbench {
    /// A text button: accent fill for the primary action, an outline otherwise.
    pub(crate) fn action_button(
        &self,
        key: &'static str,
        label: impl Into<SharedString>,
        kind: Kind,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
    ) -> impl IntoElement {
        let base = div()
            .id(key)
            .flex_none()
            .h(px(30.))
            .px_3()
            .flex()
            .items_center()
            .rounded(px(4.))
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .border_1()
            .cursor_pointer()
            .child(label.into())
            .on_click(cx.listener(move |this, _, w, cx| on_click(this, w, cx)));
        match kind {
            Kind::Primary => base
                .bg(c(theme::ACCENT))
                .border_color(c(theme::ACCENT))
                .text_color(c(theme::ON_ACCENT))
                .hover(|s| s.opacity(0.88))
                .active(|s| s.opacity(0.76)),
            Kind::Plain => base
                .border_color(c(theme::BORDER_STRONG))
                .text_color(c(theme::FG_STRONG))
                .hover(|s| s.bg(c(theme::BG_HOVER)))
                .active(|s| s.bg(c(theme::BG_ELEVATED))),
            Kind::Danger => base
                .border_color(c(theme::DANGER))
                .text_color(c(theme::DANGER))
                .hover(|s| s.bg(c(theme::BG_HOVER)))
                .active(|s| s.bg(c(theme::BG_ELEVATED))),
        }
    }
}
