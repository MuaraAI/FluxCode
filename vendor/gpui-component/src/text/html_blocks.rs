use std::borrow::Cow;

use gpui::{div, AnyElement, App, IntoElement as _, ParentElement as _, Window};
use gpui_base::text::{markdown_ast, MarkdownNode, MarkdownParseContext};

pub(crate) const HTML_FALLBACK_NODE: &str = "flux-html-fallback";

pub fn normalize_markdown_html(md: &str) -> Cow<'_, str> {
    if !md.contains('<') {
        return Cow::Borrowed(md);
    }

    let lines: Vec<&str> = md.split_inclusive('\n').collect();
    let mut out = String::with_capacity(md.len());
    let mut fence: Option<(u8, usize)> = None;
    let mut changed = false;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_end_matches(['\r', '\n']);

        if let Some((fc, flen)) = fence {
            out.push_str(line);
            if is_closing_fence(trimmed, fc, flen) {
                fence = None;
            }
            i += 1;
            continue;
        }

        if let Some(next_fence) = opening_fence(trimmed) {
            fence = Some(next_fence);
            out.push_str(line);
            i += 1;
            continue;
        }

        if is_html_block_start(trimmed) {
            let start = i;
            let mut end = i + 1;
            let mut extend_tag = open_multiline_block_tag(trimmed);

            while end < lines.len() && end - start < 240 {
                let next_trimmed = lines[end].trim_end_matches(['\r', '\n']);
                if opening_fence(next_trimmed).is_some() {
                    break;
                }
                if next_trimmed.trim().is_empty() {
                    if let Some(tag) = extend_tag {
                        if contains_closing_tag(&lines[start..end].concat(), tag) {
                            break;
                        }
                        end += 1;
                        continue;
                    }
                    break;
                }
                if extend_tag.is_none() {
                    extend_tag = open_multiline_block_tag(next_trimmed);
                }
                end += 1;
            }

            let chunk = lines[start..end].concat();
            let converted = transform_html_chunk(&chunk);
            if converted != chunk {
                changed = true;
            }
            push_block_text(&mut out, &converted);
            i = end;
            continue;
        }

        out.push_str(line);
        i += 1;
    }

    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(md)
    }
}

pub(crate) fn fallback_html_block_parser(
    node: &markdown_ast::Node,
    _cx: &MarkdownParseContext<'_>,
) -> Option<MarkdownNode> {
    let markdown_ast::Node::Html(raw) = node else {
        return None;
    };
    let text = decode_entities(&strip_all_tags(&strip_ignored_sections(&raw.value)));
    let clean = text.trim().to_string();
    Some(MarkdownNode::new(HTML_FALLBACK_NODE, clean.clone()).text(clean))
}

pub(crate) fn render_fallback_html_block(
    node: &MarkdownNode,
    _window: &mut Window,
    _cx: &mut App,
) -> AnyElement {
    let text = node.as_text();
    if text.trim().is_empty() {
        div().into_any_element()
    } else {
        div().child(text.to_string()).into_any_element()
    }
}

fn push_block_text(out: &mut String, block: &str) {
    let trimmed = block.trim();
    if trimmed.is_empty() {
        return;
    }
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
    }
    out.push_str(trimmed);
    out.push_str("\n\n");
}

fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let s = strip_up_to_3_spaces(line);
    let first = *s.as_bytes().first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let count = s.bytes().take_while(|&b| b == first).count();
    if count < 3 {
        return None;
    }
    if first == b'`' && s[count..].contains('`') {
        return None;
    }
    Some((first, count))
}

fn is_closing_fence(line: &str, fc: u8, min_len: usize) -> bool {
    let s = strip_up_to_3_spaces(line);
    let count = s.bytes().take_while(|&b| b == fc).count();
    count >= min_len && s[count..].trim().is_empty()
}

fn strip_up_to_3_spaces(s: &str) -> &str {
    let n = s.bytes().take(3).take_while(|&b| b == b' ').count();
    &s[n..]
}

