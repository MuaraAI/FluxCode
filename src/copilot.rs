//! GitHub Copilot completions through the official Copilot language server.
//!
//! FluxCode stays a thin client: the server runs as its own process
//! (`@github/copilot-language-server`, Node.js 20.8 or later) and does the
//! model work, while FluxCode only sends the open document and shows what
//! comes back as ghost text. Nothing about the model lives in this binary,
//! and no code is sent anywhere except to the Copilot server itself, over
//! the user's own Copilot sign-in.
//!
//! Protocol: LSP over stdio with `Content-Length` framing, as documented in
//! the server's README (`@github/copilot-language-server`, version 1.570).
//! The calls used here are `initialize`, `initialized`,
//! `workspace/didChangeConfiguration`, `textDocument/didOpen`,
//! `textDocument/didChange`, `textDocument/didClose`,
//! `textDocument/inlineCompletion`, `signIn` and `signOut`. After the user
//! accepts a suggestion, Copilot also wants the item's command run through
//! `workspace/executeCommand` and a `textDocument/didShowCompletion`
//! notification; the editor's ghost-text path has no hook for either, so
//! acceptance telemetry is not sent. Suggestions still work; only Copilot's
//! own accept counting misses them.
//!
//! Sign-in is the server's device flow: `signIn` returns a user code, the
//! server opens the browser, and a `didChangeStatus` notification reports
//! the new state a few seconds later. The sign-in code is shown in
//! Settings so the user can also open github.com/login/device by hand.
//!
//! The server is found on PATH first (`copilot-language-server`, installed
//! once with `npm install -g @github/copilot-language-server`), else as the
//! JavaScript distribution under the global npm root when Node.js is
//! present. `FLUXCODE_COPILOT_SERVER` overrides the discovery with a path
//! to a server binary or to a `language-server.js` file.

use lsp_types::InlineCompletionItem;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// How to install the server, shown when it is not found.
pub const INSTALL_HINT: &str =
    "npm install -g @github/copilot-language-server (needs Node.js 20.8 or later)";

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(30);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(20);
const SIGNIN_TIMEOUT: Duration = Duration::from_secs(60);

/// What the user sees in Settings. The raw server status (`didChangeStatus`)
/// is kept inside; this is the reading of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Switched off in Settings.
    Off,
    /// Starting, or waiting for the first status from the server.
    Starting,
    /// The server is not installed; Settings shows the install command.
    NotInstalled,
    /// Signed in and working.
    Ready,
    /// The server says Error, which is how it reports "not signed in".
    NotSignedIn(String),
    /// The server reports a temporary problem.
    Warning(String),
    /// The current file is ignored by Copilot (size or content exclusions).
    Inactive,
    /// Something else went wrong (spawn, initialize, the server exiting).
    Error(String),
}

#[derive(Clone, Debug)]
struct ServerStatus {
    kind: String,
    message: String,
}

struct DocState {
    version: i32,
    text: String,
}

struct State {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    next_id: u64,
    pending: HashMap<u64, mpsc::Sender<Result<Value, String>>>,
    docs: HashMap<String, DocState>,
    folders: Vec<PathBuf>,
    status: ServerStatus,
    notice: Option<String>,
    user_code: Option<String>,
    started: bool,
    starting: bool,
    not_installed: bool,
    start_error: Option<String>,
    last_attempt: Option<Instant>,
}

struct Shared {
    state: Mutex<State>,
    /// Serializes stdin writes, so a document sync always lands before the
    /// completion request that needs it, and a server request never waits
    /// behind a response that needs the same lock to arrive.
    io: Mutex<()>,
    /// Serializes starting the server, so two editors never spawn two.
    start_lock: Mutex<()>,
    enabled: AtomicBool,
}

/// A handle to the Copilot server. Cheap to clone; all state lives behind
/// the shared lock, so the editor, Settings and the reader thread can all
/// hold one.
#[derive(Clone)]
pub struct Copilot {
    shared: Arc<Shared>,
}

