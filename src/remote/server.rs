//! The helper that runs inside the distro: `fluxcode --remote-server`.
//!
//! It is the same binary as the editor, so a Linux build works in the distro
//! without a second artefact. One request is in flight at a time; see `proto.rs`.

use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use super::proto::{
    read_frame, write_frame, DirEntry, FileHitsWire, GitWire, Hello, HitWire, Request, Response, SearchWire, Stat, FRAME_BYTES, FRAME_JSON,
    PROTOCOL_VERSION,
};

/// Every folder search skips, matching `core::SKIP_DIRS`.
const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "__pycache__", ".venv", "venv", ".mypy_cache",
    ".pytest_cache", ".ruff_cache", ".tox", "dist", ".next", ".idea",
];
/// Files larger than this are not searched, matching `core::MAX_SEARCH_FILE_BYTES`.
const MAX_SEARCH_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Hits past this stop the search, matching `core::MAX_SEARCH_HITS`.
const MAX_SEARCH_HITS: usize = 5_000;
/// How wide a hit's preview is, and how much of it sits before the match.
const PREVIEW_WIDTH: usize = 160;
const PREVIEW_BEFORE: usize = 40;

/// Serve requests on stdin/stdout until the editor closes the pipe.
pub fn serve() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    while let Some((kind, payload)) = read_frame(&mut input)? {
        if kind != FRAME_JSON {
            write_frame(&mut out, FRAME_JSON, &serde_json::to_vec(&Response::error("expected a JSON request")).unwrap())?;
            continue;
        }
        let Ok(req) = serde_json::from_slice::<Request>(&payload) else {
            write_frame(&mut out, FRAME_JSON, &serde_json::to_vec(&Response::error("malformed request")).unwrap())?;
            continue;
        };
        // A write carries its bytes in the next frame, so read them before answering.
        let body = if let Request::Write { .. } = req {
            match read_frame(&mut input)? {
                Some((FRAME_BYTES, bytes)) => Some(bytes),
                _ => None,
            }
        } else {
            None
        };
        match handle(req, body) {
            Ok(Answer::Json(r)) => write_frame(&mut out, FRAME_JSON, &serde_json::to_vec(&r).unwrap())?,
            Ok(Answer::Bytes(lead, bytes)) => {
                write_frame(&mut out, FRAME_JSON, &serde_json::to_vec(&lead).unwrap())?;
                write_frame(&mut out, FRAME_BYTES, &bytes)?;
            }
            Err(e) => write_frame(&mut out, FRAME_JSON, &serde_json::to_vec(&Response::error(e)).unwrap())?,
        }
    }
    Ok(())
}

enum Answer {
    Json(Response),
    /// A JSON header (success or failure) and then the bytes.
    Bytes(Response, Vec<u8>),
}

fn handle(req: Request, body: Option<Vec<u8>>) -> Result<Answer, String> {
    Ok(match req {
        Request::Hello => Answer::Json(Response::json(Hello {
            protocol: PROTOCOL_VERSION,
            build: crate::update::BUILD.to_string(),
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            home: std::env::var("HOME").unwrap_or_default(),
        })),
        Request::Ping => Answer::Json(Response::json(serde_json::json!({"pong": true}))),
        Request::List { path } => Answer::Json(Response::json(list(&path)?)),
        Request::Stat { path } => Answer::Json(Response::json(stat(&path))),
        Request::Read { path, limit } => {
            let bytes = read_file(&path, limit)?;
            Answer::Bytes(Response::bytes(), bytes)
        }
        Request::Write { path, crlf } => {
            let Some(mut bytes) = body else { return Err("no bytes followed the write".into()) };
            // The editor sends the text with `\n`; the file's own line endings
            // are put back here, as `core::write_text` does on Windows.
            if crlf {
                let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n").replace('\n', "\r\n");
                bytes = text.into_bytes();
            }
            write_file(&path, &bytes)?;
            Answer::Json(Response::json(serde_json::json!({"written": bytes.len()})))
        }
        Request::Search { root, query, match_case, whole_word } => Answer::Json(Response::json(search(&root, &query, match_case, whole_word))),
        Request::Git { cwd, args } => Answer::Json(Response::json(git(&cwd, &args))),
    })
}

fn list(path: &str) -> Result<Vec<DirEntry>, String> {
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut out: Vec<DirEntry> = rd
        .flatten()
        .map(|e| {
            let meta = e.metadata().ok();
            DirEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                len: meta.map(|m| m.len()).unwrap_or(0),
            }
        })
        .collect();
    // Folders first, then case-insensitive by name, as the explorer draws them.
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

fn stat(path: &str) -> Stat {
    match std::fs::metadata(path) {
        Ok(m) => Stat { exists: true, is_dir: m.is_dir(), is_file: m.is_file(), len: m.len() },
        Err(_) => Stat { exists: false, is_dir: false, is_file: false, len: 0 },
    }
}

