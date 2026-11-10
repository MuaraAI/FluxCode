//! The install animation in the agent panel, drawn natively each frame (no video, no
//! images): a few filled paths, tessellated by GPUI and drawn on the GPU.
//!
//! - Downloading: a ring of liquid. Its edges move like smoke, an ochre band flows
//!   around it, and it thickens toward the centre with the real byte count, so it is
//!   a full disc when the last byte arrives. With no known size it stays a thin ring
//!   that swirls, which says "working" without claiming a percentage.
//! - Checking: the full disc breathes while the checksum runs.
//! - Unpacking ("fluxbox"): the liquid sinks into a box, the lids swing open and the
//!   contents rise out, as many as tar reports writing; the liquid inside drains as
//!   the count reaches the archive's total.
//! - Done: the disc with a check drawn in. Failed: the ring stops, breaks open and
//!   turns the danger color; nothing moves after that.
//!
//! Cost: the view owns its own ~30 fps timer and only it re-renders (not the
//! workbench); the timer ends when the view is dropped or the failure has been drawn.
//! Under the system's reduce-motion setting the liquid holds still and only the fill
//! level changes.

use crate::acp::{Phase, Snapshot, Tracker};
use crate::theme::{self, c};
use gpui_kit::{prelude::FluentBuilder, *};
use std::f32::consts::{PI, TAU};
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_millis(33);

pub struct InstallAnim {
    tracker: Tracker,
    born: Instant,
    last: Instant,
    /// Eased fill level, 0..1. Only rises.
    fill: f32,
    /// 1 when the size is known (a fill level), 0 for the swirl; eased between.
    known: f32,
    phase: Phase,
    phase_at: Instant,
    /// "Installing Cursor", under the animation.
    title: SharedString,
    /// The clock, stopped at the moment of a failure.
    frozen: Option<f32>,
    _tick: Task<()>,
}

impl InstallAnim {
    pub fn new(tracker: Tracker, title: SharedString, cx: &mut Context<Self>) -> Self {
        let tick = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
            cx.background_executor().timer(FRAME).await;
            let go = this.update(cx, |a, cx| {
                cx.notify();
                a.frozen.is_none()
            });
            if !matches!(go, Ok(true)) {
                break;
            }
        });
        let now = Instant::now();
        Self { tracker, born: now, last: now, fill: 0., known: 0., phase: Phase::Registry, phase_at: now, title, frozen: None, _tick: tick }
    }

    /// Advance the eased values to now and return what to draw.
    fn step(&mut self, reduce_motion: bool) -> (Snapshot, Frame) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let snap = self.tracker.get();
        if snap.phase != self.phase {
            self.phase = snap.phase;
            self.phase_at = now;
        }
        let (target, known) = target(&snap);
        let k = 1. - (-dt * 5.).exp();
        if let Some(f) = target {
            self.fill += (f - self.fill).max(0.) * k;
            if f >= 1. && self.fill > 0.995 {
                self.fill = 1.;
            }
        }
        self.known += ((if known { 1. } else { 0. }) - self.known) * k;
        let alive = (now - self.born).as_secs_f32();
        if self.phase == Phase::Failed && self.frozen.is_none() {
            self.frozen = Some(alive);
        }
        let t = if reduce_motion { 0. } else { self.frozen.unwrap_or(alive) };
        let since = (now - self.phase_at).as_secs_f32();
        let frame = Frame {
            t,
            fill: self.fill,
            known: self.known,
            phase: self.phase,
            since,
            unpacked: if snap.phase == Phase::Unpack { snap.fraction() } else { None },
            fg: c(theme::FG_STRONG),
            accent: c(theme::ACCENT),
            danger: c(theme::DANGER),
            ground: c(theme::BG_SIDE),
        };
        (snap, frame)
    }
}

/// Where the fill should go for a snapshot, and whether that level is a measurement.
fn target(s: &Snapshot) -> (Option<f32>, bool) {
    match s.phase {
        Phase::Download => (s.fraction(), s.fraction().is_some()),
        Phase::Verify | Phase::Unpack | Phase::Done => (Some(1.), true),
        Phase::Registry | Phase::Npm | Phase::Failed => (None, false),
    }
}