fn is_html_block_start(line: &str) -> bool {
    let s = strip_up_to_3_spaces(line);
    if !s.starts_with('<') || s.starts_with("<-") {
        return false;
    }
    let rest = &s[1..];
    if rest.starts_with("!--") || rest.starts_with('?') || rest.starts_with('!') {
        return true;
    }
    let name = rest.strip_prefix('/').unwrap_or(rest);
    let tag_len = name.bytes().take_while(u8::is_ascii_alphanumeric).count();
    if tag_len == 0 {
        return false;
    }
    let tag = &name[..tag_len];
    if is_block_tag_name(tag) {
        return matches!(
            name.as_bytes().get(tag_len),
            None | Some(b' ' | b'\t' | b'>' | b'/')
        );
    }
    // CommonMark type-7 HTML block: standalone tag(s) filling the entire trimmed line.
    if matches!(
        tag.to_ascii_lowercase().as_str(),
        "img" | "a" | "span" | "strong" | "b" | "em" | "i" | "code" | "br" | "picture" | "source"
    ) {
        if let Some(close_pos) = s.rfind('>') {
            return s[close_pos + 1..].trim().is_empty();
        }
    }
    false
}

fn open_multiline_block_tag(line: &str) -> Option<&'static str> {
    let lower = strip_up_to_3_spaces(line).to_ascii_lowercase();
    for tag in [
        "table", "h1", "h2", "h3", "h4", "h5", "h6", "p", "picture", "figure",
    ] {
        let pat = format!("<{tag}");
        if let Some(pos) = lower.find(&pat) {
            let after = lower.as_bytes().get(pos + pat.len()).copied();
            if matches!(after, None | Some(b' ' | b'\t' | b'>'))
                && !contains_closing_tag(&lower[pos..], tag)
            {
                return Some(tag);
            }
        }
    }
    None
}

fn contains_closing_tag(s: &str, tag: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains(&format!("</{tag}>"))
        || lower.contains(&format!("</{tag} "))
        || lower.contains(&format!("</{tag}\t"))
}

fn is_block_tag_name(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "h1" | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "p"
            | "div"
            | "center"
            | "details"
            | "summary"
            | "picture"
            | "figure"
            | "figcaption"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "main"
            | "nav"
            | "aside"
            | "blockquote"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "th"
            | "td"
            | "ul"
            | "ol"
            | "li"
            | "dl"
            | "dt"
            | "dd"
            | "hr"
            | "pre"
            | "style"
            | "script"
    )
}

fn transform_html_chunk(chunk: &str) -> String {
    let cleaned = strip_ignored_sections(chunk);
    if cleaned.trim().is_empty() {
        return String::new();
    }
    let with_tables = convert_tables(&cleaned);
    let with_blocks = convert_block_tags(&with_tables);
    finalize_block_lines(&with_blocks)
}

fn strip_ignored_sections(input: &str) -> String {
    let s = strip_delimited(input, "<!--", "-->");
    let s = strip_tag_section(&s, "style");
    strip_tag_section(&s, "script")
}

