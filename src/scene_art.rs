
use crate::theme::{self, mix, Tok};
use gpui_kit::*;
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub struct Mood {
    pub dark: bool,
    ink: u32,
    top: u32,
    mid: u32,
    low: u32,
    glow: u32,
    far: u32,
    hill: u32,
    near: u32,
    light: u32,
    cool: u32,
    green: u32,
    accent: u32,
    base: u32,
}

pub fn mood() -> Mood {
    let l = theme::live;
    let (title, editor, accent, blue, green, strong, muted, fg) =
        (l(Tok::BgTitle), l(Tok::BgEditor), l(Tok::Accent), l(Tok::Blue), l(Tok::Green), l(Tok::FgStrong), l(Tok::FgMuted), l(Tok::Fg));
    if theme::live_dark() {
        let ink = mix(title, 0x000000, 0.35);
        let blue = vivid(blue, 1.6);
        let top = vivid(mix(ink, blue, 0.2), 1.5);
        let mid = vivid(mix(mix(title, blue, 0.32), accent, 0.12), 1.4);
        let low = vivid(mix(title, accent, 0.42), 1.2);
        Mood {
            dark: true,
            ink,
            top,
            mid,
            low,
            glow: accent,
            far: mix(mid, ink, 0.30),
            hill: mix(top, ink, 0.55),
            near: mix(ink, 0x000000, 0.30),
            light: strong,
            cool: blue,
            green,
            accent,
            base: editor,
        }
    } else {
        let b = editor;
        Mood {
            dark: false,
            ink: mix(b, fg, 0.30),
            top: mix(b, blue, 0.30),
            mid: mix(b, blue, 0.13),
            low: mix(b, accent, 0.26),
            glow: mix(accent, 0xffffff, 0.6),
            far: mix(b, blue, 0.20),
            hill: mix(b, muted, 0.20),
            near: mix(b, muted, 0.32),
            light: 0xffffff,
            cool: mix(blue, b, 0.25),
            green: mix(green, b, 0.25),
            accent,
            base: b,
        }
    }
}

fn hash(i: u32, k: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B) ^ 0x68E3_1DA4;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0xFFFF) as f32 / 65535.
}

fn sway(t: f32, period: f32, phase: f32) -> f32 {
    (t / period * TAU + phase).sin()
}

fn vivid(hex: u32, k: f32) -> u32 {
    let mut c = Hsla::from(rgb(hex));
    c.s = (c.s * k).min(1.);
    let r = c.to_rgb();
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    (b(r.r) << 16) | (b(r.g) << 8) | b(r.b)
}

fn col(hex: u32, a: f32) -> Hsla {
    Hsla::from(rgb(hex)).opacity(a.clamp(0., 1.))
}

#[derive(Clone, Copy)]
enum Ridge {
    Smooth,
    Peaks,
    Pines,
}

struct Art<'a> {
    win: &'a mut Window,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    t: f32,
    m: Mood,
    density: f32,
    cached: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RidgeKey {
    kind: u8,
    base: u32,
    amp: u32,
    seed: u32,
    c: u32,
    iw: u32,
    ih: u32,
}

thread_local! {
    static RIDGES: std::cell::RefCell<Vec<(RidgeKey, std::sync::Arc<RenderImage>, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
    static FRAME: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn ridge_image(key: RidgeKey, top: impl Fn(f32) -> f32) -> Option<std::sync::Arc<RenderImage>> {
    let frame = FRAME.with(|f| f.get());
    if let Some(img) = RIDGES.with(|r| {
        r.borrow_mut().iter_mut().find(|e| e.0 == key).map(|e| {
            e.2 = frame;
            e.1.clone()
        })
    }) {
        return Some(img);
    }
    let (w, h) = (key.iw as usize, key.ih as usize);
    let (cr, cg, cb) = (((key.c >> 16) & 0xff) as u8, ((key.c >> 8) & 0xff) as u8, (key.c & 0xff) as u8);
    let mut px = vec![0u8; w * h * 4];
    let mut tops3 = Vec::with_capacity(w * 3);
    for x in 0..w {
        for o in [0.17f32, 0.5, 0.83] {
            tops3.push(top((x as f32 + o) / w as f32) * h as f32);
        }
    }
    crate::accel::raster_ridge(&tops3, w, h, [cr, cg, cb], &mut px);
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, px)?;
    let img = std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(buf)]));
    RIDGES.with(|r| r.borrow_mut().push((key, img.clone(), frame)));
    Some(img)
}

