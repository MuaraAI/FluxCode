
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{
    input::{Input, InputState},
    Sizable, Size, StyledExt,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

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
        let handle = liquid::keyed_state(SharedString::from(format!("seg-st-{}", self.id)), window, cx);
        let active_ix = self.items.iter().position(|it| it.active);
        let motion = !cx.reduce_motion();
        let count = self.items.len();
        let (frame, has_active_liq) = liquid::keyed_sync_and_sample(&handle, false, count, active_ix, 24.0, 200, motion);
        let view_id = window.current_view();
        let origin_probe = liquid::keyed_origin_probe(handle.clone(), view_id);

        let liquid_layer: Option<AnyElement> = frame.map(|fr| {
            let style = liquid::LiquidStyle {
                inset_a: 2.0,
                inset_b: 2.0,
                cross_base: 24.0,
                max_squeeze: 2.0,
                radius: f32::from(theme::u(5.)),
                capillary: 22.0,
                active_bg: c(theme::ACCENT),
                active_border: c(theme::ACCENT),
                hover_bg: c(theme::BG_HOVER).opacity(0.82),
                hover_border: c(theme::BORDER_STRONG).opacity(0.75),
                union_border: c(theme::ACCENT).opacity(0.65),
                bridge_bg: c(theme::ACCENT).opacity(0.55),
            };
            liquid::render_coalesced_layer(SharedString::from(format!("seg-liq-{}", self.id)), false, style, fr)
        });

        let buttons = self.items.into_iter().enumerate().map(|(i, item)| {
            let probe = liquid::keyed_item_probe(handle.clone(), i, view_id);
            let fh = window
                .use_keyed_state(SharedString::from(format!("seg-fh-{}", item.id)), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone();
            let focused = fh.is_focused(window);
            let on_click = item.on_click.clone();
            let on_key = item.on_click.clone();
            let h_hov = handle.clone();
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
                .when(item.active && !has_active_liq, |d| d.bg(c(theme::ACCENT)))
                .text_color(if item.active { c(theme::ON_ACCENT) } else { c(theme::FG_MUTED) })
                .when(!item.active && !motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))))
                .when(!item.active && motion, |d| d.hover(|s| s.text_color(c(theme::FG_STRONG))))
                .on_hover(move |entered: &bool, _, cx| {
                    if *entered && motion {
                        liquid::keyed_hover_enter(&h_hov, i, false, 24.0, 155, view_id, cx);
                    }
                })
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

        let h_leave = handle.clone();
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
            .on_hover(move |entered: &bool, _, cx| {
                if !*entered && motion {
                    liquid::keyed_hover_leave(&h_leave, view_id, cx);
                }
            })
            .child(origin_probe)
            .children(liquid_layer)
            .children(buttons)
    }
}

struct McToggleMem {
    checked: bool,
    from_on: bool,
    seq: u64,
    hovered: bool,
}

type ToggleClick = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// KiwEssentials Minecraft Ore-UI toggle switch with underdamped jelly bounce physics.
#[derive(IntoElement)]
pub struct McToggle {
    id: SharedString,
    checked: bool,
    small: bool,
    accessibility_label: Option<SharedString>,
    on_click: Option<ToggleClick>,
}

impl McToggle {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            small: false,
            accessibility_label: None,
            on_click: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for McToggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let disabled = false;
        let small = self.small;
        let id = self.id;
        let view_id = window.current_view();
        let mem = window
            .use_keyed_state(
                SharedString::from(format!("mc-tgl-st-{id}")),
                cx,
                |_, _| Rc::new(RefCell::new(McToggleMem { checked, from_on: checked, seq: 0, hovered: false })),
            )
            .read(cx)
            .clone();
        let fh = window
            .use_keyed_state(SharedString::from(format!("mc-tgl-fh-{id}")), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let focused = fh.is_focused(window) && !disabled;

        let (from_on, seq, hovered) = {
            let mut m = mem.borrow_mut();
            if m.checked != checked {
                m.from_on = m.checked;
                m.checked = checked;
                m.seq = m.seq.wrapping_add(1);
            }
            (m.from_on, m.seq, m.hovered && !disabled)
        };

        let sc = theme::scale();
        let w = if small { 38.0 } else { 46.0 } * sc;
        let h = if small { 22.0 } else { 26.0 } * sc;
        let track_h = if small { 18.0 } else { 22.0 } * sc;
        let track_top = (h - track_h) * 0.5;
        let thumb_w = if small { 18.0 } else { 22.0 } * sc;
        let thumb_h = if small { 20.0 } else { 24.0 } * sc;
        let travel = (w - thumb_w).max(0.0);
        let dark = theme::live_dark();
        let motion = !cx.reduce_motion() && seq > 0;

        let paint_at = move |canvas_div: Div, u: f32| -> Div {
            let j = liquid::jelly_toggle_eval(from_on, checked, u);
            let tbx = j.track_bump_x * sc;
            let tby = j.track_bump_y * sc;
            let th_w = thumb_w * j.sx;
            let th_h = thumb_h * j.sy;
            let cx_thumb = thumb_w * 0.5 + travel * j.pos;
            let left = (cx_thumb - th_w * 0.5).clamp(-1.5 * sc, w - th_w + 1.5 * sc);
            let top = ((h - th_h) * 0.5 + (thumb_h - th_h) * 0.15).clamp(-2.0 * sc, h - th_h + 2.0 * sc);
            let split_x = (left + th_w * 0.5).clamp(0.0, w);

            // Exact KiwEssentials toggle/{on,off,on_hover,off_hover}.png colors.
            let green_body = rgb(0x3C8527);
            let green_hi = rgb(0x639D52);
            let gray_body = rgb(0x8C8D90);
            let gray_hi = rgb(0xA3A4A6);
            let thumb_face = if hovered { rgb(0xF4F6F9) } else { rgb(0xD0D1D4) };
            let thumb_bevel = rgb(0xFFFFFF);
            let thumb_lip = rgb(0x58585A);
            let outline = if dark { rgb(0x000000) } else { rgb(0x18181B) };
            let glyph_w = (2.0 * sc).max(1.5);
            let glyph_h = (8.0 * sc).max(6.0);
            let o_sz = (6.0 * sc).max(5.0);

            let track = div()
                .absolute()
                .left(px(tbx))
                .top(px(track_top + tby))
                .w(px(w))
                .h(px(track_h))
                .rounded(px(2.0 * sc))
                .border_2()
                .border_color(outline)
                .overflow_hidden()
                .bg(gray_body)
                // OFF side top bevel + hollow square O mark on the right half
                .child(div().absolute().top_0().left_0().right_0().h(px(2.0 * sc)).bg(gray_hi))
                .child(
                    div()
                        .absolute()
                        .right(px(thumb_w * 0.5 - o_sz * 0.5 - 2.0 * sc))
                        .top(px((track_h - 4.0 * sc - o_sz) * 0.5))
                        .size(px(o_sz))
                        .rounded(px(1.))
                        .border(px(1.5 * sc))
                        .border_color(rgb(0x141416))
                        .opacity((1.0 - j.fill).clamp(0.18, 1.0)),
                )
                // ON side green body + top bevel + white vertical I mark
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .w(px(split_x))
                        .overflow_hidden()
                        .bg(green_body)
                        .child(div().absolute().top_0().left_0().right_0().h(px(2.0 * sc)).bg(green_hi))
                        .child(
                            div()
                                .absolute()
                                .left(px(thumb_w * 0.5 - glyph_w * 0.5 - 2.0 * sc))
                                .top(px((track_h - 4.0 * sc - glyph_h) * 0.5))
                                .w(px(glyph_w))
                                .h(px(glyph_h))
                                .bg(rgb(0xFFFFFF))
                                .opacity(j.fill.clamp(0.2, 1.0)),
                        ),
                );

            let thumb = div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(th_w))
                .h(px(th_h))
                .rounded(px(2.0 * sc))
                .border_2()
                .border_color(outline)
                .overflow_hidden()
                .bg(thumb_face)
                .child(div().absolute().top_0().left_0().right_0().h(px(2.0 * sc)).bg(thumb_bevel))
                .child(div().absolute().top_0().bottom_0().left_0().w(px(1.5 * sc)).bg(rgb(0xEAEAEC)))
                .child(div().absolute().bottom_0().left_0().right_0().h(px(3.0 * sc)).bg(thumb_lip));