fn strip_delimited(input: &str, start_pat: &str, end_pat: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(s) = rest.find(start_pat) {
        out.push_str(&rest[..s]);
        let after = &rest[s + start_pat.len()..];
        if let Some(e) = after.find(end_pat) {
            rest = &after[e + end_pat.len()..];
        } else {
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn strip_tag_section(input: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while pos < input.len() {
        let lower = input[pos..].to_ascii_lowercase();
        let Some(rel_s) = lower.find(&open) else {
            out.push_str(&input[pos..]);
            break;
        };
        let next_ch = lower.as_bytes().get(rel_s + open.len()).copied();
        if !matches!(
            next_ch,
            None | Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')
        ) {
            out.push_str(&input[pos..pos + rel_s + open.len()]);
            pos += rel_s + open.len();
            continue;
        }
        out.push_str(&input[pos..pos + rel_s]);
        let after = pos + rel_s + open.len();
        if let Some(rel_e) = input[after..].to_ascii_lowercase().find(&close) {
            pos = after + rel_e + close.len();
        } else {
            break;
        }
    }
    out
}

fn convert_tables(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while pos < input.len() {
        let lower = input[pos..].to_ascii_lowercase();
        let Some((rel_open, inner_start, inner_end, after_close)) =
            find_paired_tag(&lower, "table")
        else {
            out.push_str(&input[pos..]);
            break;
        };
        out.push_str(&input[pos..pos + rel_open]);
        let inner = &input[pos + inner_start..pos + inner_end];
        let table_md = table_to_markdown(inner);
        out.push_str("\n\n");
        out.push_str(&table_md);
        out.push_str("\n\n");
        pos += after_close;
    }
    out
}

fn table_to_markdown(inner: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let lower = inner.to_ascii_lowercase();
    let mut pos = 0;
    while pos < inner.len() {
        let Some((_, row_start, row_end, row_after)) = find_paired_tag(&lower[pos..], "tr") else {
            break;
        };
        let row_src = &inner[pos + row_start..pos + row_end];
        let mut cells = Vec::new();
        let row_lower = row_src.to_ascii_lowercase();
        let mut cpos = 0;
        while cpos < row_src.len() {
            let next_td = find_paired_tag(&row_lower[cpos..], "td");
            let next_th = find_paired_tag(&row_lower[cpos..], "th");
            let picked = match (next_td, next_th) {
                (Some(a), Some(b)) => {
                    if a.0 <= b.0 {
                        Some(a)
                    } else {
                        Some(b)
                    }
                }
                (a, b) => a.or(b),
            };
            let Some((_, cs, ce, ca)) = picked else {
                break;
            };
            let cell_md = inline_html_to_md(&row_src[cpos + cs..cpos + ce], true)
                .replace('|', "\\|")
                .trim()
                .to_string();
            cells.push(cell_md);
            cpos += ca;
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
        pos += row_after;
    }
    if rows.is_empty() {
        return convert_block_tags(inner);
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let mut md = String::new();
    for (r_ix, row) in rows.iter().enumerate() {
        md.push('|');
        for col in 0..cols {
            let cell = row.get(col).map(String::as_str).unwrap_or("");
            md.push(' ');
            md.push_str(cell);
            md.push_str(" |");
        }
        md.push('\n');
        if r_ix == 0 {
            md.push('|');
            for _ in 0..cols {
                md.push_str(" --- |");
            }
            md.push('\n');
        }
    }
    md
}

fn convert_block_tags(input: &str) -> String {
    let mut s = input.to_string();
    for (tag, level) in [
        ("h1", 1usize),
        ("h2", 2),
        ("h3", 3),
        ("h4", 4),
        ("h5", 5),
        ("h6", 6),
    ] {
        s = replace_paired_blocks(&s, tag, |inner| {
            let text = inline_html_to_md(inner, true);
            let t = text.trim();
            if t.is_empty() {
                String::new()
            } else {
                format!("\n\n{} {t}\n\n", "#".repeat(level))
            }
        });
    }
    s = replace_paired_blocks(&s, "summary", |inner| {
        let text = inline_html_to_md(inner, true);
        let t = text.trim();
        if t.is_empty() {
            String::new()
        } else if t.contains('<') || t.contains("![") {
            format!("\n\n{t}\n\n")
        } else {
            format!("\n\n**{t}**\n\n")
        }
    });
    s = replace_paired_blocks(&s, "blockquote", |inner| {
        let converted = convert_block_tags(inner);
        let quoted: Vec<String> = converted
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| format!("> {l}"))
            .collect();
        if quoted.is_empty() {
            String::new()
        } else {
            format!("\n\n{}\n\n", quoted.join("\n"))
        }
    });
    s = replace_paired_blocks(&s, "ol", |inner| convert_list_items(inner, true));
    s = replace_paired_blocks(&s, "ul", |inner| convert_list_items(inner, false));
    s = replace_paired_blocks(&s, "li", |inner| {
        let text = inline_html_to_md(inner, true);
        let t = text.trim();
        if t.is_empty() {
            String::new()
        } else {
            format!("\n- {t}\n")
        }
    });
    s = replace_paired_blocks(&s, "p", |inner| {
        let text = inline_html_to_md(inner, false);
        let t = text.trim();
        if t.is_empty() {
            String::new()
        } else {
            format!("\n\n{t}\n\n")
        }
    });
    s = unwrap_container_tags(&s);
    inline_html_to_md(&s, false)
}

fn convert_list_items(inner: &str, ordered: bool) -> String {
    let mut items = Vec::new();
    let lower = inner.to_ascii_lowercase();
    let mut pos = 0;
    while pos < inner.len() {
        let Some((_, is, ie, ia)) = find_paired_tag(&lower[pos..], "li") else {
            break;
        };
        let t = inline_html_to_md(&inner[pos + is..pos + ie], true);
        let t = t.trim().to_string();
        if !t.is_empty() {
            items.push(t);
        }
        pos += ia;
    }
    if items.is_empty() {
        let fallback = inline_html_to_md(inner, false);
        return format!("\n\n{}\n\n", fallback.trim());
    }
    let mut out = String::from("\n\n");
    for (idx, item) in items.iter().enumerate() {
        if ordered {
            out.push_str(&format!("{}. {item}\n", idx + 1));
        } else {
            out.push_str(&format!("- {item}\n"));
        }
    }
    out.push('\n');
    out
}

fn replace_paired_blocks<F>(input: &str, tag: &str, mut f: F) -> String
where
    F: FnMut(&str) -> String,
{
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while pos < input.len() {
        let lower = input[pos..].to_ascii_lowercase();
        let Some((rel_open, inner_start, inner_end, after_close)) = find_paired_tag(&lower, tag)
        else {
            out.push_str(&input[pos..]);
            break;
        };
        out.push_str(&input[pos..pos + rel_open]);
        out.push_str(&f(&input[pos + inner_start..pos + inner_end]));
        pos += after_close;
    }
    out
}

fn unwrap_container_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let Some(rel) = input[i..].find('<') else {
            out.push_str(&input[i..]);
            break;
        };
        out.push_str(&input[i..i + rel]);
        let start = i + rel;
        let Some(tag_end) = find_tag_end(&input[start..]) else {
            out.push_str(&input[start..]);
            break;
        };
        let raw_tag = &input[start..start + tag_end];
        if let Some((closing, name)) = parse_tag_name(raw_tag) {
            let lower = name.to_ascii_lowercase();
            if lower == "hr" {
                out.push_str("\n\n---\n\n");
                i = start + tag_end;
                continue;
            }
            if matches!(
                lower.as_str(),
                "div"
                    | "center"
                    | "details"
                    | "section"
                    | "article"
                    | "header"
                    | "footer"
                    | "main"
                    | "nav"
                    | "aside"
                    | "figure"
                    | "figcaption"
                    | "dl"
                    | "dt"
                    | "dd"
                    | "tbody"
                    | "thead"
                    | "tfoot"
                    | "tr"
                    | "td"
                    | "th"
            ) {
                let _ = closing;
                out.push_str("\n\n");
                i = start + tag_end;
                continue;
            }
            if matches!(
                lower.as_str(),
                "picture" | "source" | "p" | "ul" | "ol" | "li"
            ) {
                i = start + tag_end;
                continue;
            }
        }
        out.push_str(raw_tag);
        i = start + tag_end;
    }
    out
}

fn inline_html_to_md(input: &str, single_line: bool) -> String {
    let mut s = rewrite_anchors(input, single_line);
    for (tag, wrap) in [("strong", "**"), ("b", "**"), ("em", "*"), ("i", "*")] {
        s = replace_paired_blocks(&s, tag, |inner| {
            let inner = inline_html_to_md(inner, single_line);
            let t = inner.trim();
            if t.is_empty() {
                String::new()
            } else {
                format!("{wrap}{t}{wrap}")
            }
        });
    }
    for tag in ["del", "s", "strike"] {
        s = replace_paired_blocks(&s, tag, |inner| {
            let inner = inline_html_to_md(inner, single_line);
            let t = inner.trim();
            if t.is_empty() {
                String::new()
            } else {
                format!("~~{t}~~")
            }
        });
    }
    for tag in ["code", "kbd"] {
        s = replace_paired_blocks(&s, tag, |inner| {
            let plain = decode_entities(&strip_all_tags(inner));
            let t = plain.trim();
            if t.is_empty() {
                String::new()
            } else if t.contains('`') {
                format!("`` {t} ``")
            } else {
                format!("`{t}`")
            }
        });
    }

    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let Some(rel) = s[i..].find('<') else {
            out.push_str(&decode_entities(&s[i..]));
            break;
        };
        out.push_str(&decode_entities(&s[i..i + rel]));
        let start = i + rel;
        let Some(tag_end) = find_tag_end(&s[start..]) else {
            out.push_str(&decode_entities(&s[start..]));
            break;
        };
        let raw_tag = &s[start..start + tag_end];
        if let Some((closing, name)) = parse_tag_name(raw_tag) {
            let lower = name.to_ascii_lowercase();
            if lower == "img" && !closing {
                out.push_str(&normalize_img_tag(raw_tag, false));
                i = start + tag_end;
                continue;
            }
            if lower == "br" {
                out.push_str(if single_line { " " } else { "<br/>" });
                i = start + tag_end;
                continue;
            }
            if matches!(
                lower.as_str(),
                "span"
                    | "font"
                    | "sub"
                    | "sup"
                    | "u"
                    | "ins"
                    | "small"
                    | "mark"
                    | "picture"
                    | "source"
            ) {
                i = start + tag_end;
                continue;
            }
        }
        // Drop unrecognized raw tags to prevent stray CommonMark HTML blocks.
        if parse_tag_name(raw_tag).is_none() {
            out.push_str(raw_tag);
        }
        i = start + tag_end;
    }

    if single_line {
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        out
    }
}

fn rewrite_anchors(input: &str, single_line: bool) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while pos < input.len() {
        let lower = input[pos..].to_ascii_lowercase();
        let Some((rel_open, inner_start, inner_end, after_close)) = find_paired_tag(&lower, "a")
        else {
            out.push_str(&input[pos..]);
            break;
        };
        out.push_str(&input[pos..pos + rel_open]);
        let open_end = input[pos + rel_open..]
            .find('>')
            .map_or(pos + inner_start, |e| pos + rel_open + e + 1);
        let open_tag = &input[pos + rel_open..open_end];
        let inner = &input[pos + inner_start..pos + inner_end];
        let href = attr_value(open_tag, "href").map(|h| decode_entities(&h));

        let imgs = extract_img_tags(inner);
        let text_outside = decode_entities(&strip_all_tags(inner));
        let rendered = match href {
            Some(url) if !url.trim().is_empty() => {
                if !imgs.is_empty() && text_outside.trim().is_empty() {
                    imgs.iter()
                        .map(|img| format!("[{}]({url})", normalize_img_tag(img, true)))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    let label = inline_html_to_md(inner, true);
                    let label = label.trim();
                    if label.is_empty() {
                        url
                    } else {
                        format!("[{label}]({url})")
                    }
                }
            }
            _ => inline_html_to_md(inner, single_line),
        };
        out.push_str(&rendered);
        pos += after_close;
    }
    out
}

