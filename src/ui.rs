
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::{prelude::FluentBuilder, *};

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Primary,
    Plain,
}

impl Workbench {
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

pub fn tool_button(id: impl Into<ElementId>, icon: impl Into<component::Icon>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(theme::u(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(c(if on { theme::ACCENT_TEXT } else { theme::FG_MUTED }))
        .when(on, |d| d.bg(c(theme::BG_HOVER)))
        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(if on { theme::ACCENT_TEXT } else { theme::FG_STRONG })))
        .active(|s| s.bg(c(theme::BG_ELEVATED)))
        .child(icon.into().size(theme::u(14.)))
}

pub fn meander(size: f32, a: Hsla, b: Hsla) -> Div {
    const HALF_A: [(f32, f32); 6] = [(2., 8.), (8., 8.), (8., 2.), (2., 2.), (2., 4.), (4., 4.)];
    const HALF_B: [(f32, f32); 6] = [(4., 4.), (6., 4.), (6., 6.), (0., 6.), (0., 0.), (6., 0.)];
    let w = (size * theme::scale() / 9.).round() * 9.;
    let u = w / 9.;
    let mut root = div().relative().flex_none().size(px(w));
    for (path, col) in [(&HALF_A, a), (&HALF_B, b)] {
        for pair in path.windows(2) {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
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

pub struct SegItem {
    pub id: SharedString,
    pub label: SharedString,
    pub active: bool,
    pub on_click: std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
}

impl SegItem {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        active: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), label: label.into(), active, on_click: std::rc::Rc::new(on_click) }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct SegRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl SegRect {
    fn lerp(self, to: Self, t: f32) -> Self {
        Self {
            x: self.x + (to.x - self.x) * t,
            y: self.y + (to.y - self.y) * t,
            w: self.w + (to.w - self.w) * t,
            h: self.h + (to.h - self.h) * t,
        }
    }

    fn close_to(self, other: Self) -> bool {
        (self.x - other.x).abs() < 0.5
            && (self.y - other.y).abs() < 0.5
            && (self.w - other.w).abs() < 0.5
            && (self.h - other.h).abs() < 0.5
    }
}

#[derive(Default)]
struct SegState {
    origin: Option<Point<Pixels>>,
    rects: Vec<Option<Bounds<Pixels>>>,
    active: Option<usize>,
    from: Option<SegRect>,
    to: Option<SegRect>,
    seq: u64,
    started: Option<std::time::Instant>,
}

const SEG_GLIDE: std::time::Duration = std::time::Duration::from_millis(190);

fn ease_out_quint(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(5)
}

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: SharedString,
    items: Vec<SegItem>,
}

impl SegmentedControl {
    pub fn new(id: impl Into<SharedString>, items: impl IntoIterator<Item = SegItem>) -> Self {
        Self { id: id.into(), items: items.into_iter().collect() }
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(SharedString::from(format!("seg-st-{}", self.id)), cx, |_, _| {
            std::rc::Rc::new(std::cell::RefCell::new(SegState::default()))
        });
        let st_rc = state.read(cx).clone();
        let active_ix = self.items.iter().position(|it| it.active);
        let motion = !cx.reduce_motion();
        let count = self.items.len();

        let (glide, has_pill) = {
            let mut st = st_rc.borrow_mut();
            if st.rects.len() != count {
                st.rects.resize(count, None);
            }
            let measured = active_ix.and_then(|ix| {
                let o = st.origin?;
                let b = st.rects.get(ix).copied().flatten()?;
                Some(SegRect {
                    x: f32::from(b.origin.x - o.x),
                    y: f32::from(b.origin.y - o.y),
                    w: f32::from(b.size.width),
                    h: f32::from(b.size.height),
                })
            });
            let now = std::time::Instant::now();
            if st.active != active_ix {
                let cur = match (st.from, st.to, st.started) {
                    (Some(f), Some(t), Some(s)) => {
                        let p = (now.saturating_duration_since(s).as_secs_f32() / SEG_GLIDE.as_secs_f32()).min(1.);
                        Some(f.lerp(t, ease_out_quint(p)))
                    }
                    (_, to, _) => to,
                };
                st.active = active_ix;
                st.from = cur.or(measured);
                st.to = measured;
                st.seq = st.seq.wrapping_add(1);
                st.started = Some(now);
            } else if let Some(m) = measured {
                if st.to.is_none_or(|prev| !prev.close_to(m)) {
                    st.from = Some(m);
                    st.to = Some(m);
                    st.started = None;
                }
            }
            let glide = match (st.from, st.to) {
                (Some(from), Some(to)) if motion => Some((from, to, st.seq, !from.close_to(to))),
                _ => None,
            };
            (glide, glide.is_some())
        };

        let view_id = window.current_view();
        let origin_probe = {
            let st_rc = st_rc.clone();
            canvas(
                move |bounds, _, cx| {
                    let mut st = st_rc.borrow_mut();
                    let first = st.origin.is_none();
                    st.origin = Some(bounds.origin);
                    if first {
                        cx.defer(move |cx| cx.notify(view_id));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
            .size_full()
        };

        let pill: Option<AnyElement> = glide.map(|(from, to, seq, animate)| {
            let base = div().absolute().rounded(theme::u(5.)).bg(c(theme::ACCENT));
            if animate {
                base.with_animation(
                    SharedString::from(format!("seg-pill-{}-{seq}", self.id)),
                    Animation::new(SEG_GLIDE).with_easing(gpui_kit::ease_out_quint()),
                    move |el, t| {
                        let r = from.lerp(to, t);
                        el.left(px(r.x)).top(px(r.y)).w(px(r.w)).h(px(r.h))
                    },
                )
                .into_any_element()
            } else {
                base.left(px(to.x)).top(px(to.y)).w(px(to.w)).h(px(to.h)).into_any_element()
            }
        });

        let buttons = self.items.into_iter().enumerate().map(|(i, item)| {
            let st_rc = st_rc.clone();
            let probe = canvas(
                move |bounds, _, _| {
                    let mut st = st_rc.borrow_mut();
                    if i < st.rects.len() {
                        st.rects[i] = Some(bounds);
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
            .size_full();
            let fh = window
                .use_keyed_state(SharedString::from(format!("seg-fh-{}", item.id)), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone();
            let focused = fh.is_focused(window);
            let on_click = item.on_click.clone();
            let on_key = item.on_click.clone();
            div()
                .id(item.id)
                .relative()
                .flex_none()
                .h(theme::u(24.))
                .px_2()
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::u(5.))
                .text_xs()
                .font_weight(if item.active { FontWeight::MEDIUM } else { FontWeight::NORMAL })
                .cursor_pointer()
                .track_focus(&fh)
                .tab_stop(true)
                .border_1()
                .border_color(if focused { c(theme::ACCENT) } else { gpui::transparent_black() })
                .when(item.active && !has_pill, |d| d.bg(c(theme::ACCENT)))
                .text_color(if item.active { c(theme::ON_ACCENT) } else { c(theme::FG_MUTED) })
                .when(!item.active, |d| d.hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))))
                .on_click(move |ev, w, cx| (on_click)(ev, w, cx))
                .on_key_down(move |ev: &KeyDownEvent, w, cx| {
                    if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        (on_key)(&ClickEvent::Keyboard(gpui::KeyboardClickEvent::default()), w, cx);
                    }
                })
                .child(probe)
                .child(item.label)
        });

        gpui_kit::component::h_flex()
            .id(self.id)
            .relative()
            .p(px(2.))
            .gap(px(2.))
            .flex_wrap()
            .rounded(theme::u(7.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_SIDE))
            .child(origin_probe)
            .children(pill)
            .children(buttons)
    }
}

#[cfg(test)]
mod tests {
    use super::{SegRect, ease_out_quint};

    #[test]
    fn segmented_rect_lerp_reaches_endpoints() {
        let a = SegRect { x: 0., y: 2., w: 60., h: 24. };
        let b = SegRect { x: 64., y: 2., w: 80., h: 24. };
        assert!(a.lerp(b, 0.).close_to(a));
        assert!(a.lerp(b, 1.).close_to(b));
        let mid = a.lerp(b, ease_out_quint(0.5));
        assert!(mid.x > 60. && mid.x < 64.);
    }
}