            canvas_div.child(track).child(thumb)
        };

        let inner = div().relative().w(px(w)).h(px(h));
        let visual: AnyElement = if motion {
            inner
                .with_animation(
                    SharedString::from(format!("mc-tgl-{id}-{seq}")),
                    Animation::new(Duration::from_millis(380)),
                    paint_at,
                )
                .into_any_element()
        } else {
            paint_at(inner, 1.0).into_any_element()
        };

        let on_click = self.on_click.clone();
        let on_key = self.on_click;
        let mem_hov = mem;
        div()
            .id(SharedString::from(format!("mc-tgl-{id}")))
            .flex_none()
            .p(px(2.))
            .rounded(px(3.))
            .border_1()
            .border_color(if focused { c(theme::ACCENT) } else { gpui::transparent_black() })
            .when_some(self.accessibility_label, |d, lbl| d.aria_label(lbl))
            .when(disabled, |d| d.opacity(0.5).cursor_not_allowed())
            .when(!disabled, |d| d.cursor_pointer())
            .track_focus(&fh)
            .tab_stop(!disabled)
            .on_hover(move |entered: &bool, _, cx| {
                mem_hov.borrow_mut().hovered = *entered;
                cx.notify(view_id);
            })
            .when(!disabled, |d| {
                d.on_click(move |_, w, cx| {
                    if let Some(cb) = &on_click {
                        cb(&!checked, w, cx);
                    }
                })
                .on_key_down(move |ev: &KeyDownEvent, w, cx| {
                    if matches!(ev.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        if let Some(cb) = &on_key {
                            cb(&!checked, w, cx);
                        }
                    }
                })
            })
            .child(visual)
    }
}

/// KiwEssentials recessed 3D Minecraft cavity frame matching `kiwui/edit_box.png`.
pub fn mc_box_frame(base: Div, focused: bool, dark: bool) -> Div {
    let sc = theme::scale();
    let outer = if focused {
        rgb(if dark { 0x639D52 } else { 0x3C8527 }).into()
    } else if dark {
        rgb(0x28282C).into()
    } else {
        c(theme::BORDER_STRONG)
    };
    let fill = if dark { rgb(0x121214).into() } else { c(theme::BG_EDITOR) };
    let shadow = if dark { rgb(0x060608).into() } else { gpui::black().opacity(0.14) };
    let rim = if focused {
        rgb(0x3C8527).into()
    } else if dark {
        Hsla::from(rgb(0x686A72)).opacity(0.65)
    } else {
        gpui::white().opacity(0.9)
    };
    let bevel = (1.5 * sc).max(1.0);
    base.relative()
        .rounded(px(2.0 * sc))
        .border_1()
        .border_color(outer)
        .bg(fill)
        .overflow_hidden()
        .child(div().absolute().top_0().left_0().right_0().h(px(bevel)).bg(shadow))
        .child(div().absolute().top_0().bottom_0().left_0().w(px(bevel)).bg(shadow))
        .child(div().absolute().bottom_0().left_0().right_0().h(px(1.)).bg(rim))
        .child(div().absolute().top_0().bottom_0().right_0().w(px(1.)).bg(rim))
}

struct McInputFocusMem {
    focused: bool,
    seq: u64,
}

/// Recessed Minecraft Ore-UI input box wrapping `InputState` with subtle focus bounce.
#[derive(IntoElement)]
pub struct McInput {
    state: Entity<InputState>,
    size: Option<Size>,
    cleanable: bool,
    prefix: Option<AnyElement>,
    focus_bordered: bool,
    style: StyleRefinement,
}

impl McInput {
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            size: None,
            cleanable: false,
            prefix: None,
            focus_bordered: true,
            style: StyleRefinement::default(),
        }
    }

    pub fn small(mut self) -> Self {
        self.size = Some(Size::Small);
        self
    }

    pub fn xsmall(mut self) -> Self {
        self.size = Some(Size::XSmall);
        self
    }

    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    pub fn focus_bordered(mut self, enabled: bool) -> Self {
        self.focus_bordered = enabled;
        self
    }
}

