//! Small, testable pieces of the agent panel: `@file` mentions, the words that
//! mean "look at my error", and reading another harness's saved sessions.

use std::path::Path;

/// The `@query` being typed at `cursor` (byte offset), if any: (offset of `@`, query).
pub fn mention_query(text: &str, cursor: usize) -> Option<(usize, String)> {
    let cursor = cursor.min(text.len());
    let before = &text[..cursor];
    let at = before.rfind('@')?;
    // `@` starts a word (start of line or after a space), and no space since.
    if at > 0 && !before[..at].ends_with(char::is_whitespace) {
        return None;
    }
    let q = &before[at + 1..];
    if q.contains(char::is_whitespace) {
        return None;
    }
    Some((at, q.to_string()))
}

/// Every `@path` in a message (trailing punctuation trimmed).
pub fn mentions(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        if let Some(p) = word.strip_prefix('@') {
            let p = p.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', '\'']);
            if !p.is_empty() && !out.iter().any(|o| o == p) {
                out.push(p.to_string());
            }
        }
    }
    out
}

/// Does the message ask about an error? (English and Indonesian, as people type them.)
pub fn wants_error(text: &str) -> bool {
    let t = text.to_lowercase();
    const WORDS: &[&str] = &[
        "error", "eror", "traceback", "exception", "bug", "crash", "failed", "fails", "failing", "not working", "doesn't work",
        "broken", "why", "kenapa", "gagal", "rusak", "salah", "ga jalan", "gak jalan", "nggak jalan", "tidak jalan",
        "ga bisa", "gak bisa", "tidak bisa", "masalah", "fix",
    ];
    WORDS.iter().any(|w| t.contains(w))
}

/// Does the message ask about the terminal or program output?
pub fn wants_output(text: &str) -> bool {
    let t = text.to_lowercase();
    ["terminal", "output", "log", "hasil", "console", "printed", "keluaran"].iter().any(|w| t.contains(w))
}

/// A session saved by OpenCode (or Mura) for the open folder.
#[derive(Clone, Debug)]
pub struct RemoteSession {
    pub id: String,
    pub title: String,
    pub updated: u64,
    pub harness: &'static str,
}

/// Parse `<cli> session list --format json` output.
pub fn parse_session_list(json: &str, harness: &'static str) -> Result<Vec<RemoteSession>, String> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).map_err(|e| format!("couldn't read the session list: {e}"))?;
    let arr = v.as_array().or_else(|| v.get("sessions").and_then(|s| s.as_array())).ok_or("unexpected session list")?;
    Ok(arr
        .iter()
        .filter_map(|s| {
            let id = s.get("id").or_else(|| s.get("sessionId"))?.as_str()?.to_string();
            let title = s.get("title").and_then(|t| t.as_str()).filter(|t| !t.is_empty()).unwrap_or("Untitled session").to_string();
            let updated = s.get("updated").or_else(|| s.get("updatedAt")).and_then(|u| u.as_u64()).unwrap_or(0);
            Some(RemoteSession { id, title, updated, harness })
        })
        .collect())
}

/// Lines of a file for a selection given as byte offsets: (first line, last line), 1-based.
pub fn line_span(text: &str, start: usize, end: usize) -> (usize, usize) {
    let s = text[..start.min(text.len())].matches('\n').count() + 1;
    let e = text[..end.min(text.len())].matches('\n').count() + 1;
    (s, e.max(s))
}

/// Fence language for a file name (for selections sent as code blocks).
pub fn fence_lang(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "py" | "pyw" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" => "typescript",
        "rs" => "rust",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "md" => "markdown",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        "php" => "php",
        "sh" => "bash",
        "ps1" => "powershell",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mention_detection() {
        assert_eq!(mention_query("explain @ap", 11), Some((8, "ap".into())));
        assert_eq!(mention_query("@", 1), Some((0, "".into())));
        assert_eq!(mention_query("mail a@b.com", 12), None);
        assert_eq!(mention_query("@app.py done", 12), None);
        assert_eq!(mentions("look at @app.py, and @src/main.rs."), vec!["app.py".to_string(), "src/main.rs".to_string()]);
    }

    #[test]
    fn intent_words() {
        assert!(wants_error("kenapa ini ga jalan"));
        assert!(wants_error("Why does it crash?"));
        assert!(!wants_error("tambahkan tombol login"));
        assert!(wants_output("lihat hasil di terminal"));
    }

    #[test]
    fn session_list() {
        let j = r#"[{"id":"ses_1","title":"Migrasi","updated":1793765693036,"directory":"C:\\x"},{"id":"ses_2","title":""}]"#;
        let s = parse_session_list(j, "OpenCode").unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].title, "Untitled session");
        assert_eq!(line_span("a\nb\nc\n", 2, 5), (2, 3));
    }
}