fn extract_img_tags(s: &str) -> Vec<&str> {
    let mut imgs = Vec::new();
    let lower = s.to_ascii_lowercase();
    let mut i = 0;
    while i < lower.len() {
        let Some(rel) = lower[i..].find("<img") else {
            break;
        };
        let start = i + rel;
        let next = lower.as_bytes().get(start + 4).copied();
        if !matches!(next, Some(b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>')) {
            i = start + 4;
            continue;
        }
        let Some(end) = find_tag_end(&s[start..]) else {
            break;
        };
        imgs.push(&s[start..start + end]);
        i = start + end;
    }
    imgs
}

fn normalize_img_tag(raw_tag: &str, force_markdown: bool) -> String {
    let Some(src) = attr_value(raw_tag, "src").map(|v| decode_entities(v.trim())) else {
        return String::new();
    };
    if src.is_empty() {
        return String::new();
    }
    let alt = attr_value(raw_tag, "alt")
        .map(|a| decode_entities(&a).replace(['[', ']'], ""))
        .unwrap_or_default();
    let width = attr_value(raw_tag, "width").and_then(|v| parse_px_dim(&v));
    let height = attr_value(raw_tag, "height").and_then(|v| parse_px_dim(&v));
    let keep_html = !force_markdown
        && (width.is_some_and(|w| w < 500) || (width.is_none() && height.is_some()));

    if keep_html {
        let mut out = format!("<img src=\"{src}\"");
        if !alt.is_empty() {
            out.push_str(&format!(" alt=\"{}\"", alt.replace('"', "&quot;")));
        }
        if let Some(w) = width {
            out.push_str(&format!(" width=\"{w}\""));
        }
        if let Some(h) = height {
            out.push_str(&format!(" height=\"{h}\""));
        }
        out.push_str(" />");
        out
    } else {
        format!("![{alt}]({src})")
    }
}

fn parse_px_dim(raw: &str) -> Option<u32> {
    let digits: String = raw
        .trim()
        .trim_end_matches("px")
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let n = digits.parse::<u32>().ok()?;
    (n > 0).then_some(n)
}

fn finalize_block_lines(input: &str) -> String {
    let mut lines_out = Vec::new();
    let mut prev_blank = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_blank && !lines_out.is_empty() {
                lines_out.push(String::new());
            }
            prev_blank = true;
            continue;
        }
        prev_blank = false;
        // CommonMark Type 7 parses a line starting with bare `<img ... />` or `<br/>` as `Node::Html` (`span: None`).
        // Prefixing with a zero-width space keeps it in a spanned `Node::Paragraph`.
        if trimmed.starts_with("<img") || trimmed.starts_with("<br") {
            lines_out.push(format!("\u{200B}{trimmed}"));
        } else {
            lines_out.push(trimmed.to_string());
        }
    }

    lines_out.join("\n").trim().to_string()
}