impl Copilot {
    pub fn new(enabled: bool) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    child: None,
                    stdin: None,
                    next_id: 0,
                    pending: HashMap::new(),
                    docs: HashMap::new(),
                    folders: Vec::new(),
                    status: ServerStatus {
                        kind: String::new(),
                        message: String::new(),
                    },
                    notice: None,
                    user_code: None,
                    started: false,
                    starting: false,
                    not_installed: false,
                    start_error: None,
                    last_attempt: None,
                }),
                io: Mutex::new(()),
                start_lock: Mutex::new(()),
                enabled: AtomicBool::new(enabled),
            }),
        }
    }

    pub fn enabled(&self) -> bool {
        self.shared.enabled.load(Ordering::SeqCst)
    }

    /// Turn Copilot on or off. Off stops the server, so its memory is freed;
    /// on starts it again lazily, on the next suggestion request.
    pub fn set_enabled(&self, enabled: bool) {
        self.shared.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            self.stop();
        }
    }

    pub fn status(&self) -> Status {
        if !self.enabled() {
            return Status::Off;
        }
        let st = self.shared.state.lock().unwrap();
        if st.not_installed {
            return Status::NotInstalled;
        }
        if let Some(e) = &st.start_error {
            return Status::Error(e.clone());
        }
        if !st.started {
            return Status::Starting;
        }
        match st.status.kind.as_str() {
            "Normal" => Status::Ready,
            "Error" => Status::NotSignedIn(st.status.message.clone()),
            "Warning" => Status::Warning(st.status.message.clone()),
            "Inactive" => Status::Inactive,
            _ => Status::Starting,
        }
    }

    /// The sign-in code from `signIn`, if one was issued and not yet used.
    pub fn user_code(&self) -> Option<String> {
        self.shared.state.lock().unwrap().user_code.clone()
    }

    /// The last notice the server sent (`window/showMessageRequest`).
    pub fn notice(&self) -> Option<String> {
        self.shared.state.lock().unwrap().notice.clone()
    }

    /// Stop the server, if it is running. Safe to call when it is not.
    pub fn stop(&self) {
        let child;
        {
            let mut st = self.shared.state.lock().unwrap();
            st.started = false;
            st.starting = false;
            st.stdin = None;
            st.docs.clear();
            st.folders.clear();
            child = st.child.take();
            for (_, tx) in st.pending.drain() {
                let _ = tx.send(Err("Copilot server stopped".to_string()));
            }
        }
        if let Some(mut child) = child {
            let _ = child.kill();
            // Reap off the calling thread; the reader thread also exits
            // when the server's stdout closes.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }

    /// Start the server if it is not running, for the folder `folder`
    /// (the parent of the file being edited). Idempotent: a second call
    /// only adds a new folder. A failed start is retried at most once a
    /// minute, so a missing server does not spawn a process per keystroke.
    fn ensure_started(&self, folder: Option<PathBuf>) -> Result<(), String> {
        if !self.enabled() {
            return Err("Copilot is off".to_string());
        }
        let _start = self.shared.start_lock.lock().unwrap();
        {
            let mut st = self.shared.state.lock().unwrap();
            if st.started {
                if let Some(f) = &folder {
                    if !st.folders.contains(f) {
                        st.folders.push(f.clone());
                        let uri = uri_for_path(f);
                        drop(st);
                        self.notify(
                            "workspace/didChangeWorkspaceFolders",
                            serde_json::json!({"event": {"added": [{"uri": uri, "name": folder_name(f)}], "removed": []}}),
                        );
                    }
                }
                return Ok(());
            }
            if st.starting {
                return Err("Copilot is starting".to_string());
            }
            if let Some(last) = st.last_attempt {
                if st.start_error.is_some() && last.elapsed() < Duration::from_secs(60) {
                    return Err(st.start_error.clone().unwrap_or_default());
                }
            }
            st.last_attempt = Some(Instant::now());
            st.starting = true;
        }
        let started = self.start_inner(folder);
        let mut st = self.shared.state.lock().unwrap();
        st.starting = false;
        match started {
            Ok(()) => {
                st.started = true;
                st.start_error = None;
                st.not_installed = false;
                Ok(())
            }
            Err(e) => {
                st.start_error = Some(e.clone());
                st.not_installed = e.starts_with("not installed");
                Err(e)
            }
        }
    }

    /// Spawn the process and run `initialize`. Runs under `start_lock`.
    fn start_inner(&self, folder: Option<PathBuf>) -> Result<(), String> {
        let mut cmd = find_server()?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Could not start the Copilot server ({e}). {INSTALL_HINT}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Copilot server has no stdout".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or("Copilot server has no stdin".to_string())?;
        {
            let mut st = self.shared.state.lock().unwrap();
            st.child = Some(child);
            st.stdin = Some(stdin);
        }
        self.spawn_reader(stdout);
        let folders: Vec<Value> = folder
            .as_ref()
            .map(|f| vec![serde_json::json!({"uri": uri_for_path(f), "name": folder_name(f)})])
            .unwrap_or_default();
        let root_uri = folder.as_ref().map(|f| uri_for_path(f));
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "workspaceFolders": folders,
            "capabilities": {
                "workspace": {"workspaceFolders": true, "configuration": true},
                "textDocument": {
                    "synchronization": {"didSave": true},
                    "completion": {"completionItem": {}},
                    "inlineCompletion": {}
                },
                "window": {"showMessage": {}, "showDocument": {"support": true}}
            },
            "initializationOptions": {
                "editorInfo": {"name": "FluxCode", "version": crate::update::BUILD},
                "editorPluginInfo": {"name": "GitHub Copilot for FluxCode", "version": crate::update::BUILD}
            }
        });
        let result = self.request("initialize", Some(params), INITIALIZE_TIMEOUT);
        if let Err(e) = result {
            self.stop();
            return Err(format!("Copilot server did not answer initialize ({e})"));
        }
        self.notify("initialized", serde_json::json!({}));
        self.notify(
            "workspace/didChangeConfiguration",
            serde_json::json!({"settings": {"telemetry": {"telemetryLevel": "off"}}}),
        );
        if let Some(f) = folder {
            let mut st = self.shared.state.lock().unwrap();
            if !st.folders.contains(&f) {
                st.folders.push(f);
            }
        }
        Ok(())
    }

    fn spawn_reader(&self, stdout: ChildStdout) {
        let shared = Arc::clone(&self.shared);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut pending_fail: Vec<mpsc::Sender<Result<Value, String>>> = Vec::new();
            while let Some(msg) = read_message(&mut reader) {
                handle_server_message(&shared, msg);
            }
            // The server exited: fail what is still waiting, and reap it.
            let child;
            {
                let mut st = shared.state.lock().unwrap();
                st.started = false;
                st.stdin = None;
                child = st.child.take();
                if st.start_error.is_none() {
                    st.start_error = Some("Copilot server exited".to_string());
                }
                for (_, tx) in st.pending.drain() {
                    pending_fail.push(tx);
                }
            }
            for tx in pending_fail {
                let _ = tx.send(Err("Copilot server exited".to_string()));
            }
            if let Some(mut child) = child {
                let _ = child.wait();
            }
        });
    }

    /// Send a request and wait for its result. The write is serialized;
    /// the wait is not, so the reader thread can deliver the answer.
    /// The id is allocated and the message is written in separate short
    /// locks: holding the stdin borrow while touching the rest of the
    /// state does not compile, and the io lock already keeps other
    /// writers out between the two steps.
    fn request(
        &self,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, String> {
        let mut msg = serde_json::json!({"jsonrpc": "2.0", "method": method});
        if let Some(p) = params {
            msg["params"] = p;
        }
        let (id, rx) = {
            let _io = self.shared.io.lock().unwrap();
            let (id, rx) = {
                let mut st = self.shared.state.lock().unwrap();
                if st.stdin.is_none() {
                    return Err("Copilot server is not running".to_string());
                }
                st.next_id += 1;
                let id = st.next_id;
                let (tx, rx) = mpsc::channel();
                st.pending.insert(id, tx);
                (id, rx)
            };
            msg["id"] = serde_json::json!(id);
            let wrote = {
                let mut st = self.shared.state.lock().unwrap();
                match st.stdin.as_mut() {
                    Some(stdin) => write_message(stdin, &msg).is_ok(),
                    None => false,
                }
            };
            if !wrote {
                self.shared.state.lock().unwrap().pending.remove(&id);
                return Err("Could not write to the Copilot server".to_string());
            }
            (id, rx)
        };
        match rx.recv_timeout(timeout) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.shared.state.lock().unwrap().pending.remove(&id);
                Err(format!("Copilot server did not answer {method} in time"))
            }
            Err(_) => Err("Copilot server exited".to_string()),
        }
    }

    /// Send a notification (no answer expected).
    // This stays synchronous on purpose: `inline` sends didOpen/didChange
    // and the completion request under one lock, in order. Only didClose
    // moves off-thread; it has no ordering promise.
    fn notify(&self, method: &str, params: Value) {
        let _io = self.shared.io.lock().unwrap();
        let mut st = self.shared.state.lock().unwrap();
        if let Some(stdin) = st.stdin.as_mut() {
            let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
            let _ = write_message(stdin, &msg);
        }
    }

    /// The write goes out on its own thread: a pipe write blocks while the
    /// server is not draining it, so sending it from `close()` stalled the
    /// UI thread mid-Ctrl+W. A didClose has no ordering promise, so this is safe.
    pub fn did_close(&self, path: &Path) {
        let uri = uri_for_path(path);
        let was_open = self
            .shared
            .state
            .lock()
            .unwrap()
            .docs
            .remove(&uri)
            .is_some();
        if was_open {
            let shared = Arc::clone(&self.shared);
            std::thread::spawn(move || {
                let msg = serde_json::json!({"jsonrpc": "2.0", "method": "textDocument/didClose", "params": {"textDocument": {"uri": uri}}});
                let _io = shared.io.lock().unwrap();
                let mut st = shared.state.lock().unwrap();
                if let Some(stdin) = st.stdin.as_mut() {
                    let _ = write_message(stdin, &msg);
                }
            });
        }
    }

    /// Start the device-flow sign-in. Returns the user code; the server
    /// opens the browser for the rest. Settings shows the code as well, so
    /// the user can open github.com/login/device by hand if no browser opens.
    pub fn sign_in(&self) -> Result<String, String> {
        self.ensure_started(None)?;
        let result = self.request("signIn", None, SIGNIN_TIMEOUT)?;
        let code = result
            .get("userCode")
            .and_then(|c| c.as_str())
            .ok_or("Copilot sign-in gave no user code".to_string())?
            .to_string();
        {
            let mut st = self.shared.state.lock().unwrap();
            st.user_code = Some(code.clone());
        }
        if let Some(cmd) = result.get("command") {
            let command = cmd
                .get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            let args = cmd
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Array(Vec::new()));
            if !command.is_empty() {
                // Opens the browser; the server reports the new state with
                // didChangeStatus a few seconds after the user finishes.
                let _ = self.request(
                    "workspace/executeCommand",
                    Some(serde_json::json!({"command": command, "arguments": args})),
                    SIGNIN_TIMEOUT,
                );
            }
        }
        Ok(code)
    }

    pub fn sign_out(&self) {
        self.shared.state.lock().unwrap().user_code = None;
        if self.shared.state.lock().unwrap().started {
            let _ = self.request("signOut", None, SIGNIN_TIMEOUT);
        }
    }
}

