//! Drawing a Flux document in FluxCode, light at any size:
//!
//! - The document is a GPUI `list` of small groups of blocks (a heading starts one, a
//!   big block such as a flowchart or an image is one on its own): only the groups in
//!   view are laid out and painted, and the list keeps one entry per group, not per
//!   block. Inline marks are worked out as a block comes into view.
//! - A large document's text is kept DEFLATE-packed in 64 KiB frames; drawing unpacks
//!   just the frames on screen (a few stay unpacked). The blocks hold byte offsets.
//! - A large document opens with its first part read at once and drawn; the rest is
//!   read and packed on another thread, then takes its place.

use super::{arg, args, cells, flow, inline, join_lines, text, Align, Block, Doc, Kind, Marks, Span};
use crate::theme::{self, c};
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

/// The default width of the text column, in px (`@style width=` changes it).
const WIDTH: f32 = 860.;
/// Entries a `@toc` lists at most.
const TOC_MAX: usize = 300;
/// Blocks a list entry holds at most.
const GROUP: u32 = 24;
/// From this size the text is kept packed.
pub const PACK_FROM: usize = 1 << 20;
/// From this size a document opens with its first part, the rest read on another thread.
pub const PROGRESSIVE_FROM: usize = 256 << 10;
/// How much of it is read at once: about two screens.
const FIRST_PART: usize = 32 << 10;
const FRAME: usize = 64 << 10;
/// Unpacked frames kept for drawing.
const FRAMES_KEPT: usize = 6;

/// A document's text: as it is, or packed in frames.
enum Text {
    Plain(Rc<str>),
    Packed { frames: Vec<Box<[u8]>>, len: usize, cache: RefCell<Vec<(usize, Rc<[u8]>)>> },
}

