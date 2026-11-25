use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cmp::Ordering;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdTable {
    pub headers: Vec<String>,
    pub aligns: Vec<Align>,
    pub explicit_align: Vec<bool>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Markdown(String),
    Table(MdTable),
}

#[derive(Clone, Debug, PartialEq)]
enum SortKey {
    Num(f64),
    Text(String),
}

impl SortKey {
    fn cmp_key(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SortKey::Num(a), SortKey::Num(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (SortKey::Num(_), SortKey::Text(_)) => Ordering::Less,
            (SortKey::Text(_), SortKey::Num(_)) => Ordering::Greater,
            (SortKey::Text(a), SortKey::Text(b)) => a.cmp(b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CellDisplay {
    text: String,
    bold: bool,
    code: bool,
}

#[derive(Clone, Copy, Debug)]
struct ColSpec {
    min_w: f32,
    align: Align,
    numeric: bool,
    flex_grow: bool,
}

pub fn split_markdown_tables(src: &str) -> Vec<Segment> {
    if !src.contains('|') {
        return vec![Segment::Markdown(src.to_string())];
    }
    let lines: Vec<&str> = src.lines().collect();
    let mut out: Vec<Segment> = Vec::new();
    let mut md_buf: Vec<&str> = Vec::new();
    let mut i = 0usize;
    let mut fence: Option<char> = None;

    let flush_md = |buf: &mut Vec<&str>, out: &mut Vec<Segment>| {
        if !buf.is_empty() {
            let chunk = buf.join("\n");
            if !chunk.trim().is_empty() {
                out.push(Segment::Markdown(chunk));
            }
            buf.clear();
        }
    };

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if let Some(ch) = fence {
            if trimmed.starts_with(ch) && trimmed.chars().take_while(|&c| c == ch).count() >= 3 {
                fence = None;
            }
            md_buf.push(line);
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some('`');
            md_buf.push(line);
            i += 1;
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some('~');
            md_buf.push(line);
            i += 1;
            continue;
        }

        if i + 1 < lines.len() {
            if let Some(headers) = parse_pipe_row(line) {
                if headers.len() >= 2 {
                    if let Some((aligns, explicit_align)) = parse_separator_row(lines[i + 1], headers.len()) {
                        let mut rows: Vec<Vec<String>> = Vec::new();
                        let mut j = i + 2;
                        while j < lines.len() {
                            let row_line = lines[j];
                            let rt = row_line.trim();
                            if rt.is_empty()
                                || rt.starts_with("```")
                                || rt.starts_with("~~~")
                                || rt.starts_with('#')
                                || rt.starts_with('>')
                                || !rt.contains('|')
                            {
                                break;
                            }
                            let Some(mut cells) = parse_pipe_row(row_line) else {
                                break;
                            };
                            cells.resize(headers.len(), String::new());
                            rows.push(cells);
                            j += 1;
                        }
                        if !rows.is_empty() {
                            flush_md(&mut md_buf, &mut out);
                            out.push(Segment::Table(MdTable {
                                headers,
                                aligns,
                                explicit_align,
                                rows,
                            }));
                            i = j;
                            continue;
                        }
                    }
                }
            }
        }

        md_buf.push(line);
        i += 1;
    }

    flush_md(&mut md_buf, &mut out);
    if out.is_empty() {
        out.push(Segment::Markdown(src.to_string()));
    }
    out
}

fn parse_pipe_row(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.contains('|') {
        return None;
    }
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = strip_unescaped_trailing_pipe(inner);

    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    let mut in_code = false;
    while let Some(ch) = chars.next() {
        if ch == '\\' && !in_code {
            if let Some(&next) = chars.peek() {
                if next == '|' {
                    cur.push('|');
                    chars.next();
                    continue;
                }
            }
            cur.push(ch);
            continue;
        }
        if ch == '`' {
            in_code = !in_code;
            cur.push(ch);
            continue;
        }
        if ch == '|' && !in_code {
            cells.push(cur.trim().to_string());
            cur.clear();
            continue;
        }
        cur.push(ch);
    }
    cells.push(cur.trim().to_string());
    if cells.is_empty() {
        None
    } else {
        Some(cells)
    }
}

fn strip_unescaped_trailing_pipe(s: &str) -> &str {
    if !s.ends_with('|') {
        return s;
    }
    let bs = s[..s.len() - 1].chars().rev().take_while(|&c| c == '\\').count();
    if bs % 2 == 1 { s } else { &s[..s.len() - 1] }
}

fn parse_separator_row(line: &str, expected: usize) -> Option<(Vec<Align>, Vec<bool>)> {
    let cells = parse_pipe_row(line)?;
    if cells.len() != expected {
        return None;
    }
    let mut aligns = Vec::with_capacity(expected);
    let mut explicit = Vec::with_capacity(expected);
    for cell in &cells {
        let c = cell.trim();
        if c.is_empty() {
            return None;
        }
        let left = c.starts_with(':');
        let right = c.ends_with(':');
        let core = c.trim_start_matches(':').trim_end_matches(':');
        if core.is_empty() || !core.chars().all(|ch| ch == '-') {
            return None;
        }
        match (left, right) {
            (true, true) => {
                aligns.push(Align::Center);
                explicit.push(true);
            }
            (false, true) => {
                aligns.push(Align::Right);
                explicit.push(true);
            }
            (true, false) => {
                aligns.push(Align::Left);
                explicit.push(true);
            }
            (false, false) => {
                aligns.push(Align::Left);
                explicit.push(false);
            }
        }
    }
    Some((aligns, explicit))
}

fn clean_cell(raw: &str) -> CellDisplay {
    let mut s = raw.trim();
    let mut bold = false;
    let mut code = false;
    if let Some(inner) = s.strip_prefix("**").and_then(|x| x.strip_suffix("**")) {
        if !inner.is_empty() && !inner.contains("**") {
            bold = true;
            s = inner.trim();
        }
    } else if let Some(inner) = s.strip_prefix("__").and_then(|x| x.strip_suffix("__")) {
        if !inner.is_empty() && !inner.contains("__") {
            bold = true;
            s = inner.trim();
        }
    }
    if let Some(inner) = s.strip_prefix('`').and_then(|x| x.strip_suffix('`')) {
        if !inner.is_empty() && !inner.contains('`') {
            code = true;
            s = inner.trim();
        }
    }
    let text = strip_inline_md(s);
    CellDisplay { text, bold, code }
}

fn strip_inline_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            i += 2;
            continue;
        }
        if chars[i] == '`' {
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn parse_sort_key(raw: &str) -> SortKey {
    let c = clean_cell(raw);
    let trimmed = c
        .text
        .trim()
        .trim_start_matches(['~', '≈', '<', '>', '+', '$'])
        .trim();
    if trimmed.is_empty() {
        return SortKey::Text(String::new());
    }
    // Percentage e.g. "81.6%"
    if let Some(num) = trimmed.strip_suffix('%') {
        if let Ok(v) = num.trim().replace(',', "").parse::<f64>() {
            return SortKey::Num(v);
        }
    }
    // Split numeric prefix from optional byte unit suffix (e.g. "33.71 MB", "240KB", "214 B")
    let num_end = trimmed
        .find(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-' | '_')))
        .unwrap_or(trimmed.len());
    if num_end > 0 {
        let num_str = trimmed[..num_end].replace([',', '_'], "");
        let unit = trimmed[num_end..].trim().to_ascii_lowercase();
        if let Ok(v) = num_str.parse::<f64>() {
            let mult = match unit.as_str() {
                "" => Some(1.0),
                "b" | "byte" | "bytes" => Some(1.0),
                "k" | "kb" | "kib" => Some(1024.0),
                "m" | "mb" | "mib" => Some(1024.0 * 1024.0),
                "g" | "gb" | "gib" => Some(1024.0 * 1024.0 * 1024.0),
                "t" | "tb" | "tib" => Some(1024.0 * 1024.0 * 1024.0 * 1024.0),
                "ms" => Some(0.001),
                "s" | "sec" | "secs" => Some(1.0),
                "files" | "file" | "items" | "rows" | "lines" => Some(1.0),
                _ => None,
            };
            if let Some(m) = mult {
                return SortKey::Num(v * m);
            }
        }
    }
    SortKey::Text(c.text.to_lowercase())
}

fn is_numeric_value(raw: &str) -> bool {
    matches!(parse_sort_key(raw), SortKey::Num(_))
}

fn is_summary_row(row: &[String]) -> bool {
    let Some(first) = row.first() else {
        return false;
    };
    let clean = clean_cell(first);
    let low = clean.text.trim().to_ascii_lowercase();
    matches!(low.as_str(), "total" | "jumlah" | "subtotal" | "summary" | "ringkasan" | "all")
}

fn compute_col_specs(t: &MdTable) -> Vec<ColSpec> {
    let ncols = t.headers.len();
    let mut specs = Vec::with_capacity(ncols);
    let mut widest_text_ix: Option<(usize, usize)> = None;

    for col in 0..ncols {
        let hdr_len = clean_cell(&t.headers[col]).text.chars().count();
        let mut max_cell = 0usize;
        let mut non_empty = 0usize;
        let mut num_hits = 0usize;
        for row in &t.rows {
            if let Some(cell) = row.get(col) {
                let d = clean_cell(cell);
                if !d.text.is_empty() {
                    non_empty += 1;
                    max_cell = max_cell.max(d.text.chars().count());
                    if is_numeric_value(cell) {
                        num_hits += 1;
                    }
                }
            }
        }
        let numeric = non_empty > 0 && num_hits * 10 >= non_empty * 6;
        let align = if t.explicit_align.get(col).copied().unwrap_or(false) {
            t.aligns[col]
        } else if numeric {
            Align::Right
        } else {
            Align::Left
        };
        let content_chars = hdr_len.max(max_cell);
        let min_w = if numeric {
            // Keep numbers & byte units (e.g. "33.71 MB", "~41.2 MB", "5,307") strictly on one line.
            (content_chars as f32 * 7.8 + 28.0).clamp(64.0, 148.0)
        } else {
            // Short folder/category names stay single-line; long file paths get generous scrollable room.
            (content_chars as f32 * 7.4 + 28.0).clamp(84.0, 440.0)
        };
        if !numeric && widest_text_ix.is_none_or(|(_, best)| content_chars > best) {
            widest_text_ix = Some((col, content_chars));
        }
        specs.push(ColSpec {
            min_w,
            align,
            numeric,
            flex_grow: false,
        });
    }

    if let Some((ix, _)) = widest_text_ix {
        specs[ix].flex_grow = true;
    } else if let Some(first) = specs.first_mut() {
        first.flex_grow = true;
    }
    specs
}

pub fn table_to_markdown(t: &MdTable) -> String {
    let mut out = String::new();
    out.push('|');
    for h in &t.headers {
        out.push(' ');
        out.push_str(h);
        out.push_str(" |");
    }
    out.push('\n');
    out.push('|');
    for a in &t.aligns {
        let sep = match a {
            Align::Left => " --- |",
            Align::Center => " :---: |",
            Align::Right => " ---: |",
        };
        out.push_str(sep);
    }
    out.push('\n');
    for row in &t.rows {
        out.push('|');
        for cell in row {
            out.push(' ');
            out.push_str(cell);
            out.push_str(" |");
        }
        out.push('\n');
    }
    out
}

pub fn table_to_csv(t: &MdTable) -> String {
    let esc = |s: &str| {
        let clean = clean_cell(s).text;
        if clean.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", clean.replace('"', "\"\""))
        } else {
            clean
        }
    };
    let mut out = String::new();
    out.push_str(&t.headers.iter().map(|h| esc(h)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for row in &t.rows {
        out.push_str(&row.iter().map(|c| esc(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

fn sorted_indices(t: &MdTable, sort: Option<(usize, bool)>) -> (Vec<usize>, Option<usize>) {
    let total_ix = (t.rows.len() >= 2 && is_summary_row(&t.rows[t.rows.len() - 1])).then_some(t.rows.len() - 1);
    let body_len = total_ix.unwrap_or(t.rows.len());
    let mut order: Vec<usize> = (0..body_len).collect();
    if let Some((col, asc)) = sort {
        if col < t.headers.len() {
            order.sort_by(|&a, &b| {
                let ka = parse_sort_key(t.rows[a].get(col).map(String::as_str).unwrap_or(""));
                let kb = parse_sort_key(t.rows[b].get(col).map(String::as_str).unwrap_or(""));
                let cmp = ka.cmp_key(&kb);
                if asc { cmp } else { cmp.reverse() }
            });
        }
    }
    (order, total_ix)
}

fn resolve_workspace_file(root: Option<&std::path::Path>, text: &str) -> Option<PathBuf> {
    let root = root?;
    let trimmed = text.trim().trim_matches(['"', '\'', '`']);
    if trimmed.is_empty() || trimmed.len() > 260 || trimmed.contains('\n') {
        return None;
    }
    let candidate = root.join(trimmed);
    if candidate.is_file() {
        return Some(candidate);
    }
    // Check common pack prefixes (e.g. "KiwHitAnimation_RP\models\..." inside resource_packs or behavior_packs)
    for prefix in ["resource_packs", "behavior_packs"] {
        let nested = root.join(prefix).join(trimmed);
        if nested.is_file() {
            return Some(nested);
        }
    }
    None
}

impl Workbench {
    pub(crate) fn render_md_table(
        &self,
        item_ix: usize,
        table_ix: usize,
        table: &MdTable,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{item_ix}:{table_ix}");
        let sort = self.agent.table_sorts.get(&key).copied();
        let specs = compute_col_specs(table);
        let scale = theme::scale();
        let col_widths: Vec<f32> = specs.iter().map(|s| (s.min_w * scale).round()).collect();
        let total_min_w: f32 = col_widths.iter().sum();

        let (order, total_ix) = sorted_indices(table, sort);
        let data_rows = order.len();
        let md_copy = table_to_markdown(table);
        let csv_copy = table_to_csv(table);

        let rows_label = if data_rows == 1 {
            tr("1 row").to_string()
        } else {
            crate::trf!("{n} rows"; n => data_rows)
        };

        let toolbar_btn = |id: SharedString, label: SharedString, tip: SharedString| {
            h_flex()
                .id(id)
                .flex_none()
                .h(theme::u(18.))
                .px_1p5()
                .gap_1()
                .rounded(px(3.))
                .cursor_pointer()
                .text_size(theme::u(10.5))
                .text_color(c(theme::FG_DIM))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                .child(Icon::new(IconName::Copy).size(theme::u(10.)))
                .child(label)
        };

        let top_bar = h_flex()
            .w_full()
            .h(theme::u(24.))
            .px_2()
            .gap_1p5()
            .bg(c(theme::BG_ELEVATED))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(theme::u(10.5))
                    .text_color(c(theme::FG_DIM))
                    .child(rows_label),
            )
            .child(
                toolbar_btn(
                    SharedString::from(format!("ag-tbl-md-{item_ix}-{table_ix}")),
                    SharedString::from(tr("Copy")),
                    SharedString::from(tr("Copy table as Markdown")),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(md_copy.clone()));
                    this.status = tr("Table copied").into();
                    cx.notify();
                })),
            )
            .child(
                toolbar_btn(
                    SharedString::from(format!("ag-tbl-csv-{item_ix}-{table_ix}")),
                    SharedString::from("CSV"),
                    SharedString::from(tr("Copy table as CSV")),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(csv_copy.clone()));
                    this.status = tr("CSV copied").into();
                    cx.notify();
                })),
            );

        let header_row = h_flex()
            .w_full()
            .bg(c(theme::BG_ELEVATED))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .children(table.headers.iter().enumerate().map(|(col_ix, raw_hdr)| {
                let spec = specs[col_ix];
                let w = col_widths[col_ix];
                let display = clean_cell(raw_hdr);
                let active = sort.filter(|(c, _)| *c == col_ix);
                let sort_key = key.clone();
                let default_asc = !spec.numeric;
                let tip: SharedString = tr("Click to sort").into();

                h_flex()
                    .id(SharedString::from(format!("ag-tbl-h-{item_ix}-{table_ix}-{col_ix}")))
                    .min_w(px(w))
                    .when(spec.flex_grow, |d| d.flex_1())
                    .when(!spec.flex_grow, |d| d.w(px(w)).flex_none())
                    .px_2p5()
                    .py_1p5()
                    .gap_1()
                    .cursor_pointer()
                    .when(col_ix > 0, |d| d.border_l_1().border_color(c(theme::BORDER)))
                    .when(spec.align == Align::Right, |d| d.justify_end())
                    .when(spec.align == Align::Center, |d| d.justify_center())
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(if active.is_some() { theme::ACCENT_TEXT } else { theme::FG_MUTED }))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let next = match this.agent.table_sorts.get(&sort_key).copied() {
                            Some((c, asc)) if c == col_ix && asc == default_asc => Some((col_ix, !default_asc)),
                            Some((c, _)) if c == col_ix => None,
                            _ => Some((col_ix, default_asc)),
                        };
                        match next {
                            Some(v) => {
                                this.agent.table_sorts.insert(sort_key.clone(), v);
                            }
                            None => {
                                this.agent.table_sorts.remove(&sort_key);
                            }
                        }
                        cx.notify();
                    }))
                    .child(div().whitespace_nowrap().child(display.text))
                    .when_some(active, |d, (_, asc)| {
                        d.child(
                            Icon::new(if asc { IconName::ChevronUp } else { IconName::ChevronDown })
                                .flex_none()
                                .size(theme::u(11.))
                                .text_color(c(theme::ACCENT_TEXT)),
                        )
                    })
            }));