/// The folder's display name for a workspace folder entry.
fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

// ------------------------------------------------------------------ server discovery

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// The command that runs the server, or an error whose text starts with
/// "not installed" when nothing was found.
fn find_server() -> Result<Command, String> {
    if let Ok(p) = std::env::var("FLUXCODE_COPILOT_SERVER") {
        let p = PathBuf::from(p);
        let mut c = if p.extension().is_some_and(|e| e == "js") {
            let mut c = crate::proc::command("node");
            c.arg(&p);
            c
        } else {
            crate::proc::command(
                p.to_str()
                    .ok_or("FLUXCODE_COPILOT_SERVER is not a valid path".to_string())?,
            )
        };
        c.arg("--stdio");
        return Ok(c);
    }
    if cfg!(windows) {
        if find_on_path("copilot-language-server.exe").is_some() {
            let mut c = crate::proc::command("copilot-language-server");
            c.arg("--stdio");
            return Ok(c);
        }
        if find_on_path("copilot-language-server.cmd").is_some() {
            // A .cmd shim cannot run directly; cmd.exe runs it.
            return Ok(crate::proc::shell_command(
                "copilot-language-server --stdio",
            ));
        }
    } else if find_on_path("copilot-language-server").is_some() {
        let mut c = crate::proc::command("copilot-language-server");
        c.arg("--stdio");
        return Ok(c);
    }
    if find_on_path(if cfg!(windows) { "node.exe" } else { "node" }).is_some() {
        if let Some(js) = npm_server_js() {
            let mut c = crate::proc::command("node");
            c.arg(js).arg("--stdio");
            return Ok(c);
        }
    }
    Err(format!("not installed: {INSTALL_HINT}"))
}

