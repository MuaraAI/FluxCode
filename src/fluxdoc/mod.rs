//! Flux documents (`.flux`): Markdown's text, plus blocks for what Markdown leaves to
//! HTML or doesn't have — tables with column alignment, images and GIFs with a size,
//! videos, flowcharts, callouts, collapsible sections, columns, a table of contents,
//! sections per interface language and per-document styling. The full syntax is in
//! `docs/flux.flux` (and the README made from it).
//!
//! Built to stay light at any size:
//! - one pass over the text, line by line; nothing is copied: each block holds byte
//!   offsets into the source (about 24 bytes a block), and inline marks (bold, links,
//!   keys) are only worked out for the blocks on screen;
//! - every table row and list item is its own block, so FluxCode draws only the rows
//!   in view (`view.rs`), however long the document or the table;
//! - `md.rs` turns a document into GitHub Markdown, so a README written in Flux shows
//!   on GitHub (flowcharts become Mermaid, callouts GitHub alerts).

pub mod flow;
pub mod md;
pub mod view;

/// A byte range of the source.
pub type Span = (u32, u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Note,
    Tip,
    Warn,
    Danger,
    Details,
    Columns,
    /// `@gallery`: images side by side.
    Gallery,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Block {
    /// `@title`: the document's title.
    Title(Span),
    Heading(u8, Span),
    /// One or more lines of text.
    Para(Span),
    /// A list item: ordered (its number), nesting depth, task box (0 none, 1 open, 2 done).
    Item { num: u32, depth: u8, check: u8, text: Span },
    /// One or more `>` lines.
    Quote(Span),
    /// A code block, by index in `Doc::codes` (its language and lines: rare enough to
    /// keep out of every other block's size).
    Code(u32),
    Rule,
    /// A table row: which table, and its line. Its place among the rows is its distance
    /// from `Table::first`.
    Row { table: u32, line: Span },
    /// `@image`: its arguments (path, alt, width, caption), read when drawn.
    Image(Span),
    /// `@video`: its arguments (path, poster, alt).
    Video(Span),
    /// `@flow`: a flowchart, by index.
    Flow(u32),
    /// A box around other blocks (`inner` indexes `Doc::inner`).
    Boxed { kind: Kind, title: Span, inner: u32 },
    /// `@col` inside `@columns`: the next column starts.
    ColBreak,
    /// `@toc`: the headings, as links.
    Toc,
    /// `@release DATE` (changelog.flux): a release starts.
    Release(Span),
    /// A directive this version doesn't know: shown as is, never an error.
    Unknown(Span),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub cols: usize,
    pub align: Vec<Align>,
    /// Relative column widths (from `widths=` or measured on the first rows).
    pub widths: Vec<f32>,
    /// Rows are `| a | b |` (GitHub style) rather than `a | b`.
    pub pipes: bool,
    pub rows: u32,
    /// The first row's index among the blocks around it, and whether it is a header.
    pub first: u32,
    pub header: bool,
}

impl Table {
    /// Row `pos` (an index among the blocks around the table): its number, and header or not.
    pub fn row(&self, pos: usize) -> (u32, bool) {
        let n = (pos as u32).saturating_sub(self.first);
        (n, self.header && n == 0)
    }
}

/// The per-document look (`@style key=value ...`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub accent: Option<u32>,
    pub width: Option<f32>,
    pub size: Option<f32>,
    pub font: Option<String>,
}

#[derive(Default)]
pub struct Doc {
    pub blocks: Vec<Block>,
    /// The interface language each top-level block is for: 0 all, 1 English, 2 Indonesian.
    pub lang: Vec<u8>,
    pub tables: Vec<Table>,
    pub flows: Vec<flow::Flow>,
    pub inner: Vec<Vec<Block>>,
    /// Code blocks: (language, lines).
    pub codes: Vec<(Span, Span)>,
    pub style: Style,
    /// Indices of the headings in `blocks` (for `@toc` and the outline).
    pub headings: Vec<u32>,
}

