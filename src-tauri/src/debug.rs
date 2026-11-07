//! Python debugging without debugpy: FluxCode ships `fluxdbg.py` (a small
//! debugger on the standard library's `bdb`), starts the program under it and
//! talks to it over a loopback socket, one JSON object per line. The program's
//! own stdio stays on the normal Output pipes, so `input()` keeps working.
//!
//! Rust only relays: lines from the helper become `dbg-msg` events, and
//! `debug_send` writes a line back. JavaScript debugging needs no Rust at all
//! (the webview talks to Node's inspector over WebSocket).

use crate::{command, start_process, Processes};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const HELPER: &str = include_str!("fluxdbg.py");
/// How long the helper has to connect back before the session is given up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct DebugChannels(Mutex<HashMap<u32, TcpStream>>);

#[derive(Serialize, Clone)]
struct DbgMsg {
    id: u32,
    msg: String,
}

#[derive(Serialize, Clone)]
struct DbgEnd {
    id: u32,
}

/// The helper script on disk (rewritten only when this build's copy differs).
fn helper_path() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("fluxcode-debug");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("fluxdbg.py");
    if fs::read_to_string(&path).ok().as_deref() != Some(HELPER) {
        fs::write(&path, HELPER).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// Start `python -u fluxdbg.py PORT script args…`. Returns the process id
/// (the same ids as `spawn_process`, so Output, stdin and Stop just work).
#[tauri::command]
pub fn debug_python(
    app: AppHandle,
    procs: State<'_, Processes>,
    python: String,
    script: String,
    args: Vec<String>,
    cwd: Option<String>,
) -> Result<u32, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let mut cmd = command(&python);
    cmd.arg("-u").arg(helper_path()?).arg(port.to_string()).arg(&script).args(&args);
    if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
        cmd.current_dir(dir);
    }
    let id = start_process(&app, &procs, cmd, &python)?;
    thread::spawn(move || relay(app, id, listener));
    Ok(id)
}

fn relay(app: AppHandle, id: u32, listener: TcpListener) {
    let stream = accept(&listener);
    drop(listener);
    if let Some(stream) = stream {
        let _ = stream.set_nodelay(true);
        if let Ok(writer) = stream.try_clone() {
            app.state::<DebugChannels>().0.lock().unwrap().insert(id, writer);
            for line in BufReader::new(stream).lines() {
                let Ok(msg) = line else { break };
                let _ = app.emit("dbg-msg", DbgMsg { id, msg });
            }
            app.state::<DebugChannels>().0.lock().unwrap().remove(&id);
        }
    }
    let _ = app.emit("dbg-end", DbgEnd { id });
}

/// Wait for the helper to connect, without blocking forever if it never starts.
fn accept(listener: &TcpListener) -> Option<TcpStream> {
    listener.set_nonblocking(true).ok()?;
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).ok()?;
                return Some(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(15)),
            Err(_) => return None,
        }
    }
    None
}

/// Send one JSON message to a debug session.
#[tauri::command]
pub fn debug_send(chans: State<'_, DebugChannels>, id: u32, msg: String) -> Result<(), String> {
    let mut map = chans.0.lock().unwrap();
    let stream = map.get_mut(&id).ok_or("The debug session has ended.")?;
    stream
        .write_all(msg.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .and_then(|_| stream.flush())
        .map_err(|e| e.to_string())
}