impl Styled for McInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for McInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focused = self.focus_bordered && self.state.focus_handle(cx).is_focused(window);
        let dark = theme::live_dark();
        let mem = window
            .use_keyed_state(
                ("mc-inp-f", self.state.entity_id()),
                cx,
                |_, _| Rc::new(RefCell::new(McInputFocusMem { focused: false, seq: 0 })),
            )
            .read(cx)
            .clone();
        let pop_seq = {
            let mut m = mem.borrow_mut();
            if focused && !m.focused {
                m.seq = m.seq.wrapping_add(1);
            }
            m.focused = focused;
            if focused && !cx.reduce_motion() { m.seq } else { 0 }
        };

        let mut inner = Input::new(&self.state).appearance(false).focus_bordered(false);
        inner = match self.size {
            Some(Size::XSmall) => inner.xsmall(),
            Some(Size::Small) => inner.small(),
            _ => inner,
        };
        if self.cleanable {
            inner = inner.cleanable(true);
        }
        if let Some(p) = self.prefix {
            inner = inner.prefix(p);
        }

        let box_base = mc_box_frame(div().w_full().refine_style(&self.style), focused, dark).child(inner);
        if pop_seq > 0 {
            let eid = self.state.entity_id();
            box_base
                .with_animation(
                    ("mc-inp-pop", (eid.as_u64() ^ (pop_seq << 32))),
                    Animation::new(Duration::from_millis(190)),
                    move |d, u| {
                        let pulse = 1.0 - (1.0 - u) * (1.0 - u);
                        d.border_color(rgb(0x639D52).opacity(0.6 + 0.4 * pulse))
                    },
                )
                .into_any_element()
        } else {
            box_base.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::liquid::{Rect1D, Slide1D};
    use std::time::{Duration, Instant};

    #[test]
    fn slide_retargets_continuously_mid_flight() {
        let mut s = Slide1D::default();
        let t0 = Instant::now();
        s.snap(Rect1D { pos: 0.0, len: 60.0 }, false);
        s.retarget(Rect1D { pos: 64.0, len: 80.0 }, 200, false, 24.0, t0);
        let mid_time = t0 + Duration::from_millis(60);
        let live = s.sample(mid_time, 24.0);
        assert!(live.pos > 0.0 && live.pos < 64.0, "live mid: {live:?}");
        s.retarget(Rect1D { pos: 148.0, len: 70.0 }, 200, false, 24.0, mid_time);
        assert!((s.from.pos - live.pos).abs() < 1e-4, "must pick up from live pos");
    }
}

/// Closed-form liquid physics and Plateau-Rayleigh capillary coalescence kernels across FluxCode UI.
pub mod liquid {
    use gpui_kit::{
        canvas, div, prelude::FluentBuilder, px, Animation, AnimationExt as _, AnyElement, App,
        Bounds, Div, EntityId, Hsla, IntoElement as _, ParentElement as _, Pixels, Point,
        SharedString, Styled as _, Window,
    };
    use std::time::{Duration, Instant};