/// The JavaScript distribution under the global npm root, if installed.
fn npm_server_js() -> Option<PathBuf> {
    let out = if cfg!(windows) {
        crate::proc::shell_command("npm root -g").output().ok()?
    } else {
        Command::new("npm").args(["root", "-g"]).output().ok()?
    };
    if !out.status.success() {
        return None;
    }
    let root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let js = root.join("@github/copilot-language-server/dist/language-server.js");
    js.is_file().then_some(js)
}

fn open_url_native(uri: &str) -> bool {
    #[cfg(windows)]
    let r = Command::new("cmd").args(["/c", "start", "", uri]).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(uri).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let r = Command::new("xdg-open").arg(uri).spawn();
    r.is_ok()
}

// ------------------------------------------------------------------ framing

/// One LSP base-protocol message: `Content-Length` headers, a blank line,
/// then the JSON body.
fn write_message(stdin: &mut ChildStdin, msg: &Value) -> Result<(), String> {
    let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
    stdin
        .write_all(framed.as_bytes())
        .map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())
}

/// Read one framed message. None means the server closed stdout or sent
/// something unparsable.
fn read_message<R: BufRead>(reader: &mut R) -> Option<Value> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.trim().parse().ok();
            }
        }
    }
    let len = content_length?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

/// Route one message from the server: a response to our request, a
/// notification, or a request of its own (answered here).
fn handle_server_message(shared: &Arc<Shared>, msg: Value) {
    let id = msg.get("id").and_then(|v| v.as_u64());
    let method = msg
        .get("method")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    match (id, method) {
        (Some(id), Some(method)) => {
            // A request from the server: answer it, without holding the
            // state lock while writing (the write order is io, then state).
            let result = match method.as_str() {
                "workspace/configuration" => {
                    let n = msg
                        .get("params")
                        .and_then(|p| p.get("items"))
                        .and_then(|i| i.as_array())
                        .map_or(1, |a| a.len());
                    Value::Array(vec![Value::Object(Default::default()); n])
                }
                "window/showMessageRequest" => {
                    if let Some(text) = msg
                        .get("params")
                        .and_then(|p| p.get("message"))
                        .and_then(|m| m.as_str())
                    {
                        shared.state.lock().unwrap().notice = Some(text.to_string());
                    }
                    Value::Null
                }
                "window/showDocument" => {
                    let uri = msg
                        .get("params")
                        .and_then(|p| p.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string();
                    let ok = !uri.is_empty() && open_url_native(&uri);
                    serde_json::json!({"success": ok})
                }
                "client/registerCapability" | "client/unregisterCapability" => Value::Null,
                _ => {
                    let reply = serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not found"}});
                    send_raw(shared, &reply);
                    return;
                }
            };
            send_raw(
                shared,
                &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}),
            );
        }
        (None, Some(method)) => {
            if method == "didChangeStatus" {
                let kind = msg
                    .get("params")
                    .and_then(|p| p.get("kind"))
                    .and_then(|k| k.as_str())
                    .unwrap_or("")
                    .to_string();
                let message = msg
                    .get("params")
                    .and_then(|p| p.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut st = shared.state.lock().unwrap();
                st.status = ServerStatus { kind, message };
            }
            // window/logMessage and anything else: no action needed.
        }
        (Some(id), None) => {
            let tx = shared.state.lock().unwrap().pending.remove(&id);
            if let Some(tx) = tx {
                let answer = if let Some(r) = msg.get("result") {
                    Ok(r.clone())
                } else if let Some(e) = msg.get("error") {
                    Err(e
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Copilot server error")
                        .to_string())
                } else {
                    Err("Copilot server sent a bad response".to_string())
                };
                let _ = tx.send(answer);
            }
        }
        _ => {}
    }
}

