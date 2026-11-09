//! OpenCode over the Agent Client Protocol (ACP): FluxCode starts
//! `opencode acp` on demand and bridges its stdio JSON-RPC stream to the
//! frontend line by line. The protocol itself lives in src/agent.js; this
//! side only spawns, pipes and stops the process.

use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::command;

#[derive(Default)]
pub struct Acp {
    proc: Mutex<Option<Running>>,
    version: Mutex<Option<String>>,
}

struct Running {
    child: Child,
    stdin: ChildStdin,
}

#[derive(Clone, Serialize)]
struct Line {
    line: String,
}

#[derive(Clone, Serialize)]
struct Exit {
    code: Option<i32>,
}

#[derive(Serialize)]
pub struct Info {
    path: Option<String>,
    version: Option<String>,
    running: bool,
}

/// Where the ACP AI Agent CLI lives: prioritize native MuaraAI `omm` (oh-my-mira),
/// then fallback to `opencode`.
fn find_agent() -> Option<PathBuf> {
    // 1. Priority: native `omm` (oh-my-mira)
    let mut omm_candidates: Vec<PathBuf> = Vec::new();
    if !cfg!(windows) {
        if let Ok(home) = std::env::var("HOME") {
            omm_candidates.push(Path::new(&home).join(".local").join("bin").join("omm"));
            omm_candidates.push(Path::new(&home).join(".cargo").join("bin").join("omm"));
        }
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                omm_candidates.push(dir.join("omm"));
            }
        }
        omm_candidates.push(PathBuf::from("/usr/local/bin/omm"));
    }
    if cfg!(windows) {
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                omm_candidates.push(dir.join("omm.exe"));
            }
        }
        for var in ["USERPROFILE", "LOCALAPPDATA", "APPDATA"] {
            if let Ok(base) = std::env::var(var) {
                let b = Path::new(&base);
                omm_candidates.push(b.join(".cargo").join("bin").join("omm.exe"));
                omm_candidates.push(b.join(".local").join("bin").join("omm.exe"));
                omm_candidates.push(b.join("Programs").join("omm").join("omm.exe"));
            }
        }
    }
    if let Some(p) = omm_candidates.into_iter().find(|p| p.is_file()) {
        return Some(p);
    }

    // 2. Fallback: opencode
    find_opencode()
}