fn read_file(path: &str, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.by_ref().take(limit).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    // Atomic, as `core::write_text`: a sibling temp file, then a rename, so a
    // crash mid-save never leaves a half-written file.
    let p = Path::new(path);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = p.with_file_name(format!(".{name}.fluxcode-tmp"));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn search(root: &str, query: &str, match_case: bool, whole_word: bool) -> SearchWire {
    let needle = if match_case { query.to_string() } else { query.to_lowercase() };
    let mut out = SearchWire::default();
    if needle.is_empty() {
        return out;
    }
    let mut stack = vec![PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries.iter().rev() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                let name = e.file_name();
                if !SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                    stack.push(e.path());
                }
            }
        }
        for e in &entries {
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() || meta.len() > MAX_SEARCH_FILE_BYTES {
                continue;
            }
            let Ok(bytes) = std::fs::read(e.path()) else { continue };
            if bytes[..bytes.len().min(8192)].contains(&0) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let mut hits = Vec::new();
            for (n, line) in text.lines().enumerate() {
                let hay = if match_case { std::borrow::Cow::Borrowed(line) } else { std::borrow::Cow::Owned(line.to_lowercase()) };
                if let Some(at) = find_match(&hay, &needle, whole_word) {
                    let chars = hay[..at].chars().count();
                    hits.push(HitWire { line: n as u32 + 1, text: preview(line, chars) });
                    out.total += 1;
                    if out.total >= MAX_SEARCH_HITS {
                        out.truncated = true;
                        break;
                    }
                }
            }
            if !hits.is_empty() {
                out.files.push(FileHitsWire { path: e.path().to_string_lossy().into_owned(), hits });
            }
            if out.truncated {
                return out;
            }
        }
    }
    out
}

/// The match position, honouring whole-word, as `core::find_match` does.
fn find_match(hay: &str, needle: &str, whole_word: bool) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let at = from + rel;
        if !whole_word {
            return Some(at);
        }
        let before = hay[..at].chars().next_back();
        let after = hay[at + needle.len()..].chars().next();
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        if !word(before) && !word(after) {
            return Some(at);
        }
        from = at + needle.len().max(1);
        if from >= hay.len() {
            break;
        }
    }
    None
}

/// Short single-line preview around the match, indentation stripped, exactly as
/// `core::preview` does it so a remote search reads like a local one.
fn preview(line: &str, match_char: usize) -> String {
    let indent = line.chars().take_while(|c| c.is_whitespace()).count();
    let text: Vec<char> = line.chars().skip(indent).collect();
    let at = match_char.saturating_sub(indent);
    if text.len() <= PREVIEW_WIDTH {
        return text.into_iter().collect();
    }
    let start = at.saturating_sub(PREVIEW_BEFORE).min(text.len().saturating_sub(PREVIEW_WIDTH));
    let mut s: String = text[start..(start + PREVIEW_WIDTH).min(text.len())].iter().collect();
    if start > 0 {
        s.insert(0, '…');
    }
    s
}

fn git(cwd: &str, args: &[String]) -> GitWire {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NO_COLOR", "1")
        .output();
    match out {
        Ok(o) => GitWire {
            success: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => GitWire { success: false, stdout: String::new(), stderr: e.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_word_match_skips_a_prefix() {
        assert_eq!(find_match("cheating cat", "cat", true), Some(9));
        assert_eq!(find_match("concat", "cat", true), None);
        assert_eq!(find_match("concat", "cat", false), Some(3));
    }

    #[test]
    fn a_long_line_is_cut_around_the_match() {
        let line = format!("{}{}NEEDLE{}", "a".repeat(300), "  ", "b".repeat(300));
        let p = preview(&line, 300);
        // `core::preview` strips the indentation, cuts a 160-wide window that
        // starts before the match, and marks only the cut on the left.
        assert!(p.starts_with('…'), "{p}");
        assert!(p.contains("NEEDLE"), "{p}");
        assert_eq!(p.chars().count(), 161);
    }

    #[test]
    fn a_write_is_atomic_and_keeps_crlf() {
        let dir = std::env::temp_dir().join(format!("fluxcode-server-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        write_file(f.to_str().unwrap(), b"one\ntwo\n").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "one\ntwo\n");
        write_file(f.to_str().unwrap(), b"three\n").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "three\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_listing_puts_folders_first() {
        let dir = std::env::temp_dir().join(format!("fluxcode-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("zzz")).unwrap();
        std::fs::write(dir.join("aaa.txt"), b"x").unwrap();
        let entries = list(dir.to_str().unwrap()).unwrap();
        assert!(entries[0].is_dir, "{entries:?}");
        assert_eq!(entries[1].name, "aaa.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
