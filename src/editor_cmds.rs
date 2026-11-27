use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<usize>,
    pub text: String,
    pub select: Range<usize>,
}

fn line_block(text: &str, sel: &Range<usize>) -> Range<usize> {
    let start = text[..sel.start.min(text.len())]
        .rfind('\n')
        .map_or(0, |i| i + 1);
    let mut end_at = sel.end.min(text.len());
    if end_at > sel.start && end_at > 0 && text.as_bytes()[end_at - 1] == b'\n' {
        end_at -= 1;
    }
    let end = text[end_at..].find('\n').map_or(text.len(), |i| end_at + i);
    start..end.max(start)
}

pub fn comment_prefix(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "py" | "pyw" | "sh" | "bash" | "zsh" | "ps1" | "psm1" | "rb" | "pl" | "r" | "toml"
        | "yaml" | "yml" | "conf" | "cfg" | "ini" | "properties" | "dockerfile" | "mk"
        | "cmake" | "nim" | "jl" | "ex" | "exs" | "coffee" | "tf" => "#",
        "lua" | "sql" | "hs" | "elm" | "ada" => "--",
        "bat" | "cmd" => "REM",
        "vim" => "\"",
        "lisp" | "clj" | "cljs" | "el" | "scm" => ";",
        "erl" | "hrl" | "tex" => "%",
        "html" | "htm" | "xml" | "svg" | "md" | "markdown" | "css" | "json" | "txt" => return None,
        _ if lower == "makefile"
            || lower == "dockerfile"
            || lower.starts_with(".env")
            || lower.ends_with("gitignore") =>
        {
            "#"
        }
        _ => "//",
    })
}

pub fn block_comment_delimiters(file_name: &str) -> Option<(&'static str, &'static str)> {
    let lower = file_name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "html" | "htm" | "xml" | "svg" | "md" | "markdown" => ("<!--", "-->"),
        "py" | "pyw" => ("\"\"\"", "\"\"\""),
        "lua" => ("--[[", "]]"),
        "hs" | "elm" => ("{-", "-}"),
        "ml" | "mli" | "pas" => ("(*", "*)"),
        "txt" | "json" => return None,
        _ => ("/*", "*/"),
    })
}

pub fn toggle_block_comment(text: &str, sel: Range<usize>, open: &str, close: &str) -> Edit {
    let s = sel.start.min(text.len());
    let e = sel.end.min(text.len());
    let target = if s == e {
        line_block(text, &(s..e))
    } else {
        s..e
    };
    let slice = &text[target.clone()];
    let trimmed = slice.trim();
    if trimmed.starts_with(open)
        && trimmed.ends_with(close)
        && trimmed.len() >= open.len() + close.len()
    {
        let lead = slice.len() - slice.trim_start().len();
        let trail = slice.len() - slice.trim_end().len();
        let inner = &trimmed[open.len()..trimmed.len() - close.len()];
        let inner = inner.strip_prefix(' ').unwrap_or(inner);
        let inner = inner.strip_suffix(' ').unwrap_or(inner);
        let out = format!("{}{inner}{}", &slice[..lead], &slice[slice.len() - trail..]);
        let end = target.start + out.len();
        return Edit {
            range: target.clone(),
            text: out,
            select: target.start..end,
        };
    }
    let out = format!("{open} {slice} {close}");
    let end = target.start + out.len();
    Edit {
        range: target.clone(),
        text: out,
        select: target.start..end,
    }
}

