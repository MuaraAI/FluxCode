use lsp_types::InlineCompletionItem;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub const INSTALL_HINT: &str =
    "npm install -g @github/copilot-language-server (needs Node.js 20.8 or later)";

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(30);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(20);
const SIGNIN_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    Starting,
    NotInstalled,
    Ready,
    NotSignedIn(String),
    Warning(String),
    Inactive,
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
    user: Option<String>,
    gen: u64,
}

struct Shared {
    state: Mutex<State>,
    io: Mutex<()>,
    start_lock: Mutex<()>,
    enabled: AtomicBool,
    watchers: Mutex<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
}

impl Shared {
    fn ping(&self) {
        self.watchers
            .lock()
            .unwrap()
            .retain(|w| w.unbounded_send(()).is_ok());
    }
}

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
                    user: None,
                    gen: 0,
                }),
                io: Mutex::new(()),
                start_lock: Mutex::new(()),
                enabled: AtomicBool::new(enabled),
                watchers: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn enabled(&self) -> bool {
        self.shared.enabled.load(Ordering::SeqCst)
    }

    pub fn watch(&self) -> futures::channel::mpsc::UnboundedReceiver<()> {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        self.shared.watchers.lock().unwrap().push(tx);
        rx
    }

    pub fn untried(&self) -> bool {
        let st = self.shared.state.lock().unwrap();
        self.enabled()
            && !st.started
            && !st.starting
            && st.start_error.is_none()
            && !st.not_installed
    }

    pub fn user(&self) -> Option<String> {
        self.shared.state.lock().unwrap().user.clone()
    }

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

    pub fn user_code(&self) -> Option<String> {
        self.shared.state.lock().unwrap().user_code.clone()
    }

    pub fn notice(&self) -> Option<String> {
        self.shared.state.lock().unwrap().notice.clone()
    }

    pub fn retry(&self) {
        let mut st = self.shared.state.lock().unwrap();
        st.not_installed = false;
        st.start_error = None;
        st.last_attempt = None;
    }

    pub fn start(&self) -> Result<(), String> {
        self.ensure_started(None)
    }

    pub fn start_in(&self, folder: Option<PathBuf>) -> Result<(), String> {
        self.ensure_started(folder)
    }

    pub fn stop(&self) {
        self.stop_inner(false);
    }

    pub fn stop_and_wait(&self) {
        self.stop_inner(true);
    }

    fn stop_inner(&self, wait: bool) {
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
        self.shared.ping();
        if let Some(mut child) = child {
            let mut end = move || {
                #[cfg(windows)]
                let _ = crate::proc::command("taskkill")
                    .args(["/T", "/F", "/PID", &child.id().to_string()])
                    .status();
                let _ = child.kill();
                let _ = child.wait();
            };
            if wait {
                end();
            } else {
                std::thread::spawn(end);
            }
        }
    }

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
        self.shared.ping();
        let started = self.start_inner(folder);
        let result = {
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
        };
        if result.is_ok() {
            self.check_status();
        }
        self.shared.ping();
        result
    }

    fn check_status(&self) {
        let Ok(v) = self.request(
            "checkStatus",
            Some(serde_json::json!({})),
            Duration::from_secs(15),
        ) else {
            return;
        };
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
        let user = v
            .get("user")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let (kind, message) = match status {
            "OK" | "MaybeOK" | "AlreadySignedIn" => ("Normal", String::new()),
            "NotSignedIn" => ("Error", String::new()),
            "NotAuthorized" => (
                "Error",
                crate::trf!("{user} has no Copilot plan yet (a free plan exists at github.com/features/copilot)."; user => if user.is_empty() { "This account" } else { user.as_str() }),
            ),
            "" => return,
            other => ("Warning", other.to_string()),
        };
        {
            let mut st = self.shared.state.lock().unwrap();
            if st.status.kind.is_empty() || kind == "Normal" || status == "NotSignedIn" {
                st.status = ServerStatus {
                    kind: kind.to_string(),
                    message,
                };
            }
            st.user = (!user.is_empty()).then_some(user);
        }
        self.shared.ping();
    }

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
        let gen = {
            let mut st = self.shared.state.lock().unwrap();
            st.child = Some(child);
            st.stdin = Some(stdin);
            st.gen += 1;
            st.gen
        };
        self.spawn_reader(stdout, gen);
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

    fn spawn_reader(&self, stdout: ChildStdout, gen: u64) {
        let shared = Arc::clone(&self.shared);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut pending_fail: Vec<mpsc::Sender<Result<Value, String>>> = Vec::new();
            while let Some(msg) = read_message(&mut reader) {
                handle_server_message(&shared, msg);
            }
            let child;
            {
                let mut st = shared.state.lock().unwrap();
                if st.gen != gen || st.child.is_none() {
                    return;
                }
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
            shared.ping();
            if let Some(mut child) = child {
                let _ = child.wait();
            }
        });
    }

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

    fn notify(&self, method: &str, params: Value) {
        let _io = self.shared.io.lock().unwrap();
        let mut st = self.shared.state.lock().unwrap();
        if let Some(stdin) = st.stdin.as_mut() {
            let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
            let _ = write_message(stdin, &msg);
        }
    }

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
                let _io = shared.io.lock().unwrap_or_else(|e| e.into_inner());
                let mut st = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(stdin) = st.stdin.as_mut() {
                    let _ = write_message(stdin, &msg);
                }
            });
        }
    }

    pub fn sign_in(&self) -> Result<String, String> {
        self.ensure_started(None)?;
        let result = self.request("signIn", Some(serde_json::json!({})), SIGNIN_TIMEOUT)?;
        if result.get("status").and_then(|s| s.as_str()) == Some("AlreadySignedIn") {
            self.check_status();
            return Ok(String::new());
        }
        let code = result
            .get("userCode")
            .and_then(|c| c.as_str())
            .ok_or("Copilot sign-in gave no user code".to_string())?
            .to_string();
        {
            let mut st = self.shared.state.lock().unwrap();
            st.user_code = Some(code.clone());
        }
        self.shared.ping();
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
        {
            let mut st = self.shared.state.lock().unwrap();
            st.user_code = None;
            st.user = None;
        }
        if self.shared.state.lock().unwrap().started {
            let _ = self.request("signOut", Some(serde_json::json!({})), SIGNIN_TIMEOUT);
            self.check_status();
        }
        self.shared.ping();
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

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

pub(crate) fn npm_present() -> bool {
    find_on_path(if cfg!(windows) { "npm.cmd" } else { "npm" }).is_some()
}

pub(crate) fn npm_install_error(stderr: &str, exit: &str) -> String {
    let first = stderr.lines().map(str::trim).find(|l| !l.is_empty());
    let msg = first.map_or_else(|| format!("npm exited with {exit}"), str::to_string);
    let low = stderr.to_lowercase();
    if low.contains("eacces") || low.contains("permission denied") {
        return format!(
            "{}. {msg}",
            crate::i18n::tr("npm's global folder is not writable; see the npm docs on EACCES, or install the server by hand.")
        );
    }
    msg
}

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

fn write_message(stdin: &mut ChildStdin, msg: &Value) -> Result<(), String> {
    let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
    stdin
        .write_all(framed.as_bytes())
        .map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())
}

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