    /// 64-byte cache-line-aligned axis-agnostic liquid capsule span (Drepper 2007).
    #[repr(C, align(64))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct LiquidSpan {
        pub pos: f32,
        pub len: f32,
        pub cross: f32,
        pub cross_offset: f32,
        pub squeeze: f32,
        pub stretch: f32,
    }

    /// 64-byte cache-line-aligned Plateau-Rayleigh capillary coalescence output between two droplets.
    #[repr(C, align(64))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct LiquidCoalesce {
        pub coupling: f32,
        pub a_r_start: f32,
        pub a_r_end: f32,
        pub h_r_start: f32,
        pub h_r_end: f32,
        pub union_start: f32,
        pub union_len: f32,
        pub bridge_start: f32,
        pub bridge_len: f32,
        pub bridge_squeeze: f32,
        pub bridge_alpha: f32,
        pub seam_center: f32,
        pub seam_alpha: f32,
        pub overlap_boost: f32,
        pub split_border_alpha: f32,
        pub union_border_alpha: f32,
    }

    /// Fast inverse square root via IEEE-754 bit guess + 1 Newton-Raphson step (Lomont 2003).
    #[inline(always)]
    pub fn fast_inv_sqrt(x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        let x2 = 0.5 * x;
        let i = 0x5f37_59dfu32.wrapping_sub(x.to_bits() >> 1);
        let y = f32::from_bits(i);
        y * (1.5 - x2 * y * y)
    }

    /// Fast e^(-x) for x >= 0 via log2 range reduction and degree-3 Horner/minimax polynomial (Remez 1934, Knuth 1969).
    #[inline(always)]
    pub fn fast_exp_neg(x: f32) -> f32 {
        if x <= 0.0 {
            return 1.0;
        }
        if x >= 80.0 {
            return 0.0;
        }
        let scaled = x * std::f32::consts::LOG2_E;
        let k = scaled.floor();
        let f = scaled - k;
        let k_i = k as i32;
        let pow2_neg_k = if k_i >= 127 {
            0.0
        } else {
            f32::from_bits(((127 - k_i) as u32) << 23)
        };
        let poly = 1.0 + f * (-0.6915176 + f * (0.2310276 - 0.0395100 * f));
        pow2_neg_k * poly
    }

    /// Quintic fade C^2 smoothstep in Horner form: 6t^5 - 15t^4 + 10t^3 (Perlin 2002).
    #[inline(always)]
    pub fn quintic_fade(t: f32) -> f32 {
        let u = t.clamp(0.0, 1.0);
        u * u * u * (10.0 + u * (-15.0 + 6.0 * u))
    }

    /// Normalized critically damped harmonic oscillator (zeta = 1) with exact endpoints 0 and 1.
    #[inline(always)]
    pub fn critically_damped(t: f32, omega: f32) -> f32 {
        let u = t.clamp(0.0, 1.0);
        let w = omega.max(0.5);
        let raw = |s: f32| {
            let ws = w * s;
            1.0 - fast_exp_neg(ws) * (1.0 + ws)
        };
        let norm = raw(1.0);
        if norm <= 1e-6 { u } else { (raw(u) / norm).clamp(0.0, 1.0) }
    }

    /// Normalized overdamped bi-exponential harmonic oscillator (zeta > 1) for viscous trailing meniscus.
    #[inline(always)]
    pub fn overdamped_trail(t: f32, omega: f32, zeta: f32) -> f32 {
        let u = t.clamp(0.0, 1.0);
        let w = omega.max(0.5);
        let z = zeta.max(1.02);
        let z2_m1 = z * z - 1.0;
        let s = z2_m1 * fast_inv_sqrt(z2_m1);
        let r1 = w * (z - s);
        let r2 = w * (z + s);
        let inv_diff = 1.0 / (r2 - r1).max(1e-4);
        let raw = |tau: f32| 1.0 - (r2 * fast_exp_neg(r1 * tau) - r1 * fast_exp_neg(r2 * tau)) * inv_diff;
        let norm = raw(1.0);
        if norm <= 1e-6 { u } else { (raw(u) / norm).clamp(0.0, 1.0) }
    }

    /// Axis-agnostic area-conserving viscous liquid capsule between two 1D spans.
    #[inline(always)]
    pub fn capsule_1d(
        from_pos: f32,
        to_pos: f32,
        from_len: f32,
        to_len: f32,
        cross_base: f32,
        t: f32,
        hover: bool,
    ) -> LiquidSpan {
        let u = t.clamp(0.0, 1.0);
        let fl = from_len.max(1.0);
        let tl = to_len.max(1.0);
        let a_lead = critically_damped(u, if hover { 11.5 } else { 10.5 });
        let a_trail = overdamped_trail(u, if hover { 8.4 } else { 7.6 }, 1.28);
        let moving_fwd = (to_pos + 0.5 * tl) >= (from_pos + 0.5 * fl);
        let (a_start, a_end) = if moving_fwd { (a_trail, a_lead) } else { (a_lead, a_trail) };
        let start = from_pos + (to_pos - from_pos) * a_start;
        let end = (from_pos + fl) + ((to_pos + tl) - (from_pos + fl)) * a_end;
        let rest_len = fl + (tl - fl) * a_lead;
        let pos = start.min(end);
        let len = (end - start).abs().max(rest_len * 0.92);
        let stretch = (len / rest_len).max(0.5);
        let scale_cross = fast_inv_sqrt(stretch).clamp(0.80, 1.04);
        let cb = cross_base.max(1.0);
        let cross = cb * scale_cross;
        let cross_offset = (cb - cross) * 0.5;
        LiquidSpan {
            pos,
            len,
            cross,
            cross_offset,
            squeeze: cross_offset.max(0.0),
            stretch,
        }
    }

    /// Plateau-Rayleigh catenoid capillary coalescence kernel between active and hovered droplets.
    #[inline(always)]
    pub fn coalesce_1d(
        a_pos: f32,
        a_len: f32,
        h_pos: f32,
        h_len: f32,
        cross_base: f32,
        base_r: f32,
        capillary_range: f32,
    ) -> LiquidCoalesce {
        let a_end = a_pos + a_len.max(1.0);
        let h_end = h_pos + h_len.max(1.0);
        let a_mid = a_pos + 0.5 * a_len;
        let h_mid = h_pos + 0.5 * h_len;
        let gap = (h_pos - a_end).max(a_pos - h_end);
        let xi = (gap / capillary_range.max(1.0)).clamp(0.0, 1.0);
        let coupling = 1.0 - quintic_fade(xi);
        let center_dist = (h_mid - a_mid).abs();
        let coincident_thresh = 0.35 * a_len.min(h_len).max(1.0);
        let overlap_boost = if gap < 0.0 {
            1.0 - quintic_fade((center_dist / coincident_thresh).clamp(0.0, 1.0))
        } else {
            0.0
        };

        let inv_k = 1.0 - coupling;
        let r_face = base_r * inv_k * inv_k;
        let (a_r_start, a_r_end, h_r_start, h_r_end, seam_center) = if overlap_boost > 0.5 {
            (base_r, base_r, base_r, base_r, 0.5 * (a_mid + h_mid))
        } else if h_mid < a_mid {
            (r_face, base_r, base_r, r_face, 0.5 * (h_end + a_pos))
        } else {
            (base_r, r_face, r_face, base_r, 0.5 * (a_end + h_pos))
        };

        let union_start = a_pos.min(h_pos);
        let union_len = (a_end.max(h_end) - union_start).max(1.0);
        let blend = ((coupling - 0.55) / 0.45).clamp(0.0, 1.0);
        let union_border_alpha = blend * blend * (3.0 - 2.0 * blend);
        let split_border_alpha = 1.0 - union_border_alpha;

        let (bridge_start, bridge_len, bridge_squeeze, bridge_alpha) = if gap > 0.5 && coupling > 0.01 {
            let bs = a_end.min(h_end) - base_r * 0.35;
            let be = a_pos.max(h_pos) + base_r * 0.35;
            let s_neck = coupling * (0.42 + 0.58 * coupling);
            let sq = (cross_base * (1.0 - s_neck) * 0.5).max(0.0);
            let alpha = coupling * coupling * (3.0 - 2.0 * coupling);
            (bs, (be - bs).max(0.0), sq, alpha)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        let seam_alpha = if overlap_boost < 0.5 { union_border_alpha * 0.55 } else { 0.0 };

        LiquidCoalesce {
            coupling,
            a_r_start,
            a_r_end,
            h_r_start,
            h_r_end,
            union_start,
            union_len,
            bridge_start,
            bridge_len,
            bridge_squeeze,
            bridge_alpha,
            seam_center,
            seam_alpha,
            overlap_boost,
            split_border_alpha,
            union_border_alpha,
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Rect1D {
        pub pos: f32,
        pub len: f32,
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Slide1D {
        pub from: Rect1D,
        pub to: Rect1D,
        pub started: Option<Instant>,
        pub dur_ms: u32,
        pub hover: bool,
    }

    impl Slide1D {
        pub fn sample(&self, now: Instant, cross_base: f32) -> LiquidSpan {
            let u = match self.started {
                Some(s) if self.dur_ms > 0 => {
                    (now.saturating_duration_since(s).as_secs_f32() / (self.dur_ms as f32 * 0.001)).clamp(0.0, 1.0)
                }
                _ => 1.0,
            };
            capsule_1d(self.from.pos, self.to.pos, self.from.len, self.to.len, cross_base, u, self.hover)
        }

        pub fn snap(&mut self, rect: Rect1D, hover: bool) {
            self.from = rect;
            self.to = rect;
            self.started = None;
            self.hover = hover;
        }

        pub fn retarget(&mut self, next: Rect1D, dur_ms: u32, hover: bool, cross_base: f32, now: Instant) {
            let live = self.sample(now, cross_base);
            self.from = if self.from.len > 0.5 || self.to.len > 0.5 {
                Rect1D { pos: live.pos, len: live.len }
            } else {
                next
            };
            self.to = next;
            self.dur_ms = dur_ms;
            self.hover = hover;
            self.started = if (self.from.pos - next.pos).abs() < 0.5 && (self.from.len - next.len).abs() < 0.5 {
                None
            } else {
                Some(now)
            };
        }

        pub fn remaining_ms(&self, now: Instant) -> u32 {
            match self.started {
                Some(s) if self.dur_ms > 0 => {
                    let el = now.saturating_duration_since(s).as_millis() as u32;
                    self.dur_ms.saturating_sub(el)
                }
                _ => 0,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct AnimTrack {
        pub from: Rect1D,
        pub to: Rect1D,
        pub cross_base: f32,
        pub elapsed0: f32,
        pub dur_s: f32,
        pub hover: bool,
        pub fade_in: bool,
        pub fade_out: bool,
    }

    impl AnimTrack {
        pub fn from_slide(s: &Slide1D, cross_base: f32, now: Instant, fade_in: bool, fade_out: bool) -> Self {
            let elapsed0 = s.started.map_or(1.0, |st| now.saturating_duration_since(st).as_secs_f32());
            let dur_s = (s.dur_ms as f32 * 0.001).max(0.001);
            Self {
                from: s.from,
                to: s.to,
                cross_base,
                elapsed0: if s.started.is_some() { elapsed0 } else { dur_s },
                dur_s,
                hover: s.hover,
                fade_in,
                fade_out,
            }
        }

        pub fn eval(&self, elapsed_anim: f32) -> (LiquidSpan, f32) {
            let u = ((self.elapsed0 + elapsed_anim) / self.dur_s).clamp(0.0, 1.0);
            let span = capsule_1d(self.from.pos, self.to.pos, self.from.len, self.to.len, self.cross_base, u, self.hover);
            let op = if self.fade_out {
                1.0 - quintic_fade(u)
            } else if self.fade_in {
                quintic_fade(u)
            } else {
                1.0
            };
            (span, op)
        }
    }

    #[derive(Clone, Copy)]
    pub struct CoalesceFrame {
        pub seq: u64,
        pub anim_ms: u32,
        pub active: Option<AnimTrack>,
        pub hover: Option<AnimTrack>,
    }

    #[derive(Default)]
    pub struct LiquidTrackState {
        pub active_ix: Option<usize>,
        pub active: Slide1D,
        pub hover_ix: Option<usize>,
        pub hover: Slide1D,
        pub leave: Option<(Rect1D, Instant)>,
        pub last_hover_at: Option<Instant>,
        pub fade_in: bool,
        pub seq: u64,
    }

    impl LiquidTrackState {
        pub fn sync_active(&mut self, ix: Option<usize>, rect: Option<Rect1D>, cross: f32, dur_ms: u32, now: Instant) {
            if self.active_ix != ix {
                self.active_ix = ix;
                if let Some(r) = rect {
                    if self.active.to.len > 0.5 {
                        self.active.retarget(r, dur_ms, false, cross, now);
                    } else {
                        self.active.snap(r, false);
                    }
                    self.seq = self.seq.wrapping_add(1);
                }
            } else if let Some(r) = rect {
                if self.active.to.len <= 0.5 {
                    self.active.snap(r, false);
                } else if (self.active.to.pos - r.pos).abs() > 1.0 || (self.active.to.len - r.len).abs() > 1.0 {
                    // Layout shift or scroll snaps cleanly to stay locked to row/tab text.
                    self.active.snap(r, false);
                }
            }
        }

        pub fn sync_hover_rect(&mut self, rect: Option<Rect1D>, cross: f32, now: Instant) {
            if let Some(r) = rect {
                if (self.hover.to.pos - r.pos).abs() > 1.0 || (self.hover.to.len - r.len).abs() > 1.0 {
                    let _ = (cross, now);
                    self.hover.snap(r, true);
                }
            }
        }

        pub fn hover_enter(&mut self, ix: usize, rect: Option<Rect1D>, cross: f32, dur_ms: u32, now: Instant) -> bool {
            let Some(r) = rect else { return false };
            if self.hover_ix == Some(ix) && (self.hover.to.pos - r.pos).abs() < 0.5 && (self.hover.to.len - r.len).abs() < 0.5 {
                return false;
            }
            let gliding = self.hover_ix.is_some()
                || self.leave.is_some()
                || self.last_hover_at.is_some_and(|t| t.elapsed() < Duration::from_millis(145));
            if gliding {
                if let Some((lr, _)) = self.leave.take() {
                    if self.hover.to.len <= 0.5 {
                        self.hover.snap(lr, true);
                    }
                }
                self.hover.retarget(r, dur_ms, true, cross, now);
            } else {
                self.leave = None;
                self.hover.snap(r, true);
                self.hover.dur_ms = 115;
                self.hover.started = Some(now);
            }
            self.hover_ix = Some(ix);
            self.fade_in = !gliding;
            self.last_hover_at = Some(now);
            self.seq = self.seq.wrapping_add(1);
            true
        }

        pub fn hover_clear(&mut self, now: Instant) -> bool {
            if self.hover_ix.take().is_none() {
                return false;
            }
            self.last_hover_at = Some(now);
            let live = self.hover.sample(now, 28.0);
            self.leave = Some((Rect1D { pos: live.pos, len: live.len }, now));
            self.seq = self.seq.wrapping_add(1);
            true
        }

        pub fn sample_frame(&mut self, cross: f32, now: Instant) -> Option<CoalesceFrame> {
            let active = (self.active_ix.is_some() && self.active.to.len > 0.5)
                .then(|| AnimTrack::from_slide(&self.active, cross, now, false, false));
            let hover = if self.hover_ix.is_some() && self.hover.to.len > 0.5 {
                Some(AnimTrack::from_slide(&self.hover, cross, now, self.fade_in, false))
            } else if let Some((lr, lstart)) = self.leave {
                let el = now.saturating_duration_since(lstart).as_millis() as u32;
                if el >= 120 {
                    self.leave = None;
                    None
                } else {
                    Some(AnimTrack {
                        from: lr,
                        to: lr,
                        cross_base: cross,
                        elapsed0: el as f32 * 0.001,
                        dur_s: 0.120,
                        hover: true,
                        fade_in: false,
                        fade_out: true,
                    })
                }
            } else {
                None
            };
            if active.is_none() && hover.is_none() {
                return None;
            }
            let rem_a = if active.is_some() { self.active.remaining_ms(now) } else { 0 };
            let rem_h = if self.hover_ix.is_some() {
                self.hover.remaining_ms(now).max(if self.fade_in { 115u32.saturating_sub(self.hover.started.map_or(115, |s| now.saturating_duration_since(s).as_millis() as u32)) } else { 0 })
            } else if let Some((_, lstart)) = self.leave {
                120u32.saturating_sub(now.saturating_duration_since(lstart).as_millis() as u32)
            } else {
                0
            };
            Some(CoalesceFrame { seq: self.seq, anim_ms: rem_a.max(rem_h), active, hover })
        }
    }

    #[derive(Clone, Copy)]
    pub struct LiquidStyle {
        pub inset_a: f32,
        pub inset_b: f32,
        pub cross_base: f32,
        pub max_squeeze: f32,
        pub radius: f32,
        pub capillary: f32,
        pub active_bg: Hsla,
        pub active_border: Hsla,
        pub hover_bg: Hsla,
        pub hover_border: Hsla,
        pub union_border: Hsla,
        pub bridge_bg: Hsla,
    }

    fn with_radii(mut d: Div, tl: f32, tr: f32, br: f32, bl: f32) -> Div {
        let r = &mut d.style().corner_radii;
        r.top_left = Some(px(tl).into());
        r.top_right = Some(px(tr).into());
        r.bottom_right = Some(px(br).into());
        r.bottom_left = Some(px(bl).into());
        d
    }

    pub fn render_coalesced_layer(
        id: impl Into<SharedString>,
        vertical: bool,
        st: LiquidStyle,
        frame: CoalesceFrame,
    ) -> AnyElement {
        let key_prefix = id.into();
        let paint_at = move |layer: Div, anim_el: f32| -> Div {
            let active = frame.active.map(|a| a.eval(anim_el).0);
            let hover = frame.hover.map(|h| h.eval(anim_el));
            if vertical {
                paint_coalesced_v(layer, &st, active, hover)
            } else {
                paint_coalesced_h(layer, &st, active, hover)
            }
        };
        let base = div().absolute().inset_0().size_full();
        if frame.anim_ms == 0 {
            paint_at(base, 0.0).into_any_element()
        } else {
            let dur_s = frame.anim_ms as f32 * 0.001;
            base.with_animation(
                SharedString::from(format!("{key_prefix}-{}", frame.seq)),
                Animation::new(Duration::from_millis(frame.anim_ms as u64)),
                move |d, u| paint_at(d, u * dur_s),
            )
            .into_any_element()
        }
    }

    pub fn paint_coalesced_v(
        layer: Div,
        st: &LiquidStyle,
        active: Option<LiquidSpan>,
        hover: Option<(LiquidSpan, f32)>,
    ) -> Div {
        paint_coalesced_axis(layer, true, st, active, hover)
    }

    pub fn paint_coalesced_h(
        layer: Div,
        st: &LiquidStyle,
        active: Option<LiquidSpan>,
        hover: Option<(LiquidSpan, f32)>,
    ) -> Div {
        paint_coalesced_axis(layer, false, st, active, hover)
    }

    #[derive(Default)]
    pub struct KeyedLiquid {
        pub origin: Option<Point<Pixels>>,
        pub rects: Vec<Option<Bounds<Pixels>>>,
        pub track: LiquidTrackState,
    }

    pub type KeyedLiquidHandle = std::rc::Rc<std::cell::RefCell<KeyedLiquid>>;

    pub fn keyed_state(id: impl Into<SharedString>, window: &mut Window, cx: &mut App) -> KeyedLiquidHandle {
        window
            .use_keyed_state(id.into(), cx, |_, _| std::rc::Rc::new(std::cell::RefCell::new(KeyedLiquid::default())))
            .read(cx)
            .clone()
    }

    fn rect_1d(origin: Point<Pixels>, b: Bounds<Pixels>, vertical: bool) -> Rect1D {
        if vertical {
            Rect1D { pos: f32::from(b.origin.y - origin.y), len: f32::from(b.size.height) }
        } else {
            Rect1D { pos: f32::from(b.origin.x - origin.x), len: f32::from(b.size.width) }
        }
    }

    pub fn keyed_sync_and_sample(
        handle: &KeyedLiquidHandle,
        vertical: bool,
        count: usize,
        active_ix: Option<usize>,
        cross_base: f32,
        active_dur_ms: u32,
        motion: bool,
    ) -> (Option<CoalesceFrame>, bool) {
        let now = Instant::now();
        let mut st = handle.borrow_mut();
        if st.rects.len() != count {
            st.rects.resize(count, None);
        }
        let origin = st.origin;
        let act_rect = active_ix
            .and_then(|ix| Some(rect_1d(origin?, st.rects.get(ix).copied().flatten()?, vertical)));
        st.track.sync_active(active_ix, act_rect, cross_base, active_dur_ms, now);
        let hov_rect = st
            .track
            .hover_ix
            .and_then(|ix| Some(rect_1d(origin?, st.rects.get(ix).copied().flatten()?, vertical)));
        st.track.sync_hover_rect(hov_rect, cross_base, now);
        let frame = if motion { st.track.sample_frame(cross_base, now) } else { None };
        (frame, motion && act_rect.is_some())
    }

    pub fn keyed_origin_probe(handle: KeyedLiquidHandle, view_id: EntityId) -> AnyElement {
        canvas(
            move |bounds, _, cx| {
                let mut st = handle.borrow_mut();
                let changed = st.origin.is_none_or(|p| {
                    (f32::from(p.x) - f32::from(bounds.origin.x)).abs() > 1.0
                        || (f32::from(p.y) - f32::from(bounds.origin.y)).abs() > 1.0
                });
                st.origin = Some(bounds.origin);
                if changed {
                    cx.defer(move |cx| cx.notify(view_id));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .size_full()
        .into_any_element()
    }

    pub fn keyed_item_probe(handle: KeyedLiquidHandle, ix: usize, view_id: EntityId) -> AnyElement {
        canvas(
            move |bounds, _, cx| {
                let mut st = handle.borrow_mut();
                let changed = st.rects.get(ix).and_then(|o| *o).is_none_or(|prev| {
                    (f32::from(prev.origin.x) - f32::from(bounds.origin.x)).abs() > 1.0
                        || (f32::from(prev.origin.y) - f32::from(bounds.origin.y)).abs() > 1.0
                        || (f32::from(prev.size.width) - f32::from(bounds.size.width)).abs() > 1.0
                        || (f32::from(prev.size.height) - f32::from(bounds.size.height)).abs() > 1.0
                });
                if ix < st.rects.len() {
                    st.rects[ix] = Some(bounds);
                }
                if changed {
                    cx.defer(move |cx| cx.notify(view_id));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .size_full()
        .into_any_element()
    }

    pub fn keyed_hover_enter(
        handle: &KeyedLiquidHandle,
        ix: usize,
        vertical: bool,
        cross_base: f32,
        dur_ms: u32,
        view_id: EntityId,
        cx: &mut App,
    ) {
        let mut st = handle.borrow_mut();
        let rect = st
            .origin
            .and_then(|o| st.rects.get(ix).copied().flatten().map(|b| rect_1d(o, b, vertical)));
        if st.track.hover_enter(ix, rect, cross_base, dur_ms, Instant::now()) {
            cx.notify(view_id);
        }
    }

    pub fn keyed_hover_leave(handle: &KeyedLiquidHandle, view_id: EntityId, cx: &mut App) {
        if handle.borrow_mut().track.hover_clear(Instant::now()) {
            cx.notify(view_id);
        }
    }

    fn paint_coalesced_axis(
        layer: Div,
        vertical: bool,
        st: &LiquidStyle,
        active: Option<LiquidSpan>,
        hover: Option<(LiquidSpan, f32)>,
    ) -> Div {
        let place = |pos: f32, len: f32, sq: f32| -> Div {
            let sq = sq.min(st.max_squeeze);
            if vertical {
                div().absolute().left(px(st.inset_a + sq)).right(px(st.inset_b + sq)).top(px(pos)).h(px(len))
            } else {
                div().absolute().top(px(st.inset_a + sq)).bottom(px(st.inset_b + sq)).left(px(pos)).w(px(len))
            }
        };
        let radii_axis = |d: Div, r_start: f32, r_end: f32| -> Div {
            if vertical {
                with_radii(d, r_start, r_start, r_end, r_end)
            } else {
                with_radii(d, r_start, r_end, r_end, r_start)
            }
        };

        match (active, hover.filter(|(_, op)| *op > 0.01)) {
            (None, None) => layer,
            (Some(a), None) => layer.child(
                radii_axis(place(a.pos, a.len, a.squeeze), st.radius, st.radius)
                    .bg(st.active_bg)
                    .border_1()
                    .border_color(st.active_border),
            ),
            (None, Some((h, op))) => layer.child(
                radii_axis(place(h.pos, h.len, h.squeeze), st.radius, st.radius)
                    .bg(st.hover_bg.opacity(op))
                    .border_1()
                    .border_color(st.hover_border.opacity(op)),
            ),
            (Some(a), Some((h, op))) => {
                let mut co = coalesce_1d(a.pos, a.len, h.pos, h.len, st.cross_base, st.radius, st.capillary);
                let eff_k = co.coupling * op;
                co.bridge_alpha *= op;
                co.union_border_alpha *= op;
                co.seam_alpha *= op;
                co.split_border_alpha = 1.0 - co.union_border_alpha;
                if op < 0.99 {
                    let inv = 1.0 - eff_k;
                    let r_face = st.radius * inv * inv;
                    if a.pos <= h.pos {
                        co.a_r_end = r_face;
                        co.h_r_start = r_face;
                    } else {
                        co.h_r_end = r_face;
                        co.a_r_start = r_face;
                    }
                }
                if co.overlap_boost > 0.55 && op > 0.7 {
                    return layer.child(
                        radii_axis(place(co.union_start, co.union_len, a.squeeze.min(h.squeeze)), st.radius, st.radius)
                            .bg(st.active_bg)
                            .border_1()
                            .border_color(st.union_border),
                    );
                }
                let mut out = layer;
                if co.bridge_alpha > 0.01 && co.bridge_len > 0.5 {
                    let bsq = co.bridge_squeeze.clamp(0.0, st.max_squeeze + 3.0);
                    out = out.child(
                        place(co.bridge_start, co.bridge_len, bsq).bg(st.bridge_bg.opacity(co.bridge_alpha)),
                    );
                }
                out = out.child(
                    radii_axis(place(h.pos, h.len, h.squeeze), co.h_r_start, co.h_r_end)
                        .bg(st.hover_bg.opacity(op))
                        .when(co.split_border_alpha > 0.02, |d| {
                            d.border_1().border_color(st.hover_border.opacity(op * co.split_border_alpha))
                        }),
                );
                out = out.child(
                    radii_axis(place(a.pos, a.len, a.squeeze), co.a_r_start, co.a_r_end)
                        .bg(st.active_bg)
                        .when(co.split_border_alpha > 0.02, |d| {
                            d.border_1().border_color(st.active_border.opacity(co.split_border_alpha))
                        }),
                );
                if co.seam_alpha > 0.01 {
                    out = out.child(place(co.seam_center - 3.0, 6.0, 0.0).bg(st.bridge_bg.opacity(co.seam_alpha)));
                }
                if co.union_border_alpha > 0.01 {
                    out = out.child(
                        radii_axis(place(co.union_start, co.union_len, a.squeeze.min(h.squeeze)), st.radius, st.radius)
                            .border_1()
                            .border_color(st.union_border.opacity(co.union_border_alpha)),
                    );
                }
                out
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub struct JellySample {
        pub pos: f32,
        pub fill: f32,
        pub sx: f32,
        pub sy: f32,
        pub track_bump_x: f32,
        pub track_bump_y: f32,
    }

    /// Underdamped harmonic spring (`zeta = 0.44`) with area-conserving jelly squash/stretch (`sx * sy ~= 1`).
    pub fn jelly_toggle_eval(from_on: bool, to_on: bool, u: f32) -> JellySample {
        let from = if from_on { 1.0 } else { 0.0 };
        let to = if to_on { 1.0 } else { 0.0 };
        let dir = to - from;
        if from_on == to_on || u >= 1.0 {
            return JellySample { pos: to, fill: to, sx: 1.0, sy: 1.0, track_bump_x: 0.0, track_bump_y: 0.0 };
        }
        if u <= 0.0 {
            return JellySample { pos: from, fill: from, sx: 1.0, sy: 1.0, track_bump_x: 0.0, track_bump_y: 0.0 };
        }
        let zeta = 0.44f32;
        let omega = 15.0f32;
        let sq = (1.0 - zeta * zeta).sqrt();
        let omega_d = omega * sq;
        let ratio = zeta / sq;
        let raw = |t: f32| {
            1.0 - fast_exp_neg(zeta * omega * t) * ((omega_d * t).cos() + ratio * (omega_d * t).sin())
        };
        let norm = raw(1.0);
        let s = if norm.abs() < 1e-5 { u } else { raw(u) / norm };
        let pos = from + dir * s;
        let fill = pos.clamp(0.0, 1.0);

        let flight = fast_exp_neg(11.0 * (u - 0.10).abs()) * (u * 8.0).min(1.0) * (1.0 - u);
        let tau = (u - 0.22).max(0.0);
        let wobble = -fast_exp_neg(5.8 * tau) * (20.5 * tau).sin() * (1.0 - u);
        let sx = (1.0 + 0.28 * flight + 0.26 * wobble).clamp(0.76, 1.34);
        let sy = (1.0 / sx).clamp(0.75, 1.32);

        JellySample {
            pos,
            fill,
            sx,
            sy,
            track_bump_x: wobble * 1.6 * dir,
            track_bump_y: -wobble * 1.1,
        }
    }

    #[cfg(test)]
    mod liquid_tests {
        use super::*;

        #[test]
        fn scalar_math_kernels_stay_within_precision_bounds() {
            for i in 1..=400 {
                let x = i as f32 * 0.25;
                let exact = 1.0 / x.sqrt();
                let approx = fast_inv_sqrt(x);
                let rel = (approx - exact).abs() / exact;
                assert!(rel < 0.002, "inv_sqrt({x}): exact={exact}, approx={approx}, rel={rel}");
            }
            for i in 0..=240 {
                let x = i as f32 * 0.05;
                let exact = (-x).exp();
                let approx = fast_exp_neg(x);
                let err = (approx - exact).abs();
                assert!(err < 5e-4, "exp(-{x}): exact={exact}, approx={approx}, err={err}");
            }
            assert_eq!(quintic_fade(0.0), 0.0);
            assert_eq!(quintic_fade(0.5), 0.5);
            assert_eq!(quintic_fade(1.0), 1.0);
            assert_eq!(critically_damped(0.0, 10.5), 0.0);
            assert!((critically_damped(1.0, 10.5) - 1.0).abs() < 1e-6);

            let mut prev = -1.0f32;
            for i in 0..=100 {
                let t = i as f32 * 0.01;
                let v = overdamped_trail(t, 8.0, 1.28);
                assert!(v >= prev - 1e-6 && v <= 1.0 + 1e-6, "non-monotonic overdamped at {t}: {v}");
                prev = v;
            }
            assert_eq!(overdamped_trail(0.0, 8.0, 1.28), 0.0);
            assert!((overdamped_trail(1.0, 8.0, 1.28) - 1.0).abs() < 1e-5);
        }

        #[test]
        fn liquid_capsule_and_coalescence_behave_smoothly() {
            let (base_h, base_w) = (30.0, 32.0);
            let start = capsule_1d(0.0, 108.0, base_h, base_h, base_w, 0.0, false);
            assert!((start.pos - 0.0).abs() < 1e-4 && (start.len - base_h).abs() < 1e-4);
            let end = capsule_1d(0.0, 108.0, base_h, base_h, base_w, 1.0, false);
            assert!((end.pos - 108.0).abs() < 1e-3 && (end.len - base_h).abs() < 1e-3);

            let var_mid = capsule_1d(10.0, 120.0, 60.0, 96.0, 26.0, 0.4, false);
            assert!(var_mid.len > 96.0, "variable width stretch: {:?}", var_mid);

            let far = coalesce_1d(0.0, 20.0, 80.0, 20.0, 200.0, 6.0, 24.0);
            assert_eq!(far.coupling, 0.0);
            assert_eq!(far.union_border_alpha, 0.0);
            assert_eq!(far.bridge_alpha, 0.0);

            let near = coalesce_1d(20.0, 20.0, 50.0, 20.0, 200.0, 6.0, 24.0);
            assert!(near.coupling > 0.0 && near.coupling < 1.0);
            assert!(near.bridge_alpha > 0.0 && near.bridge_squeeze > 0.0);

            let adj_above = coalesce_1d(40.0, 20.0, 20.0, 20.0, 200.0, 6.0, 24.0);
            assert_eq!(adj_above.coupling, 1.0);
            assert_eq!(adj_above.h_r_end, 0.0);
            assert_eq!(adj_above.a_r_start, 0.0);
            assert_eq!(adj_above.h_r_start, 6.0);
            assert_eq!(adj_above.a_r_end, 6.0);
            assert_eq!(adj_above.union_border_alpha, 1.0);
            assert_eq!(adj_above.split_border_alpha, 0.0);
            assert!(adj_above.seam_alpha > 0.0);

            let same = coalesce_1d(40.0, 20.0, 40.0, 20.0, 200.0, 6.0, 24.0);
            assert!(same.overlap_boost > 0.9);
            assert_eq!(same.a_r_start, 6.0);
            assert_eq!(same.h_r_end, 6.0);
        }

        #[test]
        fn liquid_capsule_benchmark_10k_evals_under_budget() {
            let t0 = Instant::now();
            let mut acc = 0.0f32;
            for i in 0..10_000u32 {
                let t = (i % 101) as f32 * 0.01;
                let c = capsule_1d(0.0, 144.0, 20.0, 24.0, 32.0, t, (i & 1) == 0);
                let co = coalesce_1d(40.0, 20.0, c.pos, c.len, 32.0, 6.0, 24.0);
                acc += c.pos + c.len + co.coupling + co.union_len;
            }
            let elapsed = t0.elapsed();
            std::hint::black_box(acc);
            eprintln!("liquid 10,000 capsule+coalesce evals: {:?} ({:.2} ns/eval)", elapsed, elapsed.as_nanos() as f64 / 10_000.0);
            assert!(elapsed.as_millis() < 20, "10k evaluations took {:?}", elapsed);
        }

        #[test]
        fn jelly_toggle_overshoots_conserves_area_and_settles() {
            for (from, to) in [(false, true), (true, false)] {
                let s0 = jelly_toggle_eval(from, to, 0.0);
                let s1 = jelly_toggle_eval(from, to, 1.0);
                assert!((s0.pos - if from { 1.0 } else { 0.0 }).abs() < 1e-5);
                assert!((s1.pos - if to { 1.0 } else { 0.0 }).abs() < 1e-5);
                assert!((s1.sx - 1.0).abs() < 1e-5 && (s1.sy - 1.0).abs() < 1e-5);

                let mut overshot = false;
                let mut stretched = false;
                let mut squashed = false;
                for i in 1..100 {
                    let j = jelly_toggle_eval(from, to, i as f32 * 0.01);
                    assert!((j.sx * j.sy - 1.0).abs() < 0.02, "area not conserved: {j:?}");
                    if to && j.pos > 1.03 { overshot = true; }
                    if !to && j.pos < -0.03 { overshot = true; }
                    if j.sx > 1.04 { stretched = true; }
                    if j.sx < 0.96 { squashed = true; }
                }
                assert!(overshot, "expected bumble overshoot for {from}->{to}");
                assert!(stretched && squashed, "expected squash & stretch for {from}->{to}");
            }
        }
    }
}