fn sweep(window: &mut Window) {
    let frame = FRAME.with(|f| f.get());
    let old: Vec<_> = RIDGES.with(|r| {
        let mut r = r.borrow_mut();
        let (keep, old): (Vec<_>, Vec<_>) = r.drain(..).partition(|e| e.2 == frame);
        *r = keep;
        old
    });
    for (_, img, _) in old {
        _ = window.drop_image(img);
    }
}

impl Art<'_> {
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w.max(0.)), px(h.max(0.))))
    }

    fn n(&self, base: f32) -> u32 {
        (base * self.density).round().max(3.) as u32
    }

    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, bg: impl Into<Background>) {
        let r = self.rect(x, y, w, h);
        self.win.paint_quad(fill(r, bg));
    }

    fn round(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, c: Hsla) {
        let r = self.rect(x, y, w, h);
        self.win.paint_quad(quad(r, px(radius), c, px(0.), transparent_black(), BorderStyle::default()));
    }

    fn grad(&mut self, from: f32, to: f32, stops: &[(f32, Hsla)]) {
        let (x, w) = (self.x, self.w);
        for p in stops.windows(2) {
            let ((a, ca), (z, cz)) = (p[0], p[1]);
            let y0 = self.y + self.h * (from + (to - from) * a);
            let y1 = self.y + self.h * (from + (to - from) * z);
            self.fill(x, y0 - 0.5, w, y1 - y0 + 1., linear_gradient(180., linear_color_stop(ca, 0.), linear_color_stop(cz, 1.)));
        }
    }

    fn sky(&mut self, stops: &[(f32, u32)]) {
        let s: Vec<(f32, Hsla)> = stops.iter().map(|&(p, c)| (p, col(c, 1.))).collect();
        self.grad(0., 1., &s);
    }

    fn blob(&mut self, cx: f32, cy: f32, w: f32, h: f32, c: Hsla, blur: f32) {
        let r = self.rect(cx - w / 2., cy - h / 2., w, h);
        let radius = px(w.min(h) / 2.);
        self.win.paint_drop_shadows(
            r,
            Corners::all(radius),
            &[BoxShadow { color: c, offset: point(px(0.), px(0.)), blur_radius: px(blur.max(0.5)), spread_radius: px(0.), inset: false }],
        );
    }

    fn glow(&mut self, cx: f32, cy: f32, rw: f32, rh: f32, c: u32, a: f32) {
        for k in 0..3 {
            let f = 1. + k as f32 * 0.9;
            self.blob(cx, cy, rw * f, rh * f, col(c, a / f), rh * f * 0.5);
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: u32, halo: f32) {
        self.glow(cx, cy, r * 3.2, r * 3.2, c, halo);
        self.round(cx - r, cy - r, r * 2., r * 2., r, col(c, 0.92));
        self.blob(cx + r * 0.35, cy + r * 0.35, r * 1.3, r * 1.3, col(self.m.ink, 0.12), r * 0.5);
    }

    fn stars(&mut self, count: f32, bottom: f32, seed: u32) {
        let c = if self.m.dark { self.m.light } else { self.m.ink };
        let strength = if self.m.dark { 1. } else { 0.25 };
        for i in 0..self.n(count) {
            let (sx, sy) = (hash(i, seed), hash(i, seed + 1).powf(1.3) * bottom);
            let r = 0.7 + hash(i, seed + 2).powi(3) * 1.8;
            let tw = 0.6 + 0.4 * sway(self.t, 2.2 + hash(i, seed + 3) * 5., hash(i, seed + 4) * TAU);
            let a = (1. - sy / bottom).powf(0.7) * tw * (0.35 + 0.6 * hash(i, seed + 5)) * strength;
            let (x, y) = (self.x + sx * self.w, self.y + sy * self.h);
            self.round(x - r / 2., y - r / 2., r, r, r / 2., col(c, a));
        }
    }

    fn profile(kind: Ridge, xf: f32, seed: u32) -> f32 {
        let s = |k: u32| hash(seed, k) * TAU;
        match kind {
            Ridge::Smooth => {
                let v = 0.55 * (xf * TAU * 1.1 + s(1)).sin() + 0.3 * (xf * TAU * 2.7 + s(2)).sin() + 0.15 * (xf * TAU * 6.3 + s(3)).sin();
                0.5 + 0.5 * v
            }
            Ridge::Peaks => {
                let r = |f: f32, k: u32| 1. - (xf * TAU * f + s(k)).sin().abs();
                let v = 0.6 * r(1.2, 1) + 0.28 * r(2.9, 2) + 0.12 * r(7.3, 3);
                v.powf(1.15)
            }
            Ridge::Pines => {
                let ground = 0.2 * Self::profile(Ridge::Smooth, xf, seed + 7);
                let sp = 0.016;
                let i = (xf / sp).floor() as i32;
                let mut tree = 0f32;
                for k in i - 1..=i + 1 {
                    let ku = (k + 10_000) as u32;
                    let cx = (k as f32 + 0.5 + (hash(ku, seed) - 0.5) * 0.7) * sp;
                    let th = 0.45 + 0.55 * hash(ku, seed + 1);
                    let half = sp * (0.42 + 0.25 * th);
                    let d = (xf - cx).abs();
                    if d < half {
                        tree = tree.max(th * (1. - d / half));
                    }
                }
                ground + 0.8 * tree
            }
        }
    }

    fn ridge(&mut self, kind: Ridge, base: f32, amp: f32, seed: u32, c: u32) {
        let top = self.y + self.h * (base - amp) - 2.;
        let band = self.y + self.h - top;
        if self.cached {
            let k = self.win.scale_factor() * 0.5;
            let (iw, ih) = ((self.w * k).ceil().max(1.) as u32, (band * k).ceil().max(1.) as u32);
            let key = RidgeKey { kind: kind as u8, base: base.to_bits(), amp: amp.to_bits(), seed, c, iw, ih };
            if let Some(img) = ridge_image(key, |xf| (self.h * (base - amp * Self::profile(kind, xf, seed)) - (top - self.y)) / band) {
                let at = self.rect(self.x, top, self.w, band);
                _ = self.win.paint_image(at, at, Corners::default(), img, 0, false);
            }
            return;
        }
        let step = match kind {
            Ridge::Pines => (self.w / 1400.).max(1.),
            _ => (self.w / 700.).max(1.5),
        };
        let n = (self.w / step).ceil() as u32 + 1;
        let bottom = self.y + self.h;
        for i in 0..n {
            let x = self.x + i as f32 * step;
            let xf = (i as f32 * step) / self.w;
            let top = self.y + self.h * (base - amp * Self::profile(kind, xf, seed));
            self.fill(x, top, step + 0.6, bottom - top, col(c, 1.));
        }
    }

    fn mist(&mut self, from: f32, to: f32, c: u32, a: f32) {
        self.grad(from, to, &[(0., col(c, 0.)), (0.65, col(c, a)), (1., col(c, 0.))]);
    }

    fn fog(&mut self, y: f32, c: u32, a: f32, speed: f32, seed: u32) {
        for i in 0..5u32 {
            let span = self.w * 1.6;
            let x = self.x + ((hash(i, seed) * span + self.t * speed * self.w * 0.01).rem_euclid(span)) - self.w * 0.3;
            let (bw, bh) = (self.w * (0.35 + 0.3 * hash(i, seed + 1)), self.h * (0.035 + 0.03 * hash(i, seed + 2)));
            self.blob(x, self.y + self.h * (y + 0.02 * (hash(i, seed + 3) - 0.5)), bw, bh, col(c, a), bh * 0.8);
        }
    }

    fn cloud(&mut self, cx: f32, cy: f32, size: f32, shade: u32, lit: u32, a: f32, seed: u32) {
        let n = 11;
        for i in 0..n {
            let dx = (i as f32 / (n - 1) as f32 - 0.5) * 2. + (hash(i, seed) - 0.5) * 0.25;
            let lift = 1. - dx.abs().powf(1.5);
            let r = size * (0.45 + 0.35 * hash(i, seed + 1)) * (0.55 + 0.45 * lift);
            let x = cx + dx * size * 1.7;
            let y = cy - size * 0.35 * lift - r * 0.15 * hash(i, seed + 2);
            self.blob(x, y + r * 0.3, r * 3.2, r * 1.5, col(shade, a * 0.8), r * 0.32);
        }
        for i in 0..n {
            let dx = (i as f32 / (n - 1) as f32 - 0.5) * 1.8 + (hash(i, seed + 9) - 0.5) * 0.25;
            let lift = 1. - dx.abs().powf(1.5);
            let r = size * (0.32 + 0.3 * hash(i, seed + 10)) * (0.55 + 0.45 * lift);
            let x = cx + dx * size * 1.6;
            let y = cy - size * 0.42 * lift - r * 0.25;
            self.blob(x, y, r * 2.6, r * 1.05, col(lit, a * 0.55), r * 0.34);
        }
    }

    fn bank(&mut self, y: f32, size: f32, count: u32, shade: u32, lit: u32, a: f32, drift: f32, seed: u32) {
        for i in 0..count {
            let base = (i as f32 + 0.5 + (hash(i, seed) - 0.5) * 0.6) / count as f32;
            let xf = (base + self.t * drift).rem_euclid(1.2) - 0.1;
            let s = size * (0.75 + 0.5 * hash(i, seed + 1)) * self.h;
            let yy = self.y + self.h * (y + 0.04 * (hash(i, seed + 2) - 0.5));
            self.cloud(self.x + xf * self.w, yy, s, shade, lit, a, seed + i * 31);
        }
    }

    fn water(&mut self, top: f32, upper: u32, lower: u32, light: u32, lx: f32, sheen: f32) {
        self.grad(top, 1., &[(0., col(upper, 1.)), (1., col(lower, 1.))]);
        let (y0, depth) = (self.y + self.h * top, self.h * (1. - top));
        let n = self.n(26.).min(40);
        for k in 0..n {
            let f = k as f32 / n as f32;
            let y = y0 + depth * f.powf(1.7);
            let wide = self.w * (0.03 + 0.16 * f) * (0.55 + 0.45 * sway(self.t, 2.4 + hash(k, 3) * 2., hash(k, 4) * TAU));
            let x = self.x + self.w * lx + self.w * 0.012 * sway(self.t, 6., k as f32) - wide / 2.;
            let hh = 1.2 + 2.2 * f;
            self.round(x, y, wide, hh, hh / 2., col(light, sheen * (0.95 - 0.6 * f)));
        }
        let rows = self.n(14.).min(22);
        for r in 0..rows {
            let f = (r as f32 + 0.5) / rows as f32;
            let y = y0 + depth * f.powf(1.5);
            for d in 0..4u32 {
                let len = self.w * (0.02 + 0.06 * f) * (0.5 + hash(r * 7 + d, 11));
                let speed = 0.004 + 0.01 * f;
                let xf = (hash(r * 7 + d, 12) + self.t * speed * if r % 2 == 0 { 1. } else { -0.7 }).rem_euclid(1.);
                self.round(self.x + xf * self.w, y, len, 1. + f, 1., col(light, 0.07 + 0.05 * f));
            }
        }
    }

    fn fall(&mut self, count: f32, snow: bool, c: u32, seed: u32) {
        for i in 0..self.n(count) {
            let speed = if snow { 0.025 + 0.03 * hash(i, seed) } else { 0.9 + 0.5 * hash(i, seed) };
            let yf = (hash(i, seed + 1) + self.t * speed).rem_euclid(1.);
            let mut xf = hash(i, seed + 2);
            if snow {
                xf += 0.01 * sway(self.t, 4. + 3. * hash(i, seed + 3), hash(i, seed + 4) * TAU);
            } else {
                xf -= 0.02 * yf;
            }
            let (x, y) = (self.x + xf.rem_euclid(1.) * self.w, self.y + yf * self.h);
            if snow {
                let r = 1.2 + 2.4 * hash(i, seed + 5);
                self.round(x, y, r, r, r / 2., col(c, 0.35 + 0.4 * hash(i, seed + 6)));
            } else {
                let len = 10. + 18. * hash(i, seed + 5);
                self.fill(x, y, 1., len, linear_gradient(180., linear_color_stop(col(c, 0.), 0.), linear_color_stop(col(c, 0.28), 1.)));
            }
        }
    }

    fn sparks(&mut self, count: f32, from: f32, to: f32, c: u32, seed: u32) {
        for i in 0..self.n(count) {
            let xf = hash(i, seed) + 0.03 * sway(self.t, 9. + 6. * hash(i, seed + 1), hash(i, seed + 2) * TAU);
            let yf = from + (to - from) * hash(i, seed + 3) + 0.02 * sway(self.t, 7. + 5. * hash(i, seed + 4), hash(i, seed + 5) * TAU);
            let pulse = (0.5 + 0.5 * sway(self.t, 2.5 + 3. * hash(i, seed + 6), hash(i, seed + 7) * TAU)).powi(2);
            let (x, y) = (self.x + xf.rem_euclid(1.) * self.w, self.y + yf * self.h);
            self.blob(x, y, 7., 7., col(c, 0.55 * pulse), 4.);
            self.round(x - 1., y - 1., 2., 2., 1., col(self.m.light, 0.8 * pulse));
        }
    }

    fn meteor(&mut self, every: f32) {
        let k = (self.t / every).floor();
        let local = self.t / every - k;
        if local > 0.1 {
            return;
        }
        let p = local / 0.1;
        let ku = k as u32;
        let (sx, sy) = (0.25 + 0.65 * hash(ku, 1), 0.05 + 0.25 * hash(ku, 2));
        let (dx, dy) = (-0.22, 0.12);
        for d in 0..16 {
            let q = p - d as f32 * 0.015;
            if !(0. ..=1.).contains(&q) {
                continue;
            }
            let (x, y) = (self.x + self.w * (sx + dx * q), self.y + self.h * (sy + dy * q));
            let a = (1. - d as f32 / 16.) * (1. - p).min(p * 5.).min(1.);
            let r = 2.2 - d as f32 * 0.1;
            self.round(x - r / 2., y - r / 2., r, r, r / 2., col(self.m.light, a));
        }
    }

    fn city(&mut self, base: f32, tall: f32, c: u32, windows: bool, seed: u32) {
        let unit = (self.w / 1400.).clamp(0.12, 1.4);
        let mut x = 0.;
        let mut i = 0u32;
        while x < self.w {
            let bw = (26. + 64. * hash(i, seed)) * unit;
            let bh = self.h * tall * (0.3 + 0.7 * hash(i, seed + 1).powf(1.3));
            let top = self.y + self.h * base - bh;
            self.fill(self.x + x, top, bw + 0.5, self.y + self.h - top, col(c, 1.));
            if windows && unit > 0.3 {
                let (cols, rows) = (((bw - 6.) / 9.).floor().max(0.) as u32, ((bh - 8.) / 12.).floor().max(0.) as u32);
                for r in 0..rows.min(30) {
                    for q in 0..cols {
                        let id = i * 4099 + r * 131 + q;
                        let period = 7. + 16. * hash(id, seed + 2);
                        let lit = hash(id, (self.t / period + hash(id, seed + 3)).floor() as u32 + seed) > 0.74;
                        if lit {
                            let wc = mix(self.m.glow, self.m.light, 0.45);
                            self.fill(self.x + x + 4. + q as f32 * 9., top + 6. + r as f32 * 12., 4., 6., col(wc, 0.5));
                        }
                    }
                }
            }
            if windows && hash(i, seed + 4) > 0.82 {
                let on = sway(self.t, 2.2, i as f32) > 0.4;
                self.round(self.x + x + bw / 2. - 1.5, top - 4., 3., 3., 1.5, col(self.m.accent, if on { 0.9 } else { 0.15 }));
            }
            x += bw + 2. * unit;
            i += 1;
        }
    }
}