impl Text {
    fn frame(&self, f: usize) -> Rc<[u8]> {
        let Text::Packed { frames, cache, .. } = self else { return Rc::from(&[][..]) };
        // Looked up first: the borrow must end before the cache is reordered.
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

    /// The text of a span.
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

/// A document read on another thread (`prepare`), ready to show.
pub struct Prepared {
    doc: Doc,
    text: Result<String, (Vec<Box<[u8]>>, usize)>,
    ms: f32,
}

/// Read a document and pack its text when it is large. Runs anywhere (no UI).
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

struct Shared {
    text: Text,
    doc: Doc,
    /// List entries: (first block, number of blocks), all shown in this language.
    groups: Vec<(u32, u32)>,
    /// The interface language's tag (1 en, 2 id): blocks tagged 0 or this show.
    want: u8,
    base: PathBuf,
    /// `@details` boxes the reader opened (by inner index).
    open: Rc<RefCell<HashSet<u32>>>,
}

impl Shared {
    fn shown(&self, i: usize) -> bool {
        matches!(self.doc.lang.get(i), Some(&l) if l == 0 || l == self.want)
    }
}

/// Group the blocks shown in a language into list entries.
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
        // A heading starts an entry, so the contents can jump straight to it.
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

/// A parsed document and its scroll position.
pub struct FluxView {
    shared: Rc<Shared>,
    pub list: ListState,
    pub parsed_ms: f32,
    /// Only the first part is read; the rest is being read on another thread.
    pub pending: bool,
}

impl FluxView {
    fn build(doc: Doc, text: Text, base: PathBuf, lang: &str, parsed_ms: f32, open: Rc<RefCell<HashSet<u32>>>) -> Self {
        let want = if lang == "id" { 2 } else { 1 };
        let mut groups = groups(&doc, want);
        groups.shrink_to_fit();
        let list = ListState::new(groups.len(), ListAlignment::Top, px(600.));
        Self { shared: Rc::new(Shared { text, doc, groups, want, base, open }), list, parsed_ms, pending: false }
    }

    pub fn new(src: String, base: PathBuf, lang: &str) -> Self {
        Self::from_prepared(prepare(src), base, lang, Rc::default())
    }

    fn from_prepared(p: Prepared, base: PathBuf, lang: &str, open: Rc<RefCell<HashSet<u32>>>) -> Self {
        let text = match p.text {
            Ok(s) => Text::Plain(s.into()),
            Err((frames, len)) => Text::Packed { frames, len, cache: RefCell::new(Vec::new()) },
        };
        Self::build(p.doc, text, base, lang, p.ms, open)
    }

    /// The first part of a large document, drawn at once (`install` brings the rest).
    pub fn first_part(src: &str, base: PathBuf, lang: &str) -> Self {
        let mut cut = src.len().min(FIRST_PART);
        while !src.is_char_boundary(cut) {
            cut -= 1;
        }
        // End at a blank line, so no block is cut in half.
        if let Some(i) = src[..cut].rfind("\n\n") {
            cut = i + 1;
        }
        let mut v = Self::new(src[..cut].to_string(), base, lang);
        v.pending = true;
        v
    }

    /// The whole document, read on another thread, in place of its first part.
    pub fn install(&mut self, p: Prepared, lang: &str) {
        let top = self.list.logical_scroll_top();
        let v = Self::from_prepared(p, self.shared.base.clone(), lang, self.shared.open.clone());
        v.list.scroll_to(ListOffset { item_ix: top.item_ix.min(v.shared.groups.len().saturating_sub(1)), offset_in_item: top.offset_in_item });
        *self = v;
    }

    /// Show `src` instead, keeping the place in the document as far as it goes.
    pub fn reload(&mut self, src: String, lang: &str) {
        self.install(prepare(src), lang);
    }

    pub fn blocks(&self) -> usize {
        self.shared.doc.blocks.len()
    }

    /// The text kept for drawing (packed size for a packed one).
    pub fn kept_bytes(&self) -> usize {
        self.shared.text.packed_bytes()
    }

    pub fn source(&self) -> String {
        self.shared.text.full()
    }

    /// The document: a list drawing only what is in view.
    pub fn render(&self, notify: impl Fn(&mut App) + 'static) -> AnyElement {
        let sh = self.shared.clone();
        let state = self.list.clone();
        let notify = Rc::new(notify);
        let width = sh.doc.style.width.unwrap_or(WIDTH);
        let size = sh.doc.style.size.unwrap_or(15.);
        let font = sh.doc.style.font.clone();
        list(self.list.clone(), move |ix, _window, cx| {
            let Some(&(first, count)) = sh.groups.get(ix) else { return div().into_any_element() };
            let ctx = Ctx { sh: &sh, list: &state, notify: notify.clone() };
            let mut col = v_flex().w_full();
            for bi in first as usize..(first + count) as usize {
                let next = (bi + 1 < sh.doc.blocks.len() && sh.shown(bi + 1)).then(|| &sh.doc.blocks[bi + 1]);
                col = col.child(ctx.block(&sh.doc.blocks[bi], bi, bi, next, cx));
            }
            h_flex()
                .w_full()
                .justify_center()
                .child(div().w_full().max_w(theme::u(width)).px_8().when(ix == 0, |d| d.pt_8()).when(ix + 1 == sh.groups.len(), |d| d.pb_16()).child(col))
                .into_any_element()
        })
        .size_full()
        .text_size(theme::u(size))
        .when_some(font, |l, f| l.font_family(f))
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
    /// The text of a span (unpacked when the document is packed).
    fn t(&self, s: Span) -> String {
        self.sh.text.get(s)
    }

    /// Inline text with its marks, links clickable.
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

    /// One block. `pos`: its index among the blocks around it; `next`: the block after it.
    fn block(&self, b: &Block, bi: usize, pos: usize, next: Option<&Block>, cx: &mut App) -> AnyElement {
        let id = |k: &str| ElementId::Name(format!("fx-{k}-{bi}").into());
        match *b {
            Block::Title(s) => v_flex()
                .pb_4()
                .gap_2()
                .child(div().text_size(theme::u(32.)).font_weight(FontWeight::BOLD).text_color(c(theme::FG_STRONG)).child(self.rich(id("t"), &self.t(s), c(theme::FG_STRONG))))
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
                    .pt(theme::u(if l <= 2 { 22. } else { 14. }))
                    .pb_2()
                    .when(l <= 2, |d| d.border_b_1().border_color(c(theme::BORDER)).mb_3())
                    .text_size(theme::u(size))
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
                        .size(theme::u(15.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(c(if check == 2 { theme::ACCENT } else { theme::BORDER_STRONG }))
                        .when(check == 2, |d| d.bg(c(theme::ACCENT)).child(Icon::new(IconName::Check).size(theme::u(11.)).text_color(c(theme::ON_ACCENT))))
                        .flex()
                        .items_center()
                        .justify_center()
                        .into_any_element(),
                    _ if num > 0 => div().flex_none().min_w(theme::u(20.)).text_color(c(theme::FG_MUTED)).child(format!("{num}.")).into_any_element(),
                    _ => div().flex_none().w(theme::u(20.)).pt(theme::u(9.)).child(div().size(px(5.)).rounded_full().bg(if depth == 0 { accent(self.sh) } else { c(theme::FG_DIM) })).into_any_element(),
                };
                let last_in_run = !matches!(next, Some(Block::Item { .. }));
                h_flex()
                    .items_start()
                    .gap_2()
                    .pl(theme::u(4. + depth as f32 * 22.))
                    .pb(theme::u(if last_in_run { 12. } else { 4. }))
                    .line_height(relative(1.6))
                    .child(mark)
                    .child(div().flex_1().min_w_0().child(self.rich(id("li"), &join_lines(&self.t(s), false), c(if check == 2 { theme::FG_MUTED } else { theme::FG }))))
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
                // Long lines wrap: a sideways scroller here would also catch the wheel
                // while the page is being scrolled past it.
                .child(
                    div()
                        .px_3()
                        .py_2p5()
                        .font_family("Cascadia Code")
                        .text_size(theme::u(13.))
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
                .child(Icon::new(IconName::Calendar).size(theme::u(15.)).text_color(accent(self.sh)))
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(self.t(s)))
                .into_any_element(),
            Block::Unknown(s) => div().pb_2().font_family("Cascadia Code").text_xs().text_color(c(theme::FG_DIM)).child(self.t(s)).into_any_element(),
        }
    }

    fn row(&self, table: u32, line: Span, pos: usize, next: Option<&Block>) -> AnyElement {
        let t = &self.sh.doc.tables[table as usize];
        let (n, header) = t.row(pos);
        let total: f32 = t.widths.iter().map(|w| w.max(4.)).sum::<f32>().max(1.);
        // The row's own text, and its cells as spans of it.
        let line_text = self.t(line);
        let src: &str = &line_text;
        let cs = cells(src, (0, src.len() as u32), t.pipes);
        let last = !matches!(next, Some(Block::Row { table: tt, .. }) if *tt == table);
        let first = n == 0;
        h_flex()
            .w_full()
            // Cells as tall as the row, so their dividers run top to bottom.
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

    /// One picture: `width=` in px or %, `dark=` another file for dark themes, `link=`
    /// what a click opens.
    fn picture(&self, a: super::Span, id: usize) -> AnyElement {
        // The directive's own text, and its arguments as spans of it.
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
            // Images from the web aren't fetched (documents stay offline): a link to it.
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
        // A share of the page (`60%`), pixels (`96`), or the picture's own size.
        let share = width.as_deref().and_then(|w| w.strip_suffix('%')).and_then(|w| w.parse::<f32>().ok());
        let pixels = width.as_deref().filter(|w| !w.ends_with('%')).and_then(|w| w.trim_end_matches("px").parse::<f32>().ok());
        let mut im = img(self.path(p)).max_w_full().rounded(px(6.)).object_fit(ObjectFit::Contain);
        if share.is_some() {
            im = im.w_full();
        } else if let Some(px_) = pixels {
            im = im.w(theme::u(px_));
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
        // The directive's own text, and its arguments as spans of it.
        let arg_text = self.t(a);
        let src: &str = &arg_text;
        let args_ = args(src, (0, src.len() as u32));
        let caption = arg(src, &args_, "caption");
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

    /// A video: its poster with a play button; a click plays it in the system's player.
    fn video(&self, a: super::Span, bi: usize) -> AnyElement {
        // The directive's own text, and its arguments as spans of it.
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
        // Decisions are drawn here as diamonds (a box can't be turned); their text sits on top.
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
                    // Out of the right side, along the edge of the drawing, back in.
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
                    // 6 px on, 4 px off, along every segment.
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
                // Arrowhead at the end, pointing along the last segment.
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
                    Block::Image(a) => Some(self.picture(*a, bi * 1000 + k)),
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
        // A contents list is for finding your way: past TOC_MAX entries it says how many more.
        let wanted: Vec<u32> = self.sh.doc.headings.iter().copied().filter(|&h| self.sh.shown(h as usize) && matches!(self.sh.doc.blocks[h as usize], Block::Heading(l, _) if l <= 3)).collect();
        let more = wanted.len().saturating_sub(TOC_MAX);
        for (k, &h) in wanted.iter().take(TOC_MAX).enumerate() {
            let Block::Heading(l, s) = self.sh.doc.blocks[h as usize] else { continue };
            // Headings start list entries, which are in order: find its entry by halving.
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

/// Highlights may overlap (a link in bold): GPUI wants them sorted and apart, so later
/// ones split earlier ones and the pieces combine.
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

/// `.flux` files open drawn (FluxCode's own `changelog.flux` too).
pub fn is_flux(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("flux"))
}

/// What a file draws, from its extension: `Some(true)` for Markdown (drawn through
/// the shim, Source first), `Some(false)` for Flux (drawn directly, Preview first),
/// None for everything the plain editor shows.
pub fn drawable_kind(path: &std::path::Path) -> Option<bool> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "flux" => Some(false),
        "md" | "markdown" => Some(true),
        _ => None,
    }
}

/// The bar's export-to-Markdown button writes `.md` next to a Flux document; a
/// Markdown file already is one.
pub fn shows_md_export(path: &std::path::Path) -> bool {
    is_flux(path)
}

/// The interface language, for `@en` / `@id` sections.
pub fn ui_lang() -> &'static str {
    if crate::i18n::is_id() { "id" } else { "en" }
}

impl crate::workbench::Workbench {
    /// Above a Flux document: Preview or Source, what it holds, and Markdown export.
    pub(crate) fn render_flux_bar(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        use crate::i18n::tr;
        let d = &self.docs[ix];
        let Some(v) = d.flux.as_ref() else { return div().into_any_element() };
        let source = d.flux_source;
        let tab = |id: &'static str, icon: IconName, label: &'static str, on: bool, want_source: bool, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .gap_1p5()
                .px_2p5()
                .py_1()
                .rounded(px(5.))
                .cursor_pointer()
                .text_xs()
                .when(on, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .when(!on, |d| d.text_color(c(theme::FG_MUTED)).hover(|s| s.text_color(c(theme::FG))))
                .child(Icon::new(icon).size(theme::u(13.)))
                .child(label)
                .on_click(cx.listener(move |this, _, w, cx| this.flux_show(ix, want_source, w, cx)))
        };
        let md_name = d.path.with_extension("md").file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        h_flex()
            .flex_none()
            .mx_3()
            .mb_1()
            .gap_2()
            .child(
                h_flex()
                    .p_0p5()
                    .gap_0p5()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .child(tab("fx-tab-view", IconName::Eye, tr("Preview"), !source, false, cx))
                    .child(tab("fx-tab-src", IconName::FileCode, tr("Source"), source, true, cx)),
            )
            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(if v.pending {
                crate::i18n::tr("Reading the rest…").to_string()
            } else {
                crate::trf!("{n} blocks · read in {ms} ms · text in {size}"; n => thousands(v.blocks()), ms => format!("{:.1}", v.parsed_ms), size => crate::viewer::human_size(v.kept_bytes() as u64))
            }))
            // The export belongs to Flux documents only: a Markdown file already is one.
            .when(shows_md_export(&d.path), |h| {
                h.child(
                    h_flex()
                        .id("fx-to-md")
                        .gap_1p5()
                        .px_2()
                        .py_1()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::trf!("Write {name} next to it, as GitHub Markdown (flowcharts become Mermaid)"; name => md_name)).build(w, cx))
                        .child(Icon::new(IconName::Download).size(theme::u(13.)))
                        .child(tr("Save as Markdown"))
                        .on_click(cx.listener(move |this, _, _, cx| this.flux_save_markdown(ix, cx))),
                )
            })
            .into_any_element()
    }

    /// Switch a Flux document between its drawing and its source (drawn from the text
    /// in the editor, edits and all).
    fn flux_show(&mut self, ix: usize, source: bool, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.docs.len() {
            return;
        }
        // Only its first part is read yet: its editor would hold a cut copy.
        if source && self.docs[ix].flux.as_ref().is_some_and(|v| v.pending) {
            self.status = crate::i18n::tr("Still reading the document; Source opens in a moment.").into();
            return cx.notify();
        }
        if source {
            self.flux_load_source(ix, window, cx);
        }
        if !source && self.docs[ix].flux_source {
            let d = &mut self.docs[ix];
            let mut text = d.editor.read(cx).value().to_string();
            // A Markdown document draws the shim's output of its text, never the raw
            // file: rebuild it here, or every Preview click would draw the raw page.
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

    /// Put a large Flux document's text into its editor (once).
    pub(crate) fn flux_load_source(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let d = &mut self.docs[ix];
        if !d.flux_lazy {
            return;
        }
        d.flux_lazy = false;
        let text = d.flux.as_ref().map(|v| v.source()).unwrap_or_default();
        d.editor.update(cx, |e, cx| e.set_value(text, window, cx));
    }

    /// `README.flux` -> `README.md`, as GitHub Markdown.
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

/// 900004 -> "900,004".
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
        // Multi-byte characters everywhere, so frame edges fall inside some of them.
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
        // Spans on character boundaries, some crossing frame edges.
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
}

/// Bytes kept for a document's text after `prepare` (for the benchmark).
#[cfg(test)]
pub fn prepare_size(src: String) -> usize {
    match prepare(src).text {
        Ok(s) => s.len(),
        Err((frames, _)) => frames.iter().map(|f| f.len()).sum(),
    }
}