pub fn duplicate_selection(text: &str, sel: Range<usize>) -> Edit {
    if sel.start == sel.end {
        return copy_lines(text, sel, false);
    }
    let s = sel.start.min(text.len());
    let e = sel.end.min(text.len());
    let chunk = &text[s..e];
    let end = e + chunk.len();
    Edit {
        range: e..e,
        text: chunk.to_string(),
        select: e..end,
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Structural selection expansion (caret -> word -> quotes/brackets -> line -> document).
pub fn expand_selection(text: &str, sel: Range<usize>) -> Option<Range<usize>> {
    let len = text.len();
    let s = sel.start.min(len);
    let e = sel.end.min(len);
    if s == 0 && e == len {
        return None;
    }
    let bytes = text.as_bytes();
    // 1. Word around caret or partial word
    if s == e || bytes[s..e].iter().all(|&b| is_word_byte(b)) {
        let mut ws = s;
        while ws > 0 && is_word_byte(bytes[ws - 1]) {
            ws -= 1;
        }
        let mut we = e;
        while we < len && is_word_byte(bytes[we]) {
            we += 1;
        }
        if (ws < s || we > e) && ws < we {
            return Some(ws..we);
        }
    }
    // 2. Enclosing balanced brackets/quotes (inner first, then outer)
    let pairs = [
        (b'(', b')'),
        (b'[', b']'),
        (b'{', b'}'),
        (b'"', b'"'),
        (b'\'', b'\''),
    ];
    let mut best: Option<Range<usize>> = None;
    for &(open, close) in &pairs {
        if open == close {
            let mut left = None;
            let mut i = s;
            while i > 0 {
                i -= 1;
                if bytes[i] == open {
                    left = Some(i);
                    break;
                }
                if bytes[i] == b'\n' {
                    break;
                }
            }
            let mut right = None;
            let mut j = e;
            while j < len {
                if bytes[j] == close {
                    right = Some(j);
                    break;
                }
                if bytes[j] == b'\n' {
                    break;
                }
                j += 1;
            }
            if let (Some(l), Some(r)) = (left, right) {
                let cand = if s > l + 1 || e < r {
                    (l + 1)..r
                } else {
                    l..(r + 1)
                };
                if cand.start <= s
                    && cand.end >= e
                    && (cand.start < s || cand.end > e)
                    && best
                        .as_ref()
                        .is_none_or(|b| (cand.end - cand.start) < (b.end - b.start))
                {
                    best = Some(cand);
                }
            }
        } else {
            let mut depth = 0i32;
            let mut left = None;
            let mut i = s;
            while i > 0 {
                i -= 1;
                if bytes[i] == close {
                    depth += 1;
                } else if bytes[i] == open {
                    if depth == 0 {
                        left = Some(i);
                        break;
                    }
                    depth -= 1;
                }
            }
            let mut depth = 0i32;
            let mut right = None;
            let mut j = e;
            while j < len {
                if bytes[j] == open {
                    depth += 1;
                } else if bytes[j] == close {
                    if depth == 0 {
                        right = Some(j);
                        break;
                    }
                    depth -= 1;
                }
                j += 1;
            }
            if let (Some(l), Some(r)) = (left, right) {
                let cand = if s > l + 1 || e < r {
                    (l + 1)..r
                } else {
                    l..(r + 1)
                };
                if cand.start <= s
                    && cand.end >= e
                    && (cand.start < s || cand.end > e)
                    && best
                        .as_ref()
                        .is_none_or(|b| (cand.end - cand.start) < (b.end - b.start))
                {
                    best = Some(cand);
                }
            }
        }
    }
    if let Some(b) = best {
        return Some(b);
    }
    // 3. Full current line(s)
    let line = line_block(text, &(s..e));
    if line.start < s || line.end > e {
        return Some(line);
    }
    // 4. Whole document
    Some(0..len)
}

pub fn toggle_comment(text: &str, sel: Range<usize>, prefix: &str) -> Edit {
    let block = line_block(text, &sel);
    let lines: Vec<&str> = text[block.clone()].split('\n').collect();
    let marker = format!("{prefix} ");
    let content: Vec<&&str> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let all_commented =
        !content.is_empty() && content.iter().all(|l| l.trim_start().starts_with(prefix));
    let indent = content
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let out: Vec<String> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                return l.to_string();
            }
            if all_commented {
                let lead = l.len() - l.trim_start().len();
                let rest = &l[lead..];
                let rest = rest
                    .strip_prefix(marker.as_str())
                    .or_else(|| rest.strip_prefix(prefix))
                    .unwrap_or(rest);
                format!("{}{rest}", &l[..lead])
            } else {
                format!("{}{marker}{}", &l[..indent], &l[indent..])
            }
        })
        .collect();
    let new = out.join("\n");
    let delta = new.len() as isize - (block.end - block.start) as isize;
    let shift = |pos: usize| -> usize {
        if lines.len() == 1 {
            let d = new.len() as isize - lines[0].len() as isize;
            (pos as isize + d).clamp(block.start as isize, (block.start + new.len()) as isize)
                as usize
        } else {
            pos
        }
    };
    let select = if sel.is_empty() {
        let p = shift(sel.start);
        p..p
    } else {
        block.start..(block.end as isize + delta) as usize
    };
    Edit {
        range: block,
        text: new,
        select,
    }
}

