
use super::{arg, args, cells, flow, inline, join_item_lines, join_lines, text, Align, Block, Doc, Kind, Marks, Span};
use crate::theme::{self, c};
use crate::ui;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, scroll::Scrollbar, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

const WIDTH: f32 = 860.;
const TOC_MAX: usize = 300;
const GROUP: u32 = 24;
pub const PACK_FROM: usize = 1 << 20;
pub const PROGRESSIVE_FROM: usize = 256 << 10;
const FIRST_PART: usize = 32 << 10;
const FRAME: usize = 64 << 10;
const FRAMES_KEPT: usize = 6;
const DRAW_COLORS: [u32; 4] = [0xF59E0B, 0xEF4444, 0x22C55E, 0x38BDF8];
const DRAW_COLOR_NAMES: [&str; 4] = ["Amber (1)", "Red (2)", "Green (3)", "Blue (4)"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InkTool {
    #[default]
    Pen,
    Marker,
    Highlighter,
}

impl InkTool {
    pub fn next(self) -> Self {
        match self {
            Self::Pen => Self::Marker,
            Self::Marker => Self::Highlighter,
            Self::Highlighter => Self::Pen,
        }
    }

    pub fn width(self) -> f32 {
        match self {
            Self::Pen => 2.5,
            Self::Marker => 5.0,
            Self::Highlighter => 15.0,
        }
    }

    pub fn alpha(self) -> f32 {
        match self {
            Self::Pen => 1.0,
            Self::Marker => 0.95,
            Self::Highlighter => 0.30,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pen => "Pen",
            Self::Marker => "Marker",
            Self::Highlighter => "Highlighter",
        }
    }
}

#[derive(Clone, Default)]
pub struct Stroke {
    pub color: u32,
    pub width: f32,
    pub alpha: f32,
    pub pts: Vec<(f32, f32)>,
}

#[derive(Clone, Default)]
pub struct InkState {
    pub draw: bool,
    pub color: u32,
    pub tool: InkTool,
    pub strokes: Vec<Stroke>,
    pub redo: Vec<Stroke>,
    pub active: Option<Stroke>,
    pub copied_seq: u64,
    pub copied_ok: bool,
}

impl InkState {
    pub(crate) fn new() -> Self {
        Self {
            draw: false,
            color: DRAW_COLORS[0],
            tool: InkTool::Pen,
            strokes: Vec::new(),
            redo: Vec::new(),
            active: None,
            copied_seq: 0,
            copied_ok: false,
        }
    }

    pub(crate) fn push_stroke(&mut self, stroke: Stroke) {
        if !stroke.pts.is_empty() {
            self.strokes.push(stroke);
            self.redo.clear();
        }
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(last) = self.strokes.pop() else {
            return false;
        };
        self.redo.push(last);
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.strokes.push(next);
        true
    }

    pub(crate) fn clear(&mut self) {
        self.strokes.clear();
        self.redo.clear();
        self.active = None;
    }
}

enum Text {
    Plain(Rc<str>),
    Packed { frames: Vec<Box<[u8]>>, len: usize, cache: RefCell<Vec<(usize, Rc<[u8]>)>> },
}

impl Text {
    fn frame(&self, f: usize) -> Rc<[u8]> {
        let Text::Packed { frames, cache, .. } = self else { return Rc::from(&[][..]) };
        let hit = cache.borrow().iter().position(|(k, _)| *k == f);
        if let Some(i) = hit {
            let mut c = cache.borrow_mut();
            let hit = c.remove(i);
            let data = hit.1.clone();
            c.insert(0, hit);
            return data;
        }
        let data: Rc<[u8]> = miniz_oxide::inflate::decompress_to_vec(&frames[f]).unwrap_or_default().into();
        let mut c = cache.borrow_mut();
        c.insert(0, (f, data.clone()));
        c.truncate(FRAMES_KEPT);
        data
    }

    fn get(&self, (a, b): Span) -> String {
        let (a, b) = (a as usize, b as usize);
        match self {
            Text::Plain(s) => s.get(a..b).unwrap_or("").to_string(),
            Text::Packed { len, .. } => {
                let b = b.min(*len);
                if b <= a {
                    return String::new();
                }
                let mut out = Vec::with_capacity(b - a);
                for f in a / FRAME..=(b - 1) / FRAME {
                    let data = self.frame(f);
                    let start = f * FRAME;
                    let (lo, hi) = (a.max(start) - start, b.min(start + data.len()) - start);
                    if lo < hi {
                        out.extend_from_slice(&data[lo..hi]);
                    }
                }
                String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            Text::Plain(s) => s.len(),
            Text::Packed { len, .. } => *len,
        }
    }

    fn full(&self) -> String {
        self.get((0, self.len() as u32))
    }

    fn packed_bytes(&self) -> usize {
        match self {
            Text::Plain(s) => s.len(),
            Text::Packed { frames, .. } => frames.iter().map(|f| f.len()).sum(),
        }
    }
}

pub struct Prepared {
    doc: Doc,
    text: Result<String, (Vec<Box<[u8]>>, usize)>,
    ms: f32,
}

pub fn prepare(src: String) -> Prepared {
    let t = std::time::Instant::now();
    let doc = super::parse(&src);
    let ms = t.elapsed().as_secs_f32() * 1e3;
    let text = if src.len() >= PACK_FROM {
        let frames = src.as_bytes().chunks(FRAME).map(|c| miniz_oxide::deflate::compress_to_vec(c, 1).into_boxed_slice()).collect();
        Err((frames, src.len()))
    } else {
        Ok(src)
    };
    Prepared { doc, text, ms }
}

type RectBox = [f32; 4];

struct Shared {
    text: Text,
    doc: Doc,
    groups: Vec<(u32, u32)>,
    want: u8,
    base: PathBuf,
    open: Rc<RefCell<HashSet<u32>>>,
    zoom: Rc<Cell<f32>>,
}

impl Shared {
    fn shown(&self, i: usize) -> bool {
        matches!(self.doc.lang.get(i), Some(&l) if l == 0 || l == self.want)
    }

    fn u(&self, v: f32) -> Pixels {
        theme::u(v * self.zoom.get())
    }
}

fn groups(doc: &Doc, want: u8) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    let mut open = false;
    for (i, b) in doc.blocks.iter().enumerate() {
        let l = doc.lang[i];
        if l != 0 && l != want {
            open = false;
            continue;
        }
        let alone = matches!(b, Block::Flow(_) | Block::Image(_) | Block::Video(_) | Block::Boxed { .. } | Block::Code(_) | Block::Toc | Block::Title(_));
        let starts = alone || matches!(b, Block::Heading(..) | Block::Release(_));
        match out.last_mut() {
            Some(g) if open && !starts && g.1 < GROUP => g.1 += 1,
            _ => {
                out.push((i as u32, 1));
                open = !alone;
            }
        }
        if alone {
            open = false;
        }
    }
    out
}

pub struct FluxView {
    shared: Rc<Shared>,
    pub list: ListState,
    pub parsed_ms: f32,
    pub pending: bool,
}

impl FluxView {
    fn build(doc: Doc, text: Text, base: PathBuf, lang: &str, parsed_ms: f32, open: Rc<RefCell<HashSet<u32>>>, zoom: Rc<Cell<f32>>) -> Self {
        let want = if lang == "id" { 2 } else { 1 };
        let mut groups = groups(&doc, want);
        groups.shrink_to_fit();
        let list = ListState::new(groups.len(), ListAlignment::Top, px(600.));
        zoom.set(zoom.get().clamp(0.5, 2.5));
        Self {
            shared: Rc::new(Shared { text, doc, groups, want, base, open, zoom }),
            list,
            parsed_ms,
            pending: false,
        }
    }

    pub fn new(src: String, base: PathBuf, lang: &str) -> Self {
        Self::from_prepared(prepare(src), base, lang, Rc::default(), Rc::new(Cell::new(1.)))
    }

    fn from_prepared(p: Prepared, base: PathBuf, lang: &str, open: Rc<RefCell<HashSet<u32>>>, zoom: Rc<Cell<f32>>) -> Self {
        let text = match p.text {
            Ok(s) => Text::Plain(s.into()),
            Err((frames, len)) => Text::Packed { frames, len, cache: RefCell::new(Vec::new()) },
        };
        Self::build(p.doc, text, base, lang, p.ms, open, zoom)
    }

    pub fn first_part(src: &str, base: PathBuf, lang: &str) -> Self {
        let mut cut = src.len().min(FIRST_PART);
        while !src.is_char_boundary(cut) {
            cut -= 1;
        }
        if let Some(i) = src[..cut].rfind("\n\n") {
            cut = i + 1;
        }
        let mut v = Self::new(src[..cut].to_string(), base, lang);
        v.pending = true;
        v
    }

    pub fn install(&mut self, p: Prepared, lang: &str) {
        let top = self.list.logical_scroll_top();
        let v = Self::from_prepared(p, self.shared.base.clone(), lang, self.shared.open.clone(), self.shared.zoom.clone());
        v.list.scroll_to(ListOffset { item_ix: top.item_ix.min(v.shared.groups.len().saturating_sub(1)), offset_in_item: top.offset_in_item });
        *self = v;
    }

    pub fn reload(&mut self, src: String, lang: &str) {
        self.install(prepare(src), lang);
    }

    pub fn blocks(&self) -> usize {
        self.shared.doc.blocks.len()
    }

    pub fn kept_bytes(&self) -> usize {
        self.shared.text.packed_bytes()
    }

    pub fn source(&self) -> String {
        self.shared.text.full()
    }

    pub fn zoom(&self) -> f32 {
        self.shared.zoom.get()
    }

    pub fn set_zoom(&self, z: f32) {
        let z = ((z * 20.).round() / 20.).clamp(0.5, 2.5);
        let prev = self.shared.zoom.get();
        self.shared.zoom.set(z);
        if (prev - z).abs() > 1e-4 {
            let top = self.list.logical_scroll_top();
            let count = self.shared.groups.len();
            self.list.splice(0..count, count);
            self.list.scroll_to(top);
        }
    }

    pub fn step_zoom(&self, dir: i32) {
        let cur = self.zoom();
        let next = if dir == 0 { 1. } else { cur + dir as f32 * 0.1 };
        self.set_zoom(next);
    }

    pub fn render(&self, notify: impl Fn(&mut App) + 'static) -> AnyElement {
        let sh = self.shared.clone();
        let state = self.list.clone();
        let scrollbar = self.list.clone();
        let notify = Rc::new(notify);
        let width = sh.doc.style.width.unwrap_or(WIDTH);
        let size = sh.doc.style.size.unwrap_or(15.);
        let font = sh.doc.style.font.clone();
        let sh_list = sh.clone();
        let font_item = font.clone();
        let notify_list = notify.clone();
        let list_el = list(self.list.clone(), move |ix, _window, cx| {
            let Some(&(first, count)) = sh_list.groups.get(ix) else { return div().into_any_element() };
            let ctx = Ctx { sh: &sh_list, list: &state, notify: notify_list.clone() };
            let mut col = v_flex().w_full();
            for bi in first as usize..(first + count) as usize {
                let next = (bi + 1 < sh_list.doc.blocks.len() && sh_list.shown(bi + 1)).then(|| &sh_list.doc.blocks[bi + 1]);
                col = col.child(ctx.block(&sh_list.doc.blocks[bi], bi, bi, next, cx));
            }
            h_flex()
                .w_full()
                .justify_center()
                .text_size(sh_list.u(size))
                .when_some(font_item.clone(), |d, f| d.font_family(f))
                .child(div().w_full().max_w(sh_list.u(width)).px_8().when(ix == 0, |d| d.pt_8()).when(ix + 1 == sh_list.groups.len(), |d| d.pb_16()).child(col))
                .into_any_element()
        })
        .size_full()
        .text_size(sh.u(size))
        .when_some(font, |l, f| l.font_family(f));

        let sh_wheel = sh;
        let list_wheel = self.list.clone();
        let notify_wheel = notify;
        let zoom_capture = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let sh_ev = sh_wheel.clone();
                let list_ev = list_wheel.clone();
                let notify_ev = notify_wheel.clone();
                window.on_mouse_event(move |e: &ScrollEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture || !e.modifiers.control || !hitbox.is_hovered(window) {
                        return;
                    }
                    cx.stop_propagation();
                    let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
                    if dy.abs() < 0.5 {
                        return;
                    }
                    let cur = sh_ev.zoom.get();
                    let next = (((cur + if dy > 0. { 0.1 } else { -0.1 }) * 20.).round() / 20.).clamp(0.5, 2.5);
                    if (cur - next).abs() > 1e-4 {
                        sh_ev.zoom.set(next);
                        let top = list_ev.logical_scroll_top();
                        let count = sh_ev.groups.len();
                        list_ev.splice(0..count, count);
                        list_ev.scroll_to(top);
                        notify_ev(cx);
                    }
                });
            },
        )
        .absolute()
        .inset_0();

        div()
            .id("fx-preview-wrap")
            .relative()
            .size_full()
            .child(list_el)
            .child(zoom_capture)
            .child(div().absolute().inset_0().child(Scrollbar::vertical(&scrollbar).viewport_from_layout()))
            .into_any_element()
    }
}

