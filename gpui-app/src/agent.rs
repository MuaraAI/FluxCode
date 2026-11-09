//! OpenCode panel over the Agent Client Protocol (ACP), ported from
//! `src-tauri/src/acp.rs` (process) and `src/agent.js` (protocol + UI).
//! `opencode acp` speaks newline-delimited JSON-RPC on stdio.

use crate::core;
use crate::proc;
use crate::theme::{self, c};
use crate::workbench::{tool_button, SideView, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    switch::Switch,
    text::TextView,
    v_flex, IconName, Sizable,
};
use serde::{Deserialize, Serialize};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq)]
pub enum Status {
    Off,
    Starting,
    Ready,
    Busy,
    Error(String),
}

#[derive(Clone, Copy)]
enum Req {
    Initialize,
    NewSession,
    Prompt,
}

pub enum Item {
    User(String),
    Text(String),
    Thought(String),
    Tool { id: String, title: String, status: String },
    Plan(Vec<(String, String)>),
    Note(String, bool),
}

pub struct Permission {
    rpc_id: Value,
    title: String,
    options: Vec<(String, String, String)>, // (id, name, kind)
}

/// One saved conversation (user and agent messages only), in `agent-history.json`.
#[derive(Serialize, Deserialize, Clone)]
pub struct Chat {
    pub id: u64,
    pub title: String,
    /// Unix time in ms of the last message.
    pub updated: u64,
    /// (role, text) with role "user", "agent" or "note".
    pub messages: Vec<(String, String)>,
}

const HISTORY_FILE: &str = "agent-history.json";
const HISTORY_MAX: usize = 50;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// "just now", "12 min ago", "3 h ago", "yesterday", "5 days ago", "12 weeks ago".
fn ago(ms: u64) -> String {
    let s = now_ms().saturating_sub(ms) / 1000;
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        86_400..=172_799 => "yesterday".into(),
        172_800..=1_209_599 => format!("{} days ago", s / 86_400),
        _ => format!("{} weeks ago", s / 604_800),
    }
}

/// A file the agent wrote, with what was there before (None = new file).
pub struct Change {
    pub path: PathBuf,
    pub before: Option<String>,
}

pub struct AgentState {
    pub status: Status,
    proc_id: Option<u32>,
    buf: String,
    log_tail: Vec<String>,
    next_id: u64,
    pending: HashMap<u64, Req>,
    session: Option<String>,
    queued_prompt: Option<String>,
    pub items: Vec<Item>,
    permission: Option<Permission>,
    pub changes: Vec<Change>,
    /// Open editors to reload from disk after the agent wrote them (needs a Window).
    pub reloads: Vec<PathBuf>,
    input: Entity<InputState>,
    scroll: ScrollHandle,
    /// Saved chats, newest first.
    pub history: Vec<Chat>,
    /// The chat on screen, once it has been saved.
    chat_id: Option<u64>,
    pub show_history: bool,
    /// An earlier chat reopened from history: sent with the next message so a
    /// fresh OpenCode session knows what was said.
    carry: Option<String>,
    _sub: Subscription,
}

