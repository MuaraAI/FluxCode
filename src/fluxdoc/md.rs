//! A Flux document as GitHub Markdown, so a `README.flux` can show on GitHub as the
//! `README.md` made from it. What GitHub has, Flux maps to; what it doesn't, degrades:
//!
//! | Flux | GitHub Markdown |
//! | --- | --- |
//! | `@flow` | a ` ```mermaid ` flowchart (GitHub draws it) |
//! | `@note`, `@tip`, `@warn`, `@danger` | `> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`, `> [!CAUTION]` |
//! | `@details` | `<details><summary>` |
//! | `@table` | a pipe table with the same alignment |
//! | `@image ... width=` | `<img width=...>` |
//! | `@video` | the poster image, linked to the video |
//! | `{kbd X}` | `<kbd>X</kbd>` |
//! | `@toc` | a list of links to the headings |
//! | `@style`, `{color=...}`, `==mark==` | dropped (GitHub has no colors): the text stays |

use super::{arg, args, cells, flow, text, Align, Block, Doc, Kind};

/// The first line of a Markdown file made from a Flux one.
pub fn banner(input: &str, output: &str) -> String {
    format!("<!-- Made from {input} by FluxCode (fluxcode --flux-to-md {input} {output}). Edit {input}, not this file. -->")
}

/// `src` as GitHub Markdown, with the sections for `lang` ("en" or "id").
pub fn to_markdown(doc: &Doc, src: &str, lang: &str, banner: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(b) = banner {
        out.push_str(b);
        out.push_str("\n\n");
    }
    let blocks: Vec<Block> = doc.visible(lang).into_iter().map(|i| doc.blocks[i as usize]).collect();
    emit(doc, src, &blocks, &mut out, "");
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// GitHub's anchor for a heading.
pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in super::inline(title).text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            s.push(c);
        } else if c == ' ' {
            s.push('-');
        }
    }
    s
}

/// Inline Flux marks that Markdown lacks, rewritten; the rest stays as written.
pub fn inline_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(['{', '=', '`']) {
        out.push_str(&rest[..i]);
        let r = &rest[i..];
        // Code stays exactly as written.
        if r.starts_with('`') {
            let n = r.bytes().take_while(|&b| b == b'`').count();
            let fence = &r[..n];
            match r[n..].find(fence) {
                Some(e) => {
                    out.push_str(&r[..n + e + n]);
                    rest = &r[n + e + n..];
                }
                None => {
                    out.push_str(r);
                    rest = "";
                }
            }
            continue;
        }
        if let Some(body) = r.strip_prefix("==") {
            if let Some(e) = body.find("==") {
                out.push_str(&format!("**{}**", &body[..e]));
                rest = &body[e + 2..];
                continue;
            }
        }
        if r.starts_with('{') {
            if let Some(e) = r.find('}') {
                let inner = &r[1..e];
                let (kind, val) = inner.split_once(' ').unwrap_or((inner, ""));
                let done = match kind {
                    "kbd" => Some(format!("<kbd>{val}</kbd>")),
                    "badge" => Some(format!("`{val}`")),
                    k if k.starts_with("color=") => Some(val.to_string()),
                    _ => None,
                };
                if let Some(d) = done {
                    out.push_str(&d);
                    rest = &r[e + 1..];
                    continue;
                }
            }
        }
        out.push_str(&r[..1]);
        rest = &r[1..];
    }
    out.push_str(rest);
    out
}

fn line(out: &mut String, prefix: &str, s: &str) {
    for l in s.lines() {
        out.push_str(prefix);
        out.push_str(l);
        out.push('\n');
    }
    if s.is_empty() {
        out.push_str(prefix.trim_end());
        out.push('\n');
    }
}