impl Doc {
    /// The blocks to show for an interface language ("en" or "id").
    pub fn visible(&self, lang: &str) -> Vec<u32> {
        let want = if lang == "id" { 2 } else { 1 };
        (0..self.blocks.len() as u32).filter(|&i| matches!(self.lang[i as usize], 0) || self.lang[i as usize] == want).collect()
    }
}

pub fn text(src: &str, s: Span) -> &str {
    src.get(s.0 as usize..s.1 as usize).unwrap_or("")
}

/// Lines with their byte offsets (`\r` dropped).
struct Lines<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lines<'a> {
    fn next(&mut self) -> Option<(usize, usize)> {
        if self.pos >= self.src.len() {
            return None;
        }
        let start = self.pos;
        let end = match self.src.as_bytes()[start..].iter().position(|&b| b == b'\n') {
            Some(i) => {
                self.pos = start + i + 1;
                start + i
            }
            None => {
                self.pos = self.src.len();
                self.src.len()
            }
        };
        let end = if end > start && self.src.as_bytes()[end - 1] == b'\r' { end - 1 } else { end };
        Some((start, end))
    }

    fn peek(&self) -> Option<(usize, usize)> {
        Lines { src: self.src, pos: self.pos }.next()
    }
}

fn sp(a: usize, b: usize) -> Span {
    (a as u32, b as u32)
}

/// What starts a new block (ends a paragraph).
fn starts_block(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('#') && heading_level(t).is_some()
        || t.starts_with('@')
        || t.starts_with("```")
        || t.starts_with('>')
        || t.starts_with('|')
        || is_rule(t)
        || list_marker(line).is_some()
}

fn heading_level(t: &str) -> Option<u8> {
    let n = t.bytes().take_while(|&b| b == b'#').count();
    (1..=6).contains(&n).then_some(())?;
    (t.len() == n || t.as_bytes()[n] == b' ').then_some(n as u8)
}

fn is_rule(t: &str) -> bool {
    let t = t.trim_end();
    t.len() >= 3 && (t.bytes().all(|b| b == b'-') || t.bytes().all(|b| b == b'*') || t.bytes().all(|b| b == b'_'))
}

/// A list item: (depth, number (0 for bullets), offset of its text in the line, task box).
fn list_marker(line: &str) -> Option<(u8, u32, usize, u8)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let t = &line[indent..];
    let depth = (indent / 2).min(8) as u8;
    let (num, after) = if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
        (0, r)
    } else {
        let d = t.bytes().take_while(|b| b.is_ascii_digit()).count();
        if d == 0 || d > 9 {
            return None;
        }
        let r = t[d..].strip_prefix(". ").or_else(|| t[d..].strip_prefix(") "))?;
        (t[..d].parse().ok()?, r)
    };
    let (check, after) = if let Some(r) = after.strip_prefix("[ ] ") {
        (1, r)
    } else if let Some(r) = after.strip_prefix("[x] ").or_else(|| after.strip_prefix("[X] ")) {
        (2, r)
    } else {
        (0, after)
    };
    Some((depth, num, line.len() - after.len(), check))
}

/// `key=value` arguments of a directive, with the first bare word as `""`.
/// Values may be "quoted". Returns (key, value span).
pub fn args(src: &str, s: Span) -> Vec<(String, Span)> {
    let line = text(src, s);
    let base = s.0 as usize;
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let ks = i;
        while i < b.len() && b[i] != b' ' && b[i] != b'=' && b[i] != b'"' {
            i += 1;
        }
        let (key, vs, ve);
        if i < b.len() && b[i] == b'=' {
            key = line[ks..i].to_ascii_lowercase();
            i += 1;
            if i < b.len() && b[i] == b'"' {
                i += 1;
                let st = i;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                (vs, ve) = (st, i);
                i += 1;
            } else {
                let st = i;
                while i < b.len() && b[i] != b' ' {
                    i += 1;
                }
                (vs, ve) = (st, i);
            }
        } else if i < b.len() && b[i] == b'"' {
            key = String::new();
            i += 1;
            let st = i;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
            (vs, ve) = (st, i);
            i += 1;
        } else {
            key = String::new();
            (vs, ve) = (ks, i);
        }
        out.push((key, sp(base + vs, base + ve.min(b.len()))));
    }
    out
}

