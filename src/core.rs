//! UI-independent backend logic, ported from `src-tauri/src/lib.rs` (file reading/writing, find in files, file creation).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Files larger than this are refused instead of freezing the editor.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Directories never searched (VS Code's usual excludes + common heavy dirs).
pub const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "__pycache__", ".venv", "venv", ".mypy_cache",
    ".pytest_cache", ".ruff_cache", ".tox", "dist", ".next", ".idea",
];

pub struct FileText {
    pub text: String,
    /// The file used CRLF line endings (restored on save).
    pub crlf: bool,
}

pub fn read_text(path: &Path) -> Result<FileText, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("File is too large to open ({} MB).", meta.len() / 1024 / 1024));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err("The file is binary and cannot be displayed in the editor.".into());
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text = String::from_utf8_lossy(bytes);
    let crlf = text.contains("\r\n");
    Ok(FileText { text: if crlf { text.replace("\r\n", "\n") } else { text.into_owned() }, crlf })
}

/// Text from bytes already read: UTF-8 (BOM dropped) or UTF-16 with a BOM.
pub fn decode_text(bytes: &[u8]) -> FileText {
    let text = match crate::viewer::decode_utf16(bytes) {
        Some(t) => t,
        None => String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)).into_owned(),
    };
    let crlf = text.contains("\r\n");
    FileText { text: if crlf { text.replace("\r\n", "\n") } else { text }, crlf }
}

/// A line this long is past what anyone reads by scrolling sideways.
pub const LONG_LINE: usize = 300;

/// True when most of the text sits in very long lines: a minified bundle, a one-line JSON export, a data URI.
pub fn dominated_by_long_lines(text: &str) -> bool {
    let (mut long, mut short) = (0usize, 0usize);
    for line in text.lines() {
        if line.len() >= LONG_LINE {
            long += line.len();
        } else {
            short += line.len();
        }
    }
    long > 0 && long > short
}

/// Atomic save: write a sibling temp file, then rename over the original, so a crash mid-save never leaves a half-written file.
pub fn write_text(path: &Path, text: &str, crlf: bool) -> Result<(), String> {
    let data = if crlf { text.replace("\r\n", "\n").replace('\n', "\r\n") } else { text.to_string() };
    let tmp = path.with_file_name(format!(
        ".{}.fluxcode-tmp",
        path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
    ));
    fs::write(&tmp, data.as_bytes()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn create_file(path: &Path) -> Result<(), String> {
    fs::OpenOptions::new().write(true).create_new(true).open(path).map(|_| ()).map_err(|e| e.to_string())
}

pub fn create_dir(path: &Path) -> Result<(), String> {
    fs::create_dir(path).map_err(|e| e.to_string())
}

pub fn rename_path(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(format!("{} already exists", to.display()));
    }
    fs::rename(from, to).map_err(|e| e.to_string())
}

/// Move a file or folder to the Recycle Bin (Windows), so a delete from the explorer can be undone.
pub fn delete_path(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[repr(C)]
        struct ShFileOp {
            hwnd: *mut std::ffi::c_void,
            func: u32,
            from: *const u16,
            to: *const u16,
            flags: u16,
            aborted: i32,
            mappings: *mut std::ffi::c_void,
            title: *const u16,
        }
        #[link(name = "shell32")]
        extern "system" {
            fn SHFileOperationW(op: *mut ShFileOp) -> i32;
        }
        const FO_DELETE: u32 = 3;
        // FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO | FOF_NOERRORUI
        const FLAGS: u16 = 0x0004 | 0x0010 | 0x0040 | 0x0400;
        // pFrom is a list of paths ended by an extra NUL.
        let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
        let mut op = ShFileOp {
            hwnd: std::ptr::null_mut(),
            func: FO_DELETE,
            from: from.as_ptr(),
            to: std::ptr::null(),
            flags: FLAGS,
            aborted: 0,
            mappings: std::ptr::null_mut(),
            title: std::ptr::null(),
        };
        let rc = unsafe { SHFileOperationW(&mut op) };
        if rc != 0 || op.aborted != 0 {
            return Err(format!("Could not move {} to the Recycle Bin (code {rc})", path.display()));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }.map_err(|e| e.to_string())
    }
}


/// Matches returned by one search, in total.
const MAX_SEARCH_HITS: usize = 5_000;
/// Files larger than this are skipped by find-in-files (minified bundles, logs, ...).
const MAX_SEARCH_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct Hit {
    pub line: u32,
    pub text: String,
}

#[derive(Clone)]
pub struct FileHits {
    pub path: PathBuf,
    pub hits: Vec<Hit>,
}