fn emit(doc: &Doc, src: &str, blocks: &[Block], out: &mut String, prefix: &str) {
    for (bi, b) in blocks.iter().enumerate() {
        // List items run on after list items, and rows after rows of the same table;
        // everything else is a paragraph of its own.
        let runs_on = match (bi.checked_sub(1).map(|p| &blocks[p]), b) {
            (Some(Block::Item { .. }), Block::Item { .. }) => true,
            (Some(Block::Row { table: a, .. }), Block::Row { table: z, header: false, .. }) => a == z,
            _ => false,
        };
        if bi > 0 && !runs_on {
            line(out, prefix, "");
        }
        match *b {
            Block::Title(s) => line(out, prefix, &format!("# {}", inline_md(text(src, s)))),
            Block::Heading(l, s) => line(out, prefix, &format!("{} {}", "#".repeat(l as usize), inline_md(text(src, s)))),
            Block::Para(s) => line(out, prefix, &inline_md(text(src, s))),
            Block::Quote(s) => line(out, prefix, &inline_md(text(src, s))),
            Block::Item { num, depth, check, text: s } => {
                let mark = if num > 0 { format!("{num}.") } else { "-".into() };
                let bx = match check {
                    1 => "[ ] ",
                    2 => "[x] ",
                    _ => "",
                };
                line(out, prefix, &format!("{}{mark} {bx}{}", "  ".repeat(depth as usize), inline_md(text(src, s))));
            }
            Block::Code { lang, body } => {
                line(out, prefix, &format!("```{}", text(src, lang).trim()));
                line(out, prefix, text(src, body));
                line(out, prefix, "```");
            }
            Block::Rule => line(out, prefix, "---"),
            Block::Row { table, line: l, header, n } => {
                let t = &doc.tables[table as usize];
                let cs: Vec<String> = cells(src, l, t.pipes).iter().map(|c| inline_md(text(src, *c))).collect();
                if n == 0 && !header {
                    // GitHub tables need a header row: an empty one.
                    line(out, prefix, &format!("|{}", " |".repeat(t.cols)));
                    line(out, prefix, &sep(t));
                }
                line(out, prefix, &format!("|{}|", cs.iter().map(|c| if c.is_empty() { " ".to_string() } else { format!(" {c} ") }).collect::<Vec<_>>().join("|")));
                if header {
                    line(out, prefix, &sep(t));
                }
            }
            Block::Image(a) => {
                line(out, prefix, &image_md(src, a));
                let a = args(src, a);
                if let Some(c) = arg(src, &a, "caption") {
                    line(out, prefix, "");
                    line(out, prefix, &format!("*{}*", inline_md(c)));
                }
            }
            Block::Video(a) => {
                let a = args(src, a);
                let path = arg(src, &a, "").unwrap_or("");
                let alt = arg(src, &a, "alt").unwrap_or("Video");
                match arg(src, &a, "poster") {
                    Some(p) => line(out, prefix, &format!("[![{alt}]({p})]({path})")),
                    None => line(out, prefix, &format!("[▶ {alt}]({path})")),
                }
            }
            Block::Flow(f) => {
                line(out, prefix, "```mermaid");
                line(out, prefix, flow::to_mermaid(&doc.flows[f as usize]).trim_end());
                line(out, prefix, "```");
            }
            Block::Boxed { kind, title, inner } => {
                let inner = &doc.inner[inner as usize];
                let title = text(src, title).trim();
                match kind {
                    Kind::Details => {
                        line(out, prefix, &format!("<details><summary>{}</summary>", if title.is_empty() { "Details" } else { title }));
                        line(out, prefix, "");
                        emit(doc, src, inner, out, prefix);
                        line(out, prefix, "");
                        line(out, prefix, "</details>");
                    }
                    Kind::Columns => emit(doc, src, inner, out, prefix),
                    Kind::Gallery => {
                        line(out, prefix, "<p align=\"center\">");
                        for b in inner {
                            if let Block::Image(a) = b {
                                line(out, prefix, &format!("  {}", image_md(src, *a)));
                            }
                        }
                        line(out, prefix, "</p>");
                    }
                    _ => {
                        let tag = match kind {
                            Kind::Tip => "TIP",
                            Kind::Warn => "WARNING",
                            Kind::Danger => "CAUTION",
                            _ => "NOTE",
                        };
                        let q = format!("{prefix}> ");
                        line(out, &q, &format!("[!{tag}]"));
                        if !title.is_empty() {
                            line(out, &q, &format!("**{}**", inline_md(title)));
                        }
                        let mut body = String::new();
                        emit(doc, src, inner, &mut body, "");
                        line(out, &q, body.trim_end());
                    }
                }
            }
            Block::ColBreak => {}
            Block::Toc => {
                for &h in &doc.headings {
                    if let Block::Heading(l, s) = doc.blocks[h as usize] {
                        if l <= 3 {
                            let t = text(src, s);
                            line(out, prefix, &format!("{}- [{}](#{})", "  ".repeat(l.saturating_sub(2) as usize), super::inline(t).text, slug(t)));
                        }
                    }
                }
            }
            Block::Release(s) => line(out, prefix, &format!("## {}", text(src, s))),
            Block::Unknown(s) => line(out, prefix, &format!("<!-- {} -->", text(src, s).replace("--", "- -"))),
        }
    }
}