pub fn move_lines(text: &str, sel: Range<usize>, up: bool) -> Option<Edit> {
    let block = line_block(text, &sel);
    let body = &text[block.clone()];
    if up {
        if block.start == 0 {
            return None;
        }
        let prev_start = text[..block.start - 1].rfind('\n').map_or(0, |i| i + 1);
        let prev = &text[prev_start..block.start - 1];
        let new = format!("{body}\n{prev}");
        let shift = block.start - prev_start;
        Some(Edit {
            range: prev_start..block.end,
            text: new,
            select: sel.start - shift..sel.end - shift,
        })
    } else {
        if block.end >= text.len() {
            return None;
        }
        let next_end = text[block.end + 1..]
            .find('\n')
            .map_or(text.len(), |i| block.end + 1 + i);
        let next = &text[block.end + 1..next_end];
        let new = format!("{next}\n{body}");
        let shift = next.len() + 1;
        Some(Edit {
            range: block.start..next_end,
            text: new,
            select: sel.start + shift..sel.end + shift,
        })
    }
}

pub fn copy_lines(text: &str, sel: Range<usize>, up: bool) -> Edit {
    let block = line_block(text, &sel);
    let body = &text[block.clone()];
    let new = format!("{body}\n{body}");
    let shift = if up { 0 } else { body.len() + 1 };
    Edit {
        range: block,
        text: new,
        select: sel.start + shift..sel.end + shift,
    }
}

pub fn delete_lines(text: &str, sel: Range<usize>) -> Edit {
    let block = line_block(text, &sel);
    let (range, caret) = if block.end < text.len() {
        (block.start..block.end + 1, block.start)
    } else if block.start > 0 {
        let prev = text[..block.start - 1].rfind('\n').map_or(0, |i| i + 1);
        (block.start - 1..block.end, prev)
    } else {
        (block.clone(), 0)
    };
    Edit {
        range,
        text: String::new(),
        select: caret..caret,
    }
}

pub fn insert_line(text: &str, sel: Range<usize>, above: bool) -> Edit {
    let block = line_block(text, &(sel.start..sel.start));
    let line = &text[block.clone()];
    let indent: String = line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    if above {
        let at = block.start;
        let caret = at + indent.len();
        Edit {
            range: at..at,
            text: format!("{indent}\n"),
            select: caret..caret,
        }
    } else {
        let at = block.end;
        let caret = at + 1 + indent.len();
        Edit {
            range: at..at,
            text: format!("\n{indent}"),
            select: caret..caret,
        }
    }
}

