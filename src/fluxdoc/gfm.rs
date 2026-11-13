//! GitHub-flavored Markdown, bent into Flux so one renderer can draw it.
//!
//! `to_flux` is a pure, line-based rewrite: image links become `@image` directives,
//! GitHub's `> [!NOTE]` alerts become Flux callouts, a YAML front-matter block and
//! pure-HTML lines disappear, and inline HTML tags are unwrapped (their text stays).
//! Code fences are copied verbatim: a `![x](y)` line inside one is code, not a picture.
//! Everything else passes through unchanged.

pub fn to_flux(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut lines = src.lines().peekable();

    // A leading YAML front-matter block (--- ... ---) is metadata, not content.
    if lines.peek().is_some_and(|l| l.trim_end() == "---") {
        lines.next();
        for l in lines.by_ref() {
            if l.trim_end() == "---" {
                break;
            }
        }
    }

    let mut in_fence = false;
    let mut in_comment = false;
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let t = line.trim();
        if t.is_empty() {
            out.push('\n');
            continue;
        }
        // GitHub alert: a quote whose first line names a callout kind. The quoted lines
        // that follow are its body; the block ends at the first non-quote line, which
        // stays unread for the main loop.
        if let Some(kind) = alert_kind(t) {
            out.push_str(kind);
            out.push('\n');
            for b in alert_body(t, &mut lines) {
                out.push_str(&b);
                out.push('\n');
            }
            out.push_str("@end\n");
            continue;
        }
        if let Some(image) = image_directive(t) {
            out.push_str(&image);
            out.push('\n');
            continue;
        }
        // A line that is nothing but HTML tags carries no text: drop it. Otherwise the
        // tags come off the text itself, outside code spans.
        let stripped = strip_html(line, &mut in_comment);
        if stripped.is_empty() {
            continue;
        }
        out.push_str(&stripped);
        out.push('\n');
    }
    out
}

/// `> [!NOTE]` and friends, as the Flux directive that draws the same callout.
fn alert_kind(line: &str) -> Option<&'static str> {
    let t = line.trim().trim_start_matches('>').trim_start();
    for (marker, directive) in [
        ("[!NOTE]", "@note"),
        ("[!TIP]", "@tip"),
        ("[!WARNING]", "@warn"),
        ("[!CAUTION]", "@danger"),
    ] {
        if t.starts_with(marker) && t[marker.len()..].trim().is_empty() {
            return Some(directive);
        }
    }
    None
}

/// The quoted lines after an alert header, with their `>` markers taken off. A line
/// that does not start with `>` ends the block and is left for the main loop.
fn alert_body<'a, I: Iterator<Item = &'a str>>(header: &str, lines: &mut std::iter::Peekable<I>) -> Vec<String> {
    let mut body = Vec::new();
    let mut rest = header.trim().trim_start_matches('>').trim_start();
    for marker in ["[!NOTE]", "[!TIP]", "[!WARNING]", "[!CAUTION]"] {
        if let Some(r) = rest.strip_prefix(marker) {
            rest = r.trim_start();
        }
    }
    if !rest.is_empty() {
        body.push(rest.to_string());
    }
    while let Some(next) = lines.peek() {
        let t = next.trim();
        if !t.starts_with('>') {
            break;
        }
        let inner = t.trim_start_matches('>').trim_start().to_string();
        let _ = lines.next();
        body.push(inner);
    }
    body
}

/// `![alt](path)` and `[![alt](path)](link)` lines become `@image` directives.
fn image_directive(line: &str) -> Option<String> {
    let t = line.trim();
    // Only image lines get here, so the markup bytes exist: past `![` for a plain
    // image, past `[![` for a link around one.
    let nested = t.starts_with("[![");
    if !nested && !t.starts_with("![") {
        return None;
    }
    let (alt, path, rest) = bracket_target(t, if nested { 3 } else { 2 })?;
    // The nested form wraps the picture in a link: `](link)` must follow it.
    let link = if nested {
        let (l, leftover) = parens(rest.strip_prefix(']')?)?;
        if !leftover.is_empty() {
            return None;
        }
        Some(l)
    } else if !rest.is_empty() {
        return None;
    } else {
        None
    };
    Some(directive(&alt, path, link))
}