/// An image in GitHub Markdown: `![alt](path)` when that says it all, else HTML for a
/// width, alignment, link or dark-theme version (`<picture>`).
fn image_md(src: &str, a: super::Span) -> String {
    let a = args(src, a);
    let path = arg(src, &a, "").unwrap_or("");
    let alt = arg(src, &a, "alt").unwrap_or("").replace('"', "&quot;");
    let (width, align, link, dark) = (arg(src, &a, "width"), arg(src, &a, "align"), arg(src, &a, "link"), arg(src, &a, "dark"));
    if width.is_none() && align.is_none() && link.is_none() && dark.is_none() {
        return format!("![{alt}]({path})");
    }
    let mut img = format!("<img src=\"{path}\"");
    if let Some(w) = width {
        img.push_str(&format!(" width=\"{w}\""));
    }
    if let Some(al) = align {
        img.push_str(&format!(" align=\"{al}\""));
    }
    img.push_str(&format!(" alt=\"{alt}\">"));
    if let Some(d) = dark {
        img = format!("<picture>\n  <source media=\"(prefers-color-scheme: dark)\" srcset=\"{d}\">\n  {img}\n</picture>");
    }
    match link {
        Some(l) => format!("<a href=\"{l}\">{img}</a>"),
        None => img,
    }
}

fn sep(t: &super::Table) -> String {
    let cols: Vec<&str> = (0..t.cols)
        .map(|i| match t.align.get(i) {
            Some(Align::Center) => ":---:",
            Some(Align::Right) => "---:",
            _ => "---",
        })
        .collect();
    format!("| {} |", cols.join(" | "))
}

/// `fluxcode --flux-to-md IN.flux OUT.md [--check]`: write OUT.md, or with `--check`
/// only say whether it is up to date (exit code 1 when not). For CI and scripts.
pub fn cli(args: &[String]) -> i32 {
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let check = args.iter().any(|a| a == "--check");
    let (Some(input), Some(output)) = (files.first(), files.get(1)) else {
        eprintln!("usage: fluxcode --flux-to-md IN.flux OUT.md [--check]");
        return 2;
    };
    let src = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{input}: {e}");
            return 2;
        }
    };
    let md = to_markdown(&super::parse(&src), &src, "en", Some(&banner(&input.replace('\\', "/"), &output.replace('\\', "/"))));
    if check {
        let have = std::fs::read_to_string(output).unwrap_or_default().replace("\r\n", "\n");
        if have == md {
            return 0;
        }
        eprintln!("{output} is out of date: run fluxcode --flux-to-md {input} {output}");
        return 1;
    }
    match std::fs::write(output, md) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{output}: {e}");
            2
        }
    }
}