struct Ctx<'a> {
    sh: &'a Shared,
    list: &'a ListState,
    notify: Rc<dyn Fn(&mut App)>,
}

fn accent(sh: &Shared) -> Hsla {
    match sh.doc.style.accent {
        Some(h) => rgb(h).into(),
        None => c(theme::ACCENT_TEXT),
    }
}

impl Ctx<'_> {
    fn t(&self, s: Span) -> String {
        self.sh.text.get(s)
    }

    fn rich(&self, id: ElementId, s: &str, color: Hsla) -> AnyElement {
        let i = inline(s);
        let acc = accent(self.sh);
        let mut hl: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
        for (r, m) in &i.runs {
            if *m == Marks::default() {
                continue;
            }
            let mut h = HighlightStyle::default();
            if m.bold {
                h.font_weight = Some(FontWeight::SEMIBOLD);
                h.color = Some(c(theme::FG_STRONG));
            }
            if m.italic {
                h.font_style = Some(FontStyle::Italic);
            }
            if m.code || m.kbd || m.badge {
                h.background_color = Some(c(theme::BG_HOVER));
                h.color = Some(c(if m.badge { theme::ACCENT_TEXT } else { theme::FG_STRONG }));
            }
            if m.strike {
                h.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: None });
            }
            if m.mark {
                h.background_color = Some(acc.opacity(0.28));
            }
            if let Some(col) = m.color {
                h.color = Some(rgb(col).into());
            }
            hl.push((r.clone(), h));
        }
        for (r, _) in &i.links {
            hl.push((r.clone(), HighlightStyle { color: Some(acc), underline: Some(UnderlineStyle { thickness: px(1.), color: Some(acc.opacity(0.5)), wavy: false }), ..Default::default() }));
        }
        let styled = StyledText::new(i.text.clone()).with_highlights(merge(hl));
        let el = div().text_color(color);
        if i.links.is_empty() {
            return el.child(styled).into_any_element();
        }
        let ranges: Vec<_> = i.links.iter().map(|(r, _)| r.clone()).collect();
        let urls: Vec<String> = i.links.iter().map(|(_, u)| u.clone()).collect();
        let base = self.sh.base.clone();
        el.child(InteractiveText::new(id, styled).on_click(ranges, move |k, _, cx| {
            let u = &urls[k];
            if u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:") {
                cx.open_url(u);
            } else if !u.starts_with('#') {
                cx.open_with_system(&base.join(u));
            }
        }))
        .into_any_element()
    }

    fn block(&self, b: &Block, bi: usize, pos: usize, next: Option<&Block>, cx: &mut App) -> AnyElement {
        let id = |k: &str| ElementId::Name(format!("fx-{k}-{bi}").into());
        match *b {
            Block::Title(s) => v_flex()
                .pb_4()
                .gap_2()
                .child(div().text_size(self.sh.u(32.)).font_weight(FontWeight::BOLD).text_color(c(theme::FG_STRONG)).child(self.rich(id("t"), &self.t(s), c(theme::FG_STRONG))))
                .child(div().w(px(56.)).h(px(3.)).rounded_full().bg(accent(self.sh)))
                .into_any_element(),
            Block::Heading(l, s) => {
                let size = match l {
                    1 => 26.,
                    2 => 21.,
                    3 => 18.,
                    _ => 16.,
                };
                div()
                    .pt(self.sh.u(if l <= 2 { 22. } else { 14. }))
                    .pb_2()
                    .when(l <= 2, |d| d.border_b_1().border_color(c(theme::BORDER)).mb_3())
                    .text_size(self.sh.u(size))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.rich(id("h"), &self.t(s), c(theme::FG_STRONG)))
                    .into_any_element()
            }
            Block::Para(s) => div().pb_3().line_height(relative(1.6)).child(self.rich(id("p"), &join_lines(&self.t(s), false), c(theme::FG))).into_any_element(),
            Block::Quote(s) => div()
                .mb_3()
                .pl_4()
                .py_1()
                .border_l_2()
                .border_color(accent(self.sh).opacity(0.6))
                .line_height(relative(1.6))
                .child(self.rich(id("q"), &join_lines(&self.t(s), true), c(theme::FG_MUTED)))
                .into_any_element(),
            Block::Item { num, depth, check, text: s } => {
                let mark: AnyElement = match check {
                    1 | 2 => div()
                        .flex_none()
                        .mt(px(3.))
                        .size(self.sh.u(15.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(c(if check == 2 { theme::ACCENT } else { theme::BORDER_STRONG }))
                        .when(check == 2, |d| d.bg(c(theme::ACCENT)).child(Icon::new(IconName::Check).size(self.sh.u(11.)).text_color(c(theme::ON_ACCENT))))
                        .flex()
                        .items_center()
                        .justify_center()
                        .into_any_element(),
                    _ if num > 0 => div().flex_none().min_w(self.sh.u(20.)).text_color(c(theme::FG_MUTED)).child(format!("{num}.")).into_any_element(),
                    _ => div().flex_none().w(self.sh.u(20.)).pt(self.sh.u(9.)).child(div().size(px(5.)).rounded_full().bg(if depth == 0 { accent(self.sh) } else { c(theme::FG_DIM) })).into_any_element(),
                };
                let last_in_run = !matches!(next, Some(Block::Item { .. }));
                h_flex()
                    .items_start()
                    .gap_2()
                    .pl(self.sh.u(4. + depth as f32 * 22.))
                    .pb(self.sh.u(if last_in_run { 12. } else { 4. }))
                    .line_height(relative(1.6))
                    .child(mark)
                    .child(div().flex_1().min_w_0().child(self.rich(id("li"), &join_item_lines(&self.t(s)), c(if check == 2 { theme::FG_MUTED } else { theme::FG }))))
                    .into_any_element()
            }
            Block::Code(k) => {
                let (lang, body) = self.sh.doc.codes[k as usize];
                let lang = self.t(lang).trim().to_string();
                v_flex()
                .mb_4()
                .rounded(px(8.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_EDITOR))
                .overflow_hidden()
                .when(!lang.is_empty(), |v| v.child(div().px_3().pt_2().text_xs().text_color(c(theme::FG_DIM)).child(lang)))
                .child(
                    div()
                        .px_3()
                        .py_2p5()
                        .font_family("Cascadia Code")
                        .text_size(self.sh.u(13.))
                        .line_height(relative(1.5))
                        .text_color(c(theme::FG))
                        .child(self.t(body)),
                )
                .into_any_element()
            }
            Block::Rule => div().my_5().h(px(1.)).bg(c(theme::BORDER)).into_any_element(),
            Block::Row { table, line } => self.row(table, line, pos, next),
            Block::Image(a) => self.image(a, bi),
            Block::Video(a) => self.video(a, bi),
            Block::Flow(f) => self.flow(&self.sh.doc.flows[f as usize], bi),
            Block::Boxed { kind, title, inner } => self.boxed(kind, title, inner, bi, cx),
            Block::ColBreak => div().into_any_element(),
            Block::Toc => self.toc(bi),
            Block::Release(s) => h_flex()
                .pt_8()
                .pb_2()
                .gap_2()
                .child(Icon::new(IconName::Calendar).size(self.sh.u(15.)).text_color(accent(self.sh)))
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(self.t(s)))
                .into_any_element(),
            Block::Unknown(s) => div().pb_2().font_family("Cascadia Code").text_xs().text_color(c(theme::FG_DIM)).child(self.t(s)).into_any_element(),
        }
    }

    fn row(&self, table: u32, line: Span, pos: usize, next: Option<&Block>) -> AnyElement {
        let t = &self.sh.doc.tables[table as usize];
        let (n, header) = t.row(pos);
        let total: f32 = t.widths.iter().map(|w| w.max(4.)).sum::<f32>().max(1.);
        let line_text = self.t(line);
        let src: &str = &line_text;
        let cs = cells(src, (0, src.len() as u32), t.pipes);
        let last = !matches!(next, Some(Block::Row { table: tt, .. }) if *tt == table);
        let first = n == 0;
        h_flex()
            .w_full()
            .items_stretch()
            .border_l_1()
            .border_r_1()
            .border_color(c(theme::BORDER))
            .when(first, |d| d.border_t_1().rounded_t(px(8.)))
            .when(last, |d| d.border_b_1().rounded_b(px(8.)).mb_4())
            .when(header, |d| d.bg(c(theme::BG_ELEVATED)).border_b_1().font_weight(FontWeight::SEMIBOLD))
            .when(!header && n % 2 == 0, |d| d.bg(c(theme::BG_ELEVATED).opacity(0.45)))
            .children((0..t.cols).map(|k| {
                let w = t.widths.get(k).copied().unwrap_or(1.).max(4.) / total;
                let cell = cs.get(k).map(|s| text(src, *s)).unwrap_or("");
                let al = t.align.get(k).copied().unwrap_or(Align::Left);
                div()
                    .w(relative(w))
                    .min_w_0()
                    .px_3()
                    .py_1p5()
                    .when(k > 0, |d| d.border_l_1().border_color(c(theme::BORDER)))
                    .when(al == Align::Right, |d| d.text_right())
                    .when(al == Align::Center, |d| d.text_center())
                    .child(self.rich(ElementId::Name(format!("fx-td-{table}-{n}-{k}").into()), cell, c(if header { theme::FG_STRONG } else { theme::FG })))
            }))
            .into_any_element()
    }

    fn path(&self, p: &str) -> PathBuf {
        let p = PathBuf::from(p);
        if p.is_absolute() { p } else { self.sh.base.join(p) }
    }

    fn picture(&self, a: super::Span, id: usize) -> AnyElement {
        let arg_text = self.t(a);
        let src: &str = &arg_text;
        let a = args(src, (0, src.len() as u32));
        let light = arg(src, &a, "").unwrap_or("");
        let p = match arg(src, &a, "dark") {
            Some(d) if theme::palette().dark => d,
            _ => light,
        };
        let alt = arg(src, &a, "alt").unwrap_or("");
        let width = arg(src, &a, "width").map(|w| w.trim().to_string());
        let link = arg(src, &a, "link").map(String::from);
        if p.starts_with("http://") || p.starts_with("https://") {
            let url = p.to_string();
            return h_flex()
                .id(("fx-img-web", id))
                .gap_2()
                .px_3()
                .py_2()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::BORDER))
                .cursor_pointer()
                .child(Icon::new(IconName::Image).size(theme::u(15.)).text_color(c(theme::FG_MUTED)))
                .child(div().min_w_0().truncate().text_color(c(theme::FG_MUTED)).child(if alt.is_empty() { url.clone() } else { alt.to_string() }))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element();
        }
        let share = width.as_deref().and_then(|w| w.strip_suffix('%')).and_then(|w| w.parse::<f32>().ok());
        let pixels = width.as_deref().filter(|w| !w.ends_with('%')).and_then(|w| w.trim_end_matches("px").parse::<f32>().ok());
        let mut im = img(self.path(p)).max_w_full().rounded(px(6.)).object_fit(ObjectFit::Contain);
        if share.is_some() {
            im = im.w_full();
        } else if let Some(px_) = pixels {
            im = im.w(self.sh.u(px_));
        }
        let base = self.sh.base.clone();
        div()
            .id(("fx-img", id))
            .flex()
            .justify_center()
            .when_some(share, |d, f| d.w(relative(f / 100.)))
            .child(im)
            .when_some(link, |d, l| {
                d.cursor_pointer().on_click(move |_, _, cx| {
                    if l.starts_with("http") {
                        cx.open_url(&l)
                    } else {
                        cx.open_with_system(&base.join(&l))
                    }
                })
            })
            .into_any_element()
    }

    fn image(&self, a: super::Span, bi: usize) -> AnyElement {
        let arg_text = self.t(a);
        let src: &str = &arg_text;
        let args_ = args(src, (0, src.len() as u32));
        let caption = super::caption(src, &args_, self.sh.want);
        let right = arg(src, &args_, "align") == Some("right");
        v_flex()
            .pb_4()
            .gap_1p5()
            .when(right, |v| v.items_end())
            .when(!right, |v| v.items_center())
            .child(self.picture(a, bi))
            .when_some(caption, |v, cap| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(self.rich(ElementId::Name(format!("fx-cap-{bi}").into()), cap, c(theme::FG_DIM)))))
            .into_any_element()
    }

    fn video(&self, a: super::Span, bi: usize) -> AnyElement {
        let arg_text = self.t(a);
        let src: &str = &arg_text;
        let a = args(src, (0, src.len() as u32));
        let p = arg(src, &a, "").unwrap_or("").to_string();
        let alt = arg(src, &a, "alt").unwrap_or("").to_string();
        let poster = arg(src, &a, "poster").map(|x| self.path(x));
        let target = if p.starts_with("http") { None } else { Some(self.path(&p)) };
        let name = if alt.is_empty() { p.rsplit(['/', '\\']).next().unwrap_or(&p).to_string() } else { alt };
        let url = p.clone();
        div()
            .pb_4()
            .child(
                div()
                    .id(("fx-video", bi))
                    .relative()
                    .w_full()
                    .h(theme::u(280.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_EDITOR))
                    .cursor_pointer()
                    .when_some(poster, |d, pp| d.child(img(pp).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                    .child(
                        v_flex()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(div().size(theme::u(56.)).rounded_full().bg(c(theme::ACCENT)).flex().items_center().justify_center().child(Icon::new(IconName::Play).size(theme::u(24.)).text_color(c(theme::ON_ACCENT))))
                            .child(div().px_2().rounded(px(4.)).bg(c(theme::BG_EDITOR).opacity(0.8)).text_sm().text_color(c(theme::FG_STRONG)).child(name)),
                    )
                    .on_click(move |_, _, cx| match &target {
                        Some(t) => cx.open_with_system(t),
                        None => cx.open_url(&url),
                    }),
            )
            .into_any_element()
    }

    fn flow(&self, f: &flow::Flow, bi: usize) -> AnyElement {
        let acc = accent(self.sh);
        let (w, h) = (f.size.0 + 8., f.size.1 + 8.);
        let nodes = f.nodes.clone();
        let edges = f.edges.clone();
        let right = f.right;
        let line_col = c(theme::FG_DIM);
        let back_col = c(theme::FG_DIM).opacity(0.7);
        let max_x = f.size.0;
        let diamonds: Vec<((f32, f32, f32, f32), Hsla, Hsla)> = f
            .nodes
            .iter()
            .filter(|n| n.shape == flow::Shape::Diamond)
            .map(|n| (n.rect, n.fill.map(|h| rgb(h).into()).unwrap_or(c(theme::BG_ELEVATED)), n.border.map(|h| rgb(h).into()).unwrap_or(acc)))
            .collect();
        let canvas_el = canvas(|_, _, _| {}, move |b, _, window, _| {
            let o = b.origin + point(px(4.), px(4.));
            let at = |x: f32, y: f32| o + point(px(x), px(y));
            for ((x, y, w, h), fill, border) in &diamonds {
                let pts = [(x + w / 2., *y), (x + w, y + h / 2.), (x + w / 2., y + h), (*x, y + h / 2.)];
                let mut pb = PathBuilder::fill();
                pb.move_to(at(pts[0].0, pts[0].1));
                for p in &pts[1..] {
                    pb.line_to(at(p.0, p.1));
                }
                pb.close();
                if let Ok(p) = pb.build() {
                    window.paint_path(p, *fill);
                }
                let mut st = PathBuilder::stroke(px(1.5));
                st.move_to(at(pts[0].0, pts[0].1));
                for p in pts[1..].iter().chain(std::iter::once(&pts[0])) {
                    st.line_to(at(p.0, p.1));
                }
                if let Ok(p) = st.build() {
                    window.paint_path(p, *border);
                }
            }
            for e in &edges {
                let (a, z) = (nodes[e.from].rect, nodes[e.to].rect);
                let mut pts: Vec<(f32, f32)> = Vec::new();
                if e.back {
                    let (ax, ay) = (a.0 + a.2, a.1 + a.3 / 2.);
                    let (zx, zy) = (z.0 + z.2, z.1 + z.3 / 2.);
                    let side = max_x + 24.;
                    pts.extend([(ax, ay), (side, ay), (side, zy), (zx, zy)]);
                } else if right {
                    let (ax, ay) = (a.0 + a.2, a.1 + a.3 / 2.);
                    let (zx, zy) = (z.0, z.1 + z.3 / 2.);
                    let mx = (ax + zx) / 2.;
                    pts.extend([(ax, ay), (mx, ay), (mx, zy), (zx, zy)]);
                } else {
                    let (ax, ay) = (a.0 + a.2 / 2., a.1 + a.3);
                    let (zx, zy) = (z.0 + z.2 / 2., z.1);
                    let my = (ay + zy) / 2.;
                    pts.extend([(ax, ay), (ax, my), (zx, my), (zx, zy)]);
                }
                let col = if e.back { back_col } else { line_col };
                let mut pb = PathBuilder::stroke(px(1.5));
                if e.dashed {
                    for w in pts.windows(2) {
                        let (a, z) = (w[0], w[1]);
                        let len = ((z.0 - a.0).powi(2) + (z.1 - a.1).powi(2)).sqrt();
                        let mut d = 0.;
                        while d < len {
                            let e2 = (d + 6.).min(len);
                            let p = |t: f32| (a.0 + (z.0 - a.0) * t / len.max(0.001), a.1 + (z.1 - a.1) * t / len.max(0.001));
                            let (p1, p2) = (p(d), p(e2));
                            pb.move_to(at(p1.0, p1.1));
                            pb.line_to(at(p2.0, p2.1));
                            d += 10.;
                        }
                    }
                } else {
                    pb.move_to(at(pts[0].0, pts[0].1));
                    for p in &pts[1..] {
                        pb.line_to(at(p.0, p.1));
                    }
                }
                if let Ok(p) = pb.build() {
                    window.paint_path(p, col);
                }
                let (p1, p2) = (pts[pts.len() - 2], pts[pts.len() - 1]);
                let (dx, dy) = (p2.0 - p1.0, p2.1 - p1.1);
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let (ux, uy) = (dx / len, dy / len);
                let s = 7.;
                let mut tri = PathBuilder::fill();
                tri.move_to(at(p2.0, p2.1));
                tri.line_to(at(p2.0 - ux * s - uy * s * 0.6, p2.1 - uy * s + ux * s * 0.6));
                tri.line_to(at(p2.0 - ux * s + uy * s * 0.6, p2.1 - uy * s - ux * s * 0.6));
                tri.close();
                if let Ok(p) = tri.build() {
                    window.paint_path(p, col);
                }
            }
        })
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let mut stage = div().relative().w(px(w + 30.)).h(px(h)).flex_none().child(canvas_el);
        for (k, n) in f.nodes.iter().enumerate() {
            let (x, y, nw, nh) = n.rect;
            let fill: Hsla = n.fill.map(|h| rgb(h).into()).unwrap_or(c(theme::BG_ELEVATED));
            let border: Hsla = n.border.map(|h| rgb(h).into()).unwrap_or(if n.shape == flow::Shape::Diamond { acc } else { c(theme::BORDER_STRONG) });
            let textc: Hsla = n.text.map(|h| rgb(h).into()).unwrap_or(c(theme::FG_STRONG));
            let radius = match n.shape {
                flow::Shape::Box => 6.,
                flow::Shape::Round => 12.,
                flow::Shape::Pill | flow::Shape::Circle => nh / 2.,
                flow::Shape::Diamond => 4.,
            };
            stage = stage.child(
                div()
                    .id(("fx-node", bi * 1000 + k))
                    .absolute()
                    .left(px(x + 4.))
                    .top(px(y + 4.))
                    .w(px(nw))
                    .h(px(nh))
                    .when(n.shape != flow::Shape::Diamond, |d| d.rounded(px(radius)).border_1().border_color(border).bg(fill))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_2()
                    .text_size(px(13.))
                    .text_color(textc)
                    .child(n.label.clone()),
            );
        }
        for e in &f.edges {
            if e.label.is_empty() {
                continue;
            }
            let (a, z) = (f.nodes[e.from].rect, f.nodes[e.to].rect);
            let (x, y) = if e.back {
                (f.size.0 + 10., (a.1 + z.1) / 2. + a.3 / 2. - 8.)
            } else if f.right {
                ((a.0 + a.2 + z.0) / 2. - 12., (a.1 + z.1) / 2. + a.3 / 2. - 18.)
            } else {
                ((a.0 + a.2 / 2. + z.0 + z.2 / 2.) / 2. + 6., (a.1 + a.3 + z.1) / 2. - 9.)
            };
            stage = stage.child(div().absolute().left(px(x + 4.)).top(px(y + 4.)).px_1().rounded(px(3.)).bg(c(theme::BG_EDITOR)).text_size(px(11.5)).text_color(c(theme::FG_MUTED)).child(e.label.clone()));
        }
        div()
            .id(("fx-flow", bi))
            .mb_4()
            .p_4()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER))
            .overflow_hidden()
            .child(h_flex().justify_center().min_w_full().child(stage))
            .into_any_element()
    }

    fn boxed(&self, kind: Kind, title: Span, inner: u32, bi: usize, cx: &mut App) -> AnyElement {
        let blocks = &self.sh.doc.inner[inner as usize];
        let title_text = self.t(title);
        let title = title_text.trim();
        let render_inner = |blocks: &[Block], cx: &mut App| -> Vec<AnyElement> { blocks.iter().enumerate().map(|(k, b)| self.block(b, bi * 1000 + k + 1, k, blocks.get(k + 1), cx)).collect() };
        match kind {
            Kind::Columns => {
                let mut cols: Vec<Vec<Block>> = vec![Vec::new()];
                for b in blocks {
                    if matches!(b, Block::ColBreak) {
                        cols.push(Vec::new());
                    } else {
                        cols.last_mut().unwrap().push(*b);
                    }
                }
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .items_start()
                    .gap_6()
                    .pb_2()
                    .children(cols.iter().map(|col| v_flex().flex_1().min_w(theme::u(240.)).children(render_inner(col, cx))))
                    .into_any_element()
            }
            Kind::Gallery => h_flex()
                .w_full()
                .flex_wrap()
                .justify_center()
                .gap_2()
                .pb_4()
                .children(blocks.iter().enumerate().filter_map(|(k, b)| match b {
                    Block::Image(a) => {
                        let arg_text = self.t(*a);
                        let src: &str = &arg_text;
                        let cap = super::caption(src, &args(src, (0, src.len() as u32)), self.sh.want).map(str::to_string);
                        Some(
                            v_flex()
                                .gap_1()
                                .child(self.picture(*a, bi * 1000 + k))
                                .when_some(cap, |v, cap| v.child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(cap)))
                                .into_any_element(),
                        )
                    }
                    _ => None,
                }))
                .into_any_element(),
            Kind::Details => {
                let open = self.sh.open.borrow().contains(&inner);
                let notify = self.notify.clone();
                let toggles = self.sh.open.clone();
                let key = inner;
                v_flex()
                    .mb_3()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .child(
                        h_flex()
                            .id(("fx-details", bi))
                            .px_3()
                            .py_2()
                            .gap_2()
                            .cursor_pointer()
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
                            .child(div().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(if title.is_empty() { crate::i18n::tr("Details").to_string() } else { title.to_string() }))
                            .on_click(move |_, _, cx| {
                                {
                                    let mut s = toggles.borrow_mut();
                                    if !s.remove(&key) {
                                        s.insert(key);
                                    }
                                }
                                notify(cx);
                            }),
                    )
                    .when(open, |v| v.child(v_flex().px_4().pt_2().pb_1().border_t_1().border_color(c(theme::BORDER)).children(render_inner(blocks, cx))))
                    .into_any_element()
            }
            _ => {
                let (tint, icon, word) = match kind {
                    Kind::Tip => (theme::GREEN, IconName::Lightbulb, crate::i18n::tr("Tip")),
                    Kind::Warn => (theme::WARN, IconName::TriangleAlert, crate::i18n::tr("Warning")),
                    Kind::Danger => (theme::DANGER, IconName::CircleAlert, crate::i18n::tr("Danger")),
                    _ => (theme::BLUE, IconName::Info, crate::i18n::tr("Note")),
                };
                v_flex()
                    .mb_4()
                    .px_4()
                    .pt_3()
                    .pb_1()
                    .rounded(px(8.))
                    .border_l_4()
                    .border_color(c(tint))
                    .bg(c(tint).opacity(0.08))
                    .child(
                        h_flex()
                            .pb_2()
                            .gap_2()
                            .child(Icon::new(icon).size(theme::u(16.)).text_color(c(tint)))
                            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(tint)).child(if title.is_empty() { word.to_string() } else { title.to_string() })),
                    )
                    .children(render_inner(blocks, cx))
                    .into_any_element()
            }
        }
    }

    fn toc(&self, bi: usize) -> AnyElement {
        let mut v = v_flex().mb_4().px_4().py_3().gap_1().rounded(px(8.)).border_1().border_color(c(theme::BORDER)).child(div().pb_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(crate::i18n::tr("Contents")));
        let wanted: Vec<u32> = self.sh.doc.headings.iter().copied().filter(|&h| self.sh.shown(h as usize) && matches!(self.sh.doc.blocks[h as usize], Block::Heading(l, _) if l <= 3)).collect();
        let more = wanted.len().saturating_sub(TOC_MAX);
        for (k, &h) in wanted.iter().take(TOC_MAX).enumerate() {
            let Block::Heading(l, s) = self.sh.doc.blocks[h as usize] else { continue };
            let pos = self.sh.groups.partition_point(|g| g.0 <= h).saturating_sub(1);
            let list = self.list.clone();
            v = v.child(
                div()
                    .id(("fx-toc", bi * 10_000 + k))
                    .pl(theme::u((l.saturating_sub(1)) as f32 * 14.))
                    .cursor_pointer()
                    .text_color(c(if l == 1 { theme::FG_STRONG } else { theme::FG }))
                    .hover(|s| s.text_color(accent(self.sh)))
                    .child(inline(&self.t(s)).text)
                    .on_click(move |_, _, _| list.scroll_to_reveal_item(pos)),
            );
        }
        if more > 0 {
            v = v.child(div().pt_1().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("and {n} more headings"; n => more)));
        }
        v.into_any_element()
    }
}

