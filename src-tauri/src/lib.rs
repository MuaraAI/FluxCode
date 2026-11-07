//! FluxCode native backend.
//!
//! The UI lives in the WebView (HTML/CSS/JS). Everything that touches the
//! machine — the file system and child processes — lives here in Rust, so the
//! frontend stays a thin, fast view layer.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

mod terminal;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Don't flash a console window for child processes.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Files larger than this are refused instead of freezing the editor.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Quick-open stops indexing after this many files.
const MAX_INDEXED_FILES: usize = 50_000;
/// Directories never indexed by quick-open (VS Code's usual excludes + common heavy dirs).
const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "__pycache__", ".venv", "venv", ".mypy_cache",
    ".pytest_cache", ".ruff_cache", ".tox", "dist", ".next", ".idea",
];

pub(crate) fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

// ---------------------------------------------------------------- file system

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
    is_dir: bool,
}

/// One level of a directory, folders first, case-insensitive — like VS Code's explorer.
#[tauri::command]
fn list_dir(path: String) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                return None;
            }
            // Follow symlinks so linked folders still expand.
            let is_dir = fs::metadata(e.path()).map(|m| m.is_dir()).unwrap_or(false);
            Some(Entry { name, path: e.path().to_string_lossy().into_owned(), is_dir })
        })
        .collect();
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// Every file under `root` (skipping heavy folders), for Ctrl+P quick open.
#[tauri::command]
async fn list_files(root: String) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let mut stack = vec![PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                let name = e.file_name();
                if !SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                    stack.push(e.path());
                }
            } else if ft.is_file() {
                files.push(e.path().to_string_lossy().into_owned());
                if files.len() >= MAX_INDEXED_FILES {
                    return Ok(files);
                }
            }
        }
    }
    Ok(files)
}

// --------------------------------------------------------- find in files

/// Matches returned by one search, in total (keeps the UI and IPC light).
const MAX_SEARCH_HITS: usize = 5_000;
/// Files larger than this are skipped by find-in-files (minified bundles, logs, …).
const MAX_SEARCH_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize)]
struct Hit {
    line: u32,
    text: String,
}

#[derive(Serialize)]
struct FileHits {
    path: String,
    hits: Vec<Hit>,
}

#[derive(Serialize)]
struct SearchResult {
    files: Vec<FileHits>,
    total: usize,
    truncated: bool,
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
    const WIDTH: usize = 220;
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

fn search_blocking(root: String, query: String, match_case: bool, whole_word: bool) -> SearchResult {
    let needle = if match_case { query.clone() } else { query.to_lowercase() };
    let mut result = SearchResult { files: Vec::new(), total: 0, truncated: false };
    if needle.is_empty() {
        return result;
    }
    let mut stack = vec![PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        // Push dirs in reverse so they are visited in name order.
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
                continue; // binary
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
                result.files.push(FileHits { path: e.path().to_string_lossy().into_owned(), hits });
            }
            if result.truncated {
                return result;
            }
        }
    }
    result
}

/// Find in files (Ctrl+Shift+F). Runs on a blocking thread so the UI stays responsive.
#[tauri::command]
async fn search_files(root: String, query: String, match_case: bool, whole_word: bool) -> Result<SearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || search_blocking(root, query, match_case, whole_word))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct FileContent {
    text: String,
    /// True when the file was not valid UTF-8 and had to be decoded lossily.
    lossy: bool,
}

#[tauri::command]
async fn read_file(path: String) -> Result<FileContent, String> {
    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("File is too large to open ({} MB).", meta.len() / 1024 / 1024));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err("The file is binary and cannot be displayed in the editor.".into());
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes); // drop UTF-8 BOM
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(FileContent { text: s.to_owned(), lossy: false }),
        Err(_) => Ok(FileContent { text: String::from_utf8_lossy(bytes).into_owned(), lossy: true }),
    }
}