/// `alt` up to the next `]`, then `(target)`, in `t` after `skip` bytes. Returns
/// (alt, target, the text left over).
fn bracket_target(t: &str, skip: usize) -> Option<(String, &str, &str)> {
    let t = &t[skip..];
    let alt_close = t.find(']')?;
    let alt = t[..alt_close].trim();
    let (target, rest) = parens(&t[alt_close + 1..])?;
    Some((alt.to_string(), target, rest))
}

/// `(target)` at the start of `s`: the target and what follows. A quoted `"Title"`
/// after the path is dropped (GitHub display titles mean nothing here), and the
/// angle form `(<path>)` is tolerated.
fn parens(s: &str) -> Option<(&str, &str)> {
    let inner = s.strip_prefix('(')?;
    let close = inner.find(')')?;
    let mut target = inner[..close].trim();
    if let Some(q) = target.rfind(" \"") {
        if target.ends_with('"') {
            target = target[..q].trim_end();
        }
    }
    let target = target.strip_prefix('<').map(|p| p.strip_suffix('>').unwrap_or(p)).unwrap_or(target);
    if target.is_empty() {
        return None;
    }
    Some((target, &inner[close + 1..]))
}

fn directive(alt: &str, path: &str, link: Option<&str>) -> String {
    let mut out = format!("@image {path}");
    if !alt.is_empty() {
        out.push_str(&format!(" alt=\"{}\"", alt.replace('"', "'")));
    }
    if let Some(link) = link.filter(|l| !l.is_empty()) {
        out.push_str(&format!(" link=\"{}\"", link.replace('"', "'")));
    }
    out
}