/// Write one already-built message, taking only the io lock.
fn send_raw(shared: &Arc<Shared>, msg: &Value) {
    let _io = shared.io.lock().unwrap();
    let mut st = shared.state.lock().unwrap();
    if let Some(stdin) = st.stdin.as_mut() {
        let _ = write_message(stdin, msg);
    }
}

// ------------------------------------------------------------------ inline completion

impl Copilot {
    /// Ask the server for the ghost text at `offset` in `text`, the full
    /// text of the file at `path`. Syncs the document first (didOpen once,
    /// then one incremental didChange per edit), so the server always sees
    /// what the editor sees. Returns the text to insert at the cursor.
    /// Blocking: call it from a background thread, never the UI thread.
    pub fn inline(
        &self,
        path: &Path,
        language_id: &str,
        text: &str,
        offset: usize,
        tab_size: usize,
        trigger_kind: u8,
    ) -> Option<InlineCompletionItem> {
        if !self.enabled() || offset > text.len() || !text.is_char_boundary(offset) {
            return None;
        }
        let folder = path.parent().map(|p| p.to_path_buf());
        if self.ensure_started(folder).is_err() {
            return None;
        }
        let uri = uri_for_path(path);
        // Sync and send under one io lock, so the server never answers a
        // request for a document state it has not seen yet. The sync
        // messages are built in one short state lock, then written in
        // another: holding the stdin borrow while touching the rest of
        // the state does not compile, and the io lock already keeps other
        // writers out between the two steps.
        let req_id: u64;
        let rx = {
            let _io = self.shared.io.lock().unwrap();
            let mut to_send: Vec<Value> = Vec::new();
            let version = {
                let mut st = self.shared.state.lock().unwrap();
                st.stdin.as_ref()?;
                match st.docs.get_mut(&uri) {
                    None => {
                        to_send.push(serde_json::json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "languageId": language_id, "version": 0, "text": text}}}));
                        st.docs.insert(
                            uri.clone(),
                            DocState {
                                version: 0,
                                text: text.to_string(),
                            },
                        );
                        0
                    }
                    Some(doc) => {
                        if doc.text != text {
                            let (start_b, end_b, replacement) = diff_change(&doc.text, text);
                            let start = lsp_position(&doc.text, start_b);
                            let end = lsp_position(&doc.text, end_b);
                            let range_length = utf16_len(&doc.text[start_b..end_b]);
                            doc.version += 1;
                            let v = doc.version;
                            to_send.push(serde_json::json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"textDocument": {"uri": uri, "version": v}, "contentChanges": [{"range": {"start": {"line": start.0, "character": start.1}, "end": {"line": end.0, "character": end.1}}, "rangeLength": range_length, "text": replacement}]}}));
                            doc.text = text.to_string();
                            v
                        } else {
                            doc.version
                        }
                    }
                }
            };
            let (line, character) = lsp_position(text, offset);
            let rx = {
                let (tx, rx) = mpsc::channel();
                {
                    let mut st = self.shared.state.lock().unwrap();
                    st.stdin.as_ref()?;
                    st.next_id += 1;
                    req_id = st.next_id;
                    st.pending.insert(req_id, tx);
                }
                let req = serde_json::json!({"jsonrpc": "2.0", "id": req_id, "method": "textDocument/inlineCompletion", "params": {
                    "textDocument": {"uri": uri, "version": version},
                    "position": {"line": line, "character": character},
                    "context": {"triggerKind": trigger_kind},
                    "formattingOptions": {"tabSize": tab_size, "insertSpaces": true}
                }});
                let wrote = {
                    let mut st = self.shared.state.lock().unwrap();
                    match st.stdin.as_mut() {
                        Some(stdin) => {
                            let mut ok = true;
                            for m in &to_send {
                                if write_message(stdin, m).is_err() {
                                    ok = false;
                                    break;
                                }
                            }
                            if ok && write_message(stdin, &req).is_err() {
                                ok = false;
                            }
                            ok
                        }
                        None => false,
                    }
                };
                if !wrote {
                    self.shared.state.lock().unwrap().pending.remove(&req_id);
                    return None;
                }
                rx
            };
            rx
        };
        let result = match rx.recv_timeout(COMPLETION_TIMEOUT) {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => return None,
            Err(_) => {
                self.shared.state.lock().unwrap().pending.remove(&req_id);
                return None;
            }
        };
        let (insert_text, range) = parse_inline_item(&result)?;
        let ghost = to_ghost_text(&insert_text, range, text, offset)?;
        Some(InlineCompletionItem {
            insert_text: ghost,
            filter_text: None,
            range: None,
            command: None,
            insert_text_format: None,
        })
    }
}