fn find_paired_tag(lower: &str, tag: &str) -> Option<(usize, usize, usize, usize)> {
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}");
    let mut search = 0;

    let open_pos = loop {
        let rel = lower[search..].find(&open_pat)?;
        let pos = search + rel;
        let next = lower.as_bytes().get(pos + open_pat.len()).copied();
        if matches!(next, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')) {
            break pos;
        }
        search = pos + open_pat.len();
    };

    let open_end = find_tag_end(&lower[open_pos..])? + open_pos;
    if lower[open_pos..open_end].trim_end().ends_with("/>") {
        return Some((open_pos, open_end, open_end, open_end));
    }

    let mut depth = 1usize;
    let mut cur = open_end;
    while cur < lower.len() {
        let next_lt = lower[cur..].find('<')? + cur;
        if lower[next_lt..].starts_with(&close_pat) {
            let after = lower.as_bytes().get(next_lt + close_pat.len()).copied();
            if matches!(after, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>')) {
                if let Some(cend) = find_tag_end(&lower[next_lt..]) {
                    depth -= 1;
                    if depth == 0 {
                        return Some((open_pos, open_end, next_lt, next_lt + cend));
                    }
                    cur = next_lt + cend;
                    continue;
                }
            }
        } else if lower[next_lt..].starts_with(&open_pat) {
            let after = lower.as_bytes().get(next_lt + open_pat.len()).copied();
            if matches!(after, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>')) {
                if let Some(oend) = find_tag_end(&lower[next_lt..]) {
                    if !lower[next_lt..next_lt + oend].trim_end().ends_with("/>") {
                        depth += 1;
                    }
                    cur = next_lt + oend;
                    continue;
                }
            }
        }
        cur = next_lt + 1;
    }
    None
}

fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (idx, b) in s.bytes().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return Some(idx + 1),
            None => {}
        }
    }
    None
}

fn parse_tag_name(raw_tag: &str) -> Option<(bool, &str)> {
    let inner = raw_tag.strip_prefix('<')?.strip_suffix('>')?.trim_start();
    let (closing, rest) = match inner.strip_prefix('/') {
        Some(r) => (true, r.trim_start()),
        None => (false, inner),
    };
    let len = rest.bytes().take_while(u8::is_ascii_alphanumeric).count();
    (len > 0).then_some((closing, &rest[..len]))
}

pub(crate) fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let pat = name.to_ascii_lowercase();
    let mut from = 0;

    while let Some(rel) = lower[from..].find(&pat) {
        let pos = from + rel;
        let prev_ok = pos > 0 && bytes[pos - 1].is_ascii_whitespace();
        if prev_ok {
            let mut i = pos + pat.len();
            while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
                i += 1;
            }
            if bytes.get(i) == Some(&b'=') {
                i += 1;
                while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
                    i += 1;
                }
                let rest = tag.get(i..)?;
                return Some(match rest.as_bytes().first() {
                    Some(b'"') => rest[1..].split('"').next().unwrap_or("").to_string(),
                    Some(b'\'') => rest[1..].split('\'').next().unwrap_or("").to_string(),
                    _ => rest
                        .split(|c: char| c.is_ascii_whitespace() || c == '>')
                        .next()
                        .unwrap_or("")
                        .trim_end_matches('/')
                        .to_string(),
                });
            }
        }
        from = pos + pat.len();
    }
    None
}

