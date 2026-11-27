use super::vsc_registry;
use super::webview_bridge::WebviewBridge;
use futures::channel::mpsc::UnboundedSender;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const HOST_SCRIPT: &str = include_str!("ext_host.cjs");
const MAX_LOG_LINES: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtStatus {
    Idle,
    Activating,
    Active,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub description: String,
    pub tooltip: String,
    pub collapsible: bool,
    pub expanded: bool,
    pub icon: Option<String>,
    pub command: Option<String>,
    pub command_args: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusBarItem {
    pub id: String,
    pub ext_id: String,
    pub text: String,
    pub tooltip: String,
    pub command: Option<String>,
    pub priority: i32,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct WebviewInfo {
    pub handle: String,
    pub url: String,
    pub title: String,
    pub has_html: bool,
    pub html_rev: u64,
}

#[derive(Clone, Debug, Default)]
pub struct TreeState {
    pub roots: Vec<TreeNode>,
    pub children: HashMap<String, Vec<TreeNode>>,
    pub expanded: HashSet<String>,
    pub loading: bool,
}

#[derive(Clone, Debug)]
pub enum HostEvent {
    StatusChanged {
        ext_id: String,
        status: ExtStatus,
    },
    WebviewReady {
        view_id: String,
        handle: String,
        url: String,
        title: String,
        has_html: bool,
    },
    WebviewHtmlUpdated {
        handle: String,
        rev: u64,
    },
    #[allow(dead_code)]
    WebviewPanelOpened {
        handle: String,
        title: String,
        url: String,
    },
    TreeUpdated {
        view_id: String,
        parent_id: Option<String>,
        items: Vec<TreeNode>,
    },
    ContextKey {
        key: String,
        value: Value,
    },
    StatusBarUpdated(StatusBarItem),
    StatusBarRemoved(String),
    OutputLine {
        ext_id: String,
        channel: String,
        line: String,
    },
    #[allow(dead_code)]
    Message {
        level: String,
        text: String,
    },
    OpenUrl(String),
    OpenFile(PathBuf),
    RunTerminal {
        cmd: String,
    },
    FocusContainer(String),
    HostExited(String),
}

struct RunningHost {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Child>,
    #[cfg(windows)]
    job: Option<crate::proc::ProcessJob>,
}

impl RunningHost {
    fn send(&self, msg: &Value) -> Result<(), String> {
        let mut line = serde_json::to_string(msg).map_err(|e| e.to_string())?;
        line.push('\n');
        let mut stdin = self.stdin.lock().map_err(|_| "host stdin poisoned")?;
        stdin
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())
    }
}

pub struct ExtHost {
    tx: UnboundedSender<HostEvent>,
    bridge: Arc<Mutex<Option<Arc<WebviewBridge>>>>,
    proc: Option<Arc<RunningHost>>,
    proc_sink: Arc<Mutex<Option<std::sync::Weak<RunningHost>>>>,
    next_id: AtomicU64,
    handle_seq: AtomicU64,
    view_handles: HashMap<String, String>,
    pub last_error: Option<String>,
}

impl ExtHost {
    pub fn new(tx: UnboundedSender<HostEvent>) -> Self {
        Self {
            tx,
            bridge: Arc::new(Mutex::new(None)),
            proc: None,
            proc_sink: Arc::new(Mutex::new(None)),
            next_id: AtomicU64::new(1),
            handle_seq: AtomicU64::new(1),
            view_handles: HashMap::new(),
            last_error: None,
        }
    }

    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.proc.is_some()
    }

    pub fn node_available() -> bool {
        crate::acp::which("node").is_some()
    }

    fn ensure_bridge(&self, dark: bool) -> Result<Arc<WebviewBridge>, String> {
        let mut guard = self.bridge.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(b) = guard.as_ref() {
            return Ok(Arc::clone(b));
        }
        let css = default_theme_css(dark);
        let sink = Arc::clone(&self.proc_sink);
        let b = WebviewBridge::start(
            css,
            Arc::new(move |handle, msg_json| {
                let weak = sink.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(weak) = weak {
                    if let Some(host) = weak.upgrade() {
                        let _ = host.send(&json!({
                            "method": "webviewMessage",
                            "params": { "handle": handle, "message": msg_json }
                        }));
                    }
                }
            }),
        )?;
        let arc = Arc::new(b);
        *guard = Some(Arc::clone(&arc));
        Ok(arc)
    }

    pub fn update_theme_css(&self, css: String) {
        if let Some(b) = self
            .bridge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            b.update_theme_css(css);
        }
    }

    fn materialize_script() -> Result<PathBuf, String> {
        let dir = crate::settings::config_dir().join("ext-host");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let target = dir.join("host.cjs");
        let want_hash = sha1_smol::Sha1::from(HOST_SCRIPT.as_bytes())
            .digest()
            .to_string();
        let same = std::fs::read(&target)
            .ok()
            .is_some_and(|cur| sha1_smol::Sha1::from(&cur).digest().to_string() == want_hash);
        if !same {
            std::fs::write(&target, HOST_SCRIPT).map_err(|e| e.to_string())?;
        }
        Ok(target)
    }

    pub fn ensure_started(
        &mut self,
        workspace_roots: &[PathBuf],
        dark: bool,
    ) -> Result<(), String> {
        if self.proc.is_some() {
            return Ok(());
        }
        let Some(node) = crate::acp::which("node") else {
            let msg =
                crate::i18n::tr("Node.js is required to run VS Code extension code (nodejs.org).")
                    .to_string();
            self.last_error = Some(msg.clone());
            return Err(msg);
        };
        let script = Self::materialize_script()?;

        // Start bridge first so init passes bridge port/token and message callback binds cleanly.
        let bridge_guard = self.bridge.lock().unwrap_or_else(|e| e.into_inner());
        let existing_bridge = bridge_guard.clone();
        drop(bridge_guard);

        let mut cmd = crate::proc::command(&node.to_string_lossy());
        cmd.arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NODE_NO_WARNINGS", "1")
            .env("NO_COLOR", "1");

        let mut child = cmd.spawn().map_err(|e| {
            let m = format!("Failed to start Node.js extension host: {e}");
            self.last_error = Some(m.clone());
            m
        })?;

        #[cfg(windows)]
        let job = crate::proc::ProcessJob::for_child(&child);

        let stdin = child.stdin.take().ok_or("missing stdin")?;
        let stdout = child.stdout.take().ok_or("missing stdout")?;
        let stderr = child.stderr.take().ok_or("missing stderr")?;

        let running = Arc::new(RunningHost {
            stdin: Mutex::new(stdin),
            child: Mutex::new(child),
            #[cfg(windows)]
            job,
        });
        self.proc = Some(Arc::clone(&running));
        *self.proc_sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(&running));
        self.last_error = None;

        // Rebind WebviewBridge if it was not created yet, using the live RunningHost weak handle.
        let bridge = match existing_bridge {
            Some(b) => b,
            None => self.ensure_bridge(dark)?,
        };

        let tx_out = self.tx.clone();
        let br_out = Arc::clone(&bridge);
        thread::Builder::new()
            .name("flux-ext-host-out".into())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(val) = serde_json::from_str::<Value>(&line) {
                        handle_rpc_message(&val, &br_out, &tx_out);
                    }
                }
                let _ =
                    tx_out.unbounded_send(HostEvent::HostExited("Extension host exited".into()));
            })
            .map_err(|e| e.to_string())?;

        let tx_err = self.tx.clone();
        thread::Builder::new()
            .name("flux-ext-host-err".into())
            .spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        let _ = tx_err.unbounded_send(HostEvent::OutputLine {
                            ext_id: "host".into(),
                            channel: "stderr".into(),
                            line,
                        });
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        let state_dir = crate::settings::config_dir().join("ext-state");
        let _ = std::fs::create_dir_all(&state_dir);
        let folders: Vec<String> = workspace_roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let ext_metas: Vec<Value> = vsc_registry::all_runtimes()
            .into_iter()
            .map(|rt| {
                json!({
                    "id": rt.ext_id,
                    "name": rt.display,
                    "version": rt.version,
                    "folder": rt.folder.to_string_lossy(),
                })
            })
            .collect();

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        running.send(&json!({
            "id": id,
            "method": "init",
            "params": {
                "bridgePort": bridge.port(),
                "bridgeToken": bridge.token(),
                "stateDir": state_dir.to_string_lossy(),
                "workspaceFolders": folders,
                "themeKind": if dark { 2 } else { 1 },
                "extensions": ext_metas,
            }
        }))?;
        Ok(())
    }

    pub fn activate_extension(
        &mut self,
        ext_id: &str,
        reason: &str,
        workspace_roots: &[PathBuf],
        dark: bool,
    ) -> Result<(), String> {
        let Some(rt) = vsc_registry::runtime_for_ext(ext_id) else {
            return Err(format!("Unknown extension: {ext_id}"));
        };
        let Some(main_path) = rt.main else {
            return Ok(());
        };
        self.ensure_started(workspace_roots, dark)?;
        let Some(proc) = &self.proc else {
            return Err("Extension host unavailable".into());
        };
        let _ = self.tx.unbounded_send(HostEvent::StatusChanged {
            ext_id: rt.ext_id.clone(),
            status: ExtStatus::Activating,
        });
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        proc.send(&json!({
            "id": id,
            "method": "activate",
            "params": {
                "extId": rt.ext_id,
                "mainPath": main_path.to_string_lossy(),
                "folder": rt.folder.to_string_lossy(),
                "defaultConfig": rt.default_config,
                "reason": reason,
            }
        }))
    }

    pub fn resolve_webview_view(
        &mut self,
        ext_id: &str,
        view_id: &str,
        workspace_roots: &[PathBuf],
        dark: bool,
    ) -> Result<String, String> {
        self.ensure_started(workspace_roots, dark)?;
        let bridge = self.ensure_bridge(dark)?;
        let handle = if let Some(h) = self.view_handles.get(view_id) {
            h.clone()
        } else {
            let h = format!("view_{}", self.handle_seq.fetch_add(1, Ordering::Relaxed));
            if let Some(rt) = vsc_registry::runtime_for_ext(ext_id) {
                bridge.register_webview(&h, rt.folder, vec![]);
            }
            self.view_handles.insert(view_id.to_string(), h.clone());
            h
        };
        let Some(proc) = &self.proc else {
            return Err("Extension host unavailable".into());
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        proc.send(&json!({
            "id": id,
            "method": "resolveWebviewView",
            "params": {
                "extId": ext_id,
                "viewId": view_id,
                "handle": handle,
            }
        }))?;
        Ok(bridge.url_for(&handle))
    }

    pub fn request_tree_children(&mut self, view_id: &str, element_id: Option<&str>) {
        let Some(proc) = &self.proc else { return };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = proc.send(&json!({
            "id": id,
            "method": "treeGetChildren",
            "params": {
                "viewId": view_id,
                "elementId": element_id,
            }
        }));
    }

    pub fn execute_command(
        &mut self,
        ext_id: Option<&str>,
        command: &str,
        args: Vec<Value>,
        workspace_roots: &[PathBuf],
        dark: bool,
    ) -> Result<(), String> {
        if let Some(eid) = ext_id {
            self.activate_extension(eid, &format!("onCommand:{command}"), workspace_roots, dark)?;
        } else {
            self.ensure_started(workspace_roots, dark)?;
        }
        let Some(proc) = &self.proc else {
            return Err("Extension host unavailable".into());
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        proc.send(&json!({
            "id": id,
            "method": "executeCommand",
            "params": { "command": command, "args": args }
        }))
    }

    pub fn update_workspace(&self, roots: &[PathBuf], dark: bool, focused: Option<bool>) {
        let Some(proc) = &self.proc else { return };
        let folders: Vec<String> = roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let _ = proc.send(&json!({
            "method": "updateWorkspace",
            "params": {
                "workspaceFolders": folders,
                "themeKind": if dark { 2 } else { 1 },
                "focused": focused,
            }
        }));
    }

    pub fn cool(&self, trim_ws: bool) {
        #[cfg(windows)]
        if let Some(proc) = &self.proc {
            if let Some(job) = &proc.job {
                let _ = job.set_thermal_state(trim_ws, Some(true));
            }
        }
        #[cfg(not(windows))]
        let _ = trim_ws;
    }

    pub fn wake(&self) {
        #[cfg(windows)]
        if let Some(proc) = &self.proc {
            if let Some(job) = &proc.job {
                let _ = job.set_thermal_state(false, Some(false));
            }
        }
    }

    pub fn set_webview_visibility(&self, handle: &str, visible: bool) {
        let Some(proc) = &self.proc else { return };
        let _ = proc.send(&json!({
            "method": "webviewVisibility",
            "params": { "handle": handle, "visible": visible }
        }));
    }

    pub fn dispose_webview(&mut self, handle: &str) {
        if let Some(b) = self
            .bridge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            b.remove_webview(handle);
        }
        self.view_handles.retain(|_, h| h != handle);
        let Some(proc) = &self.proc else { return };
        let _ = proc.send(&json!({
            "method": "disposeWebview",
            "params": { "handle": handle }
        }));
    }

    pub fn has_sse_client(&self, handle: &str) -> bool {
        self.bridge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|b| b.has_sse_client(handle))
    }

    pub fn shutdown(&mut self) {
        *self.proc_sink.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if let Some(proc) = self.proc.take() {
            let _ = proc.send(&json!({ "id": 0, "method": "shutdown", "params": {} }));
            if let Ok(mut child) = proc.child.lock() {
                let mut exited = false;
                for _ in 0..8 {
                    if let Ok(Some(_)) = child.try_wait() {
                        exited = true;
                        break;
                    }
                    thread::sleep(std::time::Duration::from_millis(20));
                }
                if !exited {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        *self.bridge.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl Drop for ExtHost {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn default_theme_css(dark: bool) -> String {
    if dark {
        ":root { color-scheme: dark; --vscode-sideBar-background: #141820; --vscode-foreground: #d8dee9; --vscode-editor-background: #10141b; }".into()
    } else {
        ":root { color-scheme: light; --vscode-sideBar-background: #f5f6f8; --vscode-foreground: #1e2430; --vscode-editor-background: #ffffff; }".into()
    }
}

fn handle_rpc_message(val: &Value, bridge: &WebviewBridge, tx: &UnboundedSender<HostEvent>) {
    let Some(method) = val.get("method").and_then(Value::as_str) else {
        return;
    };
    let params = val.get("params").unwrap_or(&Value::Null);
    match method {
        "extStatus" => {
            let ext_id = params
                .get("extId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let st = match params.get("status").and_then(Value::as_str) {
                Some("active") => ExtStatus::Active,
                Some("error") => ExtStatus::Error(
                    params
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("Error")
                        .to_string(),
                ),
                _ => ExtStatus::Activating,
            };
            let _ = tx.unbounded_send(HostEvent::StatusChanged { ext_id, status: st });
        }
        "webviewHtml" => {
            if let (Some(h), Some(html)) = (
                params.get("handle").and_then(Value::as_str),
                params.get("html").and_then(Value::as_str),
            ) {
                let rev = bridge.set_html(h, html.to_string());
                let _ = tx.unbounded_send(HostEvent::WebviewHtmlUpdated {
                    handle: h.to_string(),
                    rev,
                });
            }
        }
        "webviewOptions" => {
            if let Some(h) = params.get("handle").and_then(Value::as_str) {
                let roots: Vec<PathBuf> = params
                    .get("localRoots")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(PathBuf::from)
                            .collect()
                    })
                    .unwrap_or_default();
                bridge.update_roots(h, roots);
            }
        }
        "webviewPostMessage" => {
            if let (Some(h), Some(msg)) = (
                params.get("handle").and_then(Value::as_str),
                params.get("messageJson").and_then(Value::as_str),
            ) {
                bridge.post_message(h, msg.to_string());
            }
        }
        "webviewViewResolved" => {
            let view_id = params
                .get("viewId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let handle = params
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let title = params
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if params.get("extFolder").and_then(Value::as_str).is_some() {
                let roots: Vec<PathBuf> = params
                    .get("localRoots")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(PathBuf::from)
                            .collect()
                    })
                    .unwrap_or_default();
                bridge.update_roots(&handle, roots);
            }
            let url = bridge.url_for(&handle);
            let _ = tx.unbounded_send(HostEvent::WebviewReady {
                view_id,
                handle,
                url,
                title,
                has_html: true,
            });
        }
        "webviewPanelCreated" => {
            let handle = params
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let title = params
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Panel")
                .to_string();
            let ext_folder = params
                .get("extFolder")
                .and_then(Value::as_str)
                .unwrap_or(".");
            let roots: Vec<PathBuf> = params
                .get("localRoots")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(PathBuf::from)
                        .collect()
                })
                .unwrap_or_default();
            bridge.register_webview(&handle, PathBuf::from(ext_folder), roots);
            let url = bridge.url_for(&handle);
            let _ = tx.unbounded_send(HostEvent::WebviewPanelOpened { handle, title, url });
        }
        "treeUpdated" => {
            let view_id = params
                .get("viewId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let parent_id = params
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_string);
            let items = params
                .get("items")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .map(|n| TreeNode {
                            id: n.get("id").and_then(Value::as_str).unwrap_or("").into(),
                            label: n.get("label").and_then(Value::as_str).unwrap_or("").into(),
                            description: n
                                .get("description")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .into(),
                            tooltip: n
                                .get("tooltip")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .into(),
                            collapsible: n
                                .get("collapsible")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            expanded: n.get("expanded").and_then(Value::as_bool).unwrap_or(false),
                            icon: n.get("icon").and_then(Value::as_str).map(str::to_string),
                            command: n.get("command").and_then(Value::as_str).map(str::to_string),
                            command_args: n
                                .get("commandArgs")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let _ = tx.unbounded_send(HostEvent::TreeUpdated {
                view_id,
                parent_id,
                items,
            });
        }
        "contextKey" => {
            if let Some(key) = params.get("key").and_then(Value::as_str) {
                let value = params.get("value").cloned().unwrap_or(Value::Null);
                let _ = tx.unbounded_send(HostEvent::ContextKey {
                    key: key.to_string(),
                    value,
                });
            }
        }
        "statusBarUpdate" => {
            let item = StatusBarItem {
                id: params
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                ext_id: params
                    .get("extId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                text: params
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                tooltip: params
                    .get("tooltip")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                command: params
                    .get("command")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                priority: params.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32,
            };
            if !item.text.is_empty() {
                let _ = tx.unbounded_send(HostEvent::StatusBarUpdated(item));
            }
        }
        "statusBarRemove" => {
            if let Some(id) = params.get("id").and_then(Value::as_str) {
                let _ = tx.unbounded_send(HostEvent::StatusBarRemoved(id.to_string()));
            }
        }
        "outputAppend" => {
            let ext_id = params
                .get("extId")
                .and_then(Value::as_str)
                .unwrap_or("host")
                .to_string();
            let channel = params
                .get("channel")
                .and_then(Value::as_str)
                .unwrap_or("output")
                .to_string();
            let text = params.get("text").and_then(Value::as_str).unwrap_or("");
            for line in text.lines() {
                let _ = tx.unbounded_send(HostEvent::OutputLine {
                    ext_id: ext_id.clone(),
                    channel: channel.clone(),
                    line: line.to_string(),
                });
            }
        }
        "showMessage" => {
            let level = params
                .get("level")
                .and_then(Value::as_str)
                .unwrap_or("info")
                .to_string();
            let text = params
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !text.is_empty() {
                let _ = tx.unbounded_send(HostEvent::Message { level, text });
            }
        }
        "openUrl" => {
            if let Some(u) = params.get("url").and_then(Value::as_str) {
                let _ = tx.unbounded_send(HostEvent::OpenUrl(u.to_string()));
            }
        }
        "openFile" => {
            if let Some(p) = params.get("path").and_then(Value::as_str) {
                let _ = tx.unbounded_send(HostEvent::OpenFile(PathBuf::from(p)));
            }
        }
        "runTerminal" => {
            if let Some(c) = params.get("cmd").and_then(Value::as_str) {
                let _ = tx.unbounded_send(HostEvent::RunTerminal { cmd: c.to_string() });
            }
        }
        "focusContainer" => {
            if let Some(id) = params.get("id").and_then(Value::as_str) {
                let _ = tx.unbounded_send(HostEvent::FocusContainer(id.to_string()));
            }
        }
        _ => {}
    }
}

pub struct ExtHostState {
    pub host: ExtHost,
    pub ext_status: HashMap<String, ExtStatus>,
    pub webviews: HashMap<String, WebviewInfo>,
    pub trees: HashMap<String, TreeState>,
    pub context_keys: HashMap<String, Value>,
    pub status_bar: Vec<StatusBarItem>,
    pub output_logs: HashMap<String, VecDeque<String>>,
    pub expanded_logs: HashSet<String>,
    pub activated_containers: HashSet<String>,
    pub host_notice: Option<String>,
}

impl ExtHostState {
    pub fn new(tx: UnboundedSender<HostEvent>) -> Self {
        Self {
            host: ExtHost::new(tx),
            ext_status: HashMap::new(),
            webviews: HashMap::new(),
            trees: HashMap::new(),
            context_keys: HashMap::new(),
            status_bar: Vec::new(),
            output_logs: HashMap::new(),
            expanded_logs: HashSet::new(),
            activated_containers: HashSet::new(),
            host_notice: None,
        }
    }

    pub fn status_for(&self, ext_id: &str) -> ExtStatus {
        self.ext_status
            .get(ext_id)
            .cloned()
            .unwrap_or(ExtStatus::Idle)
    }

    pub fn apply_event(&mut self, ev: HostEvent) -> Option<HostAction> {
        match ev {
            HostEvent::StatusChanged { ext_id, status } => {
                self.ext_status.insert(ext_id, status);
                None
            }
            HostEvent::WebviewReady {
                view_id,
                handle,
                url,
                title,
                has_html,
            } => {
                let prev_rev = self
                    .webviews
                    .get(&view_id)
                    .map(|w| w.html_rev)
                    .unwrap_or(0);
                self.webviews.insert(
                    view_id,
                    WebviewInfo {
                        handle,
                        url,
                        title,
                        has_html,
                        html_rev: prev_rev,
                    },
                );
                None
            }
            HostEvent::WebviewHtmlUpdated { handle, rev } => {
                for wv in self.webviews.values_mut() {
                    if wv.handle == handle {
                        wv.has_html = true;
                        wv.html_rev = rev;
                    }
                }
                Some(HostAction::WebviewHtmlUpdated { handle, rev })
            }
            HostEvent::WebviewPanelOpened { url, .. } => Some(HostAction::OpenPanelUrl(url)),
            HostEvent::TreeUpdated {
                view_id,
                parent_id,
                items,
            } => {
                let st = self.trees.entry(view_id).or_default();
                st.loading = false;
                match parent_id {
                    None => st.roots = items,
                    Some(pid) => {
                        st.children.insert(pid, items);
                    }
                }
                None
            }
            HostEvent::ContextKey { key, value } => {
                self.context_keys.insert(key, value);
                None
            }
            HostEvent::StatusBarUpdated(item) => {
                if let Some(pos) = self.status_bar.iter().position(|x| x.id == item.id) {
                    self.status_bar[pos] = item;
                } else {
                    self.status_bar.push(item);
                }
                self.status_bar
                    .sort_by_key(|a| std::cmp::Reverse(a.priority));
                None
            }
            HostEvent::StatusBarRemoved(id) => {
                self.status_bar.retain(|x| x.id != id);
                None
            }
            HostEvent::OutputLine {
                ext_id,
                channel,
                line,
            } => {
                let buf = self.output_logs.entry(ext_id).or_default();
                if buf.len() >= MAX_LOG_LINES {
                    buf.pop_front();
                }
                buf.push_back(format!("[{channel}] {line}"));
                None
            }
            HostEvent::Message { text, .. } => {
                self.host_notice = Some(text);
                None
            }
            HostEvent::OpenUrl(u) => Some(HostAction::OpenExternalUrl(u)),
            HostEvent::OpenFile(p) => Some(HostAction::OpenFile(p)),
            HostEvent::RunTerminal { cmd } => Some(HostAction::RunTerminal(cmd)),
            HostEvent::FocusContainer(id) => Some(HostAction::FocusContainer(id)),
            HostEvent::HostExited(reason) => {
                for st in self.ext_status.values_mut() {
                    if matches!(st, ExtStatus::Active | ExtStatus::Activating) {
                        *st = ExtStatus::Idle;
                    }
                }
                self.host_notice = Some(reason);
                None
            }
        }
    }
}

pub enum HostAction {
    OpenExternalUrl(String),
    OpenPanelUrl(String),
    OpenFile(PathBuf),
    RunTerminal(String),
    FocusContainer(String),
    WebviewHtmlUpdated { handle: String, rev: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext::market;
    use futures::channel::mpsc::unbounded;
    use std::time::{Duration, Instant};

    #[test]
    fn ext_host_smoke_test_with_node() {
        if crate::acp::which("node").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("fx-ext-host-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ext_dir = dir.join("acme.smoke-1.0.0");
        std::fs::create_dir_all(&ext_dir).unwrap();
        std::fs::write(
            ext_dir.join("package.json"),
            r#"{
                "publisher": "acme", "name": "smoke", "version": "1.0.0",
                "displayName": "Smoke", "main": "./extension.js",
                "contributes": {
                    "viewsContainers": { "activitybar": [{ "id": "smoke-bar", "title": "Smoke", "icon": "$(sparkle)" }] },
                    "views": {
                        "smoke-bar": [
                            { "type": "webview", "id": "smoke.webview", "name": "Smoke Webview" },
                            { "id": "smoke.tree", "name": "Smoke Tree" }
                        ]
                    },
                    "commands": [{ "command": "smoke.ping", "title": "Ping" }]
                }
            }"#,
        )
        .unwrap();
        std::fs::write(
            ext_dir.join("extension.js"),
            r#"
            const vscode = require('vscode');
            exports.activate = function(context) {
                vscode.commands.executeCommand('setContext', 'smoke.ready', true);
                const sb = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 10);
                sb.text = '$(sparkle) Smoke Ready';
                sb.show();
                vscode.window.registerWebviewViewProvider('smoke.webview', {
                    resolveWebviewView(view) {
                        view.webview.options = { enableScripts: true };
                        view.webview.html = '<html><body><h1>Smoke</h1></body></html>';
                    }
                });
                vscode.window.registerTreeDataProvider('smoke.tree', {
                    getChildren(el) { return el ? [] : [{ label: 'Root Item' }]; },
                    getTreeItem(el) { return new vscode.TreeItem(el.label, vscode.TreeItemCollapsibleState.None); }
                });
                vscode.commands.registerCommand('smoke.ping', () => {
                    vscode.window.showInformationMessage('Pong from smoke');
                });
            };
            "#,
        )
        .unwrap();

        let installed = vec![market::Installed {
            id: "acme.smoke".into(),
            folder: ext_dir.clone(),
            publisher: "acme".into(),
            name: "smoke".into(),
            display: "Smoke".into(),
            version: "1.0.0".into(),
            has_code: true,
            ..Default::default()
        }];
        vsc_registry::load(&installed);

        let (tx, mut rx) = unbounded();
        let mut host = ExtHost::new(tx);
        host.activate_extension("acme.smoke", "onView:smoke.webview", &[], true)
            .unwrap();
        let url = host
            .resolve_webview_view("acme.smoke", "smoke.webview", &[], true)
            .unwrap();
        assert!(url.starts_with("http://127.0.0.1:"));
        host.execute_command(Some("acme.smoke"), "smoke.ping", vec![], &[], true)
            .unwrap();

        let mut got_active = false;
        let mut got_webview = false;
        let mut got_tree = false;
        let mut got_status_bar = false;
        let mut got_ctx = false;
        let mut got_msg = false;

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline
            && !(got_active && got_webview && got_tree && got_status_bar && got_ctx && got_msg)
        {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    HostEvent::StatusChanged {
                        status: ExtStatus::Active,
                        ..
                    } => got_active = true,
                    HostEvent::WebviewReady { view_id, .. } if view_id == "smoke.webview" => {
                        got_webview = true
                    }
                    HostEvent::TreeUpdated { view_id, items, .. }
                        if view_id == "smoke.tree" && items.len() == 1 =>
                    {
                        got_tree = true
                    }
                    HostEvent::StatusBarUpdated(it) if it.text == "Smoke Ready" => {
                        got_status_bar = true
                    }
                    HostEvent::ContextKey { key, value }
                        if key == "smoke.ready" && value == Value::Bool(true) =>
                    {
                        got_ctx = true
                    }
                    HostEvent::Message { text, .. } if text == "Pong from smoke" => got_msg = true,
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        host.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            got_active && got_webview && got_tree && got_status_bar && got_ctx && got_msg,
            "active={got_active} webview={got_webview} tree={got_tree} sb={got_status_bar} ctx={got_ctx} msg={got_msg}"
        );
    }
}
