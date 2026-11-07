//! Flux Terminal backend.
//!
//! Commands run one at a time through `cmd.exe` (fast startup) or PowerShell,
//! with FluxCode keeping track of the working directory, so there is no shell
//! process idling in the background. Folder listings are done natively here
//! (no `dir` process). Every working directory has to be covered by an access
//! grant the user gave first: the workspace (for the session), a folder they
//! allowed once or always, or full disk access.

use crate::{command, start_process, Processes};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager, State};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

// ------------------------------------------------------------- access grants

#[derive(Default, Serialize, Deserialize)]
struct Grants {
    #[serde(default)]
    all: bool,
    #[serde(default)]
    roots: Vec<String>,
}

/// Folder-access grants: persisted ones (`access.json` in the app config dir)
/// plus session-only ones that are forgotten when FluxCode closes.
pub struct Access {
    saved: Mutex<Grants>,
    session: Mutex<Vec<String>>,
    file: Option<PathBuf>,
}

/// Lower-case, backslashes, no trailing separator: `C:\Users\Me`.
fn norm(path: &str) -> String {
    path.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

fn is_under(path: &str, root: &str) -> bool {
    path == root || (path.starts_with(root) && path[root.len()..].starts_with('\\'))
}

impl Access {
    pub fn load(app: &AppHandle) -> Self {
        let file = app.path().app_config_dir().ok().map(|d| d.join("access.json"));
        let saved = file
            .as_ref()
            .and_then(|f| fs::read_to_string(f).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { saved: Mutex::new(saved), session: Mutex::new(Vec::new()), file }
    }

    fn save(&self) {
        let Some(file) = &self.file else { return };
        if let Some(dir) = file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let json = serde_json::to_string_pretty(&*self.saved.lock().unwrap()).unwrap_or_default();
        let _ = fs::write(file, json);
    }

    fn allows(&self, path: &str) -> bool {
        let p = norm(path);
        let saved = self.saved.lock().unwrap();
        if saved.all {
            return true;
        }
        let session = self.session.lock().unwrap();
        saved.roots.iter().chain(session.iter()).any(|r| is_under(&p, &norm(r)))
    }
}

#[derive(Serialize)]
pub struct AccessInfo {
    granted: bool,
    all: bool,
}

#[tauri::command]
pub fn access_check(access: State<'_, Access>, path: String) -> AccessInfo {
    AccessInfo { granted: access.allows(&path), all: access.saved.lock().unwrap().all }
}

/// `scope`: "session" (until FluxCode closes), "always" (this folder, remembered)
/// or "all" (full disk access, remembered).
#[tauri::command]
pub fn access_grant(access: State<'_, Access>, path: String, scope: String) -> Result<(), String> {
    match scope.as_str() {
        "session" => access.session.lock().unwrap().push(path),
        "always" => {
            access.saved.lock().unwrap().roots.push(path);
            access.save();
        }
        "all" => {
            access.saved.lock().unwrap().all = true;
            access.save();
        }
        other => return Err(format!("Unknown access scope '{other}'.")),
    }
    Ok(())
}

/// Forget every grant (the workspace is granted again when it is reopened).
#[tauri::command]
pub fn access_reset(access: State<'_, Access>) {
    *access.saved.lock().unwrap() = Grants::default();
    access.session.lock().unwrap().clear();
    access.save();
}

// ----------------------------------------------------------------- listing

#[derive(Serialize)]
pub struct TermEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    /// Seconds since the Unix epoch.
    modified: u64,
    hidden: bool,
}

fn is_hidden(name: &str, meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    if meta.file_attributes() & 0x2 != 0 {
        return true;
    }
    let _ = meta;
    name.starts_with('.')
}

/// Native folder listing for `ls`/`dir`, the path bar and Tab completion.
#[tauri::command]
pub async fn term_list(access: State<'_, Access>, path: String) -> Result<Vec<TermEntry>, String> {
    if !access.allows(&path) {
        return Err("access-denied".into());
    }
    let mut out: Vec<TermEntry> = fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = fs::metadata(e.path()).or_else(|_| e.metadata()).ok()?;
            Some(TermEntry {
                hidden: is_hidden(&name, &meta),
                is_dir: meta.is_dir(),
                size: if meta.is_dir() { 0 } else { meta.len() },
                modified: meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()),
                path: e.path().to_string_lossy().into_owned(),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\"))
}

/// Resolve what the user typed (`..`, `...`, `~`, `D:`, `src\app`, an absolute
/// path) against `cwd` to an existing folder. Errors if it isn't one.
#[tauri::command]
pub fn term_resolve(cwd: String, input: String) -> Result<String, String> {
    let raw = input.trim().trim_matches('"');
    let target: PathBuf = if raw.is_empty() || raw == "~" {
        home_dir()
    } else if let Some(rest) = raw.strip_prefix("~\\").or_else(|| raw.strip_prefix("~/")) {
        home_dir().join(rest)
    } else if raw.len() >= 3 && raw.chars().all(|c| c == '.') {
        // "..." = up two levels, "...." = up three, …
        let mut p = PathBuf::from(&cwd);
        for _ in 1..raw.len() {
            p.push("..");
        }
        p
    } else if raw.len() == 2 && raw.ends_with(':') {
        PathBuf::from(format!("{raw}\\")) // "D:" means the drive root
    } else {
        Path::new(&cwd).join(raw)
    };
    let canon = fs::canonicalize(&target).map_err(|_| format!("'{raw}' is not a folder."))?;
    if !canon.is_dir() {
        return Err(format!("'{raw}' is not a folder."));
    }
    let s = canon.to_string_lossy();
    Ok(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

#[derive(Serialize)]
pub struct TermEnv {
    home: String,
    drives: Vec<String>,
    shells: Vec<String>,
    user: String,
    computer: String,
}

fn on_path(exe: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(exe).is_file()))
}

/// Home folder, drives and available shells (no process is started for this).
#[tauri::command]
pub fn term_env() -> TermEnv {
    let drives = (b'C'..=b'Z')
        .map(|c| format!("{}:\\", c as char))
        .filter(|d| Path::new(d).exists())
        .collect();
    let mut shells = vec!["cmd".to_string(), "powershell".to_string()];
    if on_path("pwsh.exe") {
        shells.push("pwsh".into());
    }
    TermEnv {
        home: home_dir().to_string_lossy().into_owned(),
        drives,
        shells,
        user: std::env::var("USERNAME").unwrap_or_default(),
        computer: std::env::var("COMPUTERNAME").unwrap_or_default(),
    }
}

/// Is `name` a program on PATH (any PATHEXT extension)? Lets the terminal tell
/// "go into folder `python`" apart from "run python" without starting anything.
#[tauri::command]
pub fn term_is_command(name: String) -> bool {
    let exts: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
        .split(';')
        .filter(|e| !e.is_empty())
        .map(str::to_lowercase)
        .collect();
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| exts.iter().any(|ext| dir.join(format!("{name}{ext}")).is_file()))
}

// ------------------------------------------------------------------ running

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Run one command line in `cwd`. Output streams as `proc-output`, the end as
/// `proc-exit` (same events as the Output panel); stdin/stop use the shared
/// `write_stdin` / `kill_process` commands.
#[tauri::command]
pub fn term_run(
    app: AppHandle,
    procs: State<'_, Processes>,
    access: State<'_, Access>,
    line: String,
    cwd: String,
    shell: String,
) -> Result<u32, String> {
    if !access.allows(&cwd) {
        return Err("access-denied".into());
    }
    let mut cmd = match shell.as_str() {
        "powershell" | "pwsh" => {
            // -EncodedCommand avoids every command-line quoting pitfall.
            let script = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8;$OutputEncoding=[Text.Encoding]::UTF8;{line}");
            let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let mut c = command(if shell == "pwsh" { "pwsh" } else { "powershell" });
            c.args(["-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &base64(&utf16)]);
            c
        }
        _ => {
            let mut c = command("cmd.exe");
            // /s keeps the line verbatim; chcp 65001 makes cmd's output UTF-8.
            #[cfg(windows)]
            c.raw_arg(format!("/d /s /c \"chcp 65001>nul & {line}\""));
            #[cfg(not(windows))]
            c.args(["-c", &line]);
            c
        }
    };
    cmd.current_dir(&cwd)
        // Ask tools for colors even though stdout is a pipe; the terminal renders ANSI colors.
        .env("FORCE_COLOR", "1")
        .env("CLICOLOR_FORCE", "1");
    // A GitHub sign-in in Settings makes `gh` and token-aware tools work here too.
    if let Some(token) = crate::github::token(&app) {
        cmd.env("GH_TOKEN", &token).env("GITHUB_TOKEN", &token);
    }
    start_process(&app, &procs, cmd, &shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn grants_cover_subfolders_only() {
        assert!(is_under(&norm("C:\\Users\\Me\\src"), &norm("c:/users/me")));
        assert!(is_under(&norm("C:\\Users\\Me"), &norm("C:\\Users\\Me\\")));
        assert!(!is_under(&norm("C:\\Users\\Mel"), &norm("C:\\Users\\Me")));
        assert!(is_under(&norm("D:\\Games"), &norm("D:\\")));
    }
}