#[cfg(test)]
pub fn apply(text: &str, e: &Edit) -> String {
    format!(
        "{}{}{}",
        &text[..e.range.start],
        e.text,
        &text[e.range.end..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_go_on_and_come_off() {
        let t = "fn a() {\n    x();\n\n    y();\n}";
        let sel = 9..t.len() - 2; // lines 2..4
        let e = toggle_comment(t, sel.clone(), "//");
        let on = apply(t, &e);
        assert_eq!(on, "fn a() {\n    // x();\n\n    // y();\n}");
        let e2 = toggle_comment(&on, e.select.clone(), "//");
        assert_eq!(apply(&on, &e2), t);
    }

    #[test]
    fn a_caret_comment_keeps_its_place() {
        let t = "a = 1\nb = 2";
        let e = toggle_comment(t, 7..7, "#");
        assert_eq!(apply(t, &e), "a = 1\n# b = 2");
        assert_eq!(e.select, 9..9);
        let back = toggle_comment(&apply(t, &e), 9..9, "#");
        assert_eq!(apply(&apply(t, &e), &back), t);
    }

    #[test]
    fn lines_move_and_stop_at_the_edges() {
        let t = "one\ntwo\nthree";
        let down = move_lines(t, 1..1, false).unwrap();
        assert_eq!(apply(t, &down), "two\none\nthree");
        assert_eq!(
            &apply(t, &down)[down.select.start - 1..down.select.start + 2],
            "one"
        );
        let up = move_lines(t, 9..9, true).unwrap();
        assert_eq!(apply(t, &up), "one\nthree\ntwo");
        assert!(move_lines(t, 0..0, true).is_none());
        assert!(move_lines(t, 10..10, false).is_none());
    }

    #[test]
    fn a_selection_ending_at_a_line_start_leaves_that_line() {
        let t = "a\nb\nc";
        let e = move_lines(t, 0..2, false).unwrap();
        assert_eq!(apply(t, &e), "b\na\nc");
    }

    #[test]
    fn copy_delete_and_insert() {
        let t = "  x\ny";
        assert_eq!(apply(t, &copy_lines(t, 2..2, false)), "  x\n  x\ny");
        assert_eq!(copy_lines(t, 2..2, false).select, 6..6);
        assert_eq!(apply(t, &delete_lines(t, 1..1)), "y");
        assert_eq!(apply(t, &delete_lines(t, 4..4)), "  x");
        assert_eq!(apply("solo", &delete_lines("solo", 2..2)), "");
        let below = insert_line(t, 1..1, false);
        assert_eq!(apply(t, &below), "  x\n  \ny");
        assert_eq!(below.select, 6..6);
        let above = insert_line(t, 4..4, true);
        assert_eq!(apply(t, &above), "  x\n\ny");
    }

    #[test]
    fn comment_markers_by_language() {
        assert_eq!(comment_prefix("main.rs"), Some("//"));
        assert_eq!(comment_prefix("app.py"), Some("#"));
        assert_eq!(comment_prefix("q.sql"), Some("--"));
        assert_eq!(comment_prefix("run.bat"), Some("REM"));
        assert_eq!(comment_prefix("Makefile"), Some("#"));
        assert_eq!(comment_prefix("index.html"), None);
    }

    #[test]
    fn block_comment_wraps_and_unwraps() {
        assert_eq!(block_comment_delimiters("main.rs"), Some(("/*", "*/")));
        assert_eq!(
            block_comment_delimiters("index.html"),
            Some(("<!--", "-->"))
        );
        let t = "let x = 1 + 2;";
        let e = toggle_block_comment(t, 8..13, "/*", "*/");
        let on = apply(t, &e);
        assert_eq!(on, "let x = /* 1 + 2 */;");
        let off = toggle_block_comment(&on, e.select.clone(), "/*", "*/");
        assert_eq!(apply(&on, &off), t);
    }

    #[test]
    fn duplicate_selection_handles_caret_and_range() {
        let t = "hello world";
        let e = duplicate_selection(t, 0..5);
        assert_eq!(apply(t, &e), "hellohello world");
        assert_eq!(e.select, 5..10);
        let line_dup = duplicate_selection("a\nb", 0..0);
        assert_eq!(apply("a\nb", &line_dup), "a\na\nb");
    }

    #[test]
    fn expand_selection_grows_word_brackets_line_document() {
        let t = "fn f() { call(alpha_1) }\nnext";
        let r1 = expand_selection(t, 16..16).unwrap();
        assert_eq!(&t[r1.clone()], "alpha_1");
        let r2 = expand_selection(t, r1).unwrap();
        assert_eq!(&t[r2.clone()], "(alpha_1)");
        let r3 = expand_selection(t, r2).unwrap();
        assert_eq!(&t[r3.clone()], " call(alpha_1) ");
        let r4 = expand_selection(t, r3).unwrap();
        assert_eq!(&t[r4.clone()], "{ call(alpha_1) }");
        let r5 = expand_selection(t, r4).unwrap();
        assert_eq!(&t[r5.clone()], "fn f() { call(alpha_1) }");
        let r6 = expand_selection(t, r5).unwrap();
        assert_eq!(&t[r6.clone()], t);
        assert_eq!(expand_selection(t, r6), None);
    }
}