/// A Copilot item's replace range: ((start line, start character), (end line, end character)).
type InlineRange = ((u32, u32), (u32, u32));

/// The first item of an inlineCompletion result: its insert text and the
/// range it replaces, if any. The result is an object with an `items`
/// array, or a bare array.
fn parse_inline_item(result: &Value) -> Option<(String, Option<InlineRange>)> {
    let items = if let Some(a) = result.as_array() {
        a
    } else {
        result.get("items")?.as_array()?
    };
    let first = items.first()?;
    let insert = first.get("insertText")?.as_str()?.to_string();
    let range = first.get("range").and_then(|r| {
        let pos = |p: &Value| -> Option<(u32, u32)> {
            Some((
                p.get("line")?.as_u64()? as u32,
                p.get("character")?.as_u64()? as u32,
            ))
        };
        Some((pos(r.get("start")?)?, pos(r.get("end")?)?))
    });
    Some((insert, range))
}

/// Turn a server item into ghost text: the part of `insert_text` that is
/// not already typed. The editor renders `insert_text` right after the
/// cursor and Tab inserts it there, so anything already in the document
/// must be stripped first, or accepting would duplicate it.
fn to_ghost_text(
    insert: &str,
    range: Option<InlineRange>,
    text: &str,
    offset: usize,
) -> Option<String> {
    let insert = insert.replace("\r\n", "\n").replace('\r', "\n");
    if insert.is_empty() {
        return None;
    }
    let typed: &str = match range {
        Some(((sl, sc), _)) => {
            let sb = byte_offset(text, sl, sc)?;
            if sb > offset {
                return None;
            }
            &text[sb..offset]
        }
        None => "",
    };
    let ghost = insert.strip_prefix(typed)?;
    if ghost.is_empty() {
        return None;
    }
    // The suggestion is already there after the cursor: nothing to show.
    if !text[offset..].is_empty() && text[offset..].starts_with(ghost) {
        return None;
    }
    Some(ghost.to_string())
}

