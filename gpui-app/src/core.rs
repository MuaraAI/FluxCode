//! UI-independent backend logic, ported from `src-tauri/src/lib.rs`
//! (file reading/writing, find in files, file creation).

use std::fs;
use std::path::{Path, PathBuf};

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

/// Atomic save: write a sibling temp file, then rename over the original,
/// so a crash mid-save never leaves a half-written file.
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

/// Move a file or folder to the Recycle Bin (Windows), so a delete from the
/// explorer can be undone. Elsewhere it is removed after the UI confirmed.
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

// --------------------------------------------------------- find in files

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

pub fn search(root: &Path, query: &str, match_case: bool, whole_word: bool) -> SearchResult {
    let needle = if match_case { query.to_string() } else { query.to_lowercase() };
    let mut result = SearchResult::default();
    if needle.is_empty() {
        return result;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
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
            let Ok(bytes) = fs::read(e.path()) else { continue };
            if bytes[..bytes.len().min(8192)].contains(&0) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let mut hits = Vec::new();
            for (n, line) in text.lines().enumerate() {
                let hay = if match_case { std::borrow::Cow::Borrowed(line) } else { std::borrow::Cow::Owned(line.to_lowercase()) };
                if let Some(at) = find_match(&hay, &needle, whole_word) {
                    let match_char = hay[..at].chars().count();
                    hits.push(Hit { line: n as u32 + 1, text: preview(line, match_char) });
                    result.total += 1;
                    if result.total >= MAX_SEARCH_HITS {
                        result.truncated = true;
                        break;
                    }
                }
            }
            if !hits.is_empty() {
                result.files.push(FileHits { path: e.path(), hits });
            }
            if result.truncated {
                return result;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