pub fn paint(id: &str, b: Bounds<Pixels>, t: f32, cached: bool, window: &mut Window) {
    let m = mood();
    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
    if w < 2. || h < 2. {
        return;
    }
    if cached {
        FRAME.with(|f| f.set(f.get() + 1));
    }
    let mut a = Art { win: window, x: f32::from(b.origin.x), y: f32::from(b.origin.y), w, h, t, m, density: (w * h / (1400. * 900.)).clamp(0.08, 1.8), cached };
    let (cx, cy) = (a.x, a.y);
    match id {
        "sky" => clouds(&mut a),
        "ocean" => ocean(&mut a),
        "river" => lake(&mut a),
        "beach" => beach(&mut a),
        "mountains" => mountains(&mut a),
        "anime" => city(&mut a),
        "aurora" => aurora(&mut a),
        "rain" => rain(&mut a),
        "snow" => snow(&mut a),
        "desert" => dunes(&mut a),
        "forest" => forest(&mut a),
        "space" => galaxy(&mut a),
        _ => {}
    }
    let scrim = if m.dark { col(0x000000, 0.16) } else { col(m.base, 0.18) };
    let r = a.rect(cx, cy, w, h);
    a.win.paint_quad(fill(r, scrim));
    if cached {
        sweep(a.win);
    }
}