/// HTML tags come off the text, their content stays; a `<` inside a code span
/// (`` `<div>` ``) is left alone, and so is a lone `<` that opens no tag. HTML
/// comments vanish, even across lines (`in_comment` carries the open one in).
fn strip_html(line: &str, in_comment: &mut bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    let mut chars = line.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if in_code {
            if ch == '`' {
                in_code = false;
            }
            out.push(ch);
            continue;
        }
        if ch == '`' {
            in_code = true;
            out.push(ch);
            continue;
        }
        if *in_comment {
            match line[i..].find("-->") {
                // The comment ends mid-line; the text after it stays. `rel` counts
                // from the current byte (already consumed), so it needs rel + 2 more.
                Some(rel) => {
                    for _ in 0..rel + 2 {
                        chars.next();
                    }
                    *in_comment = false;
                    continue;
                }
                None => return String::new(),
            }
        }
        if ch == '<' && line[i..].starts_with("<!--") {
            match line[i..].find("-->") {
                // Same arithmetic as above: the `<` is consumed, so rel + 2 more.
                Some(rel) => {
                    for _ in 0..rel + 2 {
                        chars.next();
                    }
                    continue;
                }
                // It stays open past this line: the rest of the line is comment,
                // the text before it still counts.
                None => {
                    *in_comment = true;
                    break;
                }
            }
        }
        if ch == '<' {
            // A tag opens with a letter or `/` right after `<`; anything else is text
            // (like `a < b` or `<3`).
            let opens_tag = chars.peek().is_some_and(|(_, n)| n.is_ascii_alphabetic() || *n == '/');
            if opens_tag {
                if let Some(end) = line[i..].find('>') {
                    for _ in 0..end {
                        chars.next();
                    }
                    continue;
                }
            }
        }
        out.push(ch);
    }
    if out.trim().is_empty() {
        String::new()
    } else {
        out.trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_is_stripped() {
        let src = "---\ntitle: Hi\n---\n\n# Hello\n";
        assert_eq!(to_flux(src), "\n# Hello\n");
    }

    #[test]
    fn badge_and_plain_image_lines_become_image_directives() {
        let src = "[![badge](https://img.shields.io/x.svg)](https://example.com)\n![shot](assets/a.png)\n";
        let out = to_flux(src);
        assert!(out.contains("@image https://img.shields.io/x.svg alt=\"badge\" link=\"https://example.com\""), "{out}");
        assert!(out.contains("@image assets/a.png alt=\"shot\""), "{out}");
    }

    #[test]
    fn image_line_variants() {
        let out = to_flux("![alt](p \"Title\")\n");
        assert_eq!(out, "@image p alt=\"alt\"\n");
        let out = to_flux("![alt](<p with spaces>)\n");
        assert_eq!(out, "@image p with spaces alt=\"alt\"\n");
    }

    #[test]
    fn github_alerts_become_flux_callouts() {
        assert_eq!(to_flux("> [!NOTE]\n> careful\n"), "@note\ncareful\n@end\n");
        assert_eq!(to_flux("> [!CAUTION]\n> hot\n"), "@danger\nhot\n@end\n");
        assert_eq!(to_flux("> [!WARNING]\n> warm\n"), "@warn\nwarm\n@end\n");
        assert_eq!(to_flux("> [!TIP]\n> handy\n"), "@tip\nhandy\n@end\n");
    }

    #[test]
    fn alert_block() {
        let src = "> [!NOTE]\n> one\n>\n> two\n\nafter\n";
        assert_eq!(to_flux(src), "@note\none\n\ntwo\n@end\n\nafter\n");
    }

    #[test]
    fn pure_html_lines_are_dropped_and_inline_tags_unwrapped() {
        let src = "<div align=\"center\">\ntext <b>x</b> tail\n</div>\n";
        assert_eq!(to_flux(src), "text x tail\n");
    }

    #[test]
    fn code_spans_keep_their_angle_brackets() {
        assert_eq!(to_flux("use `<div>` here\n"), "use `<div>` here\n");
    }

    /// The generated files carry an HTML-comment banner; a preview must not show it.
    #[test]
    fn html_comments_vanish() {
        let banner = "<!-- Made from README.flux by FluxCode. Edit README.flux, not this file. -->\n# Title\n";
        assert_eq!(to_flux(banner), "# Title\n");
        let multi = "before <!-- open\nhidden\n--> after\n";
        assert_eq!(to_flux(multi), "before\n after\n");
        let inline = to_flux("x <!-- hidden --> y\n");
        assert!(!inline.contains("<!--"), "{inline}");
        assert!(inline.contains('x') && inline.contains('y'), "{inline}");
        assert_eq!(to_flux("use `<!--` marker\n"), "use `<!--` marker\n");
    }

    /// The shipped generated files must draw without a single HTML comment or
    /// details block left over (the report that started this: `</details>` on screen).
    #[test]
    fn generated_files_draw_without_html() {
        for f in ["README.md", "README.id.md", "docs/flux.md"] {
            let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(f)).unwrap();
            let out = to_flux(&src);
            for l in out.lines() {
                assert!(!l.contains("<!--"), "{f}: {l}");
            }
            if f != "docs/flux.md" {
                // The Flux guide quotes `<details><summary>` inside code spans on
                // purpose; the README pair has no reason to carry one.
                for l in out.lines() {
                    assert!(!l.contains("</details>") && !l.contains("<details>"), "{f}: {l}");
                }
            }
        }
    }

    #[test]
    fn code_fences_untouched() {
        let src = "```rust\n![x](y)\n<div>\n```\n![after](p.png)\n";
        let out = to_flux(src);
        assert!(out.contains("![x](y)"), "{out}");
        assert!(out.contains("<div>"), "{out}");
        assert!(out.contains("@image p.png"), "{out}");
        assert_eq!(out.matches("```").count(), 2);
    }

    #[test]
    fn plain_markdown_passes_through() {
        let src = "# Title\n\n- item\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> quoted\n---\n";
        assert_eq!(to_flux(src), src);
    }

    /// A line whose first characters are multibyte (an arrow, "Über…") once
    /// panicked the byte slice in the image scanner; it must pass through.
    #[test]
    fn lines_starting_with_multibyte_characters_pass_through() {
        let src = "→ forward\nÜber all of it\n![Übersicht](plan.png)\n";
        let out = to_flux(src);
        assert!(out.starts_with("→ forward\n"), "{out}");
        assert!(out.contains("Über all of it"), "{out}");
        assert!(out.contains("@image plan.png alt=\"Übersicht\""), "{out}");
    }
}