fn merge(mut hl: Vec<(std::ops::Range<usize>, HighlightStyle)>) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    if hl.len() < 2 {
        return hl;
    }
    let mut cuts: Vec<usize> = hl.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut style: Option<HighlightStyle> = None;
        for (r, h) in &hl {
            if r.start <= a && b <= r.end {
                style = Some(match style {
                    None => *h,
                    Some(s) => s.highlight(*h),
                });
            }
        }
        if let Some(s) = style {
            out.push((a..b, s));
        }
    }
    hl.clear();
    out
}

fn render_global_ink_layer(
    ink: Rc<RefCell<InkState>>,
    last_bounds: Rc<Cell<Option<RectBox>>>,
    notify: Rc<dyn Fn(&mut App)>,
) -> AnyElement {
    let ink_draw = ink.clone();
    let ink_ev = ink;
    let draw_on = ink_ev.borrow().draw;
    canvas(
        move |bounds, window, _| draw_on.then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal)),
        move |bounds, hitbox, window, _| {
            last_bounds.set(Some([
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ]));
            let origin = bounds.origin;
            {
                let st = ink_draw.borrow();
                for s in st.strokes.iter().chain(st.active.iter()) {
                    if s.pts.is_empty() {
                        continue;
                    }
                    let col: Hsla = rgb(s.color).into();
                    let col = col.opacity(s.alpha.clamp(0.05, 1.0));
                    let width = s.width.max(1.0);
                    if s.pts.len() == 1 {
                        let (x, y) = s.pts[0];
                        let p = origin + point(px(x), px(y));
                        let mut pb = PathBuilder::stroke(px(width.max(3.0)));
                        pb.move_to(p);
                        pb.line_to(p + point(px(0.5), px(0.5)));
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, col);
                        }
                        continue;
                    }
                    let mut pb = PathBuilder::stroke(px(width));
                    let (x0, y0) = s.pts[0];
                    pb.move_to(origin + point(px(x0), px(y0)));
                    for &(x, y) in &s.pts[1..] {
                        pb.line_to(origin + point(px(x), px(y)));
                    }
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, col);
                    }
                }
            }
            let Some(hitbox) = hitbox else { return };
            let (ink_down, notify_down) = (ink_ev.clone(), notify.clone());
            let hb_down = hitbox.clone();
            window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !hb_down.is_hovered(window) {
                    return;
                }
                if ev.button == MouseButton::Right {
                    if ink_down.borrow_mut().undo() {
                        cx.stop_propagation();
                        notify_down(cx);
                    }
                    return;
                }
                if ev.button != MouseButton::Left {
                    return;
                }
                let pt = (f32::from(ev.position.x - bounds.origin.x), f32::from(ev.position.y - bounds.origin.y));
                let mut st = ink_down.borrow_mut();
                let (color, width, alpha) = (st.color, st.tool.width(), st.tool.alpha());
                st.active = Some(Stroke { color, width, alpha, pts: vec![pt] });
                drop(st);
                cx.stop_propagation();
                notify_down(cx);
            });
            let (ink_move, notify_move) = (ink_ev.clone(), notify.clone());
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if ev.pressed_button != Some(MouseButton::Left) {
                    let mut st = ink_move.borrow_mut();
                    if let Some(s) = st.active.take() {
                        st.push_stroke(s);
                        drop(st);
                        notify_move(cx);
                    }
                    return;
                }
                let mut st = ink_move.borrow_mut();
                let Some(active) = st.active.as_mut() else { return };
                let pt = (f32::from(ev.position.x - bounds.origin.x), f32::from(ev.position.y - bounds.origin.y));
                if active.pts.last().is_none_or(|&(lx, ly)| (lx - pt.0).hypot(ly - pt.1) >= 1.5) {
                    active.pts.push(pt);
                    drop(st);
                    notify_move(cx);
                }
            });
            let (ink_up, notify_up) = (ink_ev.clone(), notify.clone());
            window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || ev.button != MouseButton::Left {
                    return;
                }
                let mut st = ink_up.borrow_mut();
                if let Some(s) = st.active.take() {
                    st.push_stroke(s);
                    drop(st);
                    notify_up(cx);
                }
            });
            let (ink_key, notify_key) = (ink_ev, notify);
            window.on_key_event::<KeyDownEvent>(move |ev, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let mut st = ink_key.borrow_mut();
                if !st.draw {
                    return;
                }
                let m = &ev.keystroke.modifiers;
                let plain = !m.control && !m.alt && !m.platform;
                let changed = match ev.keystroke.key.as_str() {
                    "escape" if plain => {
                        st.draw = false;
                        st.active = None;
                        true
                    }
                    "1" if plain => {
                        st.color = DRAW_COLORS[0];
                        true
                    }
                    "2" if plain => {
                        st.color = DRAW_COLORS[1];
                        true
                    }
                    "3" if plain => {
                        st.color = DRAW_COLORS[2];
                        true
                    }
                    "4" if plain => {
                        st.color = DRAW_COLORS[3];
                        true
                    }
                    "b" if plain => {
                        st.tool = st.tool.next();
                        true
                    }
                    "z" if m.control && !m.alt && !m.platform && !m.shift => st.undo(),
                    "y" if m.control && !m.alt && !m.platform && !m.shift => st.redo(),
                    "z" if m.control && !m.alt && !m.platform && m.shift => st.redo(),
                    _ => false,
                };
                drop(st);
                if changed {
                    cx.stop_propagation();
                    notify_key(cx);
                }
            });
        },
    )
    .absolute()
    .inset_0()
    .when(draw_on, |d| d.cursor(CursorStyle::Crosshair))
    .into_any_element()
}