pub fn arg<'a>(src: &'a str, a: &[(String, Span)], key: &str) -> Option<&'a str> {
    a.iter().find(|(k, _)| k == key).map(|(_, s)| text(src, *s))
}

fn hex(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        6 => u32::from_str_radix(s, 16).ok(),
        3 => {
            let v = u32::from_str_radix(s, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            Some((r * 17) << 16 | (g * 17) << 8 | b * 17)
        }
        _ => None,
    }
}

/// Why `parse_into` stopped.
enum Stop {
    Eof,
    End,
    /// `@en`, `@id`, `@all`: what follows is for that language (0 all, 1 en, 2 id).
    Lang(u8),
    /// `@release DATE`: a new release; languages start over.
    Release(Span),
    /// `@title en: ...`: a title for one language.
    TitleFor(u8, Span),
}

pub fn parse(src: &str) -> Doc {
    let mut doc = Doc::default();
    let mut lines = Lines { src, pos: 0 };
    let mut lang = 0u8;
    let mut out = Vec::new();
    let mut tags: Vec<u8> = Vec::new();
    loop {
        let stop = parse_into(src, &mut lines, &mut doc, &mut out, true);
        tags.resize(out.len(), lang);
        match stop {
            Stop::Eof => break,
            Stop::End => {}
            Stop::Lang(l) => lang = l,
            Stop::Release(s) => {
                lang = 0;
                out.push(Block::Release(s));
                tags.push(0);
            }
            Stop::TitleFor(l, s) => {
                out.push(Block::Title(s));
                tags.push(l);
            }
        }
    }
    doc.lang = tags;
    for (i, b) in out.iter().enumerate() {
        if matches!(b, Block::Heading(..)) {
            doc.headings.push(i as u32);
        }
    }
    doc.blocks = out;
    // Grown by doubling: give back what is left over (up to half of each).
    doc.blocks.shrink_to_fit();
    doc.lang.shrink_to_fit();
    doc.headings.shrink_to_fit();
    doc
}