/// MuaraAI's native `omm` (oh-my-mira) harness, preferred over OpenCode when
/// installed (same search as the Tauri app's `find_agent`, feat/omm-acp-integration).
fn find_omm() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "omm.exe" } else { "omm" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    for var in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "HOME"] {
        if let Ok(base) = std::env::var(var) {
            let b = Path::new(&base);
            candidates.push(b.join(".cargo").join("bin").join(exe));
            candidates.push(b.join(".local").join("bin").join(exe));
            candidates.push(b.join("Programs").join("omm").join(exe));
        }
    }
    if !cfg!(windows) {
        candidates.push(PathBuf::from("/usr/local/bin/omm"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Which harness the panel talks to: "Mira" (omm) when installed, else "OpenCode".
/// Looked up once; both speak ACP over stdio (`<cli> acp`).
pub fn agent_name() -> &'static str {
    static NAME: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    NAME.get_or_init(|| if find_omm().is_some() { "Mira" } else { "OpenCode" })
}

/// Where the OpenCode CLI lives (same search as the Tauri app).
fn find_opencode() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if cfg!(windows) {
                candidates.push(dir.join("opencode.exe"));
                if dir.join("opencode.cmd").is_file() {
                    candidates.push(dir.join("node_modules").join("@opencode").join("cli").join("bin").join("opencode.exe"));
                    candidates.push(dir.join("node_modules").join("opencode-ai").join("bin").join("opencode.exe"));
                }
            } else {
                candidates.push(dir.join("opencode"));
            }
        }
    }
    for var in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "HOME"] {
        if let Ok(base) = std::env::var(var) {
            let b = Path::new(&base);
            candidates.push(b.join(".opencode").join("bin").join(if cfg!(windows) { "opencode.exe" } else { "opencode" }));
            candidates.push(b.join("Programs").join("opencode").join("opencode.exe"));
            candidates.push(b.join("npm").join("node_modules").join("@opencode").join("cli").join("bin").join("opencode.exe"));
            candidates.push(b.join("npm").join("node_modules").join("opencode-ai").join("bin").join("opencode.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn opencode_cmd() -> Option<std::process::Command> {
    if let Some(exe) = find_omm().or_else(find_opencode) {
        return Some(proc::command(exe.to_string_lossy().as_ref()));
    }
    if cfg!(windows)
        && std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("opencode.cmd").is_file()))
    {
        let mut c = proc::command("cmd.exe");
        c.args(["/d", "/c", "opencode"]);
        return Some(c);
    }
    None
}

impl AgentState {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Ask {}…", agent_name())));
        let sub = cx.subscribe_in(&input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.agent_send(window, cx);
            }
        });
        Self {
            status: Status::Off,
            proc_id: None,
            buf: String::new(),
            log_tail: Vec::new(),
            next_id: 0,
            pending: HashMap::new(),
            session: None,
            queued_prompt: None,
            items: Vec::new(),
            permission: None,
            changes: Vec::new(),
            reloads: Vec::new(),
            input,
            scroll: ScrollHandle::new(),
            history: crate::settings::load_json(HISTORY_FILE),
            chat_id: None,
            show_history: false,
            carry: None,
            _sub: sub,
        }
    }

    /// Save the chat on screen into the history (newest first).
    fn save_chat(&mut self) {
        let messages: Vec<(String, String)> = self
            .items
            .iter()
            .filter_map(|i| match i {
                Item::User(t) => Some(("user".to_string(), t.clone())),
                Item::Text(t) if !t.trim().is_empty() => Some(("agent".to_string(), t.clone())),
                Item::Note(t, _) => Some(("note".to_string(), t.clone())),
                _ => None,
            })
            .collect();
        if !messages.iter().any(|m| m.0 == "user") {
            return;
        }
        let id = *self.chat_id.get_or_insert_with(now_ms);
        let title = messages
            .iter()
            .find(|m| m.0 == "user")
            .map(|m| m.1.lines().next().unwrap_or("").chars().take(70).collect::<String>())
            .unwrap_or_default();
        self.history.retain(|c| c.id != id);
        self.history.insert(0, Chat { id, title, updated: now_ms(), messages });
        self.history.truncate(HISTORY_MAX);
        crate::settings::save_json(HISTORY_FILE, &self.history);
    }

    pub fn owns(&self, id: u32) -> bool {
        self.proc_id == Some(id)
    }

    fn append(&mut self, thought: bool, text: &str) {
        match (self.items.last_mut(), thought) {
            (Some(Item::Text(t)), false) | (Some(Item::Thought(t)), true) => t.push_str(text),
            _ => self.items.push(if thought { Item::Thought(text.into()) } else { Item::Text(text.into()) }),
        }
    }
}

impl Workbench {
    fn agent_write(&mut self, msg: Value) {
        if let Some(id) = self.agent.proc_id {
            self.procs.write_stdin(id, &format!("{msg}\n"));
        }
    }