        let render_row = |row_ix: usize, is_total: bool, is_last: bool| {
            let row = &table.rows[row_ix];
            h_flex()
                .id(SharedString::from(format!("ag-tbl-r-{item_ix}-{table_ix}-{row_ix}")))
                .w_full()
                .items_center()
                .when(is_total, |d| {
                    d.bg(c(theme::BG_ELEVATED))
                        .border_t_1()
                        .border_color(c(theme::BORDER_STRONG))
                })
                .when(!is_total && !is_last, |d| d.border_b_1().border_color(c(theme::BORDER)))
                .when(!is_total, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                .children(specs.iter().enumerate().map(|(col_ix, spec)| {
                    let w = col_widths[col_ix];
                    let raw = row.get(col_ix).map(String::as_str).unwrap_or("");
                    let cell = clean_cell(raw);
                    let open_path = if !spec.numeric && !is_total {
                        resolve_workspace_file(self.root.as_deref(), &cell.text)
                    } else {
                        None
                    };
                    let copy_val = cell.text.clone();
                    let is_link = open_path.is_some();
                    let tip: SharedString = if is_link {
                        tr("Click to open file").into()
                    } else {
                        tr("Click to copy").into()
                    };

                    div()
                        .id(SharedString::from(format!("ag-tbl-c-{item_ix}-{table_ix}-{row_ix}-{col_ix}")))
                        .min_w(px(w))
                        .when(spec.flex_grow, |d| d.flex_1())
                        .when(!spec.flex_grow, |d| d.w(px(w)).flex_none())
                        .px_2p5()
                        .py_1p5()
                        .flex()
                        .items_center()
                        .when(col_ix > 0, |d| d.border_l_1().border_color(c(theme::BORDER)))
                        .when(spec.align == Align::Right, |d| d.justify_end())
                        .when(spec.align == Align::Center, |d| d.justify_center())
                        .text_xs()
                        .when(spec.numeric || cell.code || is_link, |d| d.font_family("Cascadia Code"))
                        .when(cell.bold || is_total, |d| {
                            d.font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG))
                        })
                        .when(!cell.bold && !is_total && is_link, |d| {
                            d.text_color(c(theme::ACCENT_TEXT))
                        })
                        .when(!cell.bold && !is_total && !is_link, |d| {
                            d.text_color(c(if spec.numeric { theme::FG_STRONG } else { theme::FG }))
                        })
                        .when(!copy_val.is_empty(), |d| {
                            d.cursor_pointer()
                                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    if let Some(p) = &open_path {
                                        this.jump_to_file_smooth(p.clone(), None, w, cx);
                                    } else {
                                        cx.write_to_clipboard(ClipboardItem::new_string(copy_val.clone()));
                                        this.status = crate::trf!("Copied: {0}"; 0 => copy_val.clone()).into();
                                        cx.notify();
                                    }
                                }))
                        })
                        .child(div().whitespace_nowrap().child(cell.text))
                }))
        };

        let mut rendered_rows: Vec<AnyElement> = Vec::with_capacity(table.rows.len());
        for (pos, &rix) in order.iter().enumerate() {
            let is_last = pos + 1 == order.len() && total_ix.is_none();
            rendered_rows.push(render_row(rix, false, is_last).into_any_element());
        }
        if let Some(tix) = total_ix {
            rendered_rows.push(render_row(tix, true, true).into_any_element());
        }

        v_flex()
            .w_full()
            .min_w_0()
            .my_1p5()
            .rounded(px(4.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_SIDE))
            .overflow_hidden()
            .child(top_bar)
            .child(
                div()
                    .id(SharedString::from(format!("ag-tbl-scroll-{item_ix}-{table_ix}")))
                    .w_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .child(
                        v_flex()
                            .min_w_full()
                            .w(px(total_min_w))
                            .child(header_row)
                            .children(rendered_rows),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_col_specs, sorted_indices, split_markdown_tables, table_to_csv, table_to_markdown, Align, Segment};

    #[test]
    fn splits_markdown_and_tables_respecting_code_fences() {
        let src = "\
### Ringkasan

| Folder | Size | Files |
| --- | ---: | ---: |
| resource_packs | 33.71 MB | 5307 |
| behavior_packs | 5.68 MB | 379 |
| **total** | **~41.2 MB** | **5796** |

```markdown
| Not | A | Table |
| --- | --- | --- |
| 1 | 2 | 3 |
```

Footer note.";
        let segs = split_markdown_tables(src);
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], Segment::Markdown(s) if s.contains("Ringkasan")));
        let Segment::Table(t) = &segs[1] else { panic!("expected table") };
        assert_eq!(t.headers, vec!["Folder", "Size", "Files"]);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.aligns, vec![Align::Left, Align::Right, Align::Right]);
        assert!(matches!(&segs[2], Segment::Markdown(s) if s.contains("Not | A | Table") && s.contains("Footer note.")));
    }

    #[test]
    fn sorts_byte_sizes_and_numbers_numerically_with_pinned_total() {
        let src = "\
| Folder | Size | Files |
| --- | --- | --- |
| db | 1.60 MB | 4 |
| resource_packs | 33.71 MB | 5307 |
| tools | 240 KB | 106 |
| behavior_packs | 5.68 MB | 379 |
| **total** | **~41.2 MB** | **5796** |";
        let Segment::Table(t) = &split_markdown_tables(src)[0] else { panic!("expected table") };
        let (desc, total_ix) = sorted_indices(t, Some((1, false)));
        assert_eq!(total_ix, Some(4));
        let names: Vec<&str> = desc.iter().map(|&i| t.rows[i][0].as_str()).collect();
        assert_eq!(names, vec!["resource_packs", "behavior_packs", "db", "tools"]);

        let (asc_files, _) = sorted_indices(t, Some((2, true)));
        let by_files: Vec<&str> = asc_files.iter().map(|&i| t.rows[i][0].as_str()).collect();
        assert_eq!(by_files, vec!["db", "tools", "behavior_packs", "resource_packs"]);
    }

    #[test]
    fn numeric_columns_right_align_and_never_squish() {
        let src = "\
| Size | File |
| --- | --- |
| 2.16 MB | KiwHitAnimation_RP\\models\\entity\\kiwha\\kiwha_cut_shade.geo.json |
| 1.13 MB | db\\000299.log |";
        let Segment::Table(t) = &split_markdown_tables(src)[0] else { panic!("expected table") };
        let specs = compute_col_specs(t);
        assert!(specs[0].numeric);
        assert_eq!(specs[0].align, Align::Right);
        assert!(specs[0].min_w >= 80.0, "size column should fit '2.16 MB' on one line: {}", specs[0].min_w);
        assert!(!specs[1].numeric);
        assert!(specs[1].flex_grow);
        assert!(specs[1].min_w >= 200.0);
    }

    #[test]
    fn exports_markdown_and_csv_cleanly() {
        let src = "| A | B |\n| --- | ---: |\n| hello, world | **42** |";
        let Segment::Table(t) = &split_markdown_tables(src)[0] else { panic!("expected table") };
        assert_eq!(table_to_csv(t), "A,B\n\"hello, world\",42\n");
        assert!(table_to_markdown(t).contains("| A | B |"));
    }
}
