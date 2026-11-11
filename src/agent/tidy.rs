//! Tidy an agent's text before it is drawn as Markdown, so it reads as cleanly as the
//! agent meant it to:
//! - choices written inline ("… pilihannya (a) panel kompak, atau (b) panel penuh.")
//!   become a list, one choice per line, with the letter in bold;
//! - a thought streamed as one long run gets a paragraph break after a sentence that
//!   ends one line of reasoning and starts the next ("…too small. But actually…").
//! Code blocks are left exactly as they are. The text is only copied when something
//! changes, so the common case costs one scan.

use std::borrow::Cow;

/// Choices: `(a)` `(b)` … or `(1)` `(2)` …, in order, at least two in one line.
fn markers(line: &str) -> Vec<(usize, String)> {
    let b = line.as_bytes();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while i + 2 < b.len() {
        if b[i] == b'(' && b[i + 2] == b')' && (i == 0 || b[i - 1] == b' ') && b.get(i + 3).is_none_or(|c| *c == b' ') {
            let ch = b[i + 1];
            let want = match out.last() {
                None => matches!(ch, b'a' | b'A' | b'1'),
                Some((_, prev)) => prev.as_bytes()[1] + 1 == ch,
            };
            if want {
                out.push((i, line[i..i + 3].to_string()));
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    if out.len() >= 2 {
        out
    } else {
        Vec::new()
    }
}

/// Words that join choices and are left dangling at the end of one once it is split.
fn trim_joiner(s: &str) -> &str {
    let mut t = s.trim_end();
    for w in [", atau", " atau", ", or", " or", ", dan", ", and", ";", ","] {
        if let Some(r) = t.strip_suffix(w) {
            t = r.trim_end();
            break;
        }
    }
    t
}

fn split_choices(line: &str, out: &mut String) -> bool {
    let m = markers(line);
    if m.is_empty() {
        return false;
    }
    let lead = trim_joiner(&line[..m[0].0]);
    if !lead.is_empty() {
        out.push_str(lead.trim_end_matches(['—', '-', ':']).trim_end());
        out.push_str(if lead.ends_with(':') { "\n\n" } else { ":\n\n" });
    }
    for (k, (at, mark)) in m.iter().enumerate() {
        let end = m.get(k + 1).map_or(line.len(), |n| n.0);
        let body = line[at + 3..end].trim();
        let body = if k + 1 < m.len() { trim_joiner(body) } else { body };
        // The last choice may run on into a closing sentence: "… vanilla. Atau kalau …".
        let (body, tail) = match (k + 1 == m.len()).then(|| body.find(". ")).flatten() {
            Some(dot) => (&body[..=dot], body[dot + 1..].trim()),
            None => (body, ""),
        };
        out.push_str(&format!("- **{mark}** {body}\n"));
        if !tail.is_empty() {
            out.push('\n');
            out.push_str(tail);
            out.push('\n');
        }
    }
    true
}

/// Markdown for an answer.
pub fn answer(text: &str) -> Cow<'_, str> {
    if !text.contains("(a)") && !text.contains("(1)") && !text.contains("(A)") {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 32);
    let mut fence = false;
    let mut changed = false;
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare.trim_start().starts_with("```") {
            fence = !fence;
        }
        let listy = bare.trim_start().starts_with(['-', '*', '>', '|', '#']);
        if !fence && !listy && split_choices(bare, &mut out) {
            changed = true;
            continue;
        }
        out.push_str(line);
    }
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(text)
    }
}

/// Markdown for a thought: the answer's rules, plus paragraph breaks in a long run.
pub fn thought(text: &str) -> String {
    let t = answer(text.trim());
    let mut out = String::with_capacity(t.len() + 16);
    let mut run = 0usize;
    let mut chars = t.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        out.push(ch);
        run = if ch == '\n' { 0 } else { run + 1 };
        // ". But", "? So", "! Now": a new line of thought after a long stretch.
        if matches!(ch, '.' | '?' | '!') && run > 220 && t[i + 1..].starts_with(' ') {
            let next = t[i + 1..].trim_start();
            if next.chars().next().is_some_and(char::is_uppercase) {
                out.push_str("\n\n");
                run = 0;
                chars.next();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{answer, thought};

    #[test]
    fn inline_choices_become_a_list() {
        let t = "Pilihan yang kubayangkan — (a) panel kompak ~300×420 yang fokus ke chat saja, atau (b) tetap panel penuh (320px). Kalau ragu, pilih (b).\n";
        let a = answer(t);
        assert!(a.contains("Pilihan yang kubayangkan:\n\n- **(a)** panel kompak ~300×420 yang fokus ke chat saja\n- **(b)** tetap panel penuh (320px).\n\nKalau ragu, pilih (b).\n"), "{a}");
    }

    #[test]
    fn leaves_other_text_alone() {
        let t = "Fungsi f(a) dan g(b) saja.\n```\n(a) x (b) y\n```\n";
        assert!(matches!(answer(t), std::borrow::Cow::Borrowed(_)));
        assert!(matches!(answer("tidak ada pilihan"), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn long_thoughts_get_paragraphs() {
        let long = format!("{}. But actually the clamp is wrong.", "word ".repeat(60).trim());
        assert!(thought(&long).contains(".\n\nBut actually"), "{}", thought(&long));
        assert_eq!(thought("Short. Then more."), "Short. Then more.");
    }
}