#[derive(Clone, Default)]
pub struct SearchResult {
    pub files: Vec<FileHits>,
    pub total: usize,
    pub truncated: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Byte index of the first match of `needle` in `hay`, honoring whole-word.
fn find_match(hay: &str, needle: &str, whole_word: bool) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = hay[from..].find(needle) {
        let at = from + i;
        if !whole_word {
            return Some(at);
        }
        let before = hay[..at].chars().next_back();
        let after = hay[at + needle.len()..].chars().next();
        if !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char) {
            return Some(at);
        }
        from = at + needle.chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// Short single-line preview around the match (indentation stripped).
fn preview(line: &str, match_char: usize) -> String {
    const WIDTH: usize = 160;
    let indent = line.chars().take_while(|c| c.is_whitespace()).count();
    let text: Vec<char> = line.chars().skip(indent).collect();
    let at = match_char.saturating_sub(indent);
    if text.len() <= WIDTH {
        return text.into_iter().collect();
    }
    let start = at.saturating_sub(40).min(text.len().saturating_sub(WIDTH));
    let mut s: String = text[start..(start + WIDTH).min(text.len())].iter().collect();
    if start > 0 {
        s.insert(0, '…');
    }
    s
}

/// Find in files, giving up as soon as `stop` says so: live search starts a new search on every pause in typing, and the old one must not keep the disk busy.
pub fn search_until(root: &Path, query: &str, match_case: bool, whole_word: bool, stop: &dyn Fn() -> bool) -> SearchResult {
    let needle = if match_case { query.to_string() } else { query.to_lowercase() };
    let mut result = SearchResult::default();
    if needle.is_empty() {
        return result;
    }
    let files = git_files(root).unwrap_or_else(|| walk_files(root, stop));
    let next = AtomicUsize::new(0);
    let total = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8);
    let stop_flag = AtomicBool::new(false);
    let mut found: Vec<(usize, FileHits)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= files.len() || stop_flag.load(Ordering::Relaxed) || total.load(Ordering::Relaxed) >= MAX_SEARCH_HITS {
                            break;
                        }
                        if let Some(fh) = search_file(&files[i], &needle, match_case, whole_word) {
                            total.fetch_add(fh.hits.len(), Ordering::Relaxed);
                            mine.push((i, fh));
                        }
                    }
                    mine
                })
            })
            .collect();
        // The caller's `stop` is not Sync: it is polled here and passed on as a flag.
        while workers.iter().any(|w| !w.is_finished()) {
            if stop() {
                stop_flag.store(true, Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    found.sort_by_key(|(i, _)| *i);
    for (_, mut fh) in found {
        let room = MAX_SEARCH_HITS - result.total;
        if room == 0 {
            result.truncated = true;
            break;
        }
        if fh.hits.len() > room {
            fh.hits.truncate(room);
            result.truncated = true;
        }
        result.total += fh.hits.len();
        result.files.push(fh);
    }
    result
}

/// Every file git knows about or would not ignore, sorted; None outside a repository.
fn git_files(root: &Path) -> Option<Vec<PathBuf>> {
    if !root.join(".git").exists() {
        return None;
    }
    let out = crate::proc::command("git")
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut files: Vec<PathBuf> = out.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()).map(|p| root.join(String::from_utf8_lossy(p).as_ref())).collect();
    files.sort();
    files.dedup();
    Some(files)
}

/// Outside git: walk the folder, skipping the usual build and dependency folders.
fn walk_files(root: &Path, stop: &dyn Fn() -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if stop() {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.filter_map(Result::ok) {
            match e.file_type() {
                Ok(t) if t.is_dir() => {
                    let name = e.file_name();
                    if !SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                        stack.push(e.path());
                    }
                }
                Ok(t) if t.is_file() => files.push(e.path()),
                _ => {}
            }
        }
    }
    files.sort();
    files
}

/// The matching lines of one file (None: no match, binary, too large or unreadable).
fn search_file(path: &Path, needle: &str, match_case: bool, whole_word: bool) -> Option<FileHits> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_SEARCH_FILE_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    // ASCII folding keeps every byte where it was, so lines of the folded text line up with the original.
    let ascii = match_case || needle.is_ascii();
    let folded: std::borrow::Cow<str> = if match_case { std::borrow::Cow::Borrowed(&text) } else if ascii { text.to_ascii_lowercase().into() } else { text.to_lowercase().into() };
    if !folded.contains(needle) {
        return None;
    }
    let mut hits = Vec::new();
    let mut add = |n: usize, line: &str, hay: &str| {
        if let Some(at) = find_match(hay, needle, whole_word) {
            let match_char = hay[..at].chars().count();
            hits.push(Hit { line: n as u32 + 1, text: preview(line, match_char) });
        }
        hits.len() >= MAX_SEARCH_HITS
    };
    if ascii {
        for (n, (line, hay)) in text.lines().zip(folded.lines()).enumerate() {
            if add(n, line, hay) {
                break;
            }
        }
    } else {
        for (n, line) in text.lines().enumerate() {
            if add(n, line, &line.to_lowercase()) {
                break;
            }
        }
    }
    (!hits.is_empty()).then(|| FileHits { path: path.to_path_buf(), hits })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minified_files_count_as_long_lines() {
        let one_line = format!("{{\"matches\":[{}]}}", "\"https://*.example.com/*\",".repeat(40));
        assert!(dominated_by_long_lines(&one_line));
        let code = "fn main() {\n    println!(\"hi\");\n}\n".repeat(50);
        assert!(!dominated_by_long_lines(&code));
        // One long line among a lot of ordinary code stays unwrapped.
        assert!(!dominated_by_long_lines(&format!("{code}{}\n", "x".repeat(400))));
        assert!(!dominated_by_long_lines(""));
    }

    #[test]
    fn whole_word_skips_partial_matches() {
        assert_eq!(find_match("foobar foo", "foo", true), Some(7));
        assert_eq!(find_match("foobar", "foo", false), Some(0));
        assert_eq!(find_match("foobar", "foo", true), None);
    }

    #[test]
    fn crlf_round_trip() {
        let dir = std::env::temp_dir().join("fluxcode-gpui-core-test");
        let _ = fs::create_dir_all(&dir);
        let p = dir.join("crlf.txt");
        fs::write(&p, b"a\r\nb\r\n").unwrap();
        let t = read_text(&p).unwrap();
        assert!(t.crlf);
        assert_eq!(t.text, "a\nb\n");
        write_text(&p, "a\nb\nc\n", t.crlf).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"a\r\nb\r\nc\r\n");
    }
}