fn clouds(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.ink), (0.3, m.top), (0.55, m.mid), (0.7, m.low), (0.86, mix(m.low, m.ink, 0.5)), (1., m.ink)]);
    a.stars(110., 0.5, 10);
    a.meteor(13.);
    let (gx, gy) = (a.x + a.w * 0.5, a.y + a.h * 0.68);
    a.glow(gx, gy, a.w * 0.5 * (1. + 0.04 * sway(a.t, 14., 0.)), a.h * 0.14, m.glow, 0.22);
    let shade = mix(m.mid, m.ink, 0.35);
    let lit = mix(m.low, m.glow, 0.45);
    a.bank(0.16, 0.05, 4, mix(m.top, m.mid, 0.5), lit, 0.4, 0.0016, 100);
    a.bank(0.55, 0.11, 5, shade, lit, 0.9, -0.0022, 200);
    a.bank(0.9, 0.16, 4, mix(shade, m.ink, 0.35), lit, 0.95, 0.0012, 300);
}

fn ocean(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.ink), (0.4, m.top), (0.56, mix(m.mid, m.cool, 0.12))]);
    a.stars(120., 0.5, 20);
    a.meteor(17.);
    let (mx, my) = (a.x + a.w * 0.72, a.y + a.h * 0.2);
    let r = a.h * 0.045;
    a.disc(mx, my, r, m.light, if m.dark { 0.10 } else { 0.25 });
    a.bank(0.32, 0.045, 3, mix(m.mid, m.top, 0.4), m.light, 0.3, 0.001, 400);
    let sea_top = mix(m.top, m.cool, 0.15);
    a.water(0.56, sea_top, m.ink, m.light, 0.72, 0.45);
    a.mist(0.5, 0.58, m.mid, 0.4);
}

