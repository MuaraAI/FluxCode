//! Blocky building blocks: hard shadows, chunky buttons that lift on hover
//! (spring animation), entrance animations.

use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::*;
use std::time::Duration;

/// A neo-brutalist hard shadow: no blur, offset down-right.
pub fn hard_shadow(offset: f32, color: u32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: c(color),
        offset: point(px(offset), px(offset)),
        blur_radius: px(0.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// Overshoots a little past the end, then settles: a block "landing".
pub fn ease_out_back(t: f32) -> f32 {
    let (c1, c3) = (1.70158f32, 2.70158f32);
    1. + c3 * (t - 1.).powi(3) + c1 * (t - 1.).powi(2)
}

/// Springy, slightly bouncy: used for hover lifts.
pub fn spring() -> SpringAnimation {
    SpringAnimation::new(SpringConfig::new(520., 24., 1.))
}

/// Fade + slide in from `dx, dy` pixels when the element first appears (or its id changes).
pub fn enter<E: Styled + IntoElement + 'static>(el: E, id: impl Into<ElementId>, dx: f32, dy: f32, ms: u64) -> AnimationElement<E> {
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(ms)).with_easing(ease_out_quint()),
        move |el, t| el.opacity(t).relative().left(px(dx * (1. - t))).top(px(dy * (1. - t))),
    )
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Yellow block, black text.
    Primary,
    /// Dark block, light text.
    Plain,
    /// Pink block, black text.
    Danger,
}

impl Workbench {
    pub(crate) fn hover_cb(&self, key: SharedString, cx: &mut Context<Self>) -> Box<dyn Fn(&bool, &mut Window, &mut App) + 'static> {
        Box::new(cx.listener(move |this, on: &bool, _, cx| {
            if *on {
                this.hovered = Some(key.clone());
            } else if this.hovered.as_ref() == Some(&key) {
                this.hovered = None;
            }
            cx.notify();
        }))
    }

    pub(crate) fn is_hovered(&self, key: &str) -> bool {
        self.hovered.as_deref() == Some(key)
    }

    /// A chunky square button with a hard shadow that lifts on hover and
    /// presses flat on click.
    pub(crate) fn block_button(
        &self,
        key: &'static str,
        label: impl Into<SharedString>,
        kind: Kind,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
    ) -> impl IntoElement {
        let (bg, fg) = match kind {
            Kind::Primary => (theme::ACCENT, theme::INK),
            Kind::Plain => (theme::BG_ELEVATED, theme::FG_STRONG),
            Kind::Danger => (theme::PINK, theme::INK),
        };
        let hovered = self.is_hovered(key);
        div()
            .id(key)
            .flex_none()
            .px_3()
            .py(px(3.))
            .bg(c(bg))
            .text_color(c(fg))
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .border_2()
            .border_color(c(theme::INK))
            .cursor_pointer()
            .child(label.into())
            .on_hover(self.hover_cb(key.into(), cx))
            .active(|s| s.shadow(hard_shadow(0., theme::INK)))
            .on_click(cx.listener(move |this, _, w, cx| on_click(this, w, cx)))
            .with_spring(key, spring().to(if hovered { 1.0f32 } else { 0.0 }), |el, v| {
                el.relative().top(px(-2. * v)).left(px(-2. * v)).shadow(hard_shadow(2. + 2. * v, theme::INK))
            })
    }
}