/// Where the OpenCode CLI lives: PATH (exe, or the npm .cmd shim resolved to
/// its exe), the official installer folder, or the npm global folder.
fn find_opencode() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if !cfg!(windows) {
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                candidates.push(dir.join("opencode"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(Path::new(&home).join(".opencode").join("bin").join("opencode"));
            candidates.push(Path::new(&home).join(".local").join("bin").join("opencode"));
        }
        candidates.push(PathBuf::from("/usr/local/bin/opencode"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("opencode.exe"));
            // npm shim: <dir>\opencode.cmd runs <dir>\node_modules\@opencode\cli\bin\opencode.exe
            if dir.join("opencode.cmd").is_file() {
                candidates.push(dir.join("node_modules").join("@opencode").join("cli").join("bin").join("opencode.exe"));
                candidates.push(dir.join("node_modules").join("opencode-ai").join("bin").join("opencode.exe"));
            }
        }
    }
    for var in ["USERPROFILE", "LOCALAPPDATA", "APPDATA"] {
        if let Ok(base) = std::env::var(var) {
            let b = Path::new(&base);
            candidates.push(b.join(".opencode").join("bin").join("opencode.exe"));
            candidates.push(b.join("Programs").join("opencode").join("opencode.exe"));
            candidates.push(b.join("npm").join("node_modules").join("@opencode").join("cli").join("bin").join("opencode.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn agent_cmd() -> Option<Command> {
    if let Some(exe) = find_agent() {
        return Some(command(exe.to_string_lossy().as_ref()));
    }
    // Last resort on Windows: whatever `opencode` resolves to through cmd.exe.
    if cfg!(windows) {
        let on_path = std::env::var_os("PATH")
            .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("opencode.cmd").is_file() || d.join("opencode.bat").is_file()));
        if on_path {
            let mut c = command("cmd.exe");
            c.args(["/d", "/c", "opencode"]);
            return Some(c);
        }
    }
    None
}

#[tauri::command]
pub fn acp_info(acp: State<'_, Acp>) -> Info {
    let path = find_agent().map(|p| p.to_string_lossy().into_owned());
    let running = acp.proc.lock().unwrap().is_some();
    let mut version = acp.version.lock().unwrap().clone();
    if version.is_none() && path.is_some() {
        if let Some(mut c) = agent_cmd() {
            version = c.arg("--version").output().ok().filter(|o| o.status.success()).map(|o| {
                let raw = String::from_utf8_lossy(&o.stdout);
                raw.trim()
                    .trim_start_matches("opencode")
                    .trim_start_matches("omm")
                    .trim()
                    .to_string()
            });
            *acp.version.lock().unwrap() = version.clone();
        }
    }
    Info { path, version, running }
}

/// Start ACP agent (`omm acp` or `opencode acp`) in `cwd` (stopping a previous one first).
/// Each stdout line arrives as an `acp-line` event; stderr as `acp-log`; the end as `acp-exit`.
#[tauri::command]
pub fn acp_start(app: AppHandle, acp: State<'_, Acp>, cwd: Option<String>) -> Result<String, String> {
    stop_inner(&acp);
    let mut cmd = agent_cmd().ok_or("No ACP agent found. Install `omm` (oh-my-mira) from MuaraAI or `opencode`.")?;
    cmd.arg("acp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0");
    if let Some(dir) = cwd.as_deref().filter(|d| Path::new(d).is_dir()) {
        cmd.current_dir(dir);
    }
    if let Some(token) = crate::github::token(&app) {
        cmd.env("GH_TOKEN", &token).env("GITHUB_TOKEN", &token);
    }
    let mut child = cmd.spawn().map_err(|e| format!("OpenCode could not start: {e}"))?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take();
    let pid = child.id();

    let a = app.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                let _ = a.emit("acp-line", Line { line });
            }
        }
    });
    if let Some(err) = stderr {
        let a = app.clone();
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = a.emit("acp-log", Line { line });
            }
        });
    }
    *acp.proc.lock().unwrap() = Some(Running { child, stdin });

    // Watch for the process ending (crash, auth prompt, stop) without blocking.
    let a = app.clone();
    thread::spawn(move || loop {
        thread::sleep(std::time::Duration::from_millis(400));
        let acp = a.state::<Acp>();
        let mut guard = acp.proc.lock().unwrap();
        let Some(r) = guard.as_mut() else { break };
        if r.child.id() != pid {
            break; // a newer process replaced this one
        }
        if let Ok(Some(status)) = r.child.try_wait() {
            *guard = None;
            drop(guard);
            let _ = a.emit("acp-exit", Exit { code: status.code() });
            break;
        }
    });
    Ok(find_agent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "agent".into()))
}

/// Send one JSON-RPC message (one line) to the agent.
#[tauri::command]
pub fn acp_send(acp: State<'_, Acp>, line: String) -> Result<(), String> {
    let mut guard = acp.proc.lock().unwrap();
    let r = guard.as_mut().ok_or("Agent is not running.")?;
    r.stdin
        .write_all(line.as_bytes())
        .and_then(|_| r.stdin.write_all(b"\n"))
        .and_then(|_| r.stdin.flush())
        .map_err(|e| e.to_string())
}

fn stop_inner(acp: &Acp) {
    if let Some(mut r) = acp.proc.lock().unwrap().take() {
        #[cfg(windows)]
        {
            let _ = command("taskkill").args(["/T", "/F", "/PID", &r.child.id().to_string()]).status();
        }
        let _ = r.child.kill();
        let _ = r.child.wait();
    }
}

#[tauri::command]
pub fn acp_stop(acp: State<'_, Acp>) {
    stop_inner(&acp);
}

/// Kill the agent when the window closes, so nothing lingers.
pub fn shutdown(app: &AppHandle) {
    if let Some(acp) = app.try_state::<Acp>() {
        stop_inner(&acp);
    }
}