/// Parse blocks into `out` until the end, `@end`, or (at the top level) a language or
/// release marker.
fn parse_into(src: &str, lines: &mut Lines, doc: &mut Doc, out: &mut Vec<Block>, top: bool) -> Stop {
    while let Some((s, e)) = lines.next() {
        let line = &src[s..e];
        let t = line.trim_start();
        let ts = s + (line.len() - t.len());
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        // Code fence.
        if let Some(rest) = t.strip_prefix("```") {
            let lang_s = sp(ts + 3, ts + 3 + rest.trim_end().len());
            let body_start = lines.pos;
            let mut body_end = body_start;
            while let Some((cs, ce)) = lines.next() {
                if src[cs..ce].trim_start().starts_with("```") {
                    break;
                }
                body_end = ce;
            }
            doc.codes.push((lang_s, sp(body_start, body_end.max(body_start))));
            out.push(Block::Code(doc.codes.len() as u32 - 1));
            continue;
        }
        if let Some(d) = t.strip_prefix('@') {
            let name_len = d.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'-').count();
            let name = d[..name_len].to_ascii_lowercase();
            let rest_off = ts + 1 + name_len;
            let rest = sp(rest_off + (src[rest_off..e].len() - src[rest_off..e].trim_start().len()), e);
            match name.as_str() {
                "end" => return Stop::End,
                "en" if top => return Stop::Lang(1),
                "id" if top => return Stop::Lang(2),
                "all" if top => return Stop::Lang(0),
                "release" if top => return Stop::Release(rest),
                "title" => {
                    // changelog.flux: `@title en: ...` titles one language.
                    let r = text(src, rest);
                    let (l, body) = match r.split_once(':') {
                        Some((k, v)) if matches!(k.trim(), "en" | "id") => (if k.trim() == "en" { 1 } else { 2 }, v),
                        _ => (0, r),
                    };
                    let title = sp(rest.1 as usize - body.trim_start().len(), rest.1 as usize);
                    if l != 0 && top {
                        return Stop::TitleFor(l, title);
                    }
                    out.push(Block::Title(title));
                }
                "toc" => out.push(Block::Toc),
                "style" => {
                    let a = args(src, rest);
                    doc.style.accent = arg(src, &a, "accent").and_then(hex).or(doc.style.accent);
                    doc.style.width = arg(src, &a, "width").and_then(|v| v.parse().ok()).or(doc.style.width);
                    doc.style.size = arg(src, &a, "size").and_then(|v| v.parse().ok()).or(doc.style.size);
                    doc.style.font = arg(src, &a, "font").map(String::from).or(doc.style.font.take());
                }
                "image" => out.push(Block::Image(rest)),
                "video" => out.push(Block::Video(rest)),
                "code" => {
                    let body_start = lines.pos;
                    let mut body_end = body_start;
                    while let Some((cs, ce)) = lines.next() {
                        if src[cs..ce].trim() == "@end" {
                            break;
                        }
                        body_end = ce;
                    }
                    doc.codes.push((rest, sp(body_start, body_end.max(body_start))));
                    out.push(Block::Code(doc.codes.len() as u32 - 1));
                }
                "table" => parse_table(src, lines, doc, out, rest),
                "flow" => {
                    let body_start = lines.pos;
                    let mut body_end = body_start;
                    while let Some((cs, ce)) = lines.next() {
                        if src[cs..ce].trim() == "@end" {
                            break;
                        }
                        body_end = ce;
                    }
                    doc.flows.push(flow::parse(src, rest, sp(body_start, body_end.max(body_start))));
                    out.push(Block::Flow(doc.flows.len() as u32 - 1));
                }
                "note" | "tip" | "warn" | "warning" | "danger" | "details" | "columns" | "gallery" => {
                    let kind = match name.as_str() {
                        "note" => Kind::Note,
                        "tip" => Kind::Tip,
                        "warn" | "warning" => Kind::Warn,
                        "danger" => Kind::Danger,
                        "details" => Kind::Details,
                        "gallery" => Kind::Gallery,
                        _ => Kind::Columns,
                    };
                    let mut inner = Vec::new();
                    parse_into(src, lines, doc, &mut inner, false);
                    doc.inner.push(inner);
                    out.push(Block::Boxed { kind, title: rest, inner: doc.inner.len() as u32 - 1 });
                }
                "col" if !top => out.push(Block::ColBreak),
                _ => out.push(Block::Unknown(sp(ts, e))),
            }
            continue;
        }
        if let Some(level) = heading_level(t) {
            let body = t[level as usize..].trim_start();
            let start = e - body.len();
            out.push(Block::Heading(level, sp(start, start + body.trim_end().len())));
            continue;
        }
        if is_rule(t) {
            out.push(Block::Rule);
            continue;
        }
        if t.starts_with('>') {
            let mut end = e;
            while let Some((ns, ne)) = lines.peek() {
                if !src[ns..ne].trim_start().starts_with('>') {
                    break;
                }
                lines.next();
                end = ne;
            }
            out.push(Block::Quote(sp(ts, end)));
            continue;
        }
        if t.starts_with('|') {
            parse_pipe_table(src, lines, doc, out, (s, e));
            continue;
        }
        if let Some((depth, num, off, check)) = list_marker(line) {
            // The item goes on over indented lines that start nothing else.
            let mut end = e;
            while let Some((ns, ne)) = lines.peek() {
                let next = &src[ns..ne];
                if next.trim().is_empty() || !next.starts_with(' ') || list_marker(next).is_some() || starts_block(next.trim_start()) {
                    break;
                }
                lines.next();
                end = ne;
            }
            out.push(Block::Item { num, depth, check, text: sp(s + off, end) });
            continue;
        }
        // A paragraph: this line and the next ones up to a blank line or a block.
        let mut end = e;
        while let Some((ns, ne)) = lines.peek() {
            let next = &src[ns..ne];
            if next.trim().is_empty() || starts_block(next) {
                break;
            }
            lines.next();
            end = ne;
        }
        out.push(Block::Para(sp(ts, end)));
    }
    Stop::Eof
}