// ------------------------------------------------------------------ positions and text

/// LSP position of byte `offset`: line, and characters in UTF-16 code
/// units (what the protocol counts). A `\r` before `\n` does not count.
fn lsp_position(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() as u32;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line_text = before[line_start..]
        .strip_suffix('\r')
        .unwrap_or(&before[line_start..]);
    (line, line_text.chars().map(|c| c.len_utf16() as u32).sum())
}

/// Byte offset of an LSP position, or None when it is past the text.
fn byte_offset(text: &str, line: u32, character: u32) -> Option<usize> {
    let mut start = 0usize;
    for (idx, raw) in text.split('\n').enumerate() {
        let l = raw.strip_suffix('\r').unwrap_or(raw);
        if idx as u32 == line {
            let mut units = 0u32;
            for (bi, ch) in l.char_indices() {
                if units == character {
                    return Some(start + bi);
                }
                units += ch.len_utf16() as u32;
            }
            if units == character {
                return Some(start + l.len());
            }
            return None;
        }
        start += raw.len() + 1;
    }
    None
}

fn utf16_len(s: &str) -> u32 {
    s.chars().map(|c| c.len_utf16() as u32).sum()
}

/// The one changed span between `old` and `new`: (start byte, end byte in
/// `old`, replacement text). Prefix/suffix, both kept on char boundaries.
fn diff_change(old: &str, new: &str) -> (usize, usize, String) {
    let ob = old.as_bytes();
    let nb = new.as_bytes();
    let mut prefix = 0;
    while prefix < ob.len() && prefix < nb.len() && ob[prefix] == nb[prefix] {
        prefix += 1;
    }
    while prefix > 0 && !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = 0;
    while suffix < ob.len() - prefix
        && suffix < nb.len() - prefix
        && ob[ob.len() - 1 - suffix] == nb[nb.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0 && !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    (
        prefix,
        old.len() - suffix,
        new[prefix..new.len() - suffix].to_string(),
    )
}

/// `file://` URI for a path, with spaces and non-ASCII percent-encoded.
pub(crate) fn uri_for_path(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let text = abs.to_string_lossy();
    #[cfg(windows)]
    let text = text.replace('\\', "/");
    let mut s = String::from("file://");
    #[cfg(windows)]
    s.push('/');
    for b in text.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                s.push(*b as char)
            }
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

/// The LSP language id for FluxCode's language token. Unknown tokens pass
/// through unchanged; the server ignores what it does not know.
pub(crate) fn language_id(lang: &str) -> String {
    match lang {
        "tsx" => "typescriptreact".to_string(),
        "bash" => "shellscript".to_string(),
        "cs" => "csharp".to_string(),
        "fs" => "fsharp".to_string(),
        "kt" => "kotlin".to_string(),
        "rb" => "ruby".to_string(),
        "m" => "objective-c".to_string(),
        "mm" => "objective-cpp".to_string(),
        _ => lang.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn lsp_position_counts_utf16_units() {
        // "a" + emoji (2 UTF-16 units) + "b": the cursor after "b" is at character 4.
        assert_eq!(lsp_position("a\u{1F600}b", 6), (0, 4));
        assert_eq!(lsp_position("a\u{1F600}b", 1), (0, 1));
        assert_eq!(lsp_position("a\u{1F600}b", 5), (0, 3));
        assert_eq!(lsp_position("one\ntwo", 7), (1, 3));
        assert_eq!(lsp_position("one\ntwo", 4), (1, 0));
    }

    #[test]
    fn byte_offset_round_trips_positions() {
        let text = "a\u{1F600}b\nxy";
        for (line, ch, byte) in [
            (0, 0, 0),
            (0, 1, 1),
            (0, 3, 5),
            (0, 4, 6),
            (1, 0, 7),
            (1, 2, 9),
        ] {
            assert_eq!(byte_offset(text, line, ch), Some(byte));
        }
        assert_eq!(byte_offset(text, 0, 5), None);
        assert_eq!(byte_offset(text, 2, 0), None);
    }

    #[test]
    fn diff_change_covers_insertion_deletion_and_replacement() {
        assert_eq!(diff_change("hello", "hello!"), (5, 5, "!".to_string()));
        assert_eq!(diff_change("hello!", "hello"), (5, 6, String::new()));
        assert_eq!(
            diff_change("let a = 1;", "let a = 2;"),
            (8, 9, "2".to_string())
        );
        assert_eq!(diff_change("same", "same"), (4, 4, String::new()));
        assert_eq!(diff_change("a\nb\nc", "a\nx\nc"), (2, 3, "x".to_string()));
        // Multi-byte chars stay on boundaries.
        assert_eq!(
            diff_change("a\u{1F600}b", "a\u{1F600}c"),
            (5, 6, "c".to_string())
        );
    }

    #[test]
    fn ghost_strips_what_is_already_typed() {
        let text = "console.l";
        // The server replaces "l" (range starts at byte 8) with "log".
        assert_eq!(
            to_ghost_text("log", Some(((0, 8), (0, 9))), text, 9),
            Some("og".to_string())
        );
        // No range: the whole insert goes after the cursor.
        assert_eq!(
            to_ghost_text("()", None, "console.log", 11),
            Some("()".to_string())
        );
        // The insert does not start with what is typed: no guess.
        assert_eq!(to_ghost_text("warn", Some(((0, 8), (0, 9))), text, 9), None);
        // The suggestion is already after the cursor.
        assert_eq!(
            to_ghost_text("og()", Some(((0, 8), (0, 9))), "console.log()", 9),
            None
        );
        // CRLF in the insert becomes LF.
        assert_eq!(
            to_ghost_text("a\r\nb", None, "", 0),
            Some("a\nb".to_string())
        );
    }

    #[test]
    fn framing_round_trips() {
        let msg = serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "initialize", "params": {"x": 1}});
        let mut buf = Vec::new();
        {
            let mut cursor = Cursor::new(&mut buf);
            // write_message needs a ChildStdin; frame the same way here.
            let body = serde_json::to_string(&msg).unwrap();
            let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
            cursor.write_all(framed.as_bytes()).unwrap();
        }
        let mut reader = BufReader::new(Cursor::new(buf));
        assert_eq!(read_message(&mut reader), Some(msg));
        assert_eq!(read_message(&mut reader), None);
    }

    #[test]
    fn uris_and_language_ids() {
        assert_eq!(
            uri_for_path(Path::new("/home/u/my file.rs")),
            "file:///home/u/my%20file.rs"
        );
        assert_eq!(
            uri_for_path(Path::new("/home/u/a+b.rs")),
            "file:///home/u/a%2Bb.rs"
        );
        assert_eq!(language_id("tsx"), "typescriptreact");
        assert_eq!(language_id("bash"), "shellscript");
        assert_eq!(language_id("rust"), "rust");
    }

    #[test]
    fn inline_response_parses_both_shapes() {
        let obj = serde_json::json!({"items": [{"insertText": "log()", "range": {"start": {"line": 0, "character": 8}, "end": {"line": 0, "character": 9}}}]});
        assert_eq!(
            parse_inline_item(&obj),
            Some(("log()".to_string(), Some(((0, 8), (0, 9)))))
        );
        let arr = serde_json::json!([{"insertText": "x"}]);
        assert_eq!(parse_inline_item(&arr), Some(("x".to_string(), None)));
        assert_eq!(parse_inline_item(&serde_json::json!({"items": []})), None);
    }

    /// Live check against the real server, if one is installed: initialize
    /// answers, and an unauthenticated completion request fails or comes
    /// back empty. It never signs in and never sends real code anywhere
    /// except to the local server process. Ignored by default: it needs the
    /// server on PATH (or FLUXCODE_COPILOT_SERVER) and a network route to
    /// GitHub for the server's own status check.
    #[test]
    #[ignore]
    fn live_server_answers_initialize() {
        let copilot = Copilot::new(true);
        let folder = std::env::temp_dir().join("fluxcode-copilot-test");
        let _ = std::fs::create_dir_all(&folder);
        let path = folder.join("hello.js");
        std::fs::write(&path, "console.").unwrap();
        let item = copilot.inline(&path, "javascript", "console.", 8, 4, 2);
        // Without a Copilot sign-in there is no suggestion; the point is
        // that the spawn, framing and initialize round-trip worked. A
        // signed-in run may return Some here instead.
        match copilot.status() {
            Status::NotSignedIn(_) | Status::Warning(_) => assert!(
                item.is_none(),
                "unsigned-in server should not suggest, got {item:?}"
            ),
            Status::Ready => {}
            other => panic!("unexpected status: {other:?}"),
        }
        copilot.stop();
    }
}