fn handle_server_message(shared: &Arc<Shared>, msg: Value) {
    let id = msg.get("id").and_then(|v| v.as_u64());
    let method = msg
        .get("method")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    match (id, method) {
        (Some(id), Some(method)) => {
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
                        shared.ping();
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
                {
                    let mut st = shared.state.lock().unwrap();
                    if kind == "Normal" {
                        st.user_code = None;
                    }
                    st.status = ServerStatus { kind, message };
                }
                shared.ping();
            }
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

fn send_raw(shared: &Arc<Shared>, msg: &Value) {
    let _io = shared.io.lock().unwrap();
    let mut st = shared.state.lock().unwrap();
    if let Some(stdin) = st.stdin.as_mut() {
        let _ = write_message(stdin, msg);
    }
}

impl Copilot {
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

type InlineRange = ((u32, u32), (u32, u32));

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
    if !text[offset..].is_empty() && text[offset..].starts_with(ghost) {
        return None;
    }
    Some(ghost.to_string())
}

fn lsp_position(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() as u32;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line_text = before[line_start..]
        .strip_suffix('\r')
        .unwrap_or(&before[line_start..]);
    (line, line_text.chars().map(|c| c.len_utf16() as u32).sum())
}

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

pub(crate) fn uri_for_path(path: &Path) -> String {
    crate::docs::file_uri(path)
}

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
    fn retry_clears_failed_start_throttle() {
        let copilot = Copilot::new(true);
        {
            let mut state = copilot.shared.state.lock().unwrap();
            state.not_installed = true;
            state.start_error = Some("not installed".to_string());
            state.last_attempt = Some(Instant::now());
        }

        copilot.retry();

        let state = copilot.shared.state.lock().unwrap();
        assert!(!state.not_installed);
        assert!(state.start_error.is_none());
        assert!(state.last_attempt.is_none());
    }

    #[test]
    fn npm_install_error_puts_eacces_hint_first() {
        let stderr = "\nnpm error code EACCES\nnpm error syscall mkdir\nnpm error path /usr/lib/node_modules\n";
        let msg = npm_install_error(stderr, "exit status: 243");
        assert!(
            msg.starts_with("npm's global folder is not writable"),
            "{msg}"
        );
        assert!(msg.ends_with("npm error code EACCES"), "{msg}");
        assert!(!msg.contains('\n'));
    }

    #[test]
    fn npm_install_error_falls_back_to_exit_status() {
        assert_eq!(
            npm_install_error("  \n", "exit status: 1"),
            "npm exited with exit status: 1"
        );
        assert_eq!(
            npm_install_error("npm error 404 Not Found\nmore", "x"),
            "npm error 404 Not Found"
        );
    }

    #[test]
    fn lsp_position_counts_utf16_units() {
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
        assert_eq!(
            diff_change("a\u{1F600}b", "a\u{1F600}c"),
            (5, 6, "c".to_string())
        );
    }

    #[test]
    fn ghost_strips_what_is_already_typed() {
        let text = "console.l";
        assert_eq!(
            to_ghost_text("log", Some(((0, 8), (0, 9))), text, 9),
            Some("og".to_string())
        );
        assert_eq!(
            to_ghost_text("()", None, "console.log", 11),
            Some("()".to_string())
        );
        assert_eq!(to_ghost_text("warn", Some(((0, 8), (0, 9))), text, 9), None);
        assert_eq!(
            to_ghost_text("og()", Some(((0, 8), (0, 9))), "console.log()", 9),
            None
        );
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
        #[cfg(windows)]
        let (spaced, plus, base) = (r"C:\home\u\my file.rs", r"C:\home\u\a+b.rs", "file:///C:");
        #[cfg(not(windows))]
        let (spaced, plus, base) = ("/home/u/my file.rs", "/home/u/a+b.rs", "file://");
        assert_eq!(
            uri_for_path(Path::new(spaced)),
            format!("{base}/home/u/my%20file.rs")
        );
        assert_eq!(
            uri_for_path(Path::new(plus)),
            format!("{base}/home/u/a%2Bb.rs")
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

    #[test]
    #[ignore]
    fn live_server_answers_initialize() {
        let copilot = Copilot::new(true);
        let folder = std::env::temp_dir().join("fluxcode-copilot-test");
        let _ = std::fs::create_dir_all(&folder);
        let path = folder.join("hello.js");
        std::fs::write(&path, "console.").unwrap();
        let item = copilot.inline(&path, "javascript", "console.", 8, 4, 2);
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