pub fn is_flux(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("flux"))
}

pub fn drawable_kind(path: &std::path::Path) -> Option<bool> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "flux" => Some(false),
        "md" | "markdown" => Some(true),
        _ => None,
    }
}

pub fn shows_md_export(path: &std::path::Path) -> bool {
    is_flux(path)
}

pub fn ui_lang() -> &'static str {
    if crate::i18n::is_id() { "id" } else { "en" }
}

pub(crate) fn annotated_png_path(doc_path: &std::path::Path) -> PathBuf {
    let stem = doc_path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).unwrap_or("preview");
    let dir = doc_path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| std::path::Path::new("."));
    dir.join(format!("{stem}-annotated.png"))
}

#[cfg(windows)]
fn capture_window_crop(hwnd: isize, rect: RectBox, scale: f32) -> Option<image::RgbaImage> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_ppm: i32,
        y_ppm: i32,
        clr_used: u32,
        clr_important: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
        fn PrintWindow(hwnd: isize, hdc: isize, flags: u32) -> i32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn DeleteDC(hdc: isize) -> i32;
        fn CreateDIBSection(hdc: isize, bmi: *const BitmapInfoHeader, usage: u32, bits: *mut *mut u8, sec: isize, off: u32) -> isize;
        fn SelectObject(hdc: isize, obj: isize) -> isize;
        fn DeleteObject(obj: isize) -> i32;
        fn GdiFlush() -> i32;
    }
    if hwnd == 0 {
        return None;
    }
    unsafe {
        let mut rc = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        if GetClientRect(hwnd, &mut rc) == 0 {
            return None;
        }
        let cw = rc.right - rc.left;
        let ch = rc.bottom - rc.top;
        if cw <= 0 || ch <= 0 {
            return None;
        }
        let hdc_win = GetDC(hwnd);
        if hdc_win == 0 {
            return None;
        }
        let hdc_mem = CreateCompatibleDC(hdc_win);
        if hdc_mem == 0 {
            ReleaseDC(hwnd, hdc_win);
            return None;
        }
        let bmi = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: cw,
            height: -ch,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        };
        let mut bits: *mut u8 = std::ptr::null_mut();
        let hbm = CreateDIBSection(hdc_mem, &bmi, 0, &mut bits, 0, 0);
        if hbm == 0 || bits.is_null() {
            if hbm != 0 {
                DeleteObject(hbm);
            }
            DeleteDC(hdc_mem);
            ReleaseDC(hwnd, hdc_win);
            return None;
        }
        let old = SelectObject(hdc_mem, hbm);
        // PW_RENDERFULLCONTENT (2) captures DirectComposition swap-chain surfaces.
        let ok = PrintWindow(hwnd, hdc_mem, 2) != 0;
        GdiFlush();

        let out = if ok {
            let rx = (rect[0] * scale).round().clamp(0.0, cw as f32) as usize;
            let ry = (rect[1] * scale).round().clamp(0.0, ch as f32) as usize;
            let rw = (rect[2] * scale).round().max(1.0) as usize;
            let rh = (rect[3] * scale).round().max(1.0) as usize;
            let w = rw.min((cw as usize).saturating_sub(rx));
            let h = rh.min((ch as usize).saturating_sub(ry));
            if w > 0 && h > 0 {
                let src = std::slice::from_raw_parts(bits, cw as usize * ch as usize * 4);
                let mut rgba = vec![0u8; w * h * 4];
                let mut any_non_zero = false;
                for y in 0..h {
                    let s_row = ((ry + y) * cw as usize + rx) * 4;
                    let d_row = y * w * 4;
                    for x in 0..w {
                        let s = s_row + x * 4;
                        let d = d_row + x * 4;
                        let (b, g, r) = (src[s], src[s + 1], src[s + 2]);
                        any_non_zero |= (b | g | r) != 0;
                        rgba[d] = r;
                        rgba[d + 1] = g;
                        rgba[d + 2] = b;
                        rgba[d + 3] = 255;
                    }
                }
                if any_non_zero {
                    image::RgbaImage::from_raw(w as u32, h as u32, rgba)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let _ = c_void::from; // keep import tidy if unused
        SelectObject(hdc_mem, old);
        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_win);
        out
    }
}