/// Split a table row into its cells (byte spans).
pub fn cells(src: &str, line: Span, pipes: bool) -> Vec<Span> {
    let l = text(src, line);
    let base = line.0 as usize;
    let mut body = l.trim();
    let mut off = base + (l.len() - l.trim_start().len());
    if pipes {
        if let Some(r) = body.strip_prefix('|') {
            body = r;
            off += 1;
        }
        body = body.strip_suffix('|').unwrap_or(body);
    }
    let mut out = Vec::new();
    let b = body.as_bytes();
    let mut st = 0;
    let mut i = 0;
    while i <= b.len() {
        let at_end = i == b.len();
        if at_end || (b[i] == b'|' && (i == 0 || b[i - 1] != b'\\')) {
            let cell = &body[st..i];
            let lead = cell.len() - cell.trim_start().len();
            // An empty cell (only spaces) is an empty span where its text would start.
            out.push(sp(off + st + lead, off + st + cell.trim_end().len().max(lead)));
            st = i + 1;
        }
        i += 1;
    }
    out
}

fn measure(src: &str, t: &mut Table, line: Span) {
    for (i, c) in cells(src, line, t.pipes).iter().enumerate().take(t.cols) {
        let w = (c.1 - c.0) as f32;
        if t.widths.len() <= i {
            t.widths.resize(i + 1, 1.);
        }
        t.widths[i] = t.widths[i].max(w.min(60.));
    }
}

