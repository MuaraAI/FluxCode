//! OpenCode panel over the Agent Client Protocol (ACP), ported from
//! `src-tauri/src/acp.rs` (process) and `src/agent.js` (protocol + UI).
//! `opencode acp` speaks newline-delimited JSON-RPC on stdio.

use crate::agent_ctx::{self, RemoteSession};
use crate::core;
use crate::memory;
use crate::proc;
use crate::theme::{self, c};
use crate::workbench::{tool_button, SideView, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::DropdownMenu,
    switch::Switch,
    text::TextView,
    v_flex, Disableable, Icon, IconName, Sizable,
};
use serde::{Deserialize, Serialize};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
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
    LoadSession,
    Prompt,
}

pub enum Item {
    User(String),
    Text(String),
    Thought(String),
    /// `kind` is ACP's tool kind: read, edit, delete, move, search, execute, think, fetch, other.
    Tool { id: String, title: String, status: String, kind: String },
    Plan(Vec<(String, String)>),
    Note(String, bool),
    /// A fact the agent asked to keep (saved to memory).
    Saved(String),
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
    /// `@file` being typed: (offset of `@`, matching files, selected row).
    pub mention: Option<(usize, Vec<usize>, usize)>,
    /// Context chips the user switched off: "file", "sel", "err", "out", or "@path".
    pub off_ctx: HashSet<String>,
    /// Sessions OpenCode / Mira saved for this folder: 0 not loaded, 1 loading, 2 loaded.
    pub remote: Vec<RemoteSession>,
    pub remote_state: u8,
    pub remote_err: Option<String>,
    /// The harness can resume an old session (ACP `loadSession`).
    can_load: bool,
    /// Resume this session once the agent has started.
    pending_load: Option<String>,
    /// Replaying this session's history right now.
    loading_id: Option<String>,
    pub show_memory: bool,
    memory_input: Entity<InputState>,
    /// Memory trails sent with the current turn (reinforced on Keep, weakened on Undo).
    turn_memories: Vec<String>,
    /// The REMEMBER instruction went out in this session.
    hinted: bool,
    pub memories: Vec<memory::Trail>,
    _sub: Subscription,
    _sub2: Subscription,
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

/// The harness chosen in the panel's title menu: 0 auto, 1 OpenCode, 2 Mira.
static CHOICE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_choice(cli: &str) {
    let v = match cli {
        "opencode" => 1,
        "mira" => 2,
        _ => 0,
    };
    CHOICE.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Is MuaraAI's `omm` (Mira) on this PC? Looked up once.
pub fn mira_installed() -> bool {
    static FOUND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| find_omm().is_some())
}

/// Which harness the panel talks to. Both speak ACP over stdio (`<cli> acp`).
pub fn agent_name() -> &'static str {
    match CHOICE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => "OpenCode",
        2 => "Mira",
        _ if mira_installed() => "Mira",
        _ => "OpenCode",
    }
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
    let exe = if agent_name() == "Mira" { find_omm() } else { find_opencode() };
    if agent_name() == "Mira" {
        return exe.map(|e| proc::command(e.to_string_lossy().as_ref()));
    }
    if let Some(exe) = exe {
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
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Ask {}…  (@ adds a file)", agent_name())));
        let sub = cx.subscribe_in(&input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { shift: false, .. } if this.agent.mention.is_some() => this.agent_mention_accept(window, cx),
            InputEvent::PressEnter { shift: false, .. } => this.agent_send(window, cx),
            InputEvent::Change => this.agent_update_mention(cx),
            _ => {}
        });
        let memory_input = cx.new(|cx| InputState::new(window, cx).placeholder("Remember something about this project…"));
        let sub2 = cx.subscribe_in(&memory_input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = this.agent.memory_input.read(cx).value().to_string();
                this.agent.memory_input.update(cx, |s, cx| s.set_value("", window, cx));
                this.agent_remember(&text, cx);
            }
        });
        // Memory evaporates once per start, off the UI thread (same rule as `ant_memory.py decay`).
        std::thread::spawn(|| {
            let _ = memory::Store::default().decay();
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
            mention: None,
            off_ctx: HashSet::new(),
            remote: Vec::new(),
            remote_state: 0,
            remote_err: None,
            can_load: false,
            pending_load: None,
            loading_id: None,
            show_memory: false,
            memory_input,
            turn_memories: Vec::new(),
            hinted: false,
            memories: Vec::new(),
            _sub: sub,
            _sub2: sub2,
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

    fn append_user(&mut self, text: &str) {
        match self.items.last_mut() {
            Some(Item::User(t)) => t.push_str(text),
            _ => self.items.push(Item::User(text.into())),
        }
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
            self.agent.status = Status::Error(if agent_name() == "Mira" {
                "Mira (omm) isn't installed on this PC. Install omm, or pick OpenCode in the menu on the panel title.".into()
            } else {
                "OpenCode isn't installed. Install it with  npm i -g opencode-ai , or pick Mira in the panel title menu.".into()
            });
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
        self.agent.hinted = false;
        self.agent.loading_id = None;
        cx.notify();
    }

    fn agent_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.agent.input.read(cx).value().trim().to_string();
        if text.is_empty() || self.agent.status == Status::Busy {
            return;
        }
        self.agent.input.update(cx, |s, cx| s.set_value("", window, cx));
        self.agent.mention = None;
        if let Some(fact) = text.strip_prefix("/remember ").or_else(|| text.strip_prefix("/ingat ")) {
            self.agent_remember(fact, cx);
            return;
        }
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
        prompt.push(json!({ "type": "text", "text": text.clone() }));
        prompt.extend(self.agent_context_blocks(&text, cx));
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
                self.agent.can_load = result.pointer("/agentCapabilities/loadSession").and_then(Value::as_bool).unwrap_or(false);
                let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
                match self.agent.pending_load.take() {
                    Some(id) if self.agent.can_load => self.agent_load_session(id),
                    _ => self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": [] })),
                }
            }
            Req::LoadSession => {
                self.agent.session = self.agent.loading_id.take();
                self.agent.status = Status::Ready;
                self.agent.hinted = false;
                self.agent.save_chat();
                if let Some(text) = self.agent.queued_prompt.take() {
                    self.agent_prompt(text, cx);
                }
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
                self.agent_take_remembers();
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
            // Only while an old session replays; live prompts are shown when sent.
            "user_message_chunk" if self.agent.loading_id.is_some() => self.agent.append_user(&text(u)),
            "agent_thought_chunk" => self.agent.append(true, &text(u)),
            "tool_call" => self.agent.items.push(Item::Tool {
                id: s(u, "toolCallId").unwrap_or_default(),
                title: s(u, "title").unwrap_or_else(|| "Tool".into()),
                status: s(u, "status").unwrap_or_else(|| "pending".into()),
                kind: s(u, "kind").unwrap_or_else(|| "other".into()),
            }),
            "tool_call_update" => {
                let id = s(u, "toolCallId").unwrap_or_default();
                for item in self.agent.items.iter_mut().rev() {
                    if let Item::Tool { id: tid, title, status, kind } = item {
                        if *tid == id {
                            if let Some(k) = s(u, "kind") {
                                *kind = k;
                            }
                        }
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
            .min_w_0()
            .overflow_hidden()
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
                    .child(self.render_agent_picker())
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

    /// Switch the harness from the title menu. A running agent is stopped first.
    fn agent_use(&mut self, cli: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.agent_cli == cli {
            return;
        }
        self.agent.save_chat();
        self.agent_stop(cx);
        self.settings.agent_cli = cli.into();
        self.settings.save();
        set_choice(cli);
        let name = agent_name();
        self.agent.input.update(cx, |s, cx| s.set_placeholder(format!("Ask {name}…"), window, cx));
        cx.notify();
    }

    pub(crate) fn on_agent_use_opencode(&mut self, _: &crate::workbench::AgentUseOpenCode, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_use("opencode", w, cx);
    }

    pub(crate) fn on_agent_use_mira(&mut self, _: &crate::workbench::AgentUseMira, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_use("mira", w, cx);
    }

    /// The panel title: the harness name, which opens a menu to pick OpenCode or Mira.
    pub(crate) fn render_agent_picker(&self) -> impl IntoElement {
        let name = agent_name();
        Button::new("ag-pick")
            .label(name.to_uppercase())
            .ghost()
            .xsmall()
            .dropdown_caret(true)
            .tooltip("Choose the AI agent")
            .dropdown_menu(move |m: gpui_kit::component::menu::PopupMenu, _, _| {
                let mira = if mira_installed() { "Mira (omm)" } else { "Mira (omm), not installed" };
                m.min_w(px(220.))
                    .menu_with_check("OpenCode", name == "OpenCode", Box::new(crate::workbench::AgentUseOpenCode))
                    .menu_with_check(mira, name == "Mira", Box::new(crate::workbench::AgentUseMira))
            })
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
                Button::new("ag-memory-btn")
                    .icon(Icon::new(gpui_kit::assets::IconName::Brain))
                    .xsmall()
                    .ghost()
                    .tooltip(if self.agent.show_memory { "Back to the chat" } else { "Memory: what the agent remembers" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.show_memory = !this.agent.show_memory;
                        this.agent.show_history = false;
                        if this.agent.show_memory {
                            this.agent_refresh_memories();
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ag-history-btn")
                    .icon(if self.agent.show_history { gpui_kit::component::Icon::new(IconName::Bot) } else { gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Clock) })
                    .xsmall()
                    .ghost()
                    .tooltip(if self.agent.show_history { "Back to the chat" } else { "Saved chats" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.show_history = !this.agent.show_history;
                        this.agent.show_memory = false;
                        if this.agent.show_history {
                            this.agent_load_remote_list(cx);
                        }
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
            .child(list.child(self.render_remote_sessions(cx)))
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
        let store = memory::Store::default();
        for id in std::mem::take(&mut self.agent.turn_memories) {
            let _ = store.fail(&id);
        }
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

    // ------------------------------------------------------------ @mentions

    /// Re-read the input after each change: is an `@file` being typed?
    pub(crate) fn agent_update_mention(&mut self, cx: &mut Context<Self>) {
        let (text, cursor) = {
            let s = self.agent.input.read(cx);
            (s.value().to_string(), s.cursor())
        };
        let Some((at, q)) = agent_ctx::mention_query(&text, cursor) else {
            if self.agent.mention.take().is_some() {
                cx.notify();
            }
            return;
        };
        let qb: Vec<u8> = q.to_lowercase().into_bytes();
        let mut scored: Vec<(i32, usize)> = self
            .all_lower
            .iter()
            .enumerate()
            .filter_map(|(i, p)| crate::quick::fuzzy_lower(&qb, p.as_bytes()).map(|s| (s, i)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        let hits: Vec<usize> = scored.into_iter().take(8).map(|(_, i)| i).collect();
        self.agent.mention = Some((at, hits, 0));
        cx.notify();
    }

    /// Put the highlighted file into the message as `@relative/path `.
    pub(crate) fn agent_mention_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((at, hits, sel)) = self.agent.mention.take() else { return };
        let Some(&ix) = hits.get(sel) else {
            cx.notify();
            return;
        };
        let rel = self.rel_path(&self.all_files[ix]).replace('\\', "/");
        let (text, cursor) = {
            let s = self.agent.input.read(cx);
            (s.value().to_string(), s.cursor().min(s.value().len()))
        };
        let new = format!("{}@{} {}", &text[..at], rel, &text[cursor..]);
        let caret = (text[..at].chars().count() + rel.chars().count() + 2) as u32;
        self.agent.input.update(cx, |s, cx| {
            s.set_value(new, window, cx);
            s.set_cursor_position(gpui_kit::component::input::Position::new(0, caret), window, cx);
        });
        self.agent.mention = None;
        cx.notify();
    }

    pub(crate) fn on_agent_mention_up(&mut self, _: &crate::workbench::AgentMentionUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, _, sel)) = self.agent.mention.as_mut() {
            *sel = sel.saturating_sub(1);
            cx.notify();
        }
    }

    pub(crate) fn on_agent_mention_down(&mut self, _: &crate::workbench::AgentMentionDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, hits, sel)) = self.agent.mention.as_mut() {
            if *sel + 1 < hits.len() {
                *sel += 1;
            }
            cx.notify();
        }
    }

    pub(crate) fn on_agent_mention_accept(&mut self, _: &crate::workbench::AgentMentionAccept, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_mention_accept(w, cx);
    }

    pub(crate) fn on_agent_mention_close(&mut self, _: &crate::workbench::AgentMentionClose, _: &mut Window, cx: &mut Context<Self>) {
        if self.agent.mention.take().is_some() {
            cx.notify();
        }
    }

    fn rel_path(&self, full: &str) -> String {
        let root = self.root.as_deref().map(|r| r.to_string_lossy().len()).unwrap_or(0);
        full.get(root..).map(|s| s.trim_start_matches(['\\', '/']).to_string()).unwrap_or_else(|| full.to_string())
    }

    // ------------------------------------------------------------ context

    /// What goes with the next message, exactly as the chips above the input show it:
    /// (key, label, icon, on). The same list builds the prompt, so what you see is sent.
    fn agent_chips(&self, cx: &App) -> Vec<(String, String, Icon, bool)> {
        let text = self.agent.input.read(cx).value().to_string();
        let off = |k: &str| self.agent.off_ctx.contains(k);
        let mut chips = Vec::new();
        if let Some(d) = self.active.map(|ix| &self.docs[ix]).filter(|d| d.image.is_none()) {
            chips.push(("file".into(), d.name.to_string(), Icon::new(IconName::FileText), !off("file")));
            let ed = d.editor.read(cx);
            let range = ed.selected_range();
            if !range.is_empty() {
                let v = ed.value();
                let (a, b) = agent_ctx::line_span(&v, range.start, range.end);
                let label = if a == b { format!("Selection · line {a}") } else { format!("Selection · lines {a}-{b}") };
                chips.push(("sel".into(), label, Icon::new(gpui_kit::assets::IconName::TextCursorInput), !off("sel")));
            }
        }
        if agent_ctx::wants_error(&text) {
            if let Some(p) = self.problems.first() {
                let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let kind = p.message.split(':').next().unwrap_or("Error").to_string();
                chips.push(("err".into(), format!("{kind} · {name}:{}", p.line), Icon::new(IconName::CircleX), !off("err")));
            } else if self.agent_last_failure().is_some() {
                chips.push(("err".into(), "Last failed command".into(), Icon::new(IconName::CircleX), !off("err")));
            }
        }
        if agent_ctx::wants_output(&text) && !self.term_lines.is_empty() {
            chips.push(("out".into(), "Terminal output".into(), Icon::new(IconName::SquareTerminal), !off("out")));
        }
        for m in agent_ctx::mentions(&text) {
            if let Some(root) = &self.root {
                if root.join(&m).exists() {
                    let name = Path::new(&m).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(m.clone());
                    let key = format!("@{m}");
                    let on = !off(&key);
                    chips.push((key, name, Icon::new(gpui_kit::assets::IconName::AtSign), on));
                }
            }
        }
        chips
    }

    /// The last command that failed in the terminal (or the last failed run), with its output.
    fn agent_last_failure(&self) -> Option<String> {
        for lines in [&self.term_lines, &self.out_lines] {
            let end = lines.iter().rposition(|l| l.kind == crate::term::LineKind::Status && l.err)?;
            let start = lines[..end].iter().rposition(|l| l.kind == crate::term::LineKind::Cmd).unwrap_or(0);
            let body: Vec<&str> = lines[start..=end].iter().map(|l| l.text.as_ref()).collect();
            let tail = body.len().saturating_sub(60);
            return Some(body[tail..].join("\n"));
        }
        None
    }

    /// Context blocks for a prompt: the chips that are on, memory, and the
    /// one-time note that lets the agent save facts with `REMEMBER:`.
    fn agent_context_blocks(&mut self, text: &str, cx: &mut Context<Self>) -> Vec<Value> {
        let chips = self.agent_chips(cx);
        let mut blocks = Vec::new();
        let resource = |path: &Path, body: String| {
            json!({ "type": "resource", "resource": { "uri": format!("file:///{}", path.to_string_lossy().replace('\\', "/")), "text": body, "mimeType": "text/plain" } })
        };
        for (key, _, _, on) in &chips {
            if !on {
                continue;
            }
            match key.as_str() {
                "file" => {
                    if let Some(d) = self.active.map(|ix| &self.docs[ix]) {
                        let body = d.editor.read(cx).value().to_string();
                        if body.len() < 200_000 {
                            blocks.push(resource(&d.path, body));
                        }
                    }
                }
                "sel" => {
                    if let Some(d) = self.active.map(|ix| &self.docs[ix]) {
                        let ed = d.editor.read(cx);
                        let r = ed.selected_range();
                        let v = ed.value();
                        let (a, b) = agent_ctx::line_span(&v, r.start, r.end);
                        let sel = v.get(r.clone()).unwrap_or("").to_string();
                        let rel = self.rel_path(&d.path.to_string_lossy());
                        blocks.push(json!({ "type": "text", "text": format!(
                            "The user selected lines {a}-{b} of {rel}; the question is most likely about this part:\n```{}\n{sel}\n```",
                            agent_ctx::fence_lang(&d.path)
                        ) }));
                    }
                }
                "err" => {
                    let mut t = String::from("The most recent error in FluxCode (the user is probably asking about it):\n");
                    for p in &self.problems {
                        t.push_str(&format!("- {} at {}:{}\n", p.message, self.rel_path(&p.path.to_string_lossy()), p.line));
                    }
                    if let Some(f) = self.agent_last_failure() {
                        t.push_str(&format!("```\n{f}\n```\n"));
                    }
                    blocks.push(json!({ "type": "text", "text": t }));
                }
                "out" => {
                    let n = self.term_lines.len();
                    let tail: Vec<&str> = self.term_lines[n.saturating_sub(60)..].iter().map(|l| l.text.as_ref()).collect();
                    blocks.push(json!({ "type": "text", "text": format!("Recent terminal output:\n```\n{}\n```", tail.join("\n")) }));
                }
                k if k.starts_with('@') => {
                    if let Some(root) = &self.root {
                        let p = root.join(&k[1..]);
                        if p.is_file() {
                            if let Ok(f) = core::read_text(&p) {
                                if f.text.len() < 200_000 {
                                    blocks.push(resource(&p, f.text));
                                }
                            }
                        } else if p.is_dir() {
                            let names: Vec<String> = std::fs::read_dir(&p)
                                .map(|rd| rd.flatten().take(200).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                                .unwrap_or_default();
                            blocks.push(json!({ "type": "text", "text": format!("Contents of the folder {}:\n{}", &k[1..], names.join("\n")) }));
                        }
                    }
                }
                _ => {}
            }
        }
        // Memory: the few trails that match this question (P = tau * eta^2).
        let file = self.active.map(|ix| self.docs[ix].name.to_string()).unwrap_or_default();
        let trails = memory::Store::default().recall(&format!("{text} {file}"), 4, Some(&self.memory_scope()));
        self.agent.turn_memories = trails.iter().map(|t| t.id.clone()).collect();
        if !trails.is_empty() {
            let list: Vec<String> = trails.iter().map(|t| format!("- {}", t.content)).collect();
            blocks.push(json!({ "type": "text", "text": format!(
                "Notes from earlier sessions (FluxCode memory, may be outdated; ignore them if they don't fit):\n{}",
                list.join("\n")
            ) }));
            self.agent.items.push(Item::Note(format!("Used {} note{} from memory", trails.len(), if trails.len() == 1 { "" } else { "s" }), false));
        }
        if !self.agent.hinted {
            self.agent.hinted = true;
            blocks.push(json!({ "type": "text", "text":
                "FluxCode note: answer in the language the user writes in. When you learn a lasting fact about this project \
                 (how to run, test or deploy it, a convention, a decision), end your reply with one line `REMEMBER: <fact>`. \
                 FluxCode saves it to the user's memory and hides the line." }));
        }
        blocks
    }

    pub(crate) fn memory_scope(&self) -> String {
        let name = self.root.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        format!("project:{name}")
    }

    /// `REMEMBER:` lines at the end of the last answer go to memory and out of the text.
    fn agent_take_remembers(&mut self) {
        let Some(ix) = self.agent.items.iter().rposition(|i| matches!(i, Item::Text(_))) else { return };
        let Item::Text(t) = &self.agent.items[ix] else { return };
        let (kept, facts) = memory::take_remember_lines(t);
        if facts.is_empty() {
            return;
        }
        self.agent.items[ix] = Item::Text(kept);
        let store = memory::Store::default();
        let tags = vec!["fluxcode".to_string(), self.memory_scope()];
        for f in facts {
            if store.add(&f, &tags).is_ok() {
                self.agent.items.push(Item::Saved(f));
            }
        }
    }

    /// Save a fact the user typed (`/remember ...` or the Memory view).
    pub(crate) fn agent_remember(&mut self, text: &str, cx: &mut Context<Self>) {
        let tags = vec!["fluxcode".to_string(), self.memory_scope()];
        match memory::Store::default().add(text, &tags) {
            Ok(_) => {
                self.agent.items.push(Item::Saved(text.trim().to_string()));
                self.agent_refresh_memories();
            }
            Err(e) => self.agent.items.push(Item::Note(e, true)),
        }
        cx.notify();
    }

    fn agent_refresh_memories(&mut self) {
        let scope = self.memory_scope();
        self.agent.memories = memory::Store::default()
            .list()
            .into_iter()
            .filter(|t| t.tags.iter().any(|x| *x == scope) || !t.tags.iter().any(|x| x.starts_with("project:")))
            .collect();
    }

    // ------------------------------------------------------------ OpenCode / Mira sessions

    fn agent_load_session(&mut self, id: String) {
        let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
        self.agent.loading_id = Some(id.clone());
        self.agent.status = Status::Starting;
        self.agent_request(Req::LoadSession, "session/load", json!({ "sessionId": id, "cwd": cwd, "mcpServers": [] }));
    }

    /// Resume a session OpenCode (or Mira) saved: its history replays into the panel
    /// and the next message continues it.
    fn agent_open_remote(&mut self, id: String, cx: &mut Context<Self>) {
        self.agent.save_chat();
        self.agent.items.clear();
        self.agent.chat_id = None;
        self.agent.carry = None;
        self.agent.show_history = false;
        match self.agent.status {
            Status::Ready if self.agent.proc_id.is_some() => {
                if self.agent.can_load {
                    self.agent_load_session(id);
                } else {
                    self.agent.items.push(Item::Note(format!("{} can't reopen saved sessions.", agent_name()), true));
                }
            }
            Status::Starting | Status::Busy => self.agent.items.push(Item::Note("Wait for the current answer, then open the session again.".into(), true)),
            _ => {
                self.agent.pending_load = Some(id);
                self.agent_start(cx);
            }
        }
        cx.notify();
    }

    /// Read the harness's saved sessions for this folder (`<cli> session list --format json`), off the UI thread.
    fn agent_load_remote_list(&mut self, cx: &mut Context<Self>) {
        if self.agent.remote_state == 1 {
            return;
        }
        let (Some(root), Some(mut cmd)) = (self.root.clone(), opencode_cmd()) else {
            self.agent.remote_state = 2;
            self.agent.remote_err = Some(format!("{} isn't installed, so its sessions can't be read.", agent_name()));
            return;
        };
        let harness = agent_name();
        cmd.args(["session", "list", "--format", "json", "-n", "40"]).current_dir(&root).env("NO_COLOR", "1");
        self.agent.remote_state = 1;
        self.agent.remote_err = None;
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let out = cmd.output().map_err(|e| e.to_string())?;
                    agent_ctx::parse_session_list(&String::from_utf8_lossy(&out.stdout), harness)
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.agent.remote_state = 2;
                match res {
                    Ok(list) => this.agent.remote = list,
                    Err(e) => this.agent.remote_err = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------ code blocks

    pub(crate) fn on_agent_insert_code(&mut self, _: &crate::workbench::AgentInsertCode, window: &mut Window, cx: &mut Context<Self>) {
        let Some(code) = PENDING_INSERT.lock().ok().and_then(|mut p| p.take()) else { return };
        match self.active.filter(|ix| self.docs[*ix].image.is_none()) {
            Some(ix) => {
                let name = self.docs[ix].name.clone();
                let lines = code.lines().count();
                self.docs[ix].editor.update(cx, |s, cx| s.replace(code, window, cx));
                let h = self.docs[ix].editor.focus_handle(cx);
                h.focus(window, cx);
                self.status = format!("Inserted {lines} line{} into {name}", if lines == 1 { "" } else { "s" }).into();
            }
            None => {
                cx.write_to_clipboard(ClipboardItem::new_string(code));
                self.status = "No file is open: the code was copied instead".into();
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------ render

    fn render_memory(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let mut list = v_flex().id("ag-memory").flex_1().min_h_0().px_2().gap_1().overflow_y_scroll();
        if a.memories.is_empty() {
            list = list.child(div().px_1().py_2().text_xs().text_color(c(theme::FG_DIM)).child(
                "Nothing saved yet. The agent saves lasting facts here by itself, or type one above. Notes fade when unused and grow stronger when they help.",
            ));
        }
        for (i, t) in a.memories.iter().enumerate() {
            let id = t.id.clone();
            let strength = (t.tau / 2.0).clamp(0.04, 1.0) as f32;
            let scope = if t.tags.iter().any(|x| x.starts_with("project:")) { "this project" } else { "all projects" };
            list = list.child(
                v_flex()
                    .id(("ag-mem", i))
                    .px_2()
                    .py_1p5()
                    .gap_1()
                    .rounded(px(4.))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(div().text_xs().text_color(c(theme::FG)).child(t.content.clone()))
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(div().w(theme::u(48.)).h(px(3.)).rounded_full().bg(c(theme::BORDER)).child(
                                div().h_full().rounded_full().w(relative(strength)).bg(c(theme::ACCENT_TEXT)),
                            ))
                            .child(format!("{:.2} · {} · used {}×", t.tau, scope, t.success))
                            .child(div().flex_1())
                            .child(Button::new(("ag-mem-del", i)).icon(IconName::Delete).ghost().xsmall().tooltip("Forget (moves it to the archive)").on_click(
                                cx.listener(move |this, _, _, cx| {
                                    let _ = memory::Store::default().forget(&id);
                                    this.agent_refresh_memories();
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        }
        v_flex()
            .size_full()
            .child(div().px_3().pb_2().child(Input::new(&a.memory_input).small()))
            .child(list)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(format!("Stored in {} (shared with ant_memory.py)", memory::dir().join("trails.json").display())),
            )
            .into_any_element()
    }

    fn render_remote_sessions(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let name = agent_name();
        let mut v = v_flex().gap_px().child(
            div()
                .px_2()
                .pt_3()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_MUTED))
                .child(format!("{} SESSIONS IN THIS FOLDER", name.to_uppercase())),
        );
        if a.remote_state == 1 {
            v = v.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child(format!("Reading {name}'s sessions…")));
        }
        if let Some(e) = &a.remote_err {
            v = v.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child(e.clone()));
        }
        if a.remote_state == 2 && a.remote.is_empty() && a.remote_err.is_none() {
            v = v.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child(format!("{name} has no saved sessions for this folder.")));
        }
        for (i, s) in a.remote.iter().enumerate() {
            let id = s.id.clone();
            let current = a.session.as_deref() == Some(s.id.as_str());
            v = v.child(
                h_flex()
                    .id(("ag-remote", i))
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(current, |d| d.bg(c(theme::SELECT)))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(IconName::Bot).size(theme::u(14.)).text_color(c(theme::FG_DIM)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().truncate().text_color(c(theme::FG_STRONG)).child(s.title.clone()))
                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(format!("{} · {}", s.harness, ago(s.updated)))),
                    )
                    .when(current, |h| h.child(div().text_xs().text_color(c(theme::GREEN)).child("open")))
                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Resume this session here").build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| this.agent_open_remote(id.clone(), cx))),
            );
        }
        v.into_any_element()
    }

    fn render_context_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let chips = self.agent_chips(cx);
        h_flex().px_3().pb_1().gap_1().flex_wrap().children(chips.into_iter().enumerate().map(|(i, (key, label, icon, on))| {
            h_flex()
                .id(("ag-chip", i))
                .max_w(theme::u(200.))
                .h(theme::u(20.))
                .px_1p5()
                .gap_1()
                .rounded(px(4.))
                .border_1()
                .cursor_pointer()
                .text_xs()
                .border_color(c(if on { theme::BORDER_STRONG } else { theme::BORDER }))
                .bg(if on { c(theme::BG_ELEVATED) } else { gpui_kit::transparent_black() })
                .text_color(c(if on { theme::FG } else { theme::FG_DIM }))
                .hover(|s| s.border_color(c(theme::FG_DIM)))
                .child(icon.size(theme::u(12.)))
                .child(div().truncate().when(!on, |d| d.line_through()).child(label))
                .tooltip(move |w, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(if on { "Sent with your message. Click to leave it out" } else { "Left out. Click to send it" })
                        .build(w, cx)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.agent.off_ctx.remove(&key) {
                        this.agent.off_ctx.insert(key.clone());
                    }
                    cx.notify();
                }))
        }))
    }

    fn render_mention_popup(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, hits, sel) = self.agent.mention.as_ref()?;
        let mut list = v_flex()
            .mx_3()
            .mb_1()
            .py_1()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(gpui_kit::Hsla::from(rgb(theme::palette().swatches()[1])))
            .shadow_md();
        if hits.is_empty() {
            list = list.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child("No file matches"));
        }
        for (row, &ix) in hits.iter().enumerate() {
            let rel = self.rel_path(&self.all_files[ix]);
            let (dir, name) = match rel.rfind(['\\', '/']) {
                Some(i) => (rel[..i].to_string(), rel[i + 1..].to_string()),
                None => (String::new(), rel.clone()),
            };
            let (icon, tint) = crate::files::icon_for(&name);
            list = list.child(
                h_flex()
                    .id(("ag-mention", row))
                    .mx_1()
                    .px_2()
                    .h(theme::u(22.))
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(row == *sel, |d| d.bg(c(theme::SELECT)))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(icon).size(theme::u(13.)).text_color(theme::tint(tint)))
                    .child(div().flex_none().text_xs().text_color(c(theme::FG_STRONG)).child(name))
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(dir))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if let Some(m) = this.agent.mention.as_mut() {
                            m.2 = row;
                        }
                        this.agent_mention_accept(w, cx);
                    })),
            );
        }
        Some(list.child(div().px_3().pt_1().text_xs().text_color(c(theme::FG_DIM)).child("↑↓ choose · Tab or Enter adds · Esc closes")).into_any_element())
    }

    pub(crate) fn render_agent(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let (dot, label) = match &a.status {
            Status::Off => (theme::FG_DIM, "Off".to_string()),
            Status::Starting if a.loading_id.is_some() => (theme::FG_MUTED, "Opening the session…".into()),
            Status::Starting => (theme::FG_MUTED, "Starting…".into()),
            Status::Ready => (theme::GREEN, "Ready".into()),
            Status::Busy => (theme::BLUE, "Working…".into()),
            Status::Error(_) => (theme::DANGER, "Error".into()),
        };
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
        if a.show_memory {
            return v_flex().size_full().child(self.render_memory(cx)).into_any_element();
        }
        if a.show_history {
            return v_flex().size_full().child(header).child(self.render_history(cx)).into_any_element();
        }

        let name = agent_name();
        let mut log = v_flex().id("ag-log").flex_1().min_h_0().px_3().gap_2().overflow_y_scroll().track_scroll(&a.scroll);
        if a.items.is_empty() && !matches!(a.status, Status::Error(_)) {
            let missing = name == "Mira" && !mira_installed();
            let hint = if missing {
                "Mira (omm) isn't installed on this PC yet. Pick OpenCode in the title menu for now.".to_string()
            } else if on {
                "Type @ to add a file. Your open file, selection and last error go along when they help.".to_string()
            } else {
                "Off for now. Your first message turns it on. Type @ to add a file.".to_string()
            };
            log = log.child(
                v_flex()
                    .size_full()
                    .min_h(theme::u(220.))
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_4()
                    .child(Icon::new(IconName::Bot).size(theme::u(36.)).text_color(c(theme::FG_DIM)))
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(format!("Ask {name}")))
                    .child(div().text_xs().text_center().text_color(c(theme::FG_DIM)).child(hint)),
            );
        }
        if let Status::Error(e) = &a.status {
            log = log.child(div().p_2().rounded(px(4.)).border_1().border_color(c(theme::DANGER)).bg(c(theme::BG_ELEVATED)).text_xs().text_color(c(theme::DANGER)).child(e.clone()));
        }
        // The agent's name heads each answer, like a chat: after every user message.
        let speaker = |who: &str, icon: Icon| {
            h_flex()
                .pt_1()
                .gap_1p5()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_MUTED))
                .child(icon.size(theme::u(13.)))
                .child(who.to_string())
        };
        let mut after_user = true;
        for (i, item) in a.items.iter().enumerate() {
            let starts_answer = after_user && !matches!(item, Item::User(_) | Item::Note(..) | Item::Saved(_));
            if starts_answer {
                log = log.child(speaker(name, Icon::new(IconName::Bot)));
                after_user = false;
            }
            log = log.child(match item {
                Item::User(t) => {
                    after_user = true;
                    v_flex()
                        .items_end()
                        .gap_1()
                        .pt_1()
                        .child(speaker("You", Icon::new(IconName::User)))
                        .child(
                            div()
                                .max_w(relative(0.92))
                                .px_2p5()
                                .py_1p5()
                                .rounded(px(8.))
                                .bg(c(theme::BG_ELEVATED))
                                .border_1()
                                .border_color(c(theme::BORDER_STRONG))
                                .text_sm()
                                .text_color(c(theme::FG_STRONG))
                                .child(t.clone()),
                        )
                        .into_any_element()
                }
                Item::Text(t) => TextView::markdown(("ag-md", i), t.clone())
                    .text_sm()
                    .code_block_actions(|block, _, _| {
                        let (copy, insert) = (block.code().to_string(), block.code().to_string());
                        // A solid strip so the buttons never sit on top of the code.
                        h_flex()
                            .gap_px()
                            .p_px()
                            .rounded(px(4.))
                            .bg(c(theme::BG_ELEVATED))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .child(
                                Button::new("cb-copy")
                                    .icon(IconName::Copy)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Copy")
                                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))),
                            )
                            .child(
                                Button::new("cb-insert")
                                    .icon(Icon::new(gpui_kit::assets::IconName::TextCursorInput))
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Insert at the cursor in the open file (replaces the selection)")
                                    .on_click(move |_, w, cx| {
                                        if let Ok(mut p) = PENDING_INSERT.lock() {
                                            *p = Some(insert.clone());
                                        }
                                        w.dispatch_action(Box::new(crate::workbench::AgentInsertCode), cx);
                                    }),
                            )
                    })
                    .into_any_element(),
                Item::Thought(t) => h_flex()
                    .gap_1p5()
                    .items_start()
                    .text_xs()
                    .italic()
                    .text_color(c(theme::FG_DIM))
                    .child(Icon::new(gpui_kit::assets::IconName::Brain).size(theme::u(12.)).flex_none())
                    .child(div().flex_1().min_w_0().line_clamp(2).child(t.chars().rev().take(240).collect::<Vec<_>>().into_iter().rev().collect::<String>()))
                    .into_any_element(),
                Item::Tool { title, status, kind, .. } => {
                    let (state_icon, col): (Icon, theme::Tok) = match status.as_str() {
                        "completed" => (Icon::new(IconName::CircleCheck), theme::GREEN),
                        "failed" => (Icon::new(IconName::CircleX), theme::DANGER),
                        "in_progress" => (Icon::new(IconName::LoaderCircle), theme::BLUE),
                        _ => (Icon::new(IconName::Dash), theme::FG_DIM),
                    };
                    let kind_icon: Icon = match kind.as_str() {
                        "read" => Icon::new(IconName::FileText),
                        "edit" => Icon::new(gpui_kit::assets::IconName::Pencil),
                        "delete" => Icon::new(IconName::Delete),
                        "move" => Icon::new(IconName::ArrowRight),
                        "search" => Icon::new(IconName::Search),
                        "execute" => Icon::new(IconName::SquareTerminal),
                        "fetch" => Icon::new(IconName::Globe),
                        "think" => Icon::new(gpui_kit::assets::IconName::Brain),
                        _ => Icon::new(IconName::Settings2),
                    };
                    h_flex()
                        .gap_2()
                        .px_2()
                        .h(theme::u(24.))
                        .rounded(px(4.))
                        .bg(c(theme::BG_ELEVATED))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .text_xs()
                        .child(kind_icon.size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
                        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(title.clone()))
                        .child(state_icon.size(theme::u(13.)).text_color(c(col)))
                        .into_any_element()
                }
                Item::Plan(entries) => v_flex()
                    .gap_1()
                    .p_2()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .text_xs()
                    .children(entries.iter().map(|(t, st)| {
                        let (icon, col) = match st.as_str() {
                            "completed" => (IconName::CircleCheck, theme::GREEN),
                            "in_progress" => (IconName::LoaderCircle, theme::BLUE),
                            _ => (IconName::Dash, theme::FG_DIM),
                        };
                        h_flex()
                            .gap_1p5()
                            .child(Icon::new(icon).size(theme::u(12.)).text_color(c(col)))
                            .child(div().text_color(c(if st == "completed" { theme::FG_DIM } else { theme::FG })).child(t.clone()))
                    }))
                    .into_any_element(),
                Item::Note(t, err) => h_flex()
                    .items_start()
                    .gap_1p5()
                    .text_xs()
                    .text_color(c(if *err { theme::DANGER } else { theme::FG_DIM }))
                    .child(Icon::new(if *err { IconName::CircleX } else { IconName::Info }).size(theme::u(12.)).flex_none())
                    .child(div().flex_1().min_w_0().child(t.clone()))
                    .into_any_element(),
                Item::Saved(t) => h_flex()
                    .gap_1p5()
                    .items_start()
                    .px_2()
                    .py_1()
                    .rounded(px(4.))
                    .bg(c(theme::BG_ELEVATED))
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(Icon::new(gpui_kit::assets::IconName::Brain).size(theme::u(12.)).text_color(c(theme::ACCENT_TEXT)).flex_none())
                    .child(div().flex_1().min_w_0().child(format!("Saved to memory: {t}")))
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
                            this.agent_reinforce_turn();
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
                            this.agent_reinforce_turn();
                            cx.notify();
                        })))
                        .child(Button::new(("ag-undo", i)).label("Undo").xsmall().on_click(cx.listener(move |this, _, _, cx| this.agent_undo(i, cx))))
                }))
        });

        let busy = a.status == Status::Busy;
        v_flex()
            .size_full()
            .child(header)
            .child(log)
            .when_some(permission, |v, p| v.child(p))
            .when_some(changes, |v, ch| v.child(ch))
            .children(self.render_mention_popup(cx))
            .child(self.render_context_row(cx))
            .child(
                h_flex()
                    .px_3()
                    .pb_3()
                    .gap_1()
                    .child(div().flex_1().key_context("AgentInput").child(Input::new(&a.input).small()))
                    .child(
                        Button::new("ag-send")
                            .icon(IconName::ArrowUp)
                            .small()
                            .when(!busy, |b| b.primary())
                            .disabled(busy)
                            .tooltip("Send (Enter)")
                            .on_click(cx.listener(|this, _, w, cx| this.agent_send(w, cx))),
                    ),
            )
            .into_any_element()
    }

    /// The memories used in this turn helped (the user kept the changes).
    fn agent_reinforce_turn(&mut self) {
        let store = memory::Store::default();
        for id in std::mem::take(&mut self.agent.turn_memories) {
            let _ = store.reinforce(&id);
        }
    }
}

/// Code from an answer's Insert button, handed to `on_agent_insert_code` (the
/// button's closure can't hold the workbench, so it goes through an action).
static PENDING_INSERT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
