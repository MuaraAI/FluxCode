
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "__pycache__", ".venv", "venv", ".mypy_cache",
    ".pytest_cache", ".ruff_cache", ".tox", "dist", ".next", ".idea",
];

pub struct FileText {
    pub text: String,
    pub crlf: bool,
}

pub fn read_text(path: &Path) -> Result<FileText, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("File is too large to open ({} MB).", meta.len() / 1024 / 1024));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if crate::accel::has_zero(&bytes[..bytes.len().min(8192)]) {
        return Err("The file is binary and cannot be displayed in the editor.".into());
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let crlf = crate::accel::scan_file_stats(bytes, LONG_LINE).crlf;
    let text = String::from_utf8_lossy(bytes);
    Ok(FileText { text: if crlf { text.replace("\r\n", "\n") } else { text.into_owned() }, crlf })
}

pub fn decode_text(bytes: &[u8]) -> FileText {
    if let Some(t) = crate::viewer::decode_utf16(bytes) {
        let crlf = t.contains("\r\n");
        return FileText { text: if crlf { t.replace("\r\n", "\n") } else { t }, crlf };
    }
    let clean = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let crlf = crate::accel::scan_file_stats(clean, LONG_LINE).crlf;
    let text = String::from_utf8_lossy(clean);
    FileText { text: if crlf { text.replace("\r\n", "\n") } else { text.into_owned() }, crlf }
}

pub const LONG_LINE: usize = 300;

#[cfg(test)]
pub fn dominated_by_long_lines(text: &str) -> bool {
    crate::accel::scan_file_stats(text.as_bytes(), LONG_LINE).dominated_by_long_lines()
}

pub fn write_text(path: &Path, text: &str, crlf: bool) -> Result<(), String> {
    let data = if crlf { text.replace("\r\n", "\n").replace('\n', "\r\n") } else { text.to_string() };
    write_bytes(path, data.as_bytes())
}

pub fn write_bytes(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = path.with_file_name(format!(
        ".{}.fluxcode-tmp",
        path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
    ));
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
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
        const FLAGS: u16 = 0x0004 | 0x0010 | 0x0040 | 0x0400;
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


const MAX_SEARCH_HITS: usize = 5_000;
const MAX_SEARCH_FILE_BYTES: u64 = 8 * 1024 * 1024;

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
        while workers.iter().any(|w| !w.is_finished()) {
            if stop() {
                stop_flag.store(true, Ordering::Relaxed);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
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

pub(crate) fn git_files(root: &Path) -> Option<Vec<PathBuf>> {
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

fn search_file(path: &Path, needle: &str, match_case: bool, whole_word: bool) -> Option<FileHits> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_SEARCH_FILE_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if crate::accel::has_zero(&bytes[..bytes.len().min(8192)]) {
        return None;
    }
    let mut hits = Vec::new();
    let ascii = match_case || needle.is_ascii();
    if ascii {
        let nb = needle.as_bytes();
        let find = |hay: &[u8]| {
            if match_case {
                crate::accel::find_bytes(hay, nb)
            } else {
                crate::accel::find_ascii_ci(hay, nb)
            }
        };
        let mut offset = 0usize;
        let mut line_no = 1u32;
        let mut counted_to = 0usize;
        while offset < bytes.len() {
            let Some(rel) = find(&bytes[offset..]) else {
                break;
            };
            let pos = offset + rel;
            let line_start = bytes[..pos].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
            let nl = bytes[pos..].iter().position(|&b| b == b'\n').map(|i| pos + i);
            let mut line_end = nl.unwrap_or(bytes.len());
            if line_end > line_start && bytes[line_end - 1] == b'\r' {
                line_end -= 1;
            }
            if line_start > counted_to {
                line_no += bytes[counted_to..line_start].iter().filter(|&&b| b == b'\n').count() as u32;
                counted_to = line_start;
            }
            let raw_line = String::from_utf8_lossy(&bytes[line_start..line_end]);
            let hay_line: std::borrow::Cow<str> = if match_case {
                std::borrow::Cow::Borrowed(&raw_line)
            } else {
                raw_line.to_ascii_lowercase().into()
            };
            if let Some(at) = find_match(&hay_line, needle, whole_word) {
                let match_char = hay_line[..at].chars().count();
                hits.push(Hit { line: line_no, text: preview(&raw_line, match_char) });
                if hits.len() >= MAX_SEARCH_HITS {
                    break;
                }
            }
            offset = nl.map_or(bytes.len(), |i| i + 1);
        }
    } else {
        let text = String::from_utf8_lossy(&bytes);
        let folded = text.to_lowercase();
        if !folded.contains(needle) {
            return None;
        }
        for (n, line) in text.lines().enumerate() {
            let hay = line.to_lowercase();
            if let Some(at) = find_match(&hay, needle, whole_word) {
                let match_char = hay[..at].chars().count();
                hits.push(Hit { line: n as u32 + 1, text: preview(line, match_char) });
                if hits.len() >= MAX_SEARCH_HITS {
                    break;
                }
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

    #[test]
    fn search_file_handles_case_whole_word_crlf_unicode_and_binary() {
        let dir = std::env::temp_dir().join(format!("fluxcode-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("sample.txt");
        fs::write(&p, b"first line\r\nfoobar FoO\r\nCAF\xc3\x89 end\r\n").unwrap();

        let fh = search_file(&p, "foo", false, true).unwrap();
        assert_eq!(fh.hits.len(), 1);
        assert_eq!(fh.hits[0].line, 2);
        assert_eq!(fh.hits[0].text, "foobar FoO");

        let exact = search_file(&p, "FoO", true, false).unwrap();
        assert_eq!(exact.hits.len(), 1);
        assert_eq!(exact.hits[0].line, 2);

        let uni = search_file(&p, "café", false, false).unwrap();
        assert_eq!(uni.hits[0].line, 3);

        let bin = dir.join("bin.dat");
        fs::write(&bin, b"foo\0bar").unwrap();
        assert!(search_file(&bin, "foo", false, false).is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