fn lake(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.top), (0.45, m.mid), (0.62, m.low)]);
    a.stars(60., 0.35, 30);
    a.disc(a.x + a.w * 0.3, a.y + a.h * 0.44, a.h * 0.05, m.glow, 0.18);
    a.bank(0.24, 0.05, 3, m.mid, mix(m.low, m.glow, 0.4), 0.5, 0.0012, 500);
    a.ridge(Ridge::Peaks, 0.6, 0.22, 3, m.far);
    a.ridge(Ridge::Smooth, 0.63, 0.09, 4, m.hill);
    a.water(0.63, mix(m.low, m.hill, 0.5), m.ink, mix(m.glow, m.light, 0.3), 0.3, 0.4);
    a.fog(0.62, m.light, if m.dark { 0.08 } else { 0.35 }, 0.25, 7);
    a.ridge(Ridge::Pines, 0.98, 0.16, 5, m.near);
}

fn beach(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.top), (0.3, m.mid), (0.5, m.low), (0.6, mix(m.low, m.glow, 0.35))]);
    a.stars(40., 0.25, 40);
    let (sx, sy) = (a.x + a.w * 0.5, a.y + a.h * 0.6);
    a.disc(sx, sy, a.h * 0.09, mix(m.glow, m.light, 0.25), 0.22);
    a.bank(0.34, 0.055, 4, mix(m.mid, m.low, 0.4), mix(m.glow, m.light, 0.3), 0.55, 0.0015, 600);
    a.water(0.6, mix(m.low, m.top, 0.5), mix(m.top, m.ink, 0.4), mix(m.glow, m.light, 0.35), 0.5, 0.55);
    let sand = mix(m.low, m.near, 0.45);
    let step = (a.w / 700.).max(1.5);
    let n = (a.w / step).ceil() as u32 + 1;
    for i in 0..n {
        let xf = i as f32 * step / a.w;
        let shore = 0.8 + 0.014 * (xf * TAU * 1.7 + a.t * 0.55).sin() + 0.006 * (xf * TAU * 5.3 - a.t * 0.9).sin();
        let top = a.y + a.h * shore;
        let x = a.x + i as f32 * step;
        a.fill(x, top, step + 0.6, a.y + a.h - top, col(sand, 1.));
        a.fill(x, top - 1., step + 0.6, 2.5, col(m.light, 0.45));
    }
    a.grad(0.8, 1., &[(0., col(m.ink, 0.)), (1., col(m.ink, 0.35))]);
}

