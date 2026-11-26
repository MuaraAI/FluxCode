use gpui::{
    AppContext as _, Context, Entity, IntoElement, MouseButton, ParentElement as _, Render,
    ScrollDelta, ScrollEvent, Styled as _, TestAppContext, VisualTestContext, Window, div, point,
    px,
};

use super::{Diff, DiffMode, DiffState, document::fixture};

/// Every line changed, so the unified patch is two rows per line.
const LINES: usize = 300;

struct Host {
    state: Entity<DiffState>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(400.))
            .h(px(200.))
            .child(Diff::new(&self.state).size_full())
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        _ = window.draw(cx);
    });
}

fn long_patch(cx: &mut TestAppContext) -> (Entity<DiffState>, &mut VisualTestContext) {
    cx.update(crate::init);
    let original = (0..LINES)
        .map(|ix| format!("old {ix}\n"))
        .collect::<String>();
    let modified = (0..LINES)
        .map(|ix| format!("new {ix}\n"))
        .collect::<String>();
    let state =
        cx.new(|cx| DiffState::new([fixture::modified("long.txt", &original, &modified)], cx));
    let (_, cx) = cx.add_window_view({
        let state = state.clone();
        move |_, _| Host { state }
    });
    // The body records its size on the first paint and lays out again.
    for _ in 0..3 {
        draw(cx);
    }
    (state, cx)
}

#[gpui::test]
fn the_whole_patch_keeps_its_height_after_the_first_layout(cx: &mut TestAppContext) {
    let (state, cx) = long_patch(cx);
    let (max, viewport) = cx.update(|_, cx| {
        let list = state.read(cx).list();
        (
            list.max_offset_for_scrollbar().y,
            list.viewport_bounds().size.height,
        )
    });
    // Rows past the viewport have not been rendered; without their hinted
    // height the patch would read as barely taller than one screen.
    assert!(
        max > viewport * 10.,
        "max offset {max:?} for a {viewport:?} viewport"
    );
}

#[gpui::test]
fn the_wheel_scrolls_past_the_rows_rendered_so_far(cx: &mut TestAppContext) {
    let (state, cx) = long_patch(cx);
    for _ in 0..50 {
        cx.simulate_event(ScrollEvent {
            position: point(px(100.), px(120.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-40.))),
            ..Default::default()
        });
        draw(cx);
    }
    let top = cx.update(|_, cx| state.read(cx).list().logical_scroll_top().item_ix);
    // 2000 px of wheel travel crosses far more than the first screen of rows.
    assert!(top > 40, "scrolled to row {top}");
}

#[gpui::test]
fn split_divider_stays_inside_the_viewport_with_long_lines(cx: &mut TestAppContext) {
    cx.update(crate::init);
    let original = format!("original {}\n", "x".repeat(300));
    let modified = format!("modified {}\n", "y".repeat(300));
    let state = cx.new(|cx| {
        DiffState::new([fixture::modified("wide.txt", &original, &modified)], cx)
            .with_mode(DiffMode::Split)
    });
    let (_, cx) = cx.add_window_view({
        let state = state.clone();
        move |_, _| Host { state }
    });
    for _ in 0..3 {
        draw(cx);
    }
    let divider_x =
        |cx: &mut VisualTestContext| cx.debug_bounds("diff-split-divider").unwrap().center().x;
    let start = divider_x(cx);
    assert!((start - px(200.)).abs() <= px(2.));
    cx.update(|_, cx| {
        let state = state.read(cx);
        assert!(state.content_width() > state.viewport().width);
    });
    let at = |x| point(x, px(120.));
    cx.simulate_mouse_move(at(start), None, Default::default());
    cx.simulate_mouse_down(at(start), MouseButton::Left, Default::default());
    for x in [start + px(10.), px(280.)] {
        cx.simulate_mouse_move(at(x), MouseButton::Left, Default::default());
        draw(cx);
    }
    cx.simulate_mouse_up(at(px(280.)), MouseButton::Left, Default::default());
    draw(cx);
    let end = divider_x(cx);
    assert!((end - px(280.)).abs() <= px(2.));
    cx.simulate_event(ScrollEvent {
        position: point(px(100.), px(120.)),
        delta: ScrollDelta::Pixels(point(px(-100.), px(0.))),
        ..Default::default()
    });
    draw(cx);
    cx.update(|_, cx| assert!(state.read(cx).horizontal_scroll().offset().x < px(0.)));
    assert_eq!(divider_x(cx), end);
}