fn hex_rgba(hex: u32, alpha: u8) -> [u8; 4] {
    [((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8, alpha]
}

fn dist_to_segment_sq(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-6 {
        return (px - ax).powi(2) + (py - ay).powi(2);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let (qx, qy) = (ax + dx * t, ay + dy * t);
    (px - qx).powi(2) + (py - qy).powi(2)
}

fn blend_pixel(buf: &mut image::RgbaImage, x: u32, y: u32, src: [u8; 3], cov: f32) {
    if cov <= 0.0 {
        return;
    }
    let p = buf.get_pixel_mut(x, y);
    let a = cov.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    p[0] = (p[0] as f32 * inv + src[0] as f32 * a).round() as u8;
    p[1] = (p[1] as f32 * inv + src[1] as f32 * a).round() as u8;
    p[2] = (p[2] as f32 * inv + src[2] as f32 * a).round() as u8;
    p[3] = 255;
}

fn rasterize_ink_fallback(width: u32, height: u32, bg_hex: u32, scroll_y: f32, strokes: &[Stroke]) -> image::RgbaImage {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut visible = false;
    for s in strokes {
        for &(x, y) in &s.pts {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            let vy = y - scroll_y;
            if x >= 0.0 && x <= width as f32 && vy >= 0.0 && vy <= height as f32 {
                visible = true;
            }
        }
    }
    let (ox, oy, w, h) = if visible || strokes.is_empty() {
        (0.0, scroll_y, width.clamp(1, 4096), height.clamp(1, 4096))
    } else {
        let pad = 32.0;
        let sw = (max_x - min_x + pad * 2.0).ceil().clamp(320.0, 4096.0) as u32;
        let sh = (max_y - min_y + pad * 2.0).ceil().clamp(200.0, 4096.0) as u32;
        (min_x - pad, min_y - pad, sw, sh)
    };
    let bg = image::Rgba(hex_rgba(bg_hex, 255));
    let mut img = image::RgbaImage::from_pixel(w, h, bg);
    for s in strokes {
        if s.pts.is_empty() {
            continue;
        }
        let col = hex_rgba(s.color, 255);
        let rgb = [col[0], col[1], col[2]];
        let radius = (s.width.max(1.0)) * 0.5;
        let alpha = s.alpha.clamp(0.05, 1.0);
        let draw_seg = |img: &mut image::RgbaImage, a: (f32, f32), b: (f32, f32)| {
            let x0 = ((a.0.min(b.0) - ox - radius - 1.5).floor() as i32).max(0) as u32;
            let y0 = ((a.1.min(b.1) - oy - radius - 1.5).floor() as i32).max(0) as u32;
            let x1 = ((a.0.max(b.0) - ox + radius + 1.5).ceil() as i32).min(w as i32 - 1).max(0) as u32;
            let y1 = ((a.1.max(b.1) - oy + radius + 1.5).ceil() as i32).min(h as i32 - 1).max(0) as u32;
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let d = dist_to_segment_sq(px as f32 + 0.5, py as f32 + 0.5, a.0 - ox, a.1 - oy, b.0 - ox, b.1 - oy).sqrt();
                    let cov = (radius + 0.75 - d).clamp(0.0, 1.0) * alpha;
                    if cov > 0.0 {
                        blend_pixel(img, px, py, rgb, cov);
                    }
                }
            }
        };
        if s.pts.len() == 1 {
            draw_seg(&mut img, s.pts[0], s.pts[0]);
        } else {
            for pair in s.pts.windows(2) {
                draw_seg(&mut img, pair[0], pair[1]);
            }
        }
    }
    img
}

fn encode_png(img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

impl crate::workbench::Workbench {
    pub(crate) fn title_ink_reserve(&self) -> f32 {
        let st = self.ink.borrow();
        let draw_on = st.draw;
        let has_strokes = !st.strokes.is_empty();
        let has_redo = !st.redo.is_empty();
        drop(st);
        let sc = theme::scale();
        let mut w = 36. * sc;
        if draw_on {
            w += 156. * sc;
        }
        if has_strokes {
            w += 120. * sc;
        }
        if has_redo {
            w += 24. * sc;
        }
        w
    }

    pub(crate) fn render_title_ink_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::i18n::tr;
        let (draw_on, cur_color, cur_tool, stroke_count, has_redo, copied_ok) = {
            let st = self.ink.borrow();
            (st.draw, st.color, st.tool, st.strokes.len(), !st.redo.is_empty(), st.copied_ok)
        };
        let has_strokes = stroke_count > 0;
        let tool_tip: SharedString = crate::trf!("Brush: {tool} (B, right-click to undo)"; tool => tr(cur_tool.label())).into();
        let out_name = self
            .ink_export_path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "fluxcode-annotated.png".into());

        let toggle_btn = h_flex()
            .id("tb-draw-toggle")
            .relative()
            .w(theme::u(36.))
            .h_full()
            .flex_none()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .occlude()
            .when(draw_on, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::ACCENT_TEXT)))
            .when(!draw_on, |d| {
                d.text_color(c(theme::FG_MUTED))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            })
            .active(|s| s.bg(c(theme::BG_ELEVATED)))
            .when(draw_on, |d| {
                d.child(div().absolute().bottom_0().left(px(6.)).right(px(6.)).h(px(2.)).rounded_t_full().bg(c(theme::ACCENT)))
            })
            .when(!draw_on && has_strokes, |d| {
                d.child(div().absolute().top(px(6.)).right(px(7.)).size(px(5.)).rounded_full().bg(c(theme::ACCENT)))
            })
            .child(Icon::new(IconName::Pencil).size(theme::u(14.)))
            .tooltip(move |w, cx| {
                gpui_kit::component::tooltip::Tooltip::new(if draw_on {
                    tr("Exit annotation mode (Esc)")
                } else {
                    tr("Annotate over workspace")
                })
                .build(w, cx)
            })
            .on_click(cx.listener(|this, _, _, cx| this.ink_toggle_draw(cx)));

        h_flex()
            .flex_none()
            .h_full()
            .items_center()
            .occlude()
            .when(draw_on || has_strokes || has_redo, |h| {
                h.child(
                    h_flex()
                        .flex_none()
                        .mr_1()
                        .px_1p5()
                        .h(theme::u(24.))
                        .gap_1()
                        .items_center()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .bg(c(theme::BG_SIDE))
                        .when(draw_on, |bar| {
                            bar.children(DRAW_COLORS.into_iter().enumerate().map(|(ci, col)| {
                                let picked = cur_color == col;
                                let tip = DRAW_COLOR_NAMES[ci];
                                div()
                                    .id(("tb-ink-col", ci))
                                    .size(theme::u(14.))
                                    .rounded_full()
                                    .cursor_pointer()
                                    .bg(rgb(col))
                                    .border_2()
                                    .border_color(if picked { c(theme::FG_STRONG) } else { gpui_kit::transparent_black() })
                                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tr(tip)).build(w, cx))
                                    .on_click(cx.listener(move |this, _, _, cx| this.ink_set_color(col, cx)))
                            }))
                            .child(
                                h_flex()
                                    .id("tb-ink-tool")
                                    .h(theme::u(20.))
                                    .px_1p5()
                                    .gap_1()
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(c(theme::BORDER))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(c(theme::FG))
                                    .hover(|s| s.bg(c(theme::BG_HOVER)).border_color(c(theme::BORDER_STRONG)))
                                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tool_tip.clone()).build(w, cx))
                                    .child(Icon::new(IconName::PencilLine).size(theme::u(11.)).text_color(c(theme::ACCENT_TEXT)))
                                    .child(tr(cur_tool.label()))
                                    .on_click(cx.listener(|this, _, _, cx| this.ink_cycle_tool(cx))),
                            )
                        })
                        .when(has_strokes, |bar| {
                            bar.child(div().px_0p5().text_xs().text_color(c(theme::FG_DIM)).child(format!("{stroke_count}")))
                                .child(
                                    ui::tool_button("tb-ink-undo", IconName::Undo, false)
                                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Undo last stroke")).build(w, cx))
                                        .on_click(cx.listener(|this, _, _, cx| this.ink_undo(cx))),
                                )
                        })
                        .when(has_redo, |bar| {
                            bar.child(
                                ui::tool_button("tb-ink-redo", IconName::Redo, false)
                                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Redo last stroke")).build(w, cx))
                                    .on_click(cx.listener(|this, _, _, cx| this.ink_redo(cx))),
                            )
                        })
                        .when(has_strokes, |bar| {
                            bar.child(
                                ui::tool_button("tb-ink-clear", IconName::Delete, false)
                                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Clear annotations")).build(w, cx))
                                    .on_click(cx.listener(|this, _, _, cx| this.ink_clear(cx))),
                            )
                            .child(
                                ui::tool_button("tb-ink-copy", if copied_ok { IconName::Check } else { IconName::Copy }, copied_ok)
                                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Copy annotated PNG to clipboard")).build(w, cx))
                                    .on_click(cx.listener(|this, _, w, cx| this.ink_copy_png(w, cx))),
                            )
                            .child(
                                ui::tool_button("tb-ink-export", IconName::ImageDown, false)
                                    .tooltip(move |w, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(crate::trf!("Save annotated PNG ({name})"; name => out_name)).build(w, cx)
                                    })
                                    .on_click(cx.listener(|this, _, w, cx| this.ink_export_png(w, cx))),
                            )
                        }),
                )
            })
            .child(toggle_btn)
            .into_any_element()
    }

    pub(crate) fn render_global_ink_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let active = {
            let st = self.ink.borrow();
            st.draw || !st.strokes.is_empty() || st.active.is_some()
        };
        if !active {
            return None;
        }
        let weak = cx.entity().downgrade();
        let notify: Rc<dyn Fn(&mut App)> = Rc::new(move |cx| {
            _ = weak.update(cx, |_, cx| cx.notify());
        });
        Some(render_global_ink_layer(self.ink.clone(), self.ink_bounds.clone(), notify))
    }

    fn ink_export_path(&self) -> PathBuf {
        if let Some(p) = self.active_doc_path() {
            annotated_png_path(&p)
        } else if let Some(root) = &self.root {
            root.join("fluxcode-annotated.png")
        } else {
            PathBuf::from("fluxcode-annotated.png")
        }
    }

    fn ink_toggle_draw(&mut self, cx: &mut Context<Self>) {
        let mut st = self.ink.borrow_mut();
        st.draw = !st.draw;
        st.active = None;
        drop(st);
        cx.notify();
    }

    fn ink_set_color(&mut self, color: u32, cx: &mut Context<Self>) {
        self.ink.borrow_mut().color = color;
        cx.notify();
    }

    fn ink_cycle_tool(&mut self, cx: &mut Context<Self>) {
        let next = {
            let mut st = self.ink.borrow_mut();
            st.tool = st.tool.next();
            st.tool
        };
        self.status = crate::i18n::tr(next.label()).into();
        cx.notify();
    }

    fn ink_undo(&mut self, cx: &mut Context<Self>) {
        self.ink.borrow_mut().undo();
        cx.notify();
    }

    fn ink_redo(&mut self, cx: &mut Context<Self>) {
        self.ink.borrow_mut().redo();
        cx.notify();
    }

    fn ink_clear(&mut self, cx: &mut Context<Self>) {
        self.ink.borrow_mut().clear();
        cx.notify();
    }

    fn ink_snapshot_png(&self, window: &Window) -> Result<(Vec<u8>, u32, u32), String> {
        let bounds = self.ink_bounds.get().unwrap_or([0.0, 34.0, 1280.0, 720.0]);
        let scale = window.scale_factor().max(1.0);
        let strokes = self.ink.borrow().strokes.clone();
        #[cfg(windows)]
        let captured = capture_window_crop(crate::wintitle::capture_main_hwnd(), bounds, scale);
        #[cfg(not(windows))]
        let captured: Option<image::RgbaImage> = None;
        let img = captured.unwrap_or_else(|| {
            rasterize_ink_fallback(
                bounds[2].round().max(1.0) as u32,
                bounds[3].round().max(1.0) as u32,
                theme::palette().editor,
                0.0,
                &strokes,
            )
        });
        let (w, h) = img.dimensions();
        let bytes = encode_png(&img)?;
        Ok((bytes, w, h))
    }

    fn ink_copy_png(&mut self, window: &Window, cx: &mut Context<Self>) {
        match self.ink_snapshot_png(window) {
            Ok((bytes, w, h)) => {
                cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(ImageFormat::Png, bytes)));
                let seq = {
                    let mut st = self.ink.borrow_mut();
                    st.copied_ok = true;
                    st.copied_seq = st.copied_seq.wrapping_add(1);
                    st.copied_seq
                };
                let ink = self.ink.clone();
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(1800)).await;
                    let mut st = ink.borrow_mut();
                    if st.copied_seq == seq {
                        st.copied_ok = false;
                        drop(st);
                        _ = this.update(cx, |_, cx| cx.notify());
                    }
                })
                .detach();
                self.status = crate::trf!("Copied annotated PNG ({w}x{h})"; w => w, h => h).into();
            }
            Err(e) => {
                self.status = crate::trf!("Couldn't export PNG: {e}"; e => e).into();
            }
        }
        cx.notify();
    }

    fn ink_export_png(&mut self, window: &Window, cx: &mut Context<Self>) {
        let out = self.ink_export_path();
        match self.ink_snapshot_png(window).and_then(|(bytes, _, _)| std::fs::write(&out, bytes).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.status = crate::trf!("Saved {name}"; name => out.display()).into();
            }
            Err(e) => {
                self.status = crate::trf!("Couldn't save {name}: {e}"; name => out.display(), e => e).into();
            }
        }
        cx.notify();
    }

    pub(crate) fn render_flux_bar(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        use crate::i18n::tr;
        let d = &self.docs[ix];
        let Some(v) = d.flux.as_ref() else { return div().into_any_element() };
        let source = d.flux_source;
        let zoom_label: SharedString = format!("{}%", (v.zoom() * 100.).round()).into();
        let tab = |id: &'static str, icon: IconName, label: &'static str, on: bool, want_source: bool, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .h(theme::u(20.))
                .gap_1()
                .px_2()
                .rounded(px(4.))
                .cursor_pointer()
                .text_xs()
                .when(on, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .when(!on, |d| d.text_color(c(theme::FG_MUTED)).hover(|s| s.text_color(c(theme::FG))))
                .child(Icon::new(icon).size(theme::u(12.)))
                .child(label)
                .on_click(cx.listener(move |this, _, w, cx| this.flux_show(ix, want_source, w, cx)))
        };
        let info: SharedString = if v.pending {
            crate::i18n::tr("Reading the rest…").into()
        } else {
            crate::trf!("{n} blocks · read in {ms} ms · text in {size}"; n => thousands(v.blocks()), ms => format!("{:.1}", v.parsed_ms), size => crate::viewer::human_size(v.kept_bytes() as u64)).into()
        };
        let md_name = d.path.with_extension("md").file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        h_flex()
            .flex_none()
            .gap_1()
            .when(!source, |h| {
                h.child(
                    h_flex()
                        .gap_0p5()
                        .child(
                            ui::tool_button("fx-zoom-out", IconName::ZoomOut, false)
                                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Zoom out (Ctrl+-, Ctrl+scroll)")).build(w, cx))
                                .on_click(cx.listener(move |this, _, _, cx| this.flux_zoom_step(ix, -1, cx))),
                        )
                        .child(
                            div()
                                .id("fx-zoom-reset")
                                .h(theme::u(22.))
                                .min_w(theme::u(42.))
                                .px_1p5()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(c(theme::FG))
                                .hover(|s| s.bg(c(theme::BG_HOVER)))
                                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Reset zoom to 100% (Ctrl+0)")).build(w, cx))
                                .child(zoom_label)
                                .on_click(cx.listener(move |this, _, _, cx| this.flux_zoom_step(ix, 0, cx))),
                        )
                        .child(
                            ui::tool_button("fx-zoom-in", IconName::ZoomIn, false)
                                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Zoom in (Ctrl+=, Ctrl+scroll)")).build(w, cx))
                                .on_click(cx.listener(move |this, _, _, cx| this.flux_zoom_step(ix, 1, cx))),
                        ),
                )
            })
            .child(
                h_flex()
                    .id("fx-tabs")
                    .p(px(1.))
                    .gap(px(1.))
                    .rounded(px(5.))
                    .bg(c(theme::BG_SIDE))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(info.clone()).build(w, cx))
                    .child(tab("fx-tab-view", IconName::Eye, tr("Preview"), !source, false, cx))
                    .child(tab("fx-tab-src", IconName::FileCode, tr("Source"), source, true, cx)),
            )
            .when(shows_md_export(&d.path), |h| {
                h.child(
                    div()
                        .id("fx-to-md")
                        .size(theme::u(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_color(c(theme::FG_MUTED))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                        .tooltip(move |w, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(
                                crate::trf!("Save as Markdown: writes {name} next to it, as GitHub Markdown (flowcharts become Mermaid)"; name => md_name),
                            )
                            .build(w, cx)
                        })
                        .child(Icon::new(IconName::Download).size(theme::u(14.)))
                        .on_click(cx.listener(move |this, _, _, cx| this.flux_save_markdown(ix, cx))),
                )
            })
            .into_any_element()
    }

    pub(crate) fn flux_zoom_step(&mut self, ix: usize, dir: i32, cx: &mut Context<Self>) {
        let Some(v) = self.docs.get(ix).and_then(|d| d.flux.as_ref()) else { return };
        v.step_zoom(dir);
        let pct = (v.zoom() * 100.).round() as i32;
        self.status = crate::trf!("Preview zoom: {n}%"; n => pct).into();
        cx.notify();
    }

    fn flux_show(&mut self, ix: usize, source: bool, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        if source && self.docs[ix].flux.as_ref().is_some_and(|v| v.pending) {
            self.status = crate::i18n::tr("Still reading the document; Source opens in a moment.").into();
            return cx.notify();
        }
        if source {
            self.flux_load_source(ix, window, cx);
        }
        if !source && self.docs[ix].flux_source {
            let mut text = self.docs[ix].text(cx);
            let d = &mut self.docs[ix];
            if crate::fluxdoc::view::drawable_kind(&d.path) == Some(true) {
                text = crate::fluxdoc::gfm::to_flux(&text);
            }
            if let Some(v) = d.flux.as_mut() {
                if v.source() != text {
                    v.reload(text, ui_lang());
                }
            }
        }
        self.docs[ix].flux_source = source;
        cx.notify();
    }

    pub(crate) fn flux_load_source(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let d = &mut self.docs[ix];
        if !d.flux_lazy {
            return;
        }
        d.flux_lazy = false;
        let text = d.flux.as_ref().map(|v| v.source()).unwrap_or_default();
        if let Some(ed) = self.ensure_doc_editor(ix, window, cx) {
            ed.update(cx, |e, cx| e.set_value(text, window, cx));
        }
    }

    fn flux_save_markdown(&mut self, ix: usize, cx: &mut Context<Self>) {
        let d = &self.docs[ix];
        let src = d.text(cx);
        let out = d.path.with_extension("md");
        let banner = super::md::banner(&d.name, &out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let md = super::md::to_markdown(&super::parse(&src), &src, "en", Some(&banner));
        self.status = match std::fs::write(&out, md) {
            Ok(()) => crate::trf!("Saved {name}"; name => out.display()).into(),
            Err(e) => crate::trf!("Couldn't save {name}: {e}"; name => out.display(), e => e).into(),
        };
        cx.notify();
    }
}