fn mountains(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.ink), (0.35, m.top), (0.6, m.mid), (0.75, m.low)]);
    a.stars(110., 0.45, 50);
    a.meteor(19.);
    a.disc(a.x + a.w * 0.68, a.y + a.h * 0.24, a.h * 0.04, m.light, if m.dark { 0.1 } else { 0.25 });
    a.ridge(Ridge::Peaks, 0.58, 0.3, 11, mix(m.far, m.mid, 0.3));
    a.mist(0.42, 0.6, m.low, 0.35);
    a.ridge(Ridge::Peaks, 0.7, 0.28, 12, m.far);
    a.fog(0.66, m.light, if m.dark { 0.07 } else { 0.3 }, 0.35, 13);
    a.ridge(Ridge::Peaks, 0.84, 0.26, 14, m.hill);
    a.fog(0.82, m.light, if m.dark { 0.06 } else { 0.25 }, -0.25, 15);
    a.ridge(Ridge::Smooth, 1.0, 0.14, 16, m.near);
}

fn city(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.top), (0.35, m.mid), (0.62, m.low), (0.72, mix(m.low, m.glow, 0.3))]);
    a.stars(70., 0.4, 60);
    a.disc(a.x + a.w * 0.5, a.y + a.h * 0.62, a.h * 0.13, mix(m.glow, m.light, 0.2), 0.2);
    a.bank(0.22, 0.05, 3, m.mid, mix(m.low, m.glow, 0.4), 0.5, 0.0013, 700);
    a.city(0.78, 0.36, mix(m.far, m.mid, 0.3), false, 70);
    a.mist(0.55, 0.78, m.low, 0.3);
    a.city(1.0, 0.42, m.near, true, 80);
}