    fn agent_request(&mut self, kind: Req, method: &str, params: Value) {
        self.agent.next_id += 1;
        let id = self.agent.next_id;
        self.agent.pending.insert(id, kind);
        self.agent_write(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
    }

    fn agent_start(&mut self, cx: &mut Context<Self>) {
        if matches!(self.agent.status, Status::Starting | Status::Ready | Status::Busy) {
            return;
        }
        let Some(mut cmd) = opencode_cmd() else {
            self.agent.status = Status::Error("No agent found. Install Mira (`omm`, from MuaraAI) or OpenCode (`npm i -g opencode-ai`).".into());
            cx.notify();
            return;
        };
        cmd.arg("acp").env("NO_COLOR", "1").env("FORCE_COLOR", "0");
        crate::github::apply_env(&mut cmd);
        self.agent.status = Status::Starting;
        self.agent.buf.clear();
        self.agent.log_tail.clear();
        match self.procs.spawn(cmd, self.root.clone().as_deref()) {
            Ok(id) => {
                self.agent.proc_id = Some(id);
                self.agent_request(
                    Req::Initialize,
                    "initialize",
                    json!({
                        "protocolVersion": 1,
                        "clientCapabilities": { "fs": { "readTextFile": true, "writeTextFile": true }, "terminal": false },
                        "clientInfo": { "name": "FluxCode", "version": env!("CARGO_PKG_VERSION") }
                    }),
                );
            }
            Err(e) => self.agent.status = Status::Error(format!("{} could not start: {e}", agent_name())),
        }
        cx.notify();
    }

    pub(crate) fn agent_stop(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.agent.proc_id.take() {
            self.procs.kill(id);
        }
        self.agent.status = Status::Off;
        self.agent.session = None;
        self.agent.pending.clear();
        self.agent.permission = None;
        cx.notify();
    }

    fn agent_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.agent.input.read(cx).value().trim().to_string();
        if text.is_empty() || self.agent.status == Status::Busy {
            return;
        }
        self.agent.input.update(cx, |s, cx| s.set_value("", window, cx));
        self.agent.items.push(Item::User(text.clone()));
        self.agent.scroll.scroll_to_bottom();
        match (&self.agent.status, &self.agent.session) {
            (Status::Ready, Some(_)) => self.agent_prompt(text, cx),
            _ => {
                self.agent.queued_prompt = Some(text);
                self.agent_start(cx);
            }
        }
        cx.notify();
    }

    fn agent_prompt(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(session) = self.agent.session.clone() else { return };
        let mut prompt = Vec::new();
        if let Some(earlier) = self.agent.carry.take() {
            prompt.push(json!({ "type": "text", "text": format!("Earlier messages of this conversation, for context:\n\n{earlier}\n\n---\n") }));
        }
        prompt.push(json!({ "type": "text", "text": text }));
        // Include the active file, like the Tauri panel's "include file" default.
        if let Some(ix) = self.active {
            let d = &self.docs[ix];
            let body = d.editor.read(cx).value();
            if body.len() < 200_000 {
                prompt.push(json!({
                    "type": "resource",
                    "resource": { "uri": format!("file:///{}", d.path.to_string_lossy().replace('\\', "/")), "text": body.to_string(), "mimeType": "text/plain" }
                }));
            }
        }
        self.agent.status = Status::Busy;
        self.agent_request(Req::Prompt, "session/prompt", json!({ "sessionId": session, "prompt": prompt }));
    }