fn parse_table(src: &str, lines: &mut Lines, doc: &mut Doc, out: &mut Vec<Block>, rest: Span) {
    let a = args(src, rest);
    let ti = doc.tables.len() as u32;
    let mut t = Table { pipes: false, ..Default::default() };
    let fixed: Option<Vec<f32>> = arg(src, &a, "widths").map(|w| w.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let align: Vec<Align> = arg(src, &a, "align").map(|v| v.split(',').map(|x| match x.trim() {
        "c" | "center" => Align::Center,
        "r" | "right" => Align::Right,
        _ => Align::Left,
    }).collect()).unwrap_or_default();
    let no_header = arg(src, &a, "header") == Some("none");
    let mut n = 0u32;
    while let Some((s, e)) = lines.next() {
        let l = &src[s..e];
        if l.trim() == "@end" {
            break;
        }
        if l.trim().is_empty() {
            continue;
        }
        if n == 0 {
            t.pipes = l.trim_start().starts_with('|');
            t.cols = cells(src, sp(s, e), t.pipes).len();
            t.first = out.len() as u32;
            t.header = !no_header;
        }
        if n < 200 {
            measure(src, &mut t, sp(s, e));
        }
        out.push(Block::Row { table: ti, line: sp(s, e) });
        n += 1;
    }
    t.rows = n;
    t.align = (0..t.cols).map(|i| align.get(i).copied().unwrap_or(Align::Left)).collect();
    if let Some(f) = fixed.filter(|f| !f.is_empty()) {
        t.widths = (0..t.cols).map(|i| f.get(i).copied().unwrap_or(1.)).collect();
    }
    t.widths.resize(t.cols, 1.);
    doc.tables.push(t);
}

/// `| a | b |` lines, GitHub style: the second line (`|---|:--:|`) gives the alignment.
fn parse_pipe_table(src: &str, lines: &mut Lines, doc: &mut Doc, out: &mut Vec<Block>, first: (usize, usize)) {
    let ti = doc.tables.len() as u32;
    let mut t = Table { pipes: true, ..Default::default() };
    t.cols = cells(src, sp(first.0, first.1), true).len();
    let mut header = false;
    if let Some((s, e)) = lines.peek() {
        let sep = cells(src, sp(s, e), true);
        let is_sep = !sep.is_empty() && sep.iter().all(|c| {
            let x = text(src, *c);
            !x.is_empty() && x.bytes().all(|b| matches!(b, b'-' | b':' | b' '))
        });
        if is_sep {
            lines.next();
            header = true;
            t.align = sep
                .iter()
                .map(|c| {
                    let x = text(src, *c).trim();
                    match (x.starts_with(':'), x.ends_with(':')) {
                        (true, true) => Align::Center,
                        (false, true) => Align::Right,
                        _ => Align::Left,
                    }
                })
                .collect();
        }
    }
    measure(src, &mut t, sp(first.0, first.1));
    t.first = out.len() as u32;
    t.header = header;
    out.push(Block::Row { table: ti, line: sp(first.0, first.1) });
    let mut n = 1u32;
    while let Some((s, e)) = lines.peek() {
        if !src[s..e].trim_start().starts_with('|') {
            break;
        }
        lines.next();
        if n < 200 {
            measure(src, &mut t, sp(s, e));
        }
        out.push(Block::Row { table: ti, line: sp(s, e) });
        n += 1;
    }
    t.rows = n;
    t.align.resize(t.cols, Align::Left);
    t.widths.resize(t.cols, 1.);
    doc.tables.push(t);
}

// ---------------------------------------------------------------- inline

/// How a piece of inline text looks.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub mark: bool,
    pub kbd: bool,
    pub badge: bool,
    pub color: Option<u32>,
}

/// Inline text, worked out: the plain text, styled ranges of it, and links.
#[derive(Debug, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub runs: Vec<(std::ops::Range<usize>, Marks)>,
    pub links: Vec<(std::ops::Range<usize>, String)>,
}