/// The line under the animation: the step, and the measured amount when there is one.
pub fn detail(s: &Snapshot) -> String {
    let label = crate::i18n::tr(s.phase.label());
    let mb = |b: u64| format!("{:.1}", b as f64 / 1_048_576.);
    match (s.phase, s.total) {
        (Phase::Download, 0) if s.done > 0 => format!("{label} · {} MB", mb(s.done)),
        (Phase::Download, total) if total > 0 => {
            let pct = (s.done as f64 / total as f64 * 100.).floor().min(100.);
            format!("{label} · {} · {pct:.0}%", crate::trf!("{done} of {total} MB"; done => mb(s.done), total => mb(total)))
        }
        (Phase::Unpack, total) if s.done > 0 => {
            if total > s.done {
                format!("{label} · {}", crate::trf!("{done} of {total} files"; done => s.done, total => total))
            } else {
                format!("{label} · {}", crate::trf!("{done} files"; done => s.done))
            }
        }
        (Phase::Done, _) => label.to_string(),
        _ => format!("{label}…"),
    }
}

impl Render for InstallAnim {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (snap, frame) = self.step(cx.reduce_motion());
        let size = theme::u(132.);
        let failed = snap.phase == Phase::Failed;
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(canvas(|_, _, _| {}, move |bounds, _, window, _| frame.paint(bounds, window)).size(size))
            .when(!failed, |d| {
                d.child(div().pt_1().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(self.title.clone()))
                    .child(div().h(theme::u(18.)).text_xs().text_color(c(theme::FG_DIM)).child(detail(&snap)))
            })
    }
}

/// Everything one frame needs, copied into the canvas.
#[derive(Clone, Copy)]
struct Frame {
    t: f32,
    fill: f32,
    known: f32,
    phase: Phase,
    since: f32,
    unpacked: Option<f32>,
    fg: Hsla,
    accent: Hsla,
    danger: Hsla,
    ground: Hsla,
}

/// Smooth, looping edge noise in -1..1: three waves per edge, each drifting on its own.
pub fn wob(th: f32, t: f32, seed: f32) -> f32 {
    0.55 * (3. * th + 1.3 * t + seed).sin() + 0.30 * (5. * th - 0.9 * t + 2.1 * seed).sin() + 0.15 * (8. * th + 2.3 * t + 0.7 * seed).sin()
}

/// A closed wobbly circle: `n` points at radius `r` +- `amp`.
pub fn blob(cx: f32, cy: f32, r: f32, amp: f32, t: f32, seed: f32, n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let th = i as f32 / n as f32 * TAU;
            let rr = (r + amp * wob(th, t, seed)).max(0.);
            (cx + rr * th.cos(), cy + rr * th.sin())
        })
        .collect()
}

/// The inner radius of the liquid ring: thin while unknown, closing to 0 as it fills.
pub fn inner_radius(r: f32, fill: f32, known: f32, t: f32) -> f32 {
    let open = r * 0.80;
    let filled = open * (1. - fill.clamp(0., 1.)).powf(0.85);
    let swirl = open + r * 0.03 * (t * 2.).sin();
    swirl + (filled - swirl) * known.clamp(0., 1.)
}

fn pts(v: &[(f32, f32)]) -> Vec<Point<Pixels>> {
    v.iter().map(|&(x, y)| point(px(x), px(y))).collect()
}