fn strip_all_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let Some(rel) = s[i..].find('<') else {
            out.push_str(&s[i..]);
            break;
        };
        out.push_str(&s[i..i + rel]);
        let start = i + rel;
        if let Some(end) = find_tag_end(&s[start..]) {
            i = start + end;
        } else {
            out.push_str(&s[start..]);
            break;
        }
    }
    out
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use markdown::{mdast, ParseOptions};

    fn count_html_blocks(md: &str) -> usize {
        let Ok(root) = markdown::to_mdast(md, &ParseOptions::gfm()) else {
            return usize::MAX;
        };
        let mdast::Node::Root(r) = root else {
            return 0;
        };
        r.children
            .iter()
            .filter(|n| matches!(n, mdast::Node::Html(_)))
            .count()
    }

    #[test]
    fn normalizes_flow_icons_readme_without_html_blocks() {
        let flow = concat!(
            "<h1 align=\"center\">\n",
            "  <img width=\"160\" src=\"https://raw.githubusercontent.com/thang-nm/flow-icons/main/assets/logo.png\" />\n",
            "  <br/>\n",
            "  <a href=\"https://example.com/flow-icons\">Flow Icons</a>\n",
            "</h1>\n\n",
            "<p align=\"center\">\n",
            "💎 <strong>IMPORTANT</strong>: Use this carefully.\n",
            "</p>\n\n",
            "<p align=\"center\">\n",
            "  <a href=\"https://example.com\">\n",
            "    <img alt=\"v\" src=\"https://img.shields.io/a?x=1&amp;y=2\">&nbsp;<img alt=\"i\" src=\"https://img.shields.io/b\">\n",
            "  </a>\n",
            "</p>\n\n",
            "<picture>\n",
            "  <source media=\"(prefers-color-scheme: dark)\" srcset=\"dark.png\">\n",
            "  <img width=\"640\" src=\"https://raw.githubusercontent.com/thang-nm/flow-icons/main/assets/preview.png\">\n",
            "</picture>\n\n",
            "<hr>\n\n",
            "```jsonc\n",
            "// keep <p>literal</p> in code\n",
            "{ \"icon\": \"<img src='a'>\" }\n",
            "```\n",
        );

        let normalized = normalize_markdown_html(flow);
        assert_eq!(
            count_html_blocks(&normalized),
            0,
            "normalized:\n{normalized}"
        );
        assert!(normalized.contains("# "));
        assert!(normalized.contains("width=\"160\""));
        assert!(normalized.contains("**IMPORTANT**"));
        assert!(normalized.contains("https://img.shields.io/a?x=1&y=2"));
        assert!(normalized.contains("!["));
        assert!(normalized.contains("---"));
        assert!(normalized.contains("// keep <p>literal</p> in code"));
    }

    #[test]
    fn normalizes_details_catppuccin_and_html_tables() {
        let md = concat!(
            "<h3 align=\"center\">\n",
            "  <img src=\"a.png\" width=\"100\"/><br/>Catppuccin\n",
            "</h3>\n\n",
            "<details>\n",
            "  <summary><img src=\"icon.png\" width=\"24\"/> File Icons</summary>\n",
            "  <p>Description here</p>\n",
            "</details>\n\n",
            "<img width=\"0px\" src=\"hidden.png\">\n\n",
            "<table>\n",
            "  <tr><th>Icon</th><th>Code</th></tr>\n",
            "  <tr><td><img src=\"x.png\"/></td><td><code>foo | bar</code></td></tr>\n",
            "</table>\n",
        );

        let normalized = normalize_markdown_html(md);
        assert_eq!(
            count_html_blocks(&normalized),
            0,
            "normalized:\n{normalized}"
        );
        assert!(!normalized.contains("width=\"0\""));
        assert!(normalized.contains("| Icon | Code |"));
        assert!(normalized.contains("`foo \\| bar`"));
    }

    #[test]
    fn normalizes_unclosed_html_without_crashing() {
        let weird = "<unclosed\n<div>\n<p>tail text";
        let normalized = normalize_markdown_html(weird);
        assert!(normalized.contains("tail text"));
    }
}