fn thousands(n: usize) -> String {
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{groups, prepare, Text, GROUP, PACK_FROM};
    use std::cell::RefCell;

    #[test]
    fn packed_text_reads_back_across_frames() {
        let mut src = String::new();
        let mut i = 0;
        while src.len() < PACK_FROM + 1000 {
            src.push_str(&format!("Baris {i} dengan teks é ü 日本 dan seterusnya.\n"));
            i += 1;
        }
        let p = prepare(src.clone());
        let Err((frames, len)) = p.text else { panic!("large enough to pack") };
        let packed: usize = frames.iter().map(|f| f.len()).sum();
        assert!(packed < src.len() / 3, "packed to {packed} of {}", src.len());
        let t = Text::Packed { frames, len, cache: RefCell::new(Vec::new()) };
        let starts: Vec<usize> = src.char_indices().map(|(b, _)| b).filter(|b| b % 49_999 < 30).collect();
        for &a in &starts {
            let mut b = (a + 20_000).min(src.len());
            while !src.is_char_boundary(b) {
                b -= 1;
            }
            assert_eq!(t.get((a as u32, b as u32)), &src[a..b]);
        }
        assert_eq!(t.full(), src);
    }

    #[test]
    fn list_entries_start_at_headings_and_stay_small() {
        let mut src = String::from("# One\n");
        for i in 0..60 {
            src.push_str(&format!("- item {i}\n"));
        }
        src.push_str("## Two\n@image a.png\nText.\n");
        let d = super::super::parse(&src);
        let g = groups(&d, 1);
        assert_eq!(g[0], (0, GROUP), "a heading opens the first entry, cut at the size limit");
        assert!(g.iter().all(|e| e.1 <= GROUP));
        let two = d.blocks.iter().position(|b| matches!(b, super::Block::Heading(2, _))).unwrap() as u32;
        assert!(g.iter().any(|e| e.0 == two), "the second heading starts an entry");
        let img = two + 1;
        assert!(g.contains(&(img, 1)), "an image is an entry of its own");
        assert_eq!(g.iter().map(|e| e.1).sum::<u32>() as usize, d.blocks.len());
    }

    #[test]
    fn preview_zoom_clamps_and_steps() {
        let v = super::FluxView::new("# Hello\n".into(), std::path::PathBuf::new(), "en");
        assert_eq!(v.zoom(), 1.0);
        v.step_zoom(1);
        assert!((v.zoom() - 1.1).abs() < 1e-4);
        v.set_zoom(5.0);
        assert_eq!(v.zoom(), 2.5);
        v.set_zoom(0.1);
        assert_eq!(v.zoom(), 0.5);
        v.step_zoom(0);
        assert_eq!(v.zoom(), 1.0);
    }

    #[test]
    fn ink_undo_redo_and_tool_cycle() {
        let mut st = super::InkState::new();
        assert_eq!(st.tool, super::InkTool::Pen);
        st.tool = st.tool.next();
        assert_eq!(st.tool, super::InkTool::Marker);
        st.tool = st.tool.next();
        assert_eq!(st.tool, super::InkTool::Highlighter);
        st.tool = st.tool.next();
        assert_eq!(st.tool, super::InkTool::Pen);

        st.push_stroke(super::Stroke { color: 0xEF4444, width: 2.5, alpha: 1.0, pts: vec![(10.0, 10.0), (40.0, 10.0)] });
        assert_eq!(st.strokes.len(), 1);
        assert!(st.undo());
        assert!(st.strokes.is_empty() && st.redo.len() == 1);
        assert!(st.redo());
        assert_eq!(st.strokes.len(), 1);
        assert!(st.undo());
        // Pushing a new stroke discards the redo stack.
        st.push_stroke(super::Stroke { color: 0x22C55E, width: 5.0, alpha: 0.95, pts: vec![(20.0, 20.0)] });
        assert!(st.redo.is_empty());
        st.clear();
        assert!(st.strokes.is_empty() && st.redo.is_empty());
    }

    #[test]
    fn annotated_png_fallback_encodes_valid_png_with_strokes() {
        let strokes = vec![
            super::Stroke { color: 0xEF4444, width: 4.0, alpha: 1.0, pts: vec![(10.0, 20.0), (90.0, 20.0)] },
            super::Stroke { color: 0xF59E0B, width: 12.0, alpha: 0.30, pts: vec![(20.0, 50.0), (80.0, 50.0)] },
        ];
        let img = super::rasterize_ink_fallback(120, 80, 0x18181B, 0.0, &strokes);
        assert_eq!(img.dimensions(), (120, 80));
        assert_ne!(img.get_pixel(50, 20).0, [0x18, 0x18, 0x1B, 255]);
        let hl = img.get_pixel(50, 50).0;
        assert!(hl[0] > 0x18 && hl[0] < 0xF5, "translucent highlighter blends with bg: {hl:?}");
        let png = super::encode_png(&img).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn annotated_png_output_path_appends_suffix() {
        let p = super::annotated_png_path(std::path::Path::new("docs/README.md"));
        assert_eq!(p, std::path::PathBuf::from("docs").join("README-annotated.png"));
        let p2 = super::annotated_png_path(std::path::Path::new("notes.flux"));
        assert_eq!(p2, std::path::PathBuf::from(".").join("notes-annotated.png"));
    }
}

#[cfg(test)]
pub fn prepare_size(src: String) -> usize {
    match prepare(src).text {
        Ok(s) => s.len(),
        Err((frames, _)) => frames.iter().map(|f| f.len()).sum(),
    }
}