/// Fill one or more closed outlines as one path (even-odd, so an inner outline is a hole).
fn fill(window: &mut Window, outlines: &[Vec<(f32, f32)>], color: Hsla) {
    let mut b = PathBuilder::fill();
    // A shape with a non-finite point is skipped: the tessellator would abort the app.
    for o in outlines.iter().filter(|o| o.len() > 2 && o.iter().all(|(x, y)| x.is_finite() && y.is_finite())) {
        b.add_polygon(&pts(o), true);
    }
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

impl Frame {
    fn paint(self, b: Bounds<Pixels>, window: &mut Window) {
        let (cx, cy) = (f32::from(b.center().x), f32::from(b.center().y));
        let r = f32::from(b.size.width.min(b.size.height)) * 0.36;
        match self.phase {
            Phase::Failed => self.broken(window, cx, cy, r),
            Phase::Unpack => {
                let m = ease(self.since / 0.35);
                if m < 1. {
                    // The full disc shrinks and sinks into the box.
                    let rr = r * (1. - 0.6 * m);
                    let y = cy + m * r * 0.35;
                    fill(window, &[blob(cx, y, rr, r * 0.03, self.t, 0., 72)], self.fg.opacity(1. - m));
                }
                self.fluxbox(window, cx, cy, r, m);
            }
            _ => {
                let breathe = if self.phase == Phase::Verify { 0.82 + 0.18 * (self.since * 4.).cos() } else { 1. };
                self.ring(window, cx, cy, r, breathe);
                if self.phase == Phase::Done {
                    self.check(window, cx, cy, r);
                }
            }
        }
    }

    fn ring(self, window: &mut Window, cx: f32, cy: f32, r: f32, alpha: f32) {
        let t = self.t;
        let inner = inner_radius(r, self.fill, self.known, t);
        let n = 112;
        // Smoke: three faint, loose layers round the outside, each drifting on its own,
        // so the edge feathers instead of ending in a line.
        for k in 0..3 {
            let k = k as f32;
            let outer = blob(cx, cy, r * (1.03 + 0.035 * k), r * (0.05 + 0.025 * k), t * (0.55 + 0.17 * k), 4.0 + 1.9 * k, n);
            let inside = blob(cx, cy, r * (0.95 - 0.02 * k), r * 0.04, t * (0.7 + 0.1 * k), 1.7 + k, n);
            fill(window, &[outer, inside], self.fg.opacity((0.085 - 0.02 * k) * alpha));
        }
        let hole = inner > r * 0.05;
        let mut liquid = vec![blob(cx, cy, r, r * 0.032, t, 0., n)];
        if hole {
            liquid.push(blob(cx, cy, inner, r * 0.045 * (inner / (r * 0.3)).min(1.), t * 1.1, 2.6, n));
        }
        fill(window, &liquid, self.fg.opacity(0.9 * alpha));
        if inner > r * 0.14 {
            for k in 0..2 {
                let k = k as f32;
                let a = blob(cx, cy, inner * (1.01 - 0.02 * k), r * (0.05 + 0.02 * k), t * (0.9 - 0.15 * k), 5.2 + 2. * k, n);
                let b = blob(cx, cy, inner * (0.86 - 0.06 * k), r * 0.06, t * 0.75, 3.3 + k, n);
                fill(window, &[a, b], self.fg.opacity((0.09 - 0.03 * k) * alpha));
            }
        }
        // The flow: ochre streaks running round inside the liquid, tapered at both ends,
        // faster while the size is unknown; they fade once the install is done.
        let calm = if self.phase == Phase::Done { 1. - ease(self.since / 0.3) } else { 1. };
        if calm <= 0. {
            return;
        }
        let speed = 1.2 + 1.3 * (1. - self.known);
        let head = t * speed;
        let (lo, hi) = if hole { (inner + (r - inner) * 0.2, r - (r - inner) * 0.2) } else { (r * 0.46, r * 0.84) };
        let mid = (lo + hi) / 2.;
        let half = (hi - lo) / 2.;
        // A wide soft glow, the main streak, and a thinner one on the far side.
        for (offset, len, width, a) in [(0., 1.5, 1.35, 0.14), (0., 1.15, 1., 0.72), (PI + 0.6, 0.8, 0.6, 0.38)] {
            fill(window, &[comet(cx, cy, mid, half * width, head + offset, len, r * 0.03, t)], self.accent.opacity(a * alpha * calm));
        }
    }

    /// The check drawn into the full disc, stroke by stroke.
    fn check(self, window: &mut Window, cx: f32, cy: f32, r: f32) {
        let k = ease(self.since / 0.35);
        let a = (cx - r * 0.38, cy + r * 0.02);
        let m = (cx - r * 0.1, cy + r * 0.3);
        let z = (cx + r * 0.42, cy - r * 0.3);
        let lerp = |p: (f32, f32), q: (f32, f32), s: f32| (p.0 + (q.0 - p.0) * s, p.1 + (q.1 - p.1) * s);
        let mut line = PathBuilder::stroke(px(r * 0.16));
        line.move_to(point(px(a.0), px(a.1)));
        let end1 = lerp(a, m, (k * 2.).min(1.));
        line.line_to(point(px(end1.0), px(end1.1)));
        if k > 0.5 {
            let end2 = lerp(m, z, (k - 0.5) * 2.);
            line.line_to(point(px(end2.0), px(end2.1)));
        }
        if let Ok(p) = line.build() {
            window.paint_path(p, self.ground);
        }
    }

    /// The ring after a failure: thin, still, broken open at the top.
    fn broken(self, window: &mut Window, cx: f32, cy: f32, r: f32) {
        let t = self.t;
        fill(window, &[arc_band(cx, cy, r * 0.84, r, -PI / 2. + 0.42, -PI / 2. + TAU - 0.42, r * 0.03, t)], self.danger.opacity(0.85));
    }

    /// Unpacking: a box whose lids open and whose contents rise out as tar writes them.
    fn fluxbox(self, window: &mut Window, cx: f32, cy: f32, r: f32, show: f32) {
        let t = self.t;
        let (hw, top, bottom, w) = (r * 0.62, cy - r * 0.02, cy + r * 0.74, r * 0.08);
        let fg = self.fg.opacity(0.92 * show);
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        // The liquid inside, draining as the files come out.
        let drained = self.unpacked.unwrap_or(0.35 + 0.1 * (t * 1.3).sin());
        let depth = (bottom - top - w) * (0.78 - 0.5 * drained.clamp(0., 1.));
        let level = bottom - w - depth;
        let mut surface: Vec<(f32, f32)> = (0..=24)
            .map(|i| {
                let x = cx - hw + w + (2. * hw - 2. * w) * i as f32 / 24.;
                (x, level + r * 0.035 * ((x - cx) / hw * PI * 1.5 + t * 3.).sin())
            })
            .collect();
        surface.push((cx + hw - w, bottom - w));
        surface.push((cx - hw + w, bottom - w));
        fill(window, &[surface], self.accent.opacity(0.75 * show));
        // Walls and floor.
        fill(window, &[rect(cx - hw, top, cx - hw + w, bottom), rect(cx + hw - w, top, cx + hw, bottom), rect(cx - hw, bottom - w, cx + hw, bottom)], fg);
        // Lids: hinged at the top corners, swinging up and out.
        let open = ease((self.since - 0.15) / 0.45) * 2.25;
        let len = hw * 0.96;
        for side in [-1f32, 1.] {
            let hinge = (cx + side * hw - side * w * 0.5, top);
            let dir = (-side * open.cos(), -open.sin());
            let nrm = (-dir.1 * w * 0.5, dir.0 * w * 0.5);
            let tip = (hinge.0 + dir.0 * len, hinge.1 + dir.1 * len);
            fill(window, &[vec![(hinge.0 - nrm.0, hinge.1 - nrm.1), (tip.0 - nrm.0, tip.1 - nrm.1), (tip.0 + nrm.0, tip.1 + nrm.1), (hinge.0 + nrm.0, hinge.1 + nrm.1)]], fg);
        }
        // Contents rising once the lids are open: small squares that turn and fade.
        if open > 1.4 {
            let lift = ease((open - 1.4) / 0.85) * show;
            for i in 0..6 {
                let life = (t / 1.15 + i as f32 / 6.).fract();
                let x = cx + (i as f32 - 2.5) * r * 0.15 + (life * 4. + i as f32).sin() * r * 0.04;
                let y = top - r * 0.08 - life * r * 0.95;
                let s = r * 0.1 * (1. - 0.45 * life);
                let rot = life * 1.6 + i as f32;
                let sq: Vec<(f32, f32)> = (0..4).map(|k| {
                    let a = rot + k as f32 * PI / 2. + PI / 4.;
                    (x + s * 0.7 * a.cos(), y + s * 0.7 * a.sin())
                }).collect();
                let col = if i % 2 == 0 { self.fg } else { self.accent };
                fill(window, &[sq], col.opacity((1. - life).powf(0.6) * lift));
            }
        }
    }
}

/// A streak along the circle of radius `mid`, `half` wide at its middle and pointed at
/// both ends, ending at angle `head` and `len` radians long.
fn comet(cx: f32, cy: f32, mid: f32, half: f32, head: f32, len: f32, amp: f32, t: f32) -> Vec<(f32, f32)> {
    let steps = 40;
    let at = |i: usize| {
        let s = i as f32 / steps as f32;
        // Fuller toward the head, like a drop being pulled along.
        let taper = (s * PI).sin().max(0.).powf(0.8) * (0.55 + 0.45 * s);
        (head - len * (1. - s), half * taper)
    };
    let mut v: Vec<(f32, f32)> = (0..=steps)
        .map(|i| {
            let (th, h) = at(i);
            let rr = mid + h + amp * wob(th, t, 0.);
            (cx + rr * th.cos(), cy + rr * th.sin())
        })
        .collect();
    v.extend((0..=steps).rev().map(|i| {
        let (th, h) = at(i);
        let rr = (mid - h + amp * wob(th, t * 1.1, 2.6)).max(0.);
        (cx + rr * th.cos(), cy + rr * th.sin())
    }));
    v
}

/// A band between radii `lo` and `hi` from angle `a0` to `a1`, its edges moving like the ring's.
fn arc_band(cx: f32, cy: f32, lo: f32, hi: f32, a0: f32, a1: f32, amp: f32, t: f32) -> Vec<(f32, f32)> {
    let steps = (((a1 - a0).abs() / TAU) * 160.).ceil().max(8.) as usize;
    let at = |i: usize| a0 + (a1 - a0) * i as f32 / steps as f32;
    let mut v: Vec<(f32, f32)> = (0..=steps)
        .map(|i| {
            let th = at(i);
            let rr = hi + amp * wob(th, t, 0.);
            (cx + rr * th.cos(), cy + rr * th.sin())
        })
        .collect();
    v.extend((0..=steps).rev().map(|i| {
        let th = at(i);
        let rr = (lo + amp * wob(th, t * 1.1, 2.6)).max(0.);
        (cx + rr * th.cos(), cy + rr * th.sin())
    }));
    v
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in gpui_kit's `test` attribute, which shadows std's.
    use super::{blob, detail, inner_radius, target, wob};
    use crate::acp::{Phase, Snapshot};

    #[test]
    fn the_ring_closes_as_it_fills() {
        let r = 40.;
        let mut last = f32::MAX;
        for i in 0..=20 {
            let f = i as f32 / 20.;
            let inner = inner_radius(r, f, 1., 0.);
            assert!(inner <= last + 1e-4, "inner radius grew at fill {f}");
            assert!(inner.is_finite() && inner >= 0.);
            last = inner;
        }
        assert!(inner_radius(r, 1., 1., 0.) < 1e-3, "a full ring is a disc");
        assert!(inner_radius(r, 0., 1., 0.) > r * 0.7, "an empty ring is thin");
        // Unknown size: thin whatever the fill says.
        assert!(inner_radius(r, 1., 0., 3.) > r * 0.7);
    }

    #[test]
    fn the_edges_stay_close_to_the_circle() {
        for i in 0..200 {
            let th = i as f32 * 0.37;
            let w = wob(th, i as f32 * 0.1, 1.3);
            assert!((-1.0..=1.0).contains(&w));
        }
        for (x, y) in blob(0., 0., 10., 1., 2., 0., 64) {
            let d = (x * x + y * y).sqrt();
            assert!((9.0..=11.0).contains(&d), "{d}");
        }
    }

    #[test]
    fn every_shape_point_is_finite() {
        // A NaN point aborts the tessellator (and the app); sweep the shapes.
        for i in 0..400 {
            let t = i as f32 * 0.173;
            let head = i as f32 * 0.91 - 50.;
            for (mid, half) in [(30., 6.), (0.5, 0.2), (0., 0.), (40., 40.)] {
                for (x, y) in super::comet(50., 50., mid, half, head, 1.15, 1.2, t) {
                    assert!(x.is_finite() && y.is_finite(), "comet at t={t} head={head}");
                }
            }
            for (x, y) in super::arc_band(50., 50., 0., 40., head, head + 6.0, 1.2, t) {
                assert!(x.is_finite() && y.is_finite(), "band at t={t}");
            }
            for (x, y) in blob(50., 50., inner_radius(40., (i % 21) as f32 / 20., (i % 3) as f32 / 2., t), 2., t, 1., 96) {
                assert!(x.is_finite() && y.is_finite());
            }
        }
    }

    #[test]
    fn the_fill_target_follows_the_install() {
        let s = |phase, done, total| Snapshot { phase, done, total };
        assert_eq!(target(&s(Phase::Download, 25, 100)), (Some(0.25), true));
        assert_eq!(target(&s(Phase::Download, 25, 0)), (None, false));
        assert_eq!(target(&s(Phase::Unpack, 3, 9)), (Some(1.), true));
        assert_eq!(target(&s(Phase::Npm, 0, 0)), (None, false));
        assert_eq!(target(&s(Phase::Failed, 0, 0)), (None, false));
    }

    #[test]
    fn the_detail_line_says_what_was_measured() {
        let s = |phase, done, total| Snapshot { phase, done, total };
        assert_eq!(detail(&s(Phase::Download, 5 * 1_048_576, 20 * 1_048_576)), "Downloading · 5.0 of 20.0 MB · 25%");
        assert_eq!(detail(&s(Phase::Download, 3 * 1_048_576, 0)), "Downloading · 3.0 MB");
        assert_eq!(detail(&s(Phase::Download, 0, 0)), "Downloading…");
        assert_eq!(detail(&s(Phase::Unpack, 4, 10)), "Unpacking · 4 of 10 files");
        assert_eq!(detail(&s(Phase::Unpack, 12, 12)), "Unpacking · 12 files");
        assert_eq!(detail(&s(Phase::Verify, 0, 0)), "Checking the download…");
    }
}
