//! VS Code's line commands, as pure text edits: given the text and the selection
//! (UTF-8 byte offsets), each returns the byte range to replace, the new text for
//! it, and where the selection goes afterwards. The workbench applies the edit with
//! the editor's own `replace`, so each command is one Undo step.
//!
//! - Toggle line comment (Ctrl+/)
//! - Move line up / down (Alt+Up / Alt+Down)
//! - Copy line up / down (Shift+Alt+Up / Shift+Alt+Down)
//! - Delete line (Ctrl+Shift+K)
//! - Insert line below / above (Ctrl+Enter / Ctrl+Shift+Enter)

use std::ops::Range;

/// One edit: replace `range` with `text`, then select `select` (in the new text).
#[derive(Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: Range<usize>,
    pub text: String,
    pub select: Range<usize>,
}

/// Byte range of the whole lines the selection touches, without the last newline.
/// A selection ending at the start of a line does not include that line (as in VS Code).
fn line_block(text: &str, sel: &Range<usize>) -> Range<usize> {
    let start = text[..sel.start.min(text.len())].rfind('\n').map_or(0, |i| i + 1);
    let mut end_at = sel.end.min(text.len());
    if end_at > sel.start && end_at > 0 && text.as_bytes()[end_at - 1] == b'\n' {
        end_at -= 1;
    }
    let end = text[end_at..].find('\n').map_or(text.len(), |i| end_at + i);
    start..end.max(start)
}

/// The line-comment marker for a file name, or None for formats without one.
pub fn comment_prefix(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "py" | "pyw" | "sh" | "bash" | "zsh" | "ps1" | "psm1" | "rb" | "pl" | "r" | "toml" | "yaml" | "yml" | "conf" | "cfg" | "ini" | "properties" | "dockerfile" | "mk" | "cmake" | "nim" | "jl" | "ex" | "exs" | "coffee" | "tf" => "#",
        "lua" | "sql" | "hs" | "elm" | "ada" => "--",
        "bat" | "cmd" => "REM",
        "vim" => "\"",
        "lisp" | "clj" | "cljs" | "el" | "scm" => ";",
        "erl" | "hrl" | "tex" => "%",
        "html" | "htm" | "xml" | "svg" | "md" | "markdown" | "css" | "json" | "txt" => return None,
        _ if lower == "makefile" || lower == "dockerfile" || lower.starts_with(".env") || lower.ends_with("gitignore") => "#",
        _ => "//",
    })
}

/// Toggle a line comment on every line the selection touches. When every non-blank
/// line is already commented the markers come off; otherwise they go on, at the
/// smallest indentation of the block (blank lines stay blank).
pub fn toggle_comment(text: &str, sel: Range<usize>, prefix: &str) -> Edit {
    let block = line_block(text, &sel);
    let lines: Vec<&str> = text[block.clone()].split('\n').collect();
    let marker = format!("{prefix} ");
    let content: Vec<&&str> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let all_commented = !content.is_empty() && content.iter().all(|l| l.trim_start().starts_with(prefix));
    let indent = content.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let out: Vec<String> = lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                return l.to_string();
            }
            if all_commented {
                let lead = l.len() - l.trim_start().len();
                let rest = &l[lead..];
                let rest = rest.strip_prefix(marker.as_str()).or_else(|| rest.strip_prefix(prefix)).unwrap_or(rest);
                format!("{}{rest}", &l[..lead])
            } else {
                format!("{}{marker}{}", &l[..indent], &l[indent..])
            }
        })
        .collect();
    let new = out.join("\n");
    let delta = new.len() as isize - (block.end - block.start) as isize;
    // Keep the caret on the same text: shift by what changed on its own line.
    let shift = |pos: usize| -> usize {
        if lines.len() == 1 {
            let d = new.len() as isize - lines[0].len() as isize;
            (pos as isize + d).clamp(block.start as isize, (block.start + new.len()) as isize) as usize
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
    Edit { range: block, text: new, select }
}

/// Move the touched lines one line up (`up`) or down. None at the top or bottom.
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
        Some(Edit { range: prev_start..block.end, text: new, select: sel.start - shift..sel.end - shift })
    } else {
        if block.end >= text.len() {
            return None;
        }
        let next_end = text[block.end + 1..].find('\n').map_or(text.len(), |i| block.end + 1 + i);
        let next = &text[block.end + 1..next_end];
        let new = format!("{next}\n{body}");
        let shift = next.len() + 1;
        Some(Edit { range: block.start..next_end, text: new, select: sel.start + shift..sel.end + shift })
    }
}

/// Duplicate the touched lines above (`up`, the selection stays on the upper copy)
/// or below (the selection moves to the lower copy).
pub fn copy_lines(text: &str, sel: Range<usize>, up: bool) -> Edit {
    let block = line_block(text, &sel);
    let body = &text[block.clone()];
    let new = format!("{body}\n{body}");
    let shift = if up { 0 } else { body.len() + 1 };
    Edit { range: block, text: new, select: sel.start + shift..sel.end + shift }
}

/// Delete the touched lines, newline included; the caret goes to the start of the
/// line that takes their place.
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
    Edit { range, text: String::new(), select: caret..caret }
}

/// A new line below (or above) the caret's line, with the same indentation, and the
/// caret on it.
pub fn insert_line(text: &str, sel: Range<usize>, above: bool) -> Edit {
    let block = line_block(text, &(sel.start..sel.start));
    let line = &text[block.clone()];
    let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    if above {
        let at = block.start;
        let caret = at + indent.len();
        Edit { range: at..at, text: format!("{indent}\n"), select: caret..caret }
    } else {
        let at = block.end;
        let caret = at + 1 + indent.len();
        Edit { range: at..at, text: format!("\n{indent}"), select: caret..caret }
    }
}

/// Apply an edit to a string (for tests).
#[cfg(test)]
pub fn apply(text: &str, e: &Edit) -> String {
    format!("{}{}{}", &text[..e.range.start], e.text, &text[e.range.end..])
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
        assert_eq!(&apply(t, &down)[down.select.start - 1..down.select.start + 2], "one");
        let up = move_lines(t, 9..9, true).unwrap();
        assert_eq!(apply(t, &up), "one\nthree\ntwo");
        assert!(move_lines(t, 0..0, true).is_none());
        assert!(move_lines(t, 10..10, false).is_none());
    }

    #[test]
    fn a_selection_ending_at_a_line_start_leaves_that_line() {
        let t = "a\nb\nc";
        // "a\n" selected: only line a moves.
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
}