    fn agent_cancel(&mut self, cx: &mut Context<Self>) {
        if let (Status::Busy, Some(s)) = (&self.agent.status, self.agent.session.clone()) {
            self.agent_write(json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": s } }));
            if let Some(p) = self.agent.permission.take() {
                self.agent_write(json!({ "jsonrpc": "2.0", "id": p.rpc_id, "result": { "outcome": { "outcome": "cancelled" } } }));
            }
        }
        cx.notify();
    }

    /// Stdout/stderr of the agent process (called from `on_proc_event`).
    pub(crate) fn agent_on_output(&mut self, text: &str, err: bool, cx: &mut Context<Self>) {
        if err {
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                self.agent.log_tail.push(l.to_string());
            }
            let n = self.agent.log_tail.len();
            if n > 40 {
                self.agent.log_tail.drain(..n - 40);
            }
            return;
        }
        self.agent.buf.push_str(text);
        while let Some(nl) = self.agent.buf.find('\n') {
            let line: String = self.agent.buf.drain(..=nl).collect();
            if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
                self.agent_on_message(msg, cx);
            }
        }
        self.agent.scroll.scroll_to_bottom();
    }

    pub(crate) fn agent_on_exit(&mut self, cx: &mut Context<Self>) {
        self.agent.proc_id = None;
        self.agent.session = None;
        self.agent.pending.clear();
        if self.agent.status != Status::Off {
            let tail = self.agent.log_tail.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join("\n");
            let name = agent_name();
            self.agent.status = Status::Error(if tail.is_empty() { format!("{name} stopped.") } else { format!("{name} stopped:\n{tail}") });
        }
        cx.notify();
    }

    fn agent_on_message(&mut self, msg: Value, cx: &mut Context<Self>) {
        if let Some(method) = msg.get("method").and_then(Value::as_str) {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            match msg.get("id") {
                Some(id) => self.agent_on_request(id.clone(), method, params, cx),
                None => self.agent_on_notification(method, params),
            }
            return;
        }
        let Some(id) = msg.get("id").and_then(Value::as_u64) else { return };
        let Some(kind) = self.agent.pending.remove(&id) else { return };
        if let Some(e) = msg.get("error") {
            let m = e.get("message").and_then(Value::as_str).unwrap_or("Agent error").to_string();
            match kind {
                Req::Prompt => {
                    self.agent.items.push(Item::Note(m, true));
                    self.agent.status = Status::Ready;
                }
                _ => self.agent.status = Status::Error(m),
            }
            return;
        }
        let result = msg.get("result").cloned().unwrap_or(Value::Null);
        match kind {
            Req::Initialize => {
                let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
                self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": [] }));
            }
            Req::NewSession => {
                self.agent.session = result.get("sessionId").and_then(Value::as_str).map(String::from);
                self.agent.status = Status::Ready;
                if let Some(text) = self.agent.queued_prompt.take() {
                    self.agent_prompt(text, cx);
                }
            }
            Req::Prompt => {
                let stop = result.get("stopReason").and_then(Value::as_str).unwrap_or("end_turn");
                if stop != "end_turn" {
                    self.agent.items.push(Item::Note(stop.replace('_', " "), false));
                }
                self.agent.permission = None;
                self.agent.status = Status::Ready;
                self.agent.save_chat();
                self.refresh_tree(cx);
            }
        }
    }

    fn agent_on_notification(&mut self, method: &str, params: Value) {
        if method != "session/update" {
            return;
        }
        let Some(u) = params.get("update") else { return };
        let text = |u: &Value| u.pointer("/content/text").and_then(Value::as_str).unwrap_or("").to_string();
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        match u.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
            "agent_message_chunk" => self.agent.append(false, &text(u)),
            "agent_thought_chunk" => self.agent.append(true, &text(u)),
            "tool_call" => self.agent.items.push(Item::Tool {
                id: s(u, "toolCallId").unwrap_or_default(),
                title: s(u, "title").unwrap_or_else(|| "Tool".into()),
                status: s(u, "status").unwrap_or_else(|| "pending".into()),
            }),
            "tool_call_update" => {
                let id = s(u, "toolCallId").unwrap_or_default();
                for item in self.agent.items.iter_mut().rev() {
                    if let Item::Tool { id: tid, title, status } = item {
                        if *tid == id {
                            if let Some(t) = s(u, "title") {
                                *title = t;
                            }
                            if let Some(st) = s(u, "status") {
                                *status = st;
                            }
                            break;
                        }
                    }
                }
            }
            "plan" => {
                let entries = u
                    .get("entries")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|e| (s(e, "content").unwrap_or_default(), s(e, "status").unwrap_or_default())).collect())
                    .unwrap_or_default();
                match self.agent.items.last_mut() {
                    Some(Item::Plan(p)) => *p = entries,
                    _ => self.agent.items.push(Item::Plan(entries)),
                }
            }
            _ => {}
        }
    }

    fn agent_on_request(&mut self, id: Value, method: &str, params: Value, cx: &mut Context<Self>) {
        let respond = |this: &mut Self, result: Result<Value, String>| {
            let msg = match result {
                Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
                Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": e } }),
            };
            this.agent_write(msg);
        };
        let path = params.get("path").and_then(Value::as_str).map(PathBuf::from);
        match (method, path) {
            ("session/request_permission", _) => {
                let title = params.pointer("/toolCall/title").and_then(Value::as_str).unwrap_or("The agent wants to run a tool").to_string();
                let options: Vec<(String, String, String)> = params
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|o| {
                                let g = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                                (g("optionId"), g("name"), g("kind"))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if self.settings.agent_auto_approve {
                    if let Some((oid, ..)) = options.iter().find(|o| o.2.starts_with("allow")) {
                        let oid = oid.clone();
                        respond(self, Ok(json!({ "outcome": { "outcome": "selected", "optionId": oid } })));
                        return;
                    }
                }
                self.agent.permission = Some(Permission { rpc_id: id.clone(), title, options });
                cx.notify();
            }
            ("fs/read_text_file", Some(p)) => {
                // Unsaved editor text wins over the file on disk.
                let text = match self.docs.iter().find(|d| d.path == p) {
                    Some(d) => Ok(d.editor.read(cx).value().to_string()),
                    None => core::read_text(&p).map(|f| f.text),
                };
                let result = text.map(|t| {
                    let line = params.get("line").and_then(Value::as_u64);
                    let limit = params.get("limit").and_then(Value::as_u64);
                    let content = if line.is_some() || limit.is_some() {
                        let start = line.unwrap_or(1).saturating_sub(1) as usize;
                        let it = t.split('\n').skip(start);
                        match limit {
                            Some(n) => it.take(n as usize).collect::<Vec<_>>().join("\n"),
                            None => it.collect::<Vec<_>>().join("\n"),
                        }
                    } else {
                        t
                    };
                    json!({ "content": content })
                });
                respond(self, result);
            }
            ("fs/write_text_file", Some(p)) => {
                let content = params.get("content").and_then(Value::as_str).unwrap_or("").to_string();
                let before = std::fs::read_to_string(&p).ok();
                let crlf = before.as_deref().is_some_and(|b| b.contains("\r\n"));
                let r = core::write_text(&p, &content, crlf);
                if r.is_ok() {
                    if !self.agent.changes.iter().any(|ch| ch.path == p) {
                        self.agent.changes.push(Change { path: p.clone(), before });
                    }
                    if self.docs.iter().any(|d| d.path == p) {
                        self.agent.reloads.push(p);
                    }
                    cx.notify();
                }
                respond(self, r.map(|_| json!({})));
            }
            _ => respond(self, Err(format!("Method not supported: {method}"))),
        }
    }

    /// The ON/OFF switch: start OpenCode, or stop it (the chat stays on screen).
    fn agent_set_on(&mut self, on: bool, cx: &mut Context<Self>) {
        if on {
            self.agent_start(cx);
        } else {
            self.agent.save_chat();
            self.agent_stop(cx);
        }
    }

    /// Start a new chat; a running OpenCode gets a fresh session.
    fn agent_new_chat(&mut self, cx: &mut Context<Self>) {
        self.agent.save_chat();
        self.agent.items.clear();
        self.agent.chat_id = None;
        self.agent.carry = None;
        self.agent.show_history = false;
        if self.agent.status == Status::Ready && self.agent.proc_id.is_some() {
            self.agent.session = None;
            self.agent.status = Status::Starting;
            let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
            self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": [] }));
        }
        cx.notify();
    }

    /// Reopen a saved chat. OpenCode can't resume the old session, so the next
    /// message carries the earlier ones as context.
    fn agent_open_chat(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(chat) = self.agent.history.iter().find(|c| c.id == id).cloned() else { return };
        if self.agent.chat_id != Some(id) {
            self.agent.save_chat();
        }
        self.agent.items = chat
            .messages
            .iter()
            .map(|(role, t)| match role.as_str() {
                "user" => Item::User(t.clone()),
                "note" => Item::Note(t.clone(), false),
                _ => Item::Text(t.clone()),
            })
            .collect();
        let mut earlier = String::new();
        for (role, t) in chat.messages.iter().rev().take(20).rev() {
            let who = if role == "user" { "User" } else { "Assistant" };
            let t: String = t.chars().take(2000).collect();
            earlier.push_str(&format!("{who}: {t}\n\n"));
        }
        self.agent.carry = Some(earlier);
        self.agent.chat_id = Some(id);
        self.agent.show_history = false;
        self.agent.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn agent_delete_chat(&mut self, id: u64, cx: &mut Context<Self>) {
        self.agent.history.retain(|c| c.id != id);
        crate::settings::save_json(HISTORY_FILE, &self.agent.history);
        if self.agent.chat_id == Some(id) {
            self.agent.chat_id = None;
        }
        cx.notify();
    }

    /// Move OpenCode between the left side bar and the right dock.
    fn agent_move_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.agent_right = !self.settings.agent_right;
        self.settings.save();
        if self.settings.agent_right {
            self.agent_open = true;
            if self.view == SideView::Agent {
                self.view = SideView::Explorer;
            }
        } else {
            self.agent_open = false;
            self.view = SideView::Agent;
            self.sidebar_open = true;
        }
        let h = self.agent.input.focus_handle(cx);
        h.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn agent_input_focus(&self, cx: &App) -> FocusHandle {
        self.agent.input.focus_handle(cx)
    }

    /// OpenCode docked on the right: its own title row above the panel.
    pub(crate) fn render_agent_dock(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .bg(c(theme::BG_SIDE))
            .border_l_1()
            .border_color(c(theme::BORDER))
            .child(
                h_flex()
                    .h(theme::u(35.))
                    .pl_3()
                    .pr_2()
                    .gap_1()
                    .flex_none()
                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_MUTED)).child(agent_name().to_uppercase()))
                    .child(self.render_agent_switch(cx))
                    .child(div().flex_1())
                    .child(self.render_agent_tools(cx))
                    .child(tool_button("ag-dock-close", IconName::Close, "Hide (Ctrl+Shift+A shows it again)", cx, |this, _, cx| {
                        this.agent_open = false;
                        cx.notify();
                    })),
            )
            .child(div().flex_1().min_h_0().child(self.render_agent(window, cx)))
            .into_any_element()
    }

    /// The ON/OFF switch, shown right of the panel title.
    pub(crate) fn render_agent_switch(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on = matches!(self.agent.status, Status::Starting | Status::Ready | Status::Busy);
        let weak = cx.entity().downgrade();
        Switch::new("ag-on")
            .checked(on)
            .small()
            .tooltip(if on { format!("{} is on. Click to turn it off", agent_name()) } else { format!("{} is off. Click to turn it on", agent_name()) })
            .on_click(move |checked, _, cx| {
                let checked = *checked;
                _ = weak.update(cx, |this, cx| this.agent_set_on(checked, cx));
            })
    }

    /// History and move-to-other-side, as icons in the panel title row.
    pub(crate) fn render_agent_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let right = self.settings.agent_right;
        h_flex()
            .gap_px()
            .child(
                Button::new("ag-history-btn")
                    .icon(if self.agent.show_history { gpui_kit::component::Icon::new(IconName::Bot) } else { gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Clock) })
                    .xsmall()
                    .ghost()
                    .tooltip(if self.agent.show_history { "Back to the chat" } else { "Saved chats" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.show_history = !this.agent.show_history;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ag-dock")
                    .icon(if right { IconName::PanelLeft } else { IconName::PanelRight })
                    .xsmall()
                    .ghost()
                    .tooltip(if right { "Move to the left side bar" } else { "Move to the right side" })
                    .on_click(cx.listener(|this, _, w, cx| this.agent_move_dock(w, cx))),
            )
    }

    fn render_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let mut list = v_flex().id("ag-history").flex_1().min_h_0().px_2().gap_px().overflow_y_scroll();
        if a.history.is_empty() {
            list = list.child(div().px_1().py_2().text_sm().text_color(c(theme::FG_DIM)).child("No saved chats yet. Chats are saved after each answer."));
        }
        for (i, chat) in a.history.iter().enumerate() {
            let id = chat.id;
            let current = a.chat_id == Some(id);
            let count = chat.messages.iter().filter(|m| m.0 == "user").count();
            list = list.child(
                h_flex()
                    .id(("ag-chat", i))
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(current, |d| d.bg(c(theme::SELECT)))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().truncate().text_color(c(theme::FG_STRONG)).child(chat.title.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(format!("{} · {count} message{}", ago(chat.updated), if count == 1 { "" } else { "s" })),
                            ),
                    )
                    .child(
                        Button::new(("ag-chat-del", i))
                            .icon(IconName::Delete)
                            .ghost()
                            .xsmall()
                            .tooltip("Delete this chat")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.agent_delete_chat(id, cx);
                            })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.agent_open_chat(id, cx))),
            );
        }
        v_flex()
            .size_full()
            .child(
                div().px_3().pb_2().child(
                    Button::new("ag-new-chat")
                        .icon(IconName::Plus)
                        .label("New chat")
                        .small()
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| this.agent_new_chat(cx))),
                ),
            )
            .child(list)
            .into_any_element()
    }

    fn agent_answer_permission(&mut self, option: Option<String>, cx: &mut Context<Self>) {
        if let Some(p) = self.agent.permission.take() {
            let outcome = match option {
                Some(o) => json!({ "outcome": "selected", "optionId": o }),
                None => json!({ "outcome": "cancelled" }),
            };
            self.agent_write(json!({ "jsonrpc": "2.0", "id": p.rpc_id, "result": { "outcome": outcome } }));
        }
        cx.notify();
    }

    /// Reload open editors the agent changed on disk (render has the Window).
    pub(crate) fn apply_agent_reloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for p in std::mem::take(&mut self.agent.reloads) {
            if let (Some(d), Ok(f)) = (self.docs.iter_mut().find(|d| d.path == p), core::read_text(&p)) {
                d.crlf = f.crlf;
                d.editor.update(cx, |s, cx| s.set_value(f.text, window, cx));
                d.dirty = false;
            }
        }
    }

    fn agent_undo(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.agent.changes.len() {
            return;
        }
        let ch = self.agent.changes.remove(ix);
        let r = match &ch.before {
            Some(b) => std::fs::write(&ch.path, b).map_err(|e| e.to_string()),
            None => core::delete_path(&ch.path),
        };
        self.status = match r {
            Ok(()) => format!("Reverted {}", ch.path.display()).into(),
            Err(e) => format!("Undo failed: {e}").into(),
        };
        if self.docs.iter().any(|d| d.path == ch.path) && ch.before.is_some() {
            self.agent.reloads.push(ch.path.clone());
        }
        self.refresh_tree(cx);
        cx.notify();
    }

    pub(crate) fn render_agent(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let (dot, label) = match &a.status {
            Status::Off => (theme::FG_DIM, "Off".to_string()),
            Status::Starting => (theme::FG_MUTED, "Starting…".into()),
            Status::Ready => (theme::GREEN, "Ready".into()),
            Status::Busy => (theme::BLUE, "Working…".into()),
            Status::Error(_) => (theme::DANGER, "Error".into()),
        };
        // The header holds the switch and tools; here only a short line while it is
        // starting or working (with Stop), so the chat keeps the room.
        let on = matches!(a.status, Status::Starting | Status::Ready | Status::Busy);
        let header = h_flex()
            .px_3()
            .pb_1()
            .gap_2()
            .text_xs()
            .when(matches!(a.status, Status::Starting | Status::Busy), |h| {
                h.child(div().size(px(6.)).rounded_full().bg(c(dot)))
                    .child(div().flex_1().text_color(c(theme::FG_MUTED)).child(label))
                    .when(a.status == Status::Busy, |h| {
                        h.child(Button::new("ag-cancel").label("Stop").xsmall().on_click(cx.listener(|this, _, _, cx| this.agent_cancel(cx))))
                    })
            });
        if a.show_history {
            return v_flex().size_full().child(header).child(self.render_history(cx)).into_any_element();
        }

        let mut log = v_flex().id("ag-log").flex_1().min_h_0().px_3().gap_2().overflow_y_scroll().track_scroll(&a.scroll);
        if a.items.is_empty() {
            log = log.child(
                div()
                    .text_sm()
                    .text_color(c(theme::FG_DIM))
                    .child(if on {
                        "Ask about the open file, or anything else.".to_string()
                    } else {
                        format!("{} is off. Type a message and it turns on by itself.", agent_name())
                    }),
            );
        }
        if let Status::Error(e) = &a.status {
            log = log.child(div().p_2().rounded(px(4.)).border_1().border_color(c(theme::DANGER)).bg(c(theme::BG_ELEVATED)).text_xs().text_color(c(theme::DANGER)).child(e.clone()));
        }
        for (i, item) in a.items.iter().enumerate() {
            log = log.child(match item {
                Item::User(t) => div()
                    .p_2()
                    .rounded(px(6.))
                    .bg(c(theme::BG_ELEVATED))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG))
                    .text_sm()
                    .text_color(c(theme::FG_STRONG))
                    .child(t.clone())
                    .into_any_element(),
                Item::Text(t) => TextView::markdown(("ag-md", i), t.clone()).text_sm().into_any_element(),
                Item::Thought(t) => div()
                    .text_xs()
                    .italic()
                    .text_color(c(theme::FG_DIM))
                    .child(format!("Thinking: {}", t.chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect::<String>()))
                    .into_any_element(),
                Item::Tool { title, status, .. } => {
                    let (mark, col) = match status.as_str() {
                        "completed" => ("✓", theme::GREEN),
                        "failed" => ("✕", theme::DANGER),
                        "in_progress" => ("…", theme::BLUE),
                        _ => ("•", theme::FG_DIM),
                    };
                    h_flex()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded(px(4.))
                        .bg(c(theme::BG_ELEVATED))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .text_xs()
                        .child(div().text_color(c(col)).child(mark))
                        .child(div().truncate().text_color(c(theme::FG)).child(title.clone()))
                        .into_any_element()
                }
                Item::Plan(entries) => v_flex()
                    .gap_1()
                    .text_xs()
                    .children(entries.iter().map(|(t, st)| {
                        let mark = match st.as_str() {
                            "completed" => "☑",
                            "in_progress" => "▶",
                            _ => "☐",
                        };
                        div().text_color(c(theme::FG_MUTED)).child(format!("{mark} {t}"))
                    }))
                    .into_any_element(),
                Item::Note(t, err) => div()
                    .text_xs()
                    .text_color(c(if *err { theme::DANGER } else { theme::FG_DIM }))
                    .child(t.clone())
                    .into_any_element(),
            });
        }

        let permission = a.permission.as_ref().map(|p| {
            v_flex()
                .m_3()
                .p_2()
                .gap_2()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::ACCENT))
                .bg(c(theme::BG_ELEVATED))
                .text_xs()
                .child(div().text_color(c(theme::FG_STRONG)).child(format!("Allow: {}?", p.title)))
                .child(h_flex().gap_1().flex_wrap().children(p.options.iter().enumerate().map(|(i, (oid, name, kind))| {
                    let oid = oid.clone();
                    let b = Button::new(("perm", i)).label(name.clone()).xsmall();
                    let b = if kind.starts_with("allow") { b.primary() } else { b };
                    b.on_click(cx.listener(move |this, _, _, cx| this.agent_answer_permission(Some(oid.clone()), cx)))
                })))
        });

        let changes = (!a.changes.is_empty()).then(|| {
            v_flex()
                .mx_3()
                .mb_2()
                .p_2()
                .gap_1()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_ELEVATED))
                .text_xs()
                .child(
                    h_flex()
                        .child(div().flex_1().text_color(c(theme::FG_MUTED)).child(format!("{} file(s) changed by the agent", a.changes.len())))
                        .child(Button::new("ag-keep-all").label("Keep all").xsmall().ghost().on_click(cx.listener(|this, _, _, cx| {
                            this.agent.changes.clear();
                            cx.notify();
                        }))),
                )
                .children(a.changes.iter().enumerate().map(|(i, ch)| {
                    let path = ch.path.clone();
                    h_flex()
                        .gap_1()
                        .child(
                            div()
                                .id(("ag-ch", i))
                                .flex_1()
                                .truncate()
                                .text_color(c(theme::FG))
                                .child(format!("{}{}", if ch.before.is_none() { "+ " } else { "" }, ch.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
                                .on_click(cx.listener(move |this, _, w, cx| this.open_file(path.clone(), w, cx))),
                        )
                        .child(Button::new(("ag-keep", i)).label("Keep").xsmall().ghost().on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.agent.changes.len() {
                                this.agent.changes.remove(i);
                            }
                            cx.notify();
                        })))
                        .child(Button::new(("ag-undo", i)).label("Undo").xsmall().on_click(cx.listener(move |this, _, _, cx| this.agent_undo(i, cx))))
                }))
        });

        v_flex()
            .size_full()
            .child(header)
            .child(log)
            .when_some(permission, |v, p| v.child(p))
            .when_some(changes, |v, ch| v.child(ch))
            .child(div().p_3().child(Input::new(&a.input).small()))
            .into_any_element()
    }
}
