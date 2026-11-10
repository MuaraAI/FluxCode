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
        }
    }
}

/// The FluxCode mark, the meander from the app icon, drawn from rectangles so it
/// needs no image file and takes theme colors. `a` is the first hook (ivory in the
/// icon), `b` the second (ochre). `size` is the width in px before UI scaling.
///
/// The line runs on a 0..8 grid with stroke and gap of one ninth of the width each;
/// it is point-symmetric about the centre, where the two hooks meet.
pub fn meander(size: f32, a: Hsla, b: Hsla) -> Div {
    const HALF_A: [(f32, f32); 6] = [(2., 8.), (8., 8.), (8., 2.), (2., 2.), (2., 4.), (4., 4.)];
    const HALF_B: [(f32, f32); 6] = [(4., 4.), (6., 4.), (6., 6.), (0., 6.), (0., 0.), (6., 0.)];
    let w = (size * theme::scale() / 9.).round() * 9.;
    let u = w / 9.;
    let mut root = div().relative().flex_none().size(px(w));
    for (path, col) in [(&HALF_A, a), (&HALF_B, b)] {
        for pair in path.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            // Grid coordinate -> pixel offset of the line's near edge; the centre bar is split in half.
            let (left, top, width, height) = if x0 == x1 {
                (x0 * u, y0.min(y1) * u, u, (y0 - y1).abs() * u + u)
            } else {
                let (l, r) = (x0.min(x1), x0.max(x1));
                let le = if l == 4. { 4.5 * u } else { l * u };
                let re = if r == 4. { 4.5 * u } else { r * u + u };
                (le, y0 * u, re - le, u)
            };
            root = root.child(div().absolute().left(px(left)).top(px(top)).w(px(width)).h(px(height)).bg(col));
        }
    }
    root
}