/// Bold `**x**`, italic `*x*` / `_x_`, code `` `x` ``, strike `~~x~~`, highlight `==x==`,
/// links `[x](url)` and bare `https://...`, keys `{kbd Ctrl+S}`, badges `{badge New}`
/// and color `{color=#d0a070 x}`. A backslash keeps the next character as it is.
pub fn inline(s: &str) -> Inline {
    let mut out = Inline::default();
    let mut m = Marks::default();
    let b = s.as_bytes();
    let mut i = 0;
    let mut run_start = 0;
    let push_run = |out: &mut Inline, start: usize, m: Marks| {
        if out.text.len() > start {
            out.runs.push((start..out.text.len(), m));
        }
    };
    while i < b.len() {
        let c = b[i];
        let rest = &s[i..];
        let toggle = |m: &mut Marks, what: &str| match what {
            "**" => m.bold = !m.bold,
            "~~" => m.strike = !m.strike,
            "==" => m.mark = !m.mark,
            _ => m.italic = !m.italic,
        };
        if c == b'\\' && i + 1 < b.len() {
            let ch = rest[1..].chars().next().unwrap();
            out.text.push(ch);
            i += 1 + ch.len_utf8();
            continue;
        }
        if c == b'`' {
            if let Some(end) = rest[1..].find('`') {
                push_run(&mut out, run_start, m);
                let st = out.text.len();
                out.text.push_str(&rest[1..1 + end]);
                out.runs.push((st..out.text.len(), Marks { code: true, ..m }));
                run_start = out.text.len();
                i += end + 2;
                continue;
            }
        }
        let two = rest.get(..2).unwrap_or("");
        if matches!(two, "**" | "~~" | "==") && (rest[2..].contains(two) || closes(m, two)) {
            push_run(&mut out, run_start, m);
            toggle(&mut m, two);
            run_start = out.text.len();
            i += 2;
            continue;
        }
        if c == b'*' || (c == b'_' && word_edge(s, i)) {
            let opening = !m.italic && rest.len() > 1 && !rest[1..].starts_with(' ') && rest[1..].contains(c as char);
            let closing = m.italic && closes_italic(s, i);
            if opening || closing {
                push_run(&mut out, run_start, m);
                toggle(&mut m, "*");
                run_start = out.text.len();
                i += 1;
                continue;
            }
        }
        if c == b'[' {
            if let Some(close) = rest.find("](") {
                if let Some(par) = rest[close + 2..].find(')') {
                    let label = &rest[1..close];
                    let url = &rest[close + 2..close + 2 + par];
                    push_run(&mut out, run_start, m);
                    let st = out.text.len();
                    let inner = inline(label);
                    out.text.push_str(&inner.text);
                    out.runs.push((st..out.text.len(), m));
                    out.links.push((st..out.text.len(), url.to_string()));
                    run_start = out.text.len();
                    i += close + 3 + par;
                    continue;
                }
            }
        }
        if c == b'{' {
            if let Some(end) = rest.find('}') {
                let body = &rest[1..end];
                let (kind, val) = body.split_once(' ').unwrap_or((body, ""));
                let mark = match kind {
                    "kbd" => Some(Marks { kbd: true, ..m }),
                    "badge" => Some(Marks { badge: true, ..m }),
                    k if k.starts_with("color=") => hex(&k[6..]).map(|h| Marks { color: Some(h), ..m }),
                    _ => None,
                };
                if let Some(mk) = mark {
                    push_run(&mut out, run_start, m);
                    let st = out.text.len();
                    let inner = inline(val);
                    out.text.push_str(&inner.text);
                    out.runs.push((st..out.text.len(), mk));
                    run_start = out.text.len();
                    i += end + 1;
                    continue;
                }
            }
        }
        if (rest.starts_with("https://") || rest.starts_with("http://")) && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let len = rest.find(|ch: char| ch.is_whitespace() || ch == ')' || ch == '>').unwrap_or(rest.len());
            let url = rest[..len].trim_end_matches(['.', ',', ';', ':']);
            push_run(&mut out, run_start, m);
            let st = out.text.len();
            out.text.push_str(url);
            out.runs.push((st..out.text.len(), m));
            out.links.push((st..out.text.len(), url.to_string()));
            run_start = out.text.len();
            i += url.len();
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.text.push(if ch == '\n' { ' ' } else { ch });
        i += ch.len_utf8();
    }
    push_run(&mut out, run_start, m);
    out
}

fn closes(m: Marks, two: &str) -> bool {
    match two {
        "**" => m.bold,
        "~~" => m.strike,
        "==" => m.mark,
        _ => false,
    }
}

fn word_edge(s: &str, i: usize) -> bool {
    let b = s.as_bytes();
    let before = i == 0 || !b[i - 1].is_ascii_alphanumeric();
    let after = i + 1 >= b.len() || !b[i + 1].is_ascii_alphanumeric();
    before || after
}

fn closes_italic(s: &str, i: usize) -> bool {
    i > 0 && !s.as_bytes()[i - 1].is_ascii_whitespace()
}

/// A paragraph or quote's lines folded into one, `>` marks dropped.
pub fn join_lines(s: &str, quote: bool) -> String {
    let mut out = String::new();
    for l in s.lines() {
        let l = if quote { l.trim_start().trim_start_matches('>').trim_start() } else { l.trim() };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(l);
    }
    out
}

#[cfg(test)]
mod tests;