fn aurora(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.ink), (0.5, m.top), (0.8, mix(m.top, m.cool, 0.15))]);
    a.stars(150., 0.75, 90);
    let step = (a.w / 240.).max(3.);
    let n = (a.w / step).ceil() as u32 + 1;
    let strength = if m.dark { 1. } else { 0.55 };
    for curtain in 0..2u32 {
        let off = curtain as f32 * 1.9;
        let c1 = vivid(if curtain == 0 { m.green } else { m.cool }, 2.2);
        for i in 0..n {
            let xf = i as f32 * step / a.w;
            let wave = 0.5 * (xf * TAU * 1.1 + a.t * 0.11 + off).sin() + 0.3 * (xf * TAU * 2.8 - a.t * 0.19 + off).sin() + 0.2 * (xf * TAU * 6.1 + a.t * 0.33).sin();
            let top = 0.1 + 0.07 * wave + curtain as f32 * 0.06;
            let len = 0.2 + 0.08 * (xf * TAU * 2.1 + a.t * 0.17 + off).sin();
            let on = (0.5 + 0.5 * (xf * TAU * 3.4 - a.t * 0.25 + off * 2.).sin()).powf(1.5);
            let c = mix(c1, vivid(m.accent, 1.8), 0.25 + 0.25 * (xf * TAU * 0.7 + a.t * 0.04 + off).sin());
            let y0 = a.y + a.h * top;
            let hh = a.h * len;
            let al = (0.12 + 0.4 * on) * strength;
            let x = a.x + i as f32 * step;
            a.fill(x, y0, step + 0.5, hh, linear_gradient(180., linear_color_stop(col(c, 0.), 0.), linear_color_stop(col(c, al), 1.)));
            a.fill(x, y0 + hh - 3., step + 0.5, 3., col(mix(c, m.light, 0.3), al * 0.9));
        }
    }
    a.ridge(Ridge::Peaks, 0.8, 0.22, 21, mix(m.hill, m.light, if m.dark { 0.12 } else { 0.4 }));
    a.ridge(Ridge::Smooth, 0.92, 0.1, 22, mix(m.near, m.light, if m.dark { 0.08 } else { 0.5 }));
    a.ridge(Ridge::Pines, 1.0, 0.12, 23, m.near);
}

fn rain(a: &mut Art) {
    let m = a.m;
    let grey = mix(m.mid, m.ink, 0.25);
    a.sky(&[(0., mix(m.ink, m.top, 0.4)), (0.6, grey), (1., m.ink)]);
    a.bank(0.1, 0.12, 5, mix(grey, m.ink, 0.4), mix(grey, m.light, 0.12), 0.9, 0.0018, 800);
    a.bank(0.3, 0.1, 4, mix(grey, m.ink, 0.2), mix(grey, m.light, 0.08), 0.7, -0.0012, 900);
    a.ridge(Ridge::Smooth, 0.8, 0.14, 31, m.hill);
    a.mist(0.55, 0.8, grey, 0.5);
    a.ridge(Ridge::Pines, 1.0, 0.14, 32, m.near);
    a.fall(220., false, m.light, 33);
}

fn snow(a: &mut Art) {
    let m = a.m;
    let pale = mix(m.mid, m.light, if m.dark { 0.08 } else { 0.4 });
    a.sky(&[(0., m.top), (0.6, pale), (1., mix(pale, m.light, 0.1))]);
    a.bank(0.14, 0.09, 4, mix(m.top, pale, 0.5), mix(pale, m.light, 0.2), 0.6, 0.0009, 1000);
    let snowy = |k: f32| mix(m.hill, m.light, if m.dark { k * 0.25 } else { 0.4 + k * 0.4 });
    a.ridge(Ridge::Peaks, 0.66, 0.2, 41, snowy(0.6));
    a.ridge(Ridge::Smooth, 0.8, 0.12, 42, snowy(0.8));
    a.ridge(Ridge::Pines, 0.9, 0.12, 43, mix(m.near, m.ink, 0.2));
    a.ridge(Ridge::Smooth, 1.0, 0.07, 44, snowy(1.));
    a.fall(170., true, m.light, 45);
}