/// Atomic save: write a sibling temp file, then rename over the original,
/// so a crash mid-save never leaves a half-written file.
#[tauri::command]
async fn write_file(path: String, contents: String) -> Result<(), String> {
    let target = Path::new(&path);
    let tmp = target.with_file_name(format!(
        ".{}.fluxcode-tmp",
        target.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
    ));
    fs::write(&tmp, contents.as_bytes()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, target).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

#[tauri::command]
fn create_file(path: String) -> Result<(), String> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_dir(path: String) -> Result<(), String> {
    fs::create_dir(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_path(from: String, to: String) -> Result<(), String> {
    if Path::new(&to).exists() {
        return Err(format!("'{to}' already exists."));
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ processes

struct Proc {
    child: Child,
    stdin: Option<ChildStdin>,
}

#[derive(Default)]
pub(crate) struct Processes {
    next_id: AtomicU32,
    running: Mutex<HashMap<u32, Proc>>,
}

#[derive(Clone, Serialize)]
struct OutputEvent {
    id: u32,
    stream: &'static str,
    data: String,
}

#[derive(Clone, Serialize)]
struct ExitEvent {
    id: u32,
    code: Option<i32>,
}

/// Stream a pipe to the frontend in chunks (not lines), so prompts such as
/// `input("Name: ")` show up before the newline. Incomplete UTF-8 sequences at
/// a chunk edge are carried over to the next read.
fn pump(app: AppHandle, id: u32, stream: &'static str, mut pipe: impl Read) {
    let mut buf = vec![0u8; 64 * 1024];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let n = match pipe.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        carry.extend_from_slice(&buf[..n]);
        let valid = match std::str::from_utf8(&carry) {
            Ok(_) => carry.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(), // truncated char: wait for more
            Err(_) => carry.len(),                               // genuinely invalid: decode lossily
        };
        if valid > 0 {
            let data = String::from_utf8_lossy(&carry[..valid]).into_owned();
            carry.drain(..valid);
            let _ = app.emit("proc-output", OutputEvent { id, stream, data });
        }
    }
    if !carry.is_empty() {
        let data = String::from_utf8_lossy(&carry).into_owned();
        let _ = app.emit("proc-output", OutputEvent { id, stream, data });
    }
}

/// Start a program with piped stdio. Output arrives as `proc-output` events
/// and completion as `proc-exit`.
#[tauri::command]
fn spawn_process(
    app: AppHandle,
    procs: State<'_, Processes>,
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
) -> Result<u32, String> {
    let mut cmd = command(&program);
    cmd.args(&args)
        // Rust/cargo: keep colors off, the Output panel is plain text.
        .env("CARGO_TERM_COLOR", "never");
    if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
        cmd.current_dir(dir);
    }
    start_process(&app, &procs, cmd, &program)
}

/// Spawn `cmd` with piped stdio, register it, and stream its output/exit as events.
pub(crate) fn start_process(app: &AppHandle, procs: &Processes, mut cmd: Command, label: &str) -> Result<u32, String> {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Python: unbuffered + UTF-8 so output streams live and unicode prints work.
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1");
    let app = app.clone();
    let mut child = cmd.spawn().map_err(|e| format!("Failed to start '{label}': {e}"))?;

    let id = procs.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    procs.running.lock().unwrap().insert(id, Proc { child, stdin });

    let readers: Vec<_> = [
        stdout.map(|p| { let a = app.clone(); thread::spawn(move || pump(a, id, "stdout", p)) }),
        stderr.map(|p| { let a = app.clone(); thread::spawn(move || pump(a, id, "stderr", p)) }),
    ]
    .into_iter()
    .flatten()
    .collect();

    thread::spawn(move || {
        for r in readers {
            let _ = r.join();
        }
        let proc = {
            let procs = app.state::<Processes>();
            let removed = procs.running.lock().unwrap().remove(&id);
            removed
        };
        let code = proc.and_then(|mut p| p.child.wait().ok()).and_then(|s| s.code());
        let _ = app.emit("proc-exit", ExitEvent { id, code });
    });
    Ok(id)
}

/// Send text to a running process's stdin (for `input()` and friends).
#[tauri::command]
fn write_stdin(procs: State<'_, Processes>, id: u32, data: String) -> Result<(), String> {
    let mut running = procs.running.lock().unwrap();
    let proc = running.get_mut(&id).ok_or("Process is no longer running.")?;
    let stdin = proc.stdin.as_mut().ok_or("Process has no stdin.")?;
    stdin.write_all(data.as_bytes()).and_then(|_| stdin.flush()).map_err(|e| e.to_string())
}

/// Stop a process and everything it spawned.
#[tauri::command]
fn kill_process(procs: State<'_, Processes>, id: u32) -> Result<(), String> {
    let mut running = procs.running.lock().unwrap();
    let Some(proc) = running.get_mut(&id) else { return Ok(()) };
    #[cfg(windows)]
    {
        let pid = proc.child.id().to_string();
        let status = command("taskkill").args(["/T", "/F", "/PID", &pid]).status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }
    }
    proc.child.kill().map_err(|e| e.to_string())
}

// --------------------------------------------------------------- toolchains

#[derive(Serialize)]
struct Interpreter {
    path: String,
    version: String,
}

fn probe_version(program: &str) -> Option<String> {
    let out = command(program).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = if out.stdout.is_empty() { out.stderr } else { out.stdout };
    let text = String::from_utf8_lossy(&text).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Pick a Python the way VS Code does by default: the workspace's virtual
/// environment first, then whatever is on PATH.
#[tauri::command]
async fn resolve_python(workspace: Option<String>) -> Option<Interpreter> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(ws) = workspace.filter(|w| !w.is_empty()) {
        for venv in [".venv", "venv", "env"] {
            let p = Path::new(&ws).join(venv).join("Scripts").join("python.exe");
            if p.is_file() {
                candidates.push(p.to_string_lossy().into_owned());
            }
        }
    }
    candidates.extend(["python".into(), "py".into(), "python3".into()]);
    candidates.into_iter().find_map(|path| {
        let version = probe_version(&path)?;
        version.starts_with("Python").then_some(Interpreter { path, version })
    })
}

/// Folders/files passed on the command line (`fluxcode .`, `fluxcode main.py`).
#[tauri::command]
fn launch_paths() -> Vec<Entry> {
    std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter_map(|a| fs::canonicalize(a).ok())
        .map(|p| {
            // canonicalize() returns a verbatim `\\?\C:\...` path on Windows; show the normal form.
            let s = p.to_string_lossy();
            let path = s.strip_prefix(r"\\?\").unwrap_or(&s).to_string();
            Entry { name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), is_dir: p.is_dir(), path }
        })
        .collect()
}

// ----------------------------------------------------------------------- app

/// WebView2 switches: turn off browser services an editor never needs
/// (translate, sync, SmartScreen pings, component updates, …).
const WEBVIEW_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,\
AutofillServerCommunication,MediaRouter --disable-background-networking --disable-component-update \
--disable-sync --no-pings";

/// The main window is built here (not in tauri.conf.json) so the GPU switch
/// can be chosen at startup. Without GPU acceleration WebView2's GPU process
/// stays tiny: measured ~125 MB private memory in total vs ~275 MB with it,
/// and `--in-process-gpu` folds what is left into the browser process
/// (one process and ~10 MB fewer). Set `FLUXCODE_GPU=1` to turn hardware
/// acceleration back on.
///
/// The window has no OS frame: FluxCode draws a VS Code-style title bar
/// (menu, command center, window buttons) itself.
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    let gpu = std::env::var("FLUXCODE_GPU").is_ok_and(|v| v == "1");
    let args = if gpu { WEBVIEW_ARGS.to_string() } else { format!("{WEBVIEW_ARGS} --disable-gpu --in-process-gpu") };
    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
        .title("FluxCode")
        .inner_size(1280.0, 800.0)
        .min_inner_size(600.0, 400.0)
        .decorations(false)
        .shadow(true)
        .theme(Some(tauri::Theme::Dark))
        .background_color(tauri::window::Color(0x1f, 0x1f, 0x1f, 0xff))
        .disable_drag_drop_handler()
        .additional_browser_args(&args)
        .build()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(terminal::Access::load(app.handle()));
            Ok(create_main_window(app)?)
        })
        .manage(Processes::default())
        .invoke_handler(tauri::generate_handler![
            list_dir,
            list_files,
            read_file,
            write_file,
            create_file,
            create_dir,
            rename_path,
            spawn_process,
            write_stdin,
            kill_process,
            resolve_python,
            launch_paths,
            search_files,
            terminal::term_run,
            terminal::term_list,
            terminal::term_resolve,
            terminal::term_env,
            terminal::term_is_command,
            terminal::access_check,
            terminal::access_grant,
            terminal::access_reset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FluxCode");
}