fn dunes(a: &mut Art) {
    let m = a.m;
    let warm = mix(m.low, m.glow, 0.25);
    a.sky(&[(0., m.top), (0.35, m.mid), (0.6, warm)]);
    a.stars(40., 0.3, 70);
    a.disc(a.x + a.w * 0.62, a.y + a.h * 0.5, a.h * 0.07, mix(m.glow, m.light, 0.3), 0.2);
    let sand = |k: f32| mix(mix(m.low, m.near, k), m.glow, 0.12);
    a.ridge(Ridge::Smooth, 0.66, 0.1, 51, sand(0.25));
    a.mist(0.5, 0.66, warm, 0.3);
    a.ridge(Ridge::Smooth, 0.78, 0.12, 52, sand(0.5));
    a.ridge(Ridge::Smooth, 0.92, 0.12, 53, sand(0.75));
    for i in 0..a.n(60.) {
        let xf = (hash(i, 54) + a.t * (0.01 + 0.02 * hash(i, 55))).rem_euclid(1.);
        let yf = 0.72 + 0.26 * hash(i, 56);
        let len = a.w * (0.01 + 0.03 * hash(i, 57));
        a.round(a.x + xf * a.w, a.y + yf * a.h, len, 1., 0.5, col(m.light, 0.12));
    }
}

fn forest(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.ink), (0.4, m.top), (0.7, mix(m.mid, m.green, 0.12))]);
    a.stars(110., 0.45, 80);
    a.disc(a.x + a.w * 0.25, a.y + a.h * 0.22, a.h * 0.05, m.light, if m.dark { 0.1 } else { 0.25 });
    a.ridge(Ridge::Peaks, 0.6, 0.18, 61, m.far);
    a.ridge(Ridge::Pines, 0.74, 0.2, 62, mix(m.hill, m.green, 0.08));
    a.fog(0.72, m.light, if m.dark { 0.07 } else { 0.3 }, 0.3, 63);
    a.mist(0.6, 0.8, m.mid, 0.35);
    a.ridge(Ridge::Pines, 1.0, 0.3, 64, m.near);
    a.sparks(36., 0.55, 0.95, mix(m.green, m.glow, 0.5), 65);
}

fn galaxy(a: &mut Art) {
    let m = a.m;
    a.sky(&[(0., m.ink), (0.5, mix(m.ink, m.top, 0.6)), (1., m.ink)]);
    let neb = [m.accent, m.cool, mix(m.accent, m.cool, 0.5), m.green, m.glow, m.cool].map(|c| vivid(c, 2.));
    let strength = 0.38;
    for (i, c) in neb.iter().enumerate() {
        let i = i as u32;
        let x = a.x + a.w * (0.15 + 0.7 * hash(i, 71) + 0.03 * sway(a.t, 60. + 30. * hash(i, 72), i as f32));
        let y = a.y + a.h * (0.15 + 0.6 * hash(i, 73) + 0.02 * sway(a.t, 50. + 20. * hash(i, 74), i as f32 * 2.));
        let (bw, bh) = (a.w * (0.25 + 0.3 * hash(i, 75)), a.h * (0.12 + 0.2 * hash(i, 76)));
        a.blob(x, y, bw, bh, col(*c, strength), bh * 0.7);
    }
    a.stars(320., 1., 77);
    a.blob(a.x + a.w * 0.5, a.y + a.h * 0.42, a.w * 1.2, a.h * 0.16, col(vivid(mix(m.cool, m.light, 0.3), 1.3), 0.12), a.h * 0.1);
    a.meteor(9.);
    let (px_, py_, r) = (a.x + a.w * 0.8, a.y + a.h * 0.72, a.h * 0.11);
    a.glow(px_, py_, r * 2.2, r * 2.2, vivid(m.cool, 1.5), 0.12);
    a.round(px_ - r, py_ - r, r * 2., r * 2., r, col(vivid(mix(m.mid, m.cool, 0.45), 1.5), 1.));
    a.blob(px_ - r * 0.35, py_ - r * 0.35, r * 1.2, r * 1.2, col(m.light, 0.2), r * 0.45);
    a.blob(px_ + r * 0.45, py_ + r * 0.45, r * 1.6, r * 1.6, col(m.ink, 0.55), r * 0.4);
    a.blob(px_, py_ + r * 0.1, r * 3.2, r * 0.18, col(mix(m.light, m.cool, 0.4), 0.3), r * 0.05);
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashes_are_spread_and_stable() {
        let v: Vec<f32> = (0..1000).map(|i| super::hash(i, 3)).collect();
        assert!(v.iter().all(|x| (0. ..=1.).contains(x)));
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
        assert_eq!(super::hash(7, 9), super::hash(7, 9));
    }

    #[test]
    fn ridges_stay_in_range() {
        for kind in [super::Ridge::Smooth, super::Ridge::Peaks, super::Ridge::Pines] {
            for i in 0..500 {
                let h = super::Art::profile(kind, i as f32 / 500., 5);
                assert!((-0.01..=1.01).contains(&h), "{h}");
            }
        }
    }
}
