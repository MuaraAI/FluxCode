//! OpenCode panel over the Agent Client Protocol (ACP), ported from
//! `src-tauri/src/acp.rs` (process) and `src/agent.js` (protocol + UI).
//! `opencode acp` speaks newline-delimited JSON-RPC on stdio.

use crate::i18n::tr;
use crate::agent_ctx::{self, RemoteSession};
use crate::core;
use crate::memory;
use crate::proc;
use crate::theme::{self, c};
use crate::workbench::{tool_button, SideView, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    menu::DropdownMenu,
    switch::Switch,
    text::TextView,
    v_flex, Icon, IconName, Sizable,
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
    SetOption,
}

/// Stands for the older ACP session modes (`modes`, `session/set_mode`) among the config options.
const MODE_ID: &str = "__mode";

/// One setting the harness offers for the session (ACP `configOptions`):
/// model, effort (thought_level), mode, or anything else it adds.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigOpt {
    pub id: String,
    pub name: String,
    pub category: String,
    pub current: String,
    /// (value, name, description)
    pub choices: Vec<(String, String, String)>,
}

impl ConfigOpt {
    fn current_name(&self) -> &str {
        self.choices.iter().find(|c| c.0 == self.current).map(|c| c.1.as_str()).unwrap_or(&self.current)
    }
}

/// Models grouped for the picker: free ones first, then one group per provider.
/// Names lose the "provider/" prefix (the group says it) and a trailing "Free".
pub fn model_groups(choices: &[(String, String, String)]) -> Vec<(String, Vec<(String, String)>)> {
    let mut free = Vec::new();
    let mut by: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (value, name, _) in choices {
        let provider = value.split_once('/').map(|(p, _)| p).unwrap_or("").to_string();
        let short = name.split_once('/').map(|(_, n)| n).unwrap_or(name).trim().to_string();
        let is_free = value.ends_with("-free") || short.to_lowercase().ends_with(" free");
        if is_free {
            let clean = short.strip_suffix(" Free").or_else(|| short.strip_suffix(" free")).unwrap_or(&short).to_string();
            free.push((value.clone(), clean));
            continue;
        }
        match by.iter_mut().find(|g| g.0 == provider) {
            Some(g) => g.1.push((value.clone(), short)),
            None => by.push((provider, vec![(value.clone(), short)])),
        }
    }
    let mut out = Vec::new();
    if !free.is_empty() {
        out.push(("free".to_string(), free));
    }
    out.extend(by);
    out
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
    /// This many memory notes went with the message.
    MemoryUsed(usize),
    /// The chat continues with another harness from here (a branch, not a new chat).
    Branch(String),
}

/// A message typed while the agent was busy (or not running yet), with its attachments.
pub struct Queued {
    pub text: String,
    pub files: Vec<PathBuf>,
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

/// The last messages of a chat as plain text, for a fresh session to read.
fn transcript(items: &[Item]) -> String {
    let mut lines: Vec<String> = items
        .iter()
        .filter_map(|i| match i {
            Item::User(t) => Some(format!("User: {}", t.chars().take(2000).collect::<String>())),
            Item::Text(t) if !t.trim().is_empty() => Some(format!("Assistant: {}", t.chars().take(2000).collect::<String>())),
            _ => None,
        })
        .collect();
    let n = lines.len();
    lines.drain(..n.saturating_sub(20));
    lines.join("\n\n")
}

/// A file the agent wrote, with what was there before (None = new file).
pub struct Change {
    pub path: PathBuf,
    /// The file before the agent touched it (None: the agent created it).
    pub before: Option<String>,
    /// Lines added and removed so far (counted when an edit finishes).
    pub added: usize,
    pub removed: usize,
}

pub struct AgentState {
    pub status: Status,
    proc_id: Option<u32>,
    buf: String,
    log_tail: Vec<String>,
    next_id: u64,
    pending: HashMap<u64, Req>,
    session: Option<String>,
    /// Messages waiting for the agent: sent together when the current answer ends.
    pub queue: Vec<Queued>,
    /// The user asked to interrupt the answer and send the queue now.
    steer: bool,
    /// Files attached to the message being typed.
    pub attachments: Vec<PathBuf>,
    /// Thinking blocks the user expanded.
    open_thoughts: HashSet<usize>,
    /// The agent accepts image blocks (ACP promptCapabilities.image).
    can_image: bool,
    pub items: Vec<Item>,
    permission: Option<Permission>,
    pub changes: Vec<Change>,
    /// Open editors to reload from disk after the agent wrote them (needs a Window).
    pub reloads: Vec<PathBuf>,
    pub(crate) input: Entity<TextareaState>,
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
    /// Sessions OpenCode / Mura saved for this folder: 0 not loaded, 1 loading, 2 loaded.
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
    pub(crate) memory_input: Entity<InputState>,
    /// Memory trails sent with the current turn (reinforced on Keep, weakened on Undo).
    turn_memories: Vec<String>,
    /// The REMEMBER instruction went out in this session.
    hinted: bool,
    /// Signature of the workspace overview the agent has (0: none yet this session).
    ws_sig: u64,
    pub memories: Vec<memory::Trail>,
    /// `/command` list while a `/` is being typed: (matches, selected row).
    pub slash: Option<(Vec<SlashCmd>, usize)>,
    /// Commands the agent announced for this session.
    pub commands: Vec<SlashCmd>,
    /// Settings the harness offers for this session (model, effort, mode, ...).
    pub config: Vec<ConfigOpt>,
    /// The settings view (model, effort, mode) is showing instead of the chat.
    pub show_options: bool,
    /// Editing an earlier message (index in `items`): Enter restarts from there.
    pub editing: Option<usize>,
    /// The next answer that ends was cancelled on purpose (an edit): no "cancelled" note.
    quiet_stop: bool,
    /// Stamp of the last finished turn; the idle timer only fires if it's unchanged.
    last_active: Option<std::time::Instant>,
    /// Resuming after an idle stop: the replayed history is already on screen.
    silent_load: bool,
    /// The session to resume on the next message after an idle stop.
    resume_id: Option<String>,
    _sub: Subscription,
    _sub2: Subscription,
}

/// MuaraAI's Mura harness (`mura`, one Rust binary, github.com/MuaraAI/Mura-Harness),
/// installed on its own next to FluxCode (its README puts it in ~/.local/bin).
fn find_mura() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "mura.exe" } else { "mura" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    for var in ["USERPROFILE", "HOME"] {
        if let Ok(base) = std::env::var(var) {
            let b = Path::new(&base);
            candidates.push(b.join(".local").join("bin").join(exe));
            candidates.push(b.join(".fluxcode").join("bin").join(exe));
            candidates.push(b.join(".cargo").join("bin").join(exe));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

static MURA_FOUND: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

pub fn mura_installed() -> bool {
    let Ok(mut f) = MURA_FOUND.lock() else { return find_mura().is_some() };
    *f.get_or_insert_with(|| find_mura().is_some())
}

/// The harness chosen in the panel's title menu: 0 auto, 1 OpenCode, 3 Mura Harness.
static CHOICE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_choice(cli: &str) {
    let v = match cli {
        "opencode" => 1,
        "mura" => 3,
        _ => 0,
    };
    CHOICE.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// A `/command`: FluxCode's own, or one the agent announced (ACP available_commands_update).
#[derive(Clone)]
pub struct SlashCmd {
    pub name: String,
    pub desc: String,
    pub builtin: bool,
    /// Needs text after the name (`/remember <fact>`): Enter fills it in instead of sending.
    pub takes_input: bool,
}

fn builtin_commands() -> Vec<SlashCmd> {
    let c = |name: &str, desc: &str, takes_input: bool| SlashCmd { name: name.into(), desc: desc.into(), builtin: true, takes_input };
    vec![
        c("remember", crate::i18n::tr("Save a fact about this project to memory"), true),
        c("new", crate::i18n::tr("Start a new chat"), false),
        c("history", crate::i18n::tr("Saved chats and agent sessions for this folder"), false),
        c("memory", crate::i18n::tr("What the agent remembers"), false),
        c("stop", crate::i18n::tr("Turn the agent off"), false),
    ]
}

/// Which harness the panel talks to. Both speak ACP over stdio (`<cli> acp`).
pub fn agent_name() -> &'static str {
    match CHOICE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => "OpenCode",
        3 => "Mura",
        _ if mura_installed() => "Mura",
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
    match agent_name() {
        "Mura" => return find_mura().map(|e| proc::command(e.to_string_lossy().as_ref())),
        _ => {}
    }
    let exe = find_opencode();
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
        // Grows with the text (1 to 8 lines); Enter sends, Shift+Enter starts a new line.
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .submit_on_enter(true)
                .placeholder(crate::trf!("Ask {name}…"; name => agent_name()))
        });
        let sub = cx.subscribe_in(&input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { shift: false, .. } if this.agent.slash.is_some() => this.agent_slash_accept(window, cx),
            InputEvent::PressEnter { shift: false, .. } if this.agent.mention.is_some() => this.agent_mention_accept(window, cx),
            InputEvent::PressEnter { shift: false, .. } => this.agent_send(window, cx),
            InputEvent::Change => this.agent_update_mention(cx),
            _ => {}
        });
        let memory_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Remember something about this project…")));
        let sub2 = cx.subscribe_in(&memory_input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = this.agent.memory_input.read(cx).value().to_string();
                this.agent.memory_input.update(cx, |s, cx| s.set_value("", window, cx));
                this.agent_remember(&text, cx);
            }
        });
        // Memory evaporates once per start, off the UI thread (same rule as `ant_memory.py decay`).
        if !crate::settings::Settings::load().memory_off {
            std::thread::spawn(|| {
                let _ = memory::Store::default().decay();
            });
        }
        Self {
            status: Status::Off,
            proc_id: None,
            buf: String::new(),
            log_tail: Vec::new(),
            next_id: 0,
            pending: HashMap::new(),
            session: None,
            queue: Vec::new(),
            steer: false,
            attachments: Vec::new(),
            open_thoughts: HashSet::new(),
            can_image: false,
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
            ws_sig: 0,
            memories: Vec::new(),
            slash: None,
            commands: Vec::new(),
            config: Vec::new(),
            show_options: false,
            editing: None,
            quiet_stop: false,
            last_active: None,
            silent_load: false,
            resume_id: None,
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

    /// The "model" select from ACP `configOptions` (session/new, session/load,
    /// set_config_option results and config_option_update notifications).
    fn agent_take_config(&mut self, v: &Value) {
        let Some(opts) = v.get("configOptions").and_then(Value::as_array) else { return };
        let str_of = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        self.agent.config = opts
            .iter()
            .filter(|o| o.get("type").and_then(Value::as_str).unwrap_or("select") == "select")
            .map(|o| ConfigOpt {
                id: str_of(o, "id"),
                name: str_of(o, "name"),
                category: str_of(o, "category"),
                current: str_of(o, "currentValue"),
                choices: o
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|c| (str_of(c, "value"), str_of(c, "name"), str_of(c, "description"))).filter(|c| !c.0.is_empty()).collect())
                    .unwrap_or_default(),
            })
            .filter(|o| !o.id.is_empty() && !o.choices.is_empty())
            .map(|mut o| {
                // Harnesses that leave out `category` (Mura) still name their options plainly.
                if o.category.is_empty() {
                    o.category = match o.id.as_str() {
                        "model" => "model",
                        "effort" | "reasoning" | "reasoning_effort" | "thought_level" => "thought_level",
                        "mode" | "agent" => "mode",
                        _ => "",
                    }
                    .into();
                }
                o
            })
            .collect();
        // The older ACP way to offer modes: `modes.availableModes`, switched with session/set_mode.
        if let Some(m) = v.get("modes").filter(|m| m.is_object()) {
            if !self.agent.config.iter().any(|o| o.category == "mode") {
                let choices: Vec<(String, String, String)> = m
                    .get("availableModes")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|c| (str_of(c, "id"), str_of(c, "name"), str_of(c, "description"))).filter(|c| !c.0.is_empty()).collect())
                    .unwrap_or_default();
                if !choices.is_empty() {
                    self.agent.config.push(ConfigOpt { id: MODE_ID.into(), name: "Mode".into(), category: "mode".into(), current: str_of(m, "currentModeId"), choices });
                }
            }
        }
    }

    /// Change one session setting (model, effort, mode, ...). `remember` keeps it
    /// for new sessions of this harness.
    pub(crate) fn agent_set_option(&mut self, id: &str, value: String, remember: bool, cx: &mut Context<Self>) {
        if remember {
            let key = format!("{}:{id}", self.settings.agent_cli);
            if self.settings.agent_options.get(&key) != Some(&value) {
                self.settings.agent_options.insert(key, value.clone());
                self.settings.save();
            }
        }
        if let Some(o) = self.agent.config.iter_mut().find(|o| o.id == id) {
            o.current = value.clone();
        }
        if let Some(session) = self.agent.session.clone() {
            if id == MODE_ID {
                self.agent_request(Req::SetOption, "session/set_mode", json!({ "sessionId": session, "modeId": value }));
            } else {
                self.agent_request(Req::SetOption, "session/set_config_option", json!({ "sessionId": session, "configId": id, "value": value }));
            }
        }
        cx.notify();
    }

    pub(crate) fn agent_set_access(&mut self, level: &str, cx: &mut Context<Self>) {
        self.settings.agent_access = level.into();
        self.settings.save();
        cx.notify();
    }

    pub(crate) fn on_agent_show_options(&mut self, _: &crate::workbench::AgentShowOptions, _: &mut Window, cx: &mut Context<Self>) {
        self.agent_open_options(cx);
    }

    fn agent_open_options(&mut self, cx: &mut Context<Self>) {
        self.agent.show_options = true;
        self.agent.show_memory = false;
        self.agent.show_history = false;
        if self.settings.agent_right {
            self.agent_open = true;
        } else {
            self.view = crate::workbench::SideView::Agent;
            self.sidebar_open = true;
        }
        cx.notify();
    }

    /// Start editing an earlier message: its text goes back into the input.
    fn agent_edit(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Item::User(t)) = self.agent.items.get(ix) else { return };
        let t = t.clone();
        self.agent.editing = Some(ix);
        self.agent.input.update(cx, |s, cx| {
            let end = t.len();
            s.set_value(t, window, cx);
            s.set_selected_range(end..end, cx);
        });
        let h = self.agent.input.focus_handle(cx);
        h.focus(window, cx);
        cx.notify();
    }

    fn agent_cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.agent.editing.take().is_some() {
            self.agent.input.update(cx, |s, cx| s.set_value("", window, cx));
            cx.notify();
        }
    }

    /// Send an edited message: the chat goes back to just before it, and a fresh
    /// session gets everything said up to there as context, then the new text.
    fn agent_send_edit(&mut self, ix: usize, text: String, cx: &mut Context<Self>) {
        let files = std::mem::take(&mut self.agent.attachments);
        self.agent.items.truncate(ix);
        self.agent.items.push(Item::Note(crate::i18n::tr("Edited: the answer starts over from this message.").into(), false));
        self.agent.carry = self.agent.items.iter().any(|i| matches!(i, Item::User(_) | Item::Text(_))).then(|| transcript(&self.agent.items));
        self.agent.queue = vec![Queued { text, files }];
        self.agent.changes.clear();
        self.agent.permission = None;
        self.agent.resume_id = None;
        self.agent.hinted = false;
        self.agent.ws_sig = 0;
        match self.agent.status {
            Status::Busy | Status::Ready if self.agent.proc_id.is_some() => {
                if self.agent.status == Status::Busy {
                    self.agent.quiet_stop = true;
                    if let Some(s) = self.agent.session.clone() {
                        self.agent_write(json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": s } }));
                    }
                }
                self.agent.session = None;
                self.agent.status = Status::Starting;
                let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
                self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": [] }));
            }
            Status::Starting => {}
            _ => self.agent_start(cx),
        }
        self.agent.scroll.scroll_to_bottom();
        cx.notify();
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
            self.agent.status = Status::Error(if agent_name() == "Mura" {
                crate::i18n::tr("Mura isn't installed on this PC (FluxCode looks on PATH and in ~/.local/bin). Build it from github.com/MuaraAI/Mura-Harness, or pick OpenCode in the panel title menu.").into()
            } else {
                "OpenCode isn't installed. Install it with  npm i -g opencode-ai , or pick Mura in the panel title menu.".into()
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
        self.agent.ws_sig = 0;
        self.agent.loading_id = None;
        cx.notify();
    }

    fn agent_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.agent.input.read(cx).value().trim().to_string();
        if text.is_empty() && self.agent.attachments.is_empty() {
            return;
        }
        self.agent.input.update(cx, |s, cx| s.set_value("", window, cx));
        self.agent.mention = None;
        self.agent.slash = None;
        if let Some(fact) = text.strip_prefix("/remember ").or_else(|| text.strip_prefix("/ingat ")) {
            self.agent_remember(fact, cx);
            return;
        }
        match text.as_str() {
            "/new" => return self.agent_new_chat(cx),
            "/history" => {
                self.agent.show_history = true;
                self.agent.show_memory = false;
                self.agent_load_remote_list(cx);
                return cx.notify();
            }
            "/memory" => {
                self.agent.show_memory = true;
                self.agent.show_history = false;
                self.agent_refresh_memories();
                return cx.notify();
            }
            "/stop" => return self.agent_set_on(false, cx),
            _ => {}
        }
        if let Some(ix) = self.agent.editing.take() {
            return self.agent_send_edit(ix, text, cx);
        }
        let files = std::mem::take(&mut self.agent.attachments);
        self.agent.queue.push(Queued { text, files });
        self.agent.scroll.scroll_to_bottom();
        match (&self.agent.status, &self.agent.session) {
            (Status::Ready, Some(_)) => self.agent_flush_queue(cx),
            // Still answering: the message waits in the queue (Send now interrupts).
            (Status::Busy, _) | (Status::Starting, _) => {}
            _ => {
                if let Some(id) = self.agent.resume_id.take() {
                    // Stopped while idle: continue the same session without replaying it.
                    self.agent.pending_load = Some(id);
                    self.agent.silent_load = true;
                }
                self.agent_start(cx);
            }
        }
        cx.notify();
    }

    /// Send everything queued as one message (the agent sees it in order, in the same
    /// session, so it keeps the context of what came before).
    fn agent_flush_queue(&mut self, cx: &mut Context<Self>) {
        if self.agent.queue.is_empty() || self.agent.session.is_none() {
            return;
        }
        let queued = std::mem::take(&mut self.agent.queue);
        let mut text = queued.iter().map(|q| q.text.as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n");
        let files: Vec<PathBuf> = queued.into_iter().flat_map(|q| q.files).collect();
        let shown = text.clone();
        if std::mem::take(&mut self.agent.steer) {
            text = format!("(I stopped your previous answer to send this. Take it into account and continue.)\n\n{text}");
        }
        self.agent.items.push(Item::User(if shown.is_empty() { "(attachments)".into() } else { shown }));
        if !files.is_empty() {
            let names: Vec<String> = files.iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
            self.agent.items.push(Item::Note(crate::trf!("Attached: {0}"; 0 => names.join(", ")), false));
        }
        self.agent_prompt(text, files, cx);
    }

    /// Interrupt the current answer and send the queue now (it keeps the context).
    fn agent_steer(&mut self, cx: &mut Context<Self>) {
        if self.agent.status == Status::Busy {
            self.agent.steer = true;
            self.agent_cancel(cx);
        } else {
            self.agent_flush_queue(cx);
        }
        cx.notify();
    }

    fn agent_prompt(&mut self, text: String, files: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(session) = self.agent.session.clone() else { return };
        let mut prompt = Vec::new();
        if let Some(earlier) = self.agent.carry.take() {
            prompt.push(json!({ "type": "text", "text": format!("Earlier messages of this conversation, for context:\n\n{earlier}\n\n---\n") }));
        }
        prompt.push(json!({ "type": "text", "text": text.clone() }));
        prompt.extend(self.agent_attachment_blocks(&files));
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
                    let low = m.to_lowercase();
                    let model_problem = ["credential", "api key", "apikey", "unauthorized", "401", "403", "quota", "rate limit", "model"]
                        .iter()
                        .any(|k| low.contains(k));
                    self.agent.items.push(Item::Note(m, true));
                    let models = self.agent.config.iter().find(|o| o.category == "model").map_or(0, |o| o.choices.len());
                    if model_problem && models > 1 {
                        self.agent.items.push(Item::Note(crate::i18n::tr("This looks like a problem with the model. Pick another one with the model button under the message box, then send again.").into(), false));
                    }
                    self.agent.status = Status::Ready;
                }
                Req::SetOption => self.agent.items.push(Item::Note(crate::trf!("Couldn't change the setting: {m}"; m => m), true)),
                Req::LoadSession if self.agent.silent_load => {
                    self.agent.silent_load = false;
                    self.agent.loading_id = None;
                    self.agent.carry = Some(transcript(&self.agent.items));
                    let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
                    self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": [] }));
                }
                _ => self.agent.status = Status::Error(m),
            }
            return;
        }
        let result = msg.get("result").cloned().unwrap_or(Value::Null);
        match kind {
            Req::Initialize => {
                self.agent.can_load = result.pointer("/agentCapabilities/loadSession").and_then(Value::as_bool).unwrap_or(false);
                self.agent.can_image = result.pointer("/agentCapabilities/promptCapabilities/image").and_then(Value::as_bool).unwrap_or(false);
                let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
                match self.agent.pending_load.take() {
                    Some(id) if self.agent.can_load => self.agent_load_session(id),
                    _ => {
                        if std::mem::take(&mut self.agent.silent_load) {
                            self.agent.carry = Some(transcript(&self.agent.items));
                        }
                        self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": [] }))
                    }
                }
            }
            Req::LoadSession => {
                self.agent_take_config(&result);
                self.agent.session = self.agent.loading_id.take();
                self.agent.status = Status::Ready;
                self.agent.silent_load = false;
                self.agent_schedule_idle(cx);
                self.agent.hinted = false;
                self.agent.ws_sig = 0;
                self.agent.save_chat();
                self.agent_flush_queue(cx);
            }
            Req::NewSession => {
                self.agent.session = result.get("sessionId").and_then(Value::as_str).map(String::from);
                self.agent_take_config(&result);
                // What was picked last time (model, effort, mode), when the harness still offers it.
                let harness = self.settings.agent_cli.clone();
                let wanted: Vec<(String, String)> = self
                    .agent
                    .config
                    .iter()
                    .filter_map(|o| {
                        let v = self.settings.agent_options.get(&format!("{harness}:{}", o.id))?;
                        (o.current != *v && o.choices.iter().any(|c| c.0 == *v)).then(|| (o.id.clone(), v.clone()))
                    })
                    .collect();
                for (id, v) in wanted {
                    self.agent_set_option(&id, v, false, cx);
                }
                self.agent.status = Status::Ready;
                self.agent_schedule_idle(cx);
                self.agent_flush_queue(cx);
            }
            Req::SetOption => self.agent_take_config(&result),
            Req::Prompt => {
                let stop = result.get("stopReason").and_then(Value::as_str).unwrap_or("end_turn");
                let quiet = std::mem::take(&mut self.agent.quiet_stop);
                if stop != "end_turn" && !quiet {
                    self.agent.items.push(Item::Note(stop.replace('_', " "), false));
                }
                self.agent.permission = None;
                self.agent.status = Status::Ready;
                self.agent_take_remembers();
                self.agent.save_chat();
                self.agent_schedule_idle(cx);
                self.refresh_tree(cx);
                self.agent_flush_queue(cx);
            }
        }
    }

    fn agent_on_notification(&mut self, method: &str, params: Value) {
        if method != "session/update" {
            return;
        }
        let Some(u) = params.get("update") else { return };
        if u.get("sessionUpdate").and_then(Value::as_str) == Some("config_option_update") {
            self.agent_take_config(u);
            return;
        }
        if u.get("sessionUpdate").and_then(Value::as_str) == Some("available_commands_update") {
            self.agent.commands = u
                .get("availableCommands")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|c| {
                            Some(SlashCmd {
                                name: c.get("name")?.as_str()?.to_string(),
                                desc: c.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                                builtin: false,
                                takes_input: c.get("input").is_some_and(|i| !i.is_null()),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            return;
        }
        if self.agent.silent_load {
            return;
        }
        let text = |u: &Value| u.pointer("/content/text").and_then(Value::as_str).unwrap_or("").to_string();
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        match u.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
            "agent_message_chunk" => self.agent.append(false, &text(u)),
            // Only while an old session replays; live prompts are shown when sent.
            "user_message_chunk" if self.agent.loading_id.is_some() => self.agent.append_user(&text(u)),
            "agent_thought_chunk" => self.agent.append(true, &text(u)),
            "tool_call" => {
                let kind = s(u, "kind").unwrap_or_else(|| "other".into());
                let status = s(u, "status").unwrap_or_else(|| "pending".into());
                self.agent_track_edit(u, &kind, status == "completed");
                self.agent.items.push(Item::Tool {
                    id: s(u, "toolCallId").unwrap_or_default(),
                    title: s(u, "title").unwrap_or_else(|| "Tool".into()),
                    status,
                    kind,
                });
            }
            "tool_call_update" => {
                let id = s(u, "toolCallId").unwrap_or_default();
                let known = self.agent.items.iter().rev().find_map(|i| match i {
                    Item::Tool { id: tid, kind, .. } if *tid == id => Some(kind.clone()),
                    _ => None,
                });
                let kind = s(u, "kind").or(known).unwrap_or_default();
                self.agent_track_edit(u, &kind, s(u, "status").as_deref() == Some("completed"));
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
                // The access level answers what it can; the rest asks.
                let kind = params.pointer("/toolCall/kind").and_then(Value::as_str).unwrap_or("other");
                let reads = matches!(kind, "read" | "search" | "fetch" | "think");
                let writes = matches!(kind, "edit" | "delete" | "move");
                let auto = match self.settings.access() {
                    "full" => Some(true),
                    "write" if reads || writes => Some(true),
                    "read" if reads => Some(true),
                    "read" => Some(false),
                    _ => None,
                };
                if let Some(allow) = auto {
                    let want = if allow { "allow" } else { "reject" };
                    if let Some((oid, ..)) = options.iter().find(|o| o.2.starts_with(want)) {
                        let oid = oid.clone();
                        if !allow {
                            self.agent.items.push(Item::Note(crate::trf!("Refused (read only): {0}"; 0 => title), false));
                            cx.notify();
                        }
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
            ("fs/write_text_file", Some(_)) if self.settings.access() == "read" => {
                respond(self, Err(crate::i18n::tr("FluxCode is in read-only mode: files can't be changed. Describe the change instead.").into()));
            }
            ("fs/write_text_file", Some(p)) => {
                let content = params.get("content").and_then(Value::as_str).unwrap_or("").to_string();
                let before = std::fs::read_to_string(&p).ok();
                let crlf = before.as_deref().is_some_and(|b| b.contains("\r\n"));
                let r = core::write_text(&p, &content, crlf);
                if r.is_ok() {
                    if !self.agent.changes.iter().any(|ch| ch.path == p) {
                        self.agent.changes.push(Change { path: p.clone(), before, added: 0, removed: 0 });
                    }
                    if let Some(ch) = self.agent.changes.iter_mut().find(|ch| ch.path == p) {
                        (ch.added, ch.removed) = crate::diff::counts(ch.before.as_deref().unwrap_or(""), &content);
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
                    .child(tool_button("ag-dock-close", IconName::Close, crate::i18n::tr("Hide (Ctrl+L)"), cx, |this, _, cx| {
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
        let old = agent_name();
        self.agent.save_chat();
        self.agent_stop(cx);
        self.settings.agent_cli = cli.into();
        self.settings.save();
        set_choice(cli);
        let name = agent_name();
        // Same chat, new harness: it gets the conversation so far with its first message.
        if self.agent.items.iter().any(|i| matches!(i, Item::User(_) | Item::Text(_))) {
            self.agent.carry = Some(transcript(&self.agent.items));
            self.agent.items.push(Item::Branch(crate::trf!("Continued with {name}. It gets the conversation with {old} above as context."; name => name, old => old)));
        }
        self.agent.resume_id = None;
        self.agent.config.clear();
        self.agent.input.update(cx, |s, cx| s.set_placeholder(crate::trf!("Ask {name}…"; name => name), window, cx));
        if !self.agent.queue.is_empty() {
            self.agent_start(cx);
        }
        cx.notify();
    }

    pub(crate) fn on_agent_use_opencode(&mut self, _: &crate::workbench::AgentUseOpenCode, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_use("opencode", w, cx);
    }

    pub(crate) fn on_agent_use_mura(&mut self, _: &crate::workbench::AgentUseMura, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_use("mura", w, cx);
    }

    /// The panel title: the harness name, which opens a menu to pick Mura or OpenCode.
    pub(crate) fn render_agent_picker(&self) -> impl IntoElement {
        let name = agent_name();
        Button::new("ag-pick")
            .label(name.to_uppercase())
            .ghost()
            .xsmall()
            .dropdown_caret(true)
            .tooltip(tr("Choose the AI agent"))
            .dropdown_menu(move |m: gpui_kit::component::menu::PopupMenu, _, _| {
                m.min_w(px(220.))
                    .menu_with_check(if mura_installed() { "Mura" } else { crate::i18n::tr("Mura, not installed") }, name == "Mura", Box::new(crate::workbench::AgentUseMura))
                    .menu_with_check("OpenCode", name == "OpenCode", Box::new(crate::workbench::AgentUseOpenCode))
                    .separator()
                    .menu(crate::trf!("{name} settings: model, effort, mode…"; name => name), Box::new(crate::workbench::AgentShowOptions))
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
                    .tooltip(if self.agent.show_memory { crate::i18n::tr("Back to the chat") } else { crate::i18n::tr("Memory: what the agent remembers") })
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
                    .tooltip(if self.agent.show_history { crate::i18n::tr("Back to the chat") } else { crate::i18n::tr("Saved chats") })
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
                    .tooltip(if right { crate::i18n::tr("Move to the left side bar") } else { crate::i18n::tr("Move to the right side") })
                    .on_click(cx.listener(|this, _, w, cx| this.agent_move_dock(w, cx))),
            )
    }

    fn render_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let mut list = v_flex().id("ag-history").flex_1().min_h_0().px_2().gap_px().overflow_y_scroll();
        if a.history.is_empty() {
            list = list.child(div().px_1().py_2().text_sm().text_color(c(theme::FG_DIM)).child(tr("No saved chats yet. Chats are saved after each answer.")));
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
                            .tooltip(tr("Delete this chat"))
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
                        .label(tr("New chat"))
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

    /// Follow an edit tool: keep the file as it was before (from the `diff` content
    /// the agent sends, else read before the edit runs) and, once it finishes,
    /// count the lines it added and removed. Feeds the Changes card.
    fn agent_track_edit(&mut self, u: &Value, kind: &str, done: bool) {
        let has_diff = u.get("content").and_then(Value::as_array).is_some_and(|c| c.iter().any(|x| x.get("type").and_then(Value::as_str) == Some("diff")));
        if !matches!(kind, "edit" | "delete" | "move") && !has_diff {
            return;
        }
        let abs = |p: &str| -> PathBuf {
            let p = PathBuf::from(p);
            if p.is_absolute() { p } else { self.root.clone().unwrap_or_default().join(p) }
        };
        let mut touched: Vec<PathBuf> = Vec::new();
        for c in u.get("content").and_then(Value::as_array).into_iter().flatten() {
            if c.get("type").and_then(Value::as_str) != Some("diff") {
                continue;
            }
            let Some(p) = c.get("path").and_then(Value::as_str).map(abs) else { continue };
            if !self.agent.changes.iter().any(|ch| ch.path == p) {
                let before = c.get("oldText").and_then(Value::as_str).map(String::from);
                self.agent.changes.push(Change { path: p.clone(), before, added: 0, removed: 0 });
            }
            touched.push(p);
        }
        for l in u.get("locations").and_then(Value::as_array).into_iter().flatten() {
            let Some(p) = l.get("path").and_then(Value::as_str).map(abs) else { continue };
            if !self.agent.changes.iter().any(|ch| ch.path == p) {
                if done {
                    continue; // too late to know how it looked before
                }
                let before = std::fs::read_to_string(&p).ok();
                self.agent.changes.push(Change { path: p.clone(), before, added: 0, removed: 0 });
            }
            touched.push(p);
        }
        if !done {
            return;
        }
        for p in touched {
            let after = std::fs::read_to_string(&p).unwrap_or_default();
            if let Some(ix) = self.agent.changes.iter().position(|ch| ch.path == p) {
                let ch = &mut self.agent.changes[ix];
                (ch.added, ch.removed) = crate::diff::counts(ch.before.as_deref().unwrap_or(""), &after);
                // Looked at but not changed: not a change.
                if ch.added == 0 && ch.removed == 0 && ch.before.is_some() && p.exists() {
                    self.agent.changes.remove(ix);
                }
            }
            if self.docs.iter().any(|d| d.path == p) && !self.agent.reloads.contains(&p) {
                self.agent.reloads.push(p);
            }
        }
    }

    /// Open what the agent changed in a file as a unified diff, in a tab.
    fn agent_show_diff(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ch) = self.agent.changes.get(ix) else { return };
        let after = std::fs::read_to_string(&ch.path).unwrap_or_default();
        let rel = self.rel_display(&ch.path.to_string_lossy()).replace('\\', "/");
        let text = crate::diff::unified(&rel, ch.before.as_deref().unwrap_or(""), &after);
        let name = ch.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "changes".into());
        let dir = std::env::temp_dir().join("fluxcode-diff");
        let file = dir.join(format!("{name}.diff"));
        if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&file, text).is_ok() {
            if let Some(i) = self.docs.iter().position(|d| d.path == file) {
                self.close(i, window, cx);
            }
            self.open_file(file, window, cx);
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
            Ok(()) => crate::trf!("Reverted {0}"; 0 => ch.path.display()).into(),
            Err(e) => crate::trf!("Undo failed: {e}"; e => e).into(),
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
        // `/` at the start: the command list (FluxCode's and the agent's).
        let head = &text[..cursor.min(text.len())];
        if head.starts_with('/') && !head.contains(char::is_whitespace) {
            let q = head[1..].to_lowercase();
            let mut list: Vec<SlashCmd> = builtin_commands()
                .into_iter()
                .chain(self.agent.commands.iter().cloned())
                .filter(|c| c.name.to_lowercase().contains(&q))
                .collect();
            list.sort_by_key(|c| (!c.name.to_lowercase().starts_with(&q), !c.builtin, c.name.len()));
            list.truncate(12);
            self.agent.slash = Some((list, 0));
            self.agent.mention = None;
            cx.notify();
            return;
        }
        if self.agent.slash.take().is_some() {
            cx.notify();
        }
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
        let caret = at + rel.len() + 2;
        self.agent.input.update(cx, |s, cx| {
            s.set_value(new, window, cx);
            s.set_selected_range(caret..caret, cx);
        });
        self.agent.mention = None;
        cx.notify();
    }

    pub(crate) fn on_agent_mention_up(&mut self, _: &crate::workbench::AgentMentionUp, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, sel)) = self.agent.slash.as_mut() {
            *sel = sel.saturating_sub(1);
            return cx.notify();
        }
        if let Some((_, _, sel)) = self.agent.mention.as_mut() {
            *sel = sel.saturating_sub(1);
            return cx.notify();
        }
        // No list open: Up moves the caret between lines as usual.
        window.dispatch_action(Box::new(gpui_kit::component::input::MoveUp), cx);
    }

    pub(crate) fn on_agent_mention_down(&mut self, _: &crate::workbench::AgentMentionDown, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((list, sel)) = self.agent.slash.as_mut() {
            if *sel + 1 < list.len() {
                *sel += 1;
            }
            return cx.notify();
        }
        if let Some((_, hits, sel)) = self.agent.mention.as_mut() {
            if *sel + 1 < hits.len() {
                *sel += 1;
            }
            return cx.notify();
        }
        window.dispatch_action(Box::new(gpui_kit::component::input::MoveDown), cx);
    }

    pub(crate) fn on_agent_mention_accept(&mut self, _: &crate::workbench::AgentMentionAccept, w: &mut Window, cx: &mut Context<Self>) {
        if self.agent.slash.is_some() {
            // Tab only completes the name; Enter also runs a command that needs nothing more.
            return self.agent_slash_fill(w, cx).map_or((), |_| ());
        }
        if self.agent.mention.is_some() {
            self.agent_mention_accept(w, cx);
        }
    }

    pub(crate) fn on_agent_mention_close(&mut self, _: &crate::workbench::AgentMentionClose, _w: &mut Window, cx: &mut Context<Self>) {
        if self.agent.mention.take().is_some() || self.agent.slash.take().is_some() {
            cx.notify();
        } else if self.agent.editing.is_some() {
            self.agent_cancel_edit(_w, cx);
        }
    }

    /// Put the highlighted command into the input as `/name `; returns it.
    fn agent_slash_fill(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<SlashCmd> {
        let (list, sel) = self.agent.slash.take()?;
        let cmd = list.get(sel)?.clone();
        let text = format!("/{} ", cmd.name);
        let caret = text.len();
        self.agent.input.update(cx, |s, cx| {
            s.set_value(text, window, cx);
            s.set_selected_range(caret..caret, cx);
        });
        self.agent.slash = None;
        cx.notify();
        Some(cmd)
    }

    /// Enter on the command list: fill it in, and run it right away when it needs no text.
    fn agent_slash_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cmd) = self.agent_slash_fill(window, cx) {
            if !cmd.takes_input {
                self.agent_send(window, cx);
            }
        }
    }

    /// Stop the agent after `agent_idle_min` quiet minutes; the next message resumes the session.
    fn agent_schedule_idle(&mut self, cx: &mut Context<Self>) {
        let mins = self.settings.agent_idle_min;
        if mins == 0 {
            return;
        }
        let stamp = std::time::Instant::now();
        self.agent.last_active = Some(stamp);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(mins as u64 * 60)).await;
            _ = this.update(cx, |this, cx| {
                if this.agent.last_active == Some(stamp) && this.agent.status == Status::Ready && this.agent.permission.is_none() {
                    this.agent_idle_stop(mins, cx);
                }
            });
        })
        .detach();
    }

    fn agent_idle_stop(&mut self, mins: u32, cx: &mut Context<Self>) {
        let session = self.agent.session.clone();
        let can_load = self.agent.can_load;
        self.agent.save_chat();
        self.agent_stop(cx);
        if can_load {
            self.agent.resume_id = session;
        } else {
            self.agent.carry = Some(transcript(&self.agent.items));
        }
        let name = agent_name();
        self.agent.items.push(Item::Note(
            format!("{name} turned off after {mins} quiet minutes to free memory. Your next message continues this chat."),
            false,
        ));
        cx.notify();
    }

    fn rel_path(&self, full: &str) -> String {
        self.rel_display(full)
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
                let label = if a == b { crate::trf!("Selection · line {a}"; a => a) } else { crate::trf!("Selection · lines {a}-{b}"; a => a, b => b) };
                chips.push(("sel".into(), label, Icon::new(gpui_kit::assets::IconName::TextCursorInput), !off("sel")));
            }
        }
        if agent_ctx::wants_error(&text) {
            if let Some(p) = self.problems.first() {
                let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let kind = p.message.split(':').next().unwrap_or("Error").to_string();
                chips.push(("err".into(), format!("{kind} · {name}:{}", p.line), Icon::new(IconName::CircleX), !off("err")));
            } else if self.agent_last_failure().is_some() {
                chips.push(("err".into(), crate::i18n::tr("Last failed command").into(), Icon::new(IconName::CircleX), !off("err")));
            }
        }
        if agent_ctx::wants_output(&text) && !self.term_lines.is_empty() {
            chips.push(("out".into(), crate::i18n::tr("Terminal output").into(), Icon::new(IconName::SquareTerminal), !off("out")));
        }
        if self.root.is_some() && self.agent_ws_signature(cx) != self.agent.ws_sig {
            chips.insert(0, ("ws".into(), crate::i18n::tr("Workspace").into(), Icon::new(IconName::FolderOpen), !off("ws")));
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

    /// Folders and open tabs for the workspace overview (the active tab first, with its line).
    fn agent_ws_parts(&self, cx: &App) -> (Vec<PathBuf>, Vec<String>) {
        let mut tabs = Vec::new();
        if let Some(d) = self.active.map(|ix| &self.docs[ix]) {
            let line = d.editor.read(cx).cursor_position().line + 1;
            tabs.push(format!("{} (active, line {line})", self.rel_path(&d.path.to_string_lossy())));
        }
        for (i, d) in self.docs.iter().enumerate() {
            if Some(i) != self.active {
                tabs.push(self.rel_path(&d.path.to_string_lossy()));
            }
        }
        (self.roots(), tabs)
    }

    fn agent_ws_signature(&self, cx: &App) -> u64 {
        let (roots, tabs) = self.agent_ws_parts(cx);
        crate::ws_context::signature(&crate::ws_context::Inputs {
            roots: &roots,
            files: &self.all_files,
            git: self.scm.repo.as_ref().map(|_| &self.scm.status),
            tabs,
            python: None,
        })
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
                "ws" => {
                    let (roots, tabs) = self.agent_ws_parts(cx);
                    let inputs = crate::ws_context::Inputs {
                        roots: &roots,
                        files: &self.all_files,
                        git: self.scm.repo.as_ref().map(|_| &self.scm.status),
                        tabs,
                        python: self.python_version.as_deref(),
                    };
                    self.agent.ws_sig = crate::ws_context::signature(&inputs);
                    blocks.push(json!({ "type": "text", "text": crate::ws_context::overview(&inputs) }));
                }
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
        let trails = if self.settings.memory_off {
            Vec::new()
        } else {
            memory::Store::default().recall(&format!("{text} {file}"), 4, Some(&self.memory_scope()))
        };
        self.agent.turn_memories = trails.iter().map(|t| t.id.clone()).collect();
        if !trails.is_empty() {
            let list: Vec<String> = trails.iter().map(|t| format!("- {}", t.content)).collect();
            blocks.push(json!({ "type": "text", "text": format!(
                "Notes from earlier sessions (FluxCode memory, may be outdated; ignore them if they don't fit):\n{}",
                list.join("\n")
            ) }));
            self.agent.items.push(Item::MemoryUsed(trails.len()));
        }
        if self.settings.access() == "read" {
            blocks.push(json!({ "type": "text", "text":
                "FluxCode is in read-only mode for this message: read files and explain, but do not edit files or run commands \
                 that change anything. If a change is needed, describe it or show it as a code block." }));
        }
        if !self.agent.hinted {
            self.agent.hinted = true;
            // FluxCode keeps the memory natively; the agent's own memory skill would start
            // a Python process for the same store, so it is asked not to.
            let text = if self.settings.memory_off {
                "FluxCode note: answer in the language the user writes in. Memory is switched off for now: don't run a \
                 memory skill or ant_memory.py."
            } else {
                "FluxCode note: answer in the language the user writes in. FluxCode keeps the project memory itself (relevant \
                 notes come with the message), so don't run a memory skill or ant_memory.py. When you learn a lasting fact about \
                 this project (how to run, test or deploy it, a convention, a decision), end your reply with one line \
                 `REMEMBER: <fact>`; FluxCode saves it and hides the line."
            };
            blocks.push(json!({ "type": "text", "text": text }));
        }
        blocks
    }

    pub(crate) fn memory_scope(&self) -> String {
        let name = self.root.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        format!("project:{name}")
    }

    /// `REMEMBER:` lines at the end of the last answer go to memory and out of the text.
    fn agent_take_remembers(&mut self) {
        if self.settings.memory_off {
            return;
        }
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

    // ------------------------------------------------------------ OpenCode / Mura sessions

    fn agent_load_session(&mut self, id: String) {
        let cwd = self.root.clone().unwrap_or_else(|| self.term_cwd.clone());
        self.agent.loading_id = Some(id.clone());
        self.agent.status = Status::Starting;
        self.agent_request(Req::LoadSession, "session/load", json!({ "sessionId": id, "cwd": cwd, "mcpServers": [] }));
    }

    /// Resume a session OpenCode or Mura Harness saved: its history replays into the panel
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
            Status::Starting | Status::Busy => self.agent.items.push(Item::Note(crate::i18n::tr("Wait for the current answer, then open the session again.").into(), true)),
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
        if harness == "OpenCode" {
            // A private server that exits with the command, instead of starting OpenCode's
            // background service (which would stay resident after FluxCode closes).
            cmd.arg("--standalone");
        }
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
                self.status = if lines == 1 { crate::trf!("Inserted 1 line into {name}"; name => name) } else { crate::trf!("Inserted {lines} lines into {name}"; lines => lines, name => name) }.into();
            }
            None => {
                cx.write_to_clipboard(ClipboardItem::new_string(code));
                self.status = tr("No file is open: the code was copied instead").into();
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------ render

    /// The memory view: what the agent remembers for this project and for all
    /// projects, each note with how strong it is and how often it helped.
    fn render_memory(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let off = self.settings.memory_off;
        let weak = cx.entity().downgrade();
        let header = h_flex()
            .px_3()
            .pb_2()
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::i18n::tr("Memory")))
            .when(!off && !a.memories.is_empty(), |h| h.child(div().text_xs().text_color(c(theme::FG_DIM)).child(a.memories.len().to_string())))
            .child(div().flex_1())
            .child(
                Switch::new("ag-mem-on")
                    .checked(!off)
                    .small()
                    .tooltip(if off { crate::i18n::tr("Memory is off. Click to turn it on") } else { crate::i18n::tr("Memory is on. Click to turn it off") })
                    .on_click(move |on, _, cx| {
                        let on = *on;
                        _ = weak.update(cx, |this, cx| {
                            this.settings.memory_off = !on;
                            this.settings.save();
                            this.agent_refresh_memories();
                            cx.notify();
                        });
                    }),
            );
        let mut list = v_flex().id("ag-memory").flex_1().min_h_0().px_2().overflow_y_scroll();
        if off {
            list = list.child(div().px_1().py_2().text_xs().text_color(c(theme::FG_DIM)).child(crate::i18n::tr(
                "Memory is off: nothing is read or saved, and the agent gets no notes. Your notes stay in the file.",
            )));
        } else if a.memories.is_empty() {
            list = list.child(div().px_1().py_2().text_xs().text_color(c(theme::FG_DIM)).child(
                crate::i18n::tr("Nothing saved yet. The agent saves lasting facts here by itself, or type one above. Notes fade when unused and grow stronger when they help."),
            ));
        }
        if !off {
            let (here, everywhere): (Vec<(usize, &memory::Trail)>, Vec<(usize, &memory::Trail)>) =
                a.memories.iter().enumerate().partition(|(_, t)| t.tags.iter().any(|x| x.starts_with("project:")));
            for (title, group) in [(crate::i18n::tr("This project"), here), (crate::i18n::tr("All projects"), everywhere)] {
                if group.is_empty() {
                    continue;
                }
                list = list.child(div().px_1().pt_2().pb_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(title));
                for (i, t) in group {
                    let id = t.id.clone();
                    // A word instead of the raw strength (tau): fades below 0.4, strong from 1.
                    let (strength, tone) = match t.tau {
                        x if x >= 1.0 => (crate::i18n::tr("Strong"), theme::GREEN),
                        x if x >= 0.4 => (crate::i18n::tr("Normal"), theme::FG_DIM),
                        _ => (crate::i18n::tr("Fading"), theme::WARN),
                    };
                    let used = if t.success == 0 { crate::i18n::tr("not used yet").to_string() } else { crate::trf!("used {n}×"; n => t.success) };
                    list = list.child(
                        h_flex()
                            .id(("ag-mem", i))
                            .items_start()
                            .gap_2()
                            .px_2()
                            .py_1p5()
                            .rounded(px(4.))
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_sm().text_color(c(theme::FG)).child(t.content.clone()))
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .text_xs()
                                            .text_color(c(theme::FG_DIM))
                                            .child(div().flex_none().size(px(6.)).rounded_full().bg(c(tone)))
                                            .child(strength)
                                            .child("·")
                                            .child(used),
                                    ),
                            )
                            .child(
                                div()
                                    .id(("ag-mem-del", i))
                                    .flex_none()
                                    .size(theme::u(22.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_color(c(theme::FG_DIM))
                                    .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
                                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Forget (moves it to the archive)")).build(w, cx))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let _ = memory::Store::default().forget(&id);
                                        this.agent_refresh_memories();
                                        cx.notify();
                                    }))
                                    .child(Icon::new(IconName::Close).size(theme::u(12.))),
                            ),
                    );
                }
            }
        }
        // The file, short: the home folder as ~, the full path in the tooltip.
        let full = memory::dir().join("trails.json");
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
        let shown = full.display().to_string();
        let shown = if !home.is_empty() && shown.starts_with(&home) { format!("~{}", &shown[home.len()..]) } else { shown };
        let full_s: SharedString = full.display().to_string().into();
        v_flex()
            .size_full()
            .child(header)
            .when(!off, |v| v.child(div().px_3().pb_1().child(Input::new(&a.memory_input).small())))
            .child(list)
            .child(
                h_flex()
                    .id("ag-mem-file")
                    .px_3()
                    .py_2()
                    .gap_1()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .border_t_1()
                    .border_color(c(theme::BORDER))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full_s.clone()).build(w, cx))
                    .child(div().flex_none().child(crate::i18n::tr("File:")))
                    .child(div().min_w_0().truncate().child(shown)),
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
            v = v.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("Reading {name}'s sessions…"; name => name)));
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
                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Resume this session here")).build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| this.agent_open_remote(id.clone(), cx))),
            );
        }
        v.into_any_element()
    }

    /// "Working…" / "Starting…" for the input box's bottom row (nothing while idle).
    fn agent_status_line(&self) -> Option<(theme::Tok, String)> {
        match &self.agent.status {
            Status::Busy => Some((theme::BLUE, crate::i18n::tr("Working…").into())),
            Status::Starting if self.agent.loading_id.is_some() => Some((theme::FG_MUTED, crate::i18n::tr("Opening the session…").into())),
            Status::Starting => Some((theme::FG_MUTED, crate::i18n::tr("Starting…").into())),
            _ => None,
        }
    }

    /// Click on a chip's name: show what it stands for.
    fn agent_chip_go(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match key {
            "file" | "sel" => {
                if let Some(ix) = self.active {
                    let h = self.docs[ix].editor.focus_handle(cx);
                    h.focus(window, cx);
                }
            }
            "err" => {
                if let Some(p) = self.problems.first() {
                    let (path, line) = (p.path.clone(), p.line);
                    self.open_location(path, line, 0, window, cx);
                } else {
                    self.panel_open = true;
                    self.panel_tab = crate::workbench::PanelTab::Terminal;
                }
            }
            "out" => {
                self.panel_open = true;
                self.panel_tab = crate::workbench::PanelTab::Terminal;
            }
            "ws" => {
                self.view = crate::workbench::SideView::Explorer;
                self.sidebar_open = true;
            }
            k if k.starts_with('@') => {
                if let Some(root) = self.root.clone() {
                    let p = root.join(&k[1..]);
                    if p.is_file() {
                        self.open_file(p, window, cx);
                    } else if p.is_dir() {
                        self.selected = Some(p);
                        self.view = crate::workbench::SideView::Explorer;
                        self.sidebar_open = true;
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn render_context_chips(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let chips = self.agent_chips(cx);
        chips
            .into_iter()
            .enumerate()
            .map(|(i, (key, label, icon, on))| {
                let toggle_key = key.clone();
                chip_shell(SharedString::from(format!("ag-chip-{i}")), on)
                    .child(
                        chip_x(SharedString::from(format!("ag-chip-x-{i}")), if on { IconName::Close } else { IconName::Plus }, cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if !this.agent.off_ctx.remove(&toggle_key) {
                                this.agent.off_ctx.insert(toggle_key.clone());
                            }
                            cx.notify();
                        }))
                        .tooltip(move |w, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(if on { crate::i18n::tr("Leave it out of the message") } else { crate::i18n::tr("Send it with the message") }).build(w, cx)
                        }),
                    )
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("ag-chip-open-{i}")))
                            .min_w_0()
                            .pr_1p5()
                            .gap_1()
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Go to it")).build(w, cx))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                cx.stop_propagation();
                                this.agent_chip_go(&key, w, cx);
                            }))
                            .child(icon.size(theme::u(12.)))
                            .child(div().min_w_0().truncate().when(!on, |d| d.line_through()).child(label)),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_slash_popup(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (list, sel) = self.agent.slash.as_ref()?;
        let name = agent_name();
        let mut v = v_flex()
            .mx_3()
            .mb_1()
            .py_1()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(gpui_kit::Hsla::from(rgb(theme::palette().swatches()[1])))
            .shadow_md();
        if list.is_empty() {
            v = v.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("No command matches")));
        }
        for (row, cmd) in list.iter().enumerate() {
            v = v.child(
                h_flex()
                    .id(("ag-slash", row))
                    .mx_1()
                    .px_2()
                    .h(theme::u(24.))
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(row == *sel, |d| d.bg(c(theme::SELECT)))
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(div().flex_shrink_0().max_w(relative(0.6)).truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(format!("/{}", cmd.name)))
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(cmd.desc.clone()))
                    .child(
                        div()
                            .flex_none()
                            .px_1()
                            .rounded(px(3.))
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .bg(c(theme::BG_ELEVATED))
                            .child(if cmd.builtin { "FluxCode".to_string() } else { name.to_string() }),
                    )
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if let Some(s) = this.agent.slash.as_mut() {
                            s.1 = row;
                        }
                        this.agent_slash_accept(w, cx);
                    })),
            );
        }
        let more = if self.agent.commands.is_empty() && !matches!(self.agent.status, Status::Ready | Status::Busy) {
            format!(" · {name}'s own commands appear once it is on")
        } else {
            String::new()
        };
        Some(v.child(div().px_3().pt_1().text_xs().text_color(c(theme::FG_DIM)).child(format!("↑↓ choose · Tab fills · Enter runs · Esc closes{more}"))).into_any_element())
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
            list = list.child(div().px_2().py_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("No file matches")));
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
        Some(list.child(div().px_3().pt_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("↑↓ choose · Tab or Enter adds · Esc closes"))).into_any_element())
    }

    pub(crate) fn render_agent(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let (dot, label) = match &a.status {
            Status::Off => (theme::FG_DIM, crate::i18n::tr("Off").to_string()),
            Status::Starting if a.loading_id.is_some() => (theme::FG_MUTED, crate::i18n::tr("Opening the session…").into()),
            Status::Starting => (theme::FG_MUTED, crate::i18n::tr("Starting…").into()),
            Status::Ready => (theme::GREEN, crate::i18n::tr("Ready").into()),
            Status::Busy => (theme::BLUE, crate::i18n::tr("Working…").into()),
            Status::Error(_) => (theme::DANGER, "Error".into()),
        };
        let on = matches!(a.status, Status::Starting | Status::Ready | Status::Busy);
        // The working / starting state is shown in the input box's bottom row.
        let _ = (dot, label);
        let header = div();
        if a.show_memory {
            return v_flex().size_full().child(self.render_memory(cx)).into_any_element();
        }
        if a.show_history {
            return v_flex().size_full().child(header).child(self.render_history(cx)).into_any_element();
        }
        if a.show_options {
            return self.render_options(cx);
        }

        let name = agent_name();
        let mut log = v_flex().id("ag-log").size_full().px_3().pb_3().gap_2().overflow_y_scroll().track_scroll(&a.scroll);
        if a.items.is_empty() && !matches!(a.status, Status::Error(_)) {
            let missing = name == "Mura" && !mura_installed();
            let hint = if missing {
                crate::i18n::tr("Mura isn't installed on this PC (FluxCode looks on PATH and in ~/.local/bin). Build it from github.com/MuaraAI/Mura-Harness, or pick OpenCode in the panel title menu.").to_string()
            } else if on {
                crate::i18n::tr("Type @ to add a file. Your open file, selection and last error go along when they help.").to_string()
            } else {
                crate::i18n::tr("Off for now. Your first message turns it on. Type @ to add a file.").to_string()
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
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("Ask {name}"; name => name)))
                    .child(div().text_xs().text_center().text_color(c(theme::FG_DIM)).child(hint))
                    .when(missing, |v| {
                        v.child(
                            h_flex()
                                .pt_2()
                                .gap_2()
                                .child(Button::new("ag-use-oc").label(tr("Use OpenCode")).small().primary().on_click(cx.listener(|this, _, w, cx| {
                                    this.on_agent_use_opencode(&crate::workbench::AgentUseOpenCode, w, cx)
                                }))),
                        )
                    }),
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
            if matches!(item, Item::Branch(_)) {
                after_user = true;
            }
            let starts_answer = after_user && !matches!(item, Item::User(_) | Item::Note(..) | Item::Saved(_) | Item::MemoryUsed(_) | Item::Branch(_));
            if starts_answer {
                log = log.child(speaker(name, Icon::new(IconName::Bot)));
                after_user = false;
            }
            log = log.child(match item {
                Item::User(t) => {
                    after_user = true;
                    let group = SharedString::from(format!("ag-user-{i}"));
                    let editing = a.editing == Some(i);
                    v_flex()
                        .items_end()
                        .gap_1()
                        .pt_1()
                        .child(speaker(crate::i18n::tr("You"), Icon::new(IconName::User)))
                        .child(
                            h_flex()
                                .group(group.clone())
                                .w_full()
                                .justify_end()
                                .items_center()
                                .gap_1()
                                // Edit sits left of the bubble, dim until pointed at.
                                .child(
                                    div()
                                        .id(("ag-edit", i))
                                        .flex_none()
                                        .size(theme::u(22.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(4.))
                                        .cursor_pointer()
                                        .text_color(c(theme::FG_DIM))
                                        .when(editing, |d| d.text_color(c(theme::ACCENT_TEXT)))
                                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Edit this message")).build(w, cx))
                                        .on_click(cx.listener(move |this, _, w, cx| this.agent_edit(i, w, cx)))
                                        .child(Icon::new(gpui_kit::assets::IconName::Pencil).size(theme::u(13.))),
                                )
                                .child(
                            div()
                                .max_w(relative(0.88))
                                .px_2p5()
                                .py_1p5()
                                .rounded(px(8.))
                                .bg(c(theme::BG_ELEVATED))
                                .border_1()
                                .border_color(c(theme::BORDER_STRONG))
                                .text_sm()
                                .text_color(c(theme::FG_STRONG))
                                .child(t.clone()),
                                ),
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
                                    .tooltip(tr("Copy"))
                                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))),
                            )
                            .child(
                                Button::new("cb-insert")
                                    .icon(Icon::new(gpui_kit::assets::IconName::TextCursorInput))
                                    .ghost()
                                    .xsmall()
                                    .tooltip(tr("Insert at the cursor in the open file (replaces the selection)"))
                                    .on_click(move |_, w, cx| {
                                        if let Ok(mut p) = PENDING_INSERT.lock() {
                                            *p = Some(insert.clone());
                                        }
                                        w.dispatch_action(Box::new(crate::workbench::AgentInsertCode), cx);
                                    }),
                            )
                    })
                    .into_any_element(),
                Item::Thought(t) => {
                    let open = a.open_thoughts.contains(&i);
                    let last = t.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_string();
                    v_flex()
                        .id(("ag-thought", i))
                        .gap_1()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(Icon::new(gpui_kit::assets::IconName::Brain).size(theme::u(12.)).flex_none())
                                .child(div().flex_none().font_weight(FontWeight::MEDIUM).child(tr("Thinking")))
                                .when(!open, |h| h.child(div().flex_1().min_w_0().truncate().italic().child(last)))
                                .when(open, |h| h.child(div().flex_1()))
                                .child(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(12.)).flex_none()),
                        )
                        .when(open, |v| v.child(div().pl(theme::u(18.)).italic().child(t.clone())))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.agent.open_thoughts.remove(&i) {
                                this.agent.open_thoughts.insert(i);
                            }
                            cx.notify();
                        }))
                        .into_any_element()
                }
                Item::Branch(t) => h_flex()
                    .py_1()
                    .gap_2()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(div().flex_1().h(px(1.)).bg(c(theme::BORDER)))
                    .child(Icon::new(gpui_kit::assets::IconName::GitBranch).size(theme::u(12.)).flex_none())
                    .child(div().max_w(relative(0.75)).text_center().child(t.clone()))
                    .child(div().flex_1().h(px(1.)).bg(c(theme::BORDER)))
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
                // Notes: the icon sits in a box one text line tall, so it centers on the first line.
                Item::Note(t, err) => h_flex()
                    .items_start()
                    .gap_1p5()
                    .text_xs()
                    .line_height(theme::u(18.))
                    .text_color(c(if *err { theme::DANGER } else { theme::FG_DIM }))
                    .child(line_icon(Icon::new(if *err { IconName::CircleX } else { IconName::Info })))
                    .child(div().flex_1().min_w_0().child(t.clone()))
                    .into_any_element(),
                // Memory stays out of the way: one dim line, the full note in its tooltip and in the Memory view.
                Item::Saved(t) => {
                    let full: SharedString = t.clone().into();
                    memory_line(("ag-mem-saved", i), crate::i18n::tr("Saved to memory"), t.clone())
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.agent.show_memory = true;
                            this.agent_refresh_memories();
                            cx.notify();
                        }))
                        .into_any_element()
                }
                Item::MemoryUsed(n) => memory_line(
                    ("ag-mem-used", i),
                    if *n == 1 { crate::i18n::tr("Used 1 note from memory") } else { "" },
                    if *n == 1 { String::new() } else { crate::trf!("Used {n} notes from memory"; n => n) },
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.agent.show_memory = true;
                    this.agent_refresh_memories();
                    cx.notify();
                }))
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
                .child(div().text_color(c(theme::FG_STRONG)).child(crate::trf!("Allow: {0}?"; 0 => p.title)))
                .child(h_flex().gap_1().flex_wrap().children(p.options.iter().enumerate().map(|(i, (oid, name, kind))| {
                    let oid = oid.clone();
                    // One loud choice (allow once); the lasting ones and reject stay quiet.
                    let label: String = match kind.as_str() {
                        "allow_once" => crate::i18n::tr("Allow once").into(),
                        "allow_always" => crate::i18n::tr("Always allow").into(),
                        "reject_once" => crate::i18n::tr("Reject").into(),
                        "reject_always" => crate::i18n::tr("Always reject").into(),
                        _ => name.clone(),
                    };
                    let b = Button::new(("perm", i)).label(label).xsmall();
                    let b = match kind.as_str() {
                        "allow_once" => b.primary(),
                        k if k.starts_with("reject") => b.ghost(),
                        _ => b,
                    };
                    b.on_click(cx.listener(move |this, _, _, cx| this.agent_answer_permission(Some(oid.clone()), cx)))
                })))
        });

        // What the agent changed this turn, like a worktree: each file with its
        // lines added / removed, a diff, and undo.
        let changes = (!a.changes.is_empty()).then(|| {
            let (add, del) = a.changes.iter().fold((0, 0), |t, ch| (t.0 + ch.added, t.1 + ch.removed));
            let files = if a.changes.len() == 1 { crate::i18n::tr("1 file").to_string() } else { crate::trf!("{n} files"; n => a.changes.len()) };
            let counts = |added: usize, removed: usize| {
                h_flex()
                    .flex_none()
                    .gap_1()
                    .font_family("Cascadia Code")
                    .child(div().text_color(c(theme::GREEN)).child(format!("+{added}")))
                    .child(div().text_color(c(theme::DANGER)).child(format!("-{removed}")))
            };
            let rows = a.changes.iter().enumerate().map(|(i, ch)| {
                let path = ch.path.clone();
                let name = ch.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let rel = self.rel_display(&ch.path.to_string_lossy());
                let dir = rel.rsplit_once(['\\', '/']).map(|(d, _)| d.to_string()).unwrap_or_default();
                let (icon, tint) = crate::files::icon_for(&name);
                h_flex()
                    .id(("ag-ch", i))
                    .px_2()
                    .h(theme::u(26.))
                    .gap_2()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(
                        h_flex()
                            .id(("ag-ch-open", i))
                            .flex_1()
                            .min_w_0()
                            .gap_1p5()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, w, cx| this.open_file(path.clone(), w, cx)))
                            .child(Icon::new(icon).size(theme::u(13.)).text_color(theme::tint(tint)))
                            .child(div().flex_none().text_color(c(theme::FG)).child(name))
                            .when(ch.before.is_none(), |h| h.child(div().flex_none().text_color(c(theme::GREEN)).child(crate::i18n::tr("new"))))
                            .child(div().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(dir)),
                    )
                    .child(counts(ch.added, ch.removed))
                    .child(
                        div()
                            .id(("ag-ch-diff", i))
                            .flex_none()
                            .size(theme::u(22.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_color(c(theme::FG_DIM))
                            .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Show the diff")).build(w, cx))
                            .on_click(cx.listener(move |this, _, w, cx| this.agent_show_diff(i, w, cx)))
                            .child(Icon::new(gpui_kit::assets::IconName::FileDiff).size(theme::u(13.))),
                    )
                    .child(
                        div()
                            .id(("ag-ch-undo", i))
                            .flex_none()
                            .size(theme::u(22.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_color(c(theme::FG_DIM))
                            .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Undo this file")).build(w, cx))
                            .on_click(cx.listener(move |this, _, _, cx| this.agent_undo(i, cx)))
                            .child(Icon::new(gpui_kit::assets::IconName::Undo2).size(theme::u(13.))),
                    )
            });
            v_flex()
                .mx_3()
                .mb_2()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_ELEVATED))
                .text_xs()
                .overflow_hidden()
                .child(
                    h_flex()
                        .px_2()
                        .h(theme::u(30.))
                        .gap_2()
                        .border_b_1()
                        .border_color(c(theme::BORDER))
                        .child(Icon::new(gpui_kit::assets::IconName::FileDiff).size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
                        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::i18n::tr("Changes")))
                        .child(div().flex_none().text_color(c(theme::FG_DIM)).child(files))
                        .child(counts(add, del))
                        .child(div().flex_1())
                        .child(change_btn("ag-undo-all", gpui_kit::assets::IconName::Undo2, crate::i18n::tr("Undo all")).on_click(cx.listener(|this, _, _, cx| {
                            for i in (0..this.agent.changes.len()).rev() {
                                this.agent_undo(i, cx);
                            }
                        })))
                        .child(change_btn("ag-keep-all", IconName::Check, tr("Keep all")).on_click(cx.listener(|this, _, _, cx| {
                            this.agent.changes.clear();
                            this.agent_reinforce_turn();
                            cx.notify();
                        }))),
                )
                .child(v_flex().id("ag-changes").max_h(theme::u(156.)).overflow_y_scroll().py_0p5().children(rows))
        });

        v_flex()
            .id("agent-panel")
            .size_full()
            // Files dropped anywhere on the panel are attached to the next message.
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(c(theme::BG_HOVER)))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                // Attached here, not opened by the window.
                cx.stop_propagation();
                this.agent_attach(paths.paths().to_vec(), cx);
            }))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(log)
                    .child(self.smooth_scroll_layer(crate::workbench::ScrollTarget::Handle(CHAT_SCROLL_KEY, a.scroll.clone()), cx)),
            )
            .when_some(permission, |v, p| v.child(p))
            .when_some(changes, |v, ch| v.child(ch))
            .children(self.render_queue(cx))
            .children(self.render_slash_popup(cx))
            .children(self.render_edit_strip(cx))
            .children(self.render_mention_popup(cx))
            .child(self.render_composer(cx))
            .into_any_element()
    }

    /// Add files to the next message (button, drop or paste). Folders become @mentions.
    pub(crate) fn agent_attach(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for p in paths {
            if p.exists() && !self.agent.attachments.contains(&p) {
                self.agent.attachments.push(p);
            }
        }
        cx.notify();
    }

    fn agent_pick_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("Attach files".into()) });
        cx.spawn_in(window, async move |this, cx| {
            let picked = paths.await.ok()?.ok()??;
            this.update(cx, |this, cx| this.agent_attach(picked, cx)).ok()
        })
        .detach();
    }

    /// How attached files go to the agent (see docs.rs): images as images, text and
    /// Office documents as text, anything else as a link the agent opens itself.
    fn agent_attachment_blocks(&self, files: &[PathBuf]) -> Vec<Value> {
        use base64::Engine;
        let mut out = Vec::new();
        for p in files {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            if p.is_dir() {
                let names: Vec<String> = std::fs::read_dir(p)
                    .map(|rd| rd.flatten().take(300).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                    .unwrap_or_default();
                out.push(json!({ "type": "text", "text": format!("The user attached the folder {} ({}). It contains:\n{}", name, p.display(), names.join("\n")) }));
                continue;
            }
            let link = json!({ "type": "resource_link", "uri": crate::docs::file_uri(p), "name": name, "mimeType": crate::docs::mime(p) });
            match crate::docs::kind(p) {
                crate::docs::Kind::Image if self.agent.can_image && size <= 5 * 1024 * 1024 => {
                    if let Ok(bytes) = std::fs::read(p) {
                        out.push(json!({ "type": "image", "mimeType": crate::docs::mime(p), "data": base64::engine::general_purpose::STANDARD.encode(bytes) }));
                        continue;
                    }
                    out.push(link);
                }
                crate::docs::Kind::Text if size <= 400_000 => match std::fs::read(p).ok().and_then(|b| String::from_utf8(b).ok()) {
                    Some(text) => out.push(json!({ "type": "resource", "resource": { "uri": crate::docs::file_uri(p), "text": text, "mimeType": crate::docs::mime(p) } })),
                    None => out.push(link),
                },
                crate::docs::Kind::Office => {
                    match crate::docs::office_text(p) {
                        Ok(text) => out.push(json!({ "type": "text", "text": format!("Text of the attached file {name} (read by FluxCode; layout and images are not included):\n```\n{text}\n```") })),
                        Err(e) => out.push(json!({ "type": "text", "text": format!("FluxCode couldn't read the text of {name}: {e}.") })),
                    }
                    out.push(link);
                }
                _ => {
                    out.push(json!({ "type": "text", "text": format!("The user attached {name} ({}). It is at {}; open it with your tools if you need its content.", crate::docs::mime(p), p.display()) }));
                    out.push(link);
                }
            }
        }
        out
    }

    fn render_queue(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let a = &self.agent;
        if a.queue.is_empty() {
            return None;
        }
        let name = agent_name();
        let status = match &a.status {
            Status::Busy => crate::trf!("Waiting for {name} to finish, then these are sent together"; name => name),
            Status::Starting => crate::trf!("Sent as soon as {name} is ready"; name => name),
            Status::Error(_) => crate::trf!("Not sent: {name} couldn't start. Switch the agent in the title menu, they will go to it"; name => name),
            _ => crate::i18n::tr("Sent with your next message").to_string(),
        };
        let busy = a.status == Status::Busy;
        Some(
            v_flex()
                .mx_3()
                .mb_1()
                .gap_1()
                .child(div().text_xs().text_color(c(theme::FG_DIM)).child(status))
                .children(a.queue.iter().enumerate().map(|(i, q)| {
                    h_flex()
                        .id(("ag-queued", i))
                        .px_2()
                        .h(theme::u(26.))
                        .gap_2()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .bg(c(theme::BG_ELEVATED))
                        .text_xs()
                        .child(Icon::new(gpui_kit::assets::IconName::Clock).size(theme::u(12.)).text_color(c(theme::FG_DIM)).flex_none())
                        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(q.text.lines().next().unwrap_or("").to_string()))
                        .when(!q.files.is_empty(), |h| h.child(div().flex_none().text_color(c(theme::FG_DIM)).child(format!("+{} file{}", q.files.len(), if q.files.len() == 1 { "" } else { "s" }))))
                        .when(busy && i == 0, |h| {
                            h.child(
                                Button::new(("ag-steer", i))
                                    .label(tr("Send now"))
                                    .xsmall()
                                    .ghost()
                                    .tooltip(tr("Stop the current answer and send this now (the agent keeps the context)"))
                                    .on_click(cx.listener(|this, _, _, cx| this.agent_steer(cx))),
                            )
                        })
                        .child(Button::new(("ag-unqueue", i)).icon(IconName::Close).xsmall().ghost().tooltip(tr("Remove from the queue")).on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.agent.queue.len() {
                                this.agent.queue.remove(i);
                            }
                            cx.notify();
                        })))
                })),
        )
        .map(|v| v.into_any_element())
    }

    /// The message box: chips (what goes along), the text (grows with it), and a
    /// bottom row with attach, @, the keys hint and Send / Stop.
    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.agent;
        let busy = a.status == Status::Busy;
        let has_text = !a.input.read(cx).value().trim().is_empty() || !a.attachments.is_empty();
        let weak = cx.entity().downgrade();
        let attachments = a.attachments.iter().enumerate().map(|(i, p)| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let (icon, tint) = crate::files::icon_for(&name);
            let path = p.clone();
            chip_shell(SharedString::from(format!("ag-att-{i}")), true)
                .child(chip_x(SharedString::from(format!("ag-att-x-{i}")), IconName::Close, cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if i < this.agent.attachments.len() {
                        this.agent.attachments.remove(i);
                    }
                    cx.notify();
                })))
                .child(
                    h_flex()
                        .id(SharedString::from(format!("ag-att-open-{i}")))
                        .min_w_0()
                        .pr_1p5()
                        .gap_1()
                        .cursor_pointer()
                        .hover(|s| s.text_color(c(theme::FG_STRONG)))
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Open the file")).build(w, cx))
                        .on_click(cx.listener(move |this, _, w, cx| {
                            cx.stop_propagation();
                            if path.is_file() {
                                this.open_file(path.clone(), w, cx);
                            }
                        }))
                        .child(Icon::new(icon).size(theme::u(12.)).text_color(theme::tint(tint)))
                        .child(div().min_w_0().truncate().child(name)),
                )
                .into_any_element()
        });
        let mut chips: Vec<AnyElement> = attachments.collect();
        chips.extend(self.render_context_chips(cx));
        v_flex()
            .id("ag-composer")
            .mx_3()
            .mb_3()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(c(theme::BG_ELEVATED))
            .cursor_text()
            // A click anywhere in the box puts the caret in the text.
            .on_click(cx.listener(|this, _, w, cx| {
                let h = this.agent.input.focus_handle(cx);
                if !h.is_focused(w) {
                    h.focus(w, cx);
                }
            }))
            .when(!chips.is_empty(), |v| v.child(h_flex().px_2().pt_2().gap_1().flex_wrap().children(chips)))
            .child(
                div().px_1().pt_1().key_context("AgentInput").child(
                    Textarea::new(&a.input).appearance(false).on_paste(move |item, _, cx| {
                        // Pasted screenshots and copied files become attachments.
                        let mut files: Vec<PathBuf> = Vec::new();
                        for entry in item.entries() {
                            match entry {
                                ClipboardEntry::Image(img) => {
                                    let ext = match img.format {
                                        ImageFormat::Jpeg => "jpg",
                                        ImageFormat::Gif => "gif",
                                        ImageFormat::Webp => "webp",
                                        ImageFormat::Bmp => "bmp",
                                        _ => "png",
                                    };
                                    let dir = std::env::temp_dir().join("fluxcode-paste");
                                    let _ = std::fs::create_dir_all(&dir);
                                    let p = dir.join(format!("pasted-{}.{ext}", now_ms()));
                                    if std::fs::write(&p, &img.bytes).is_ok() {
                                        files.push(p);
                                    }
                                }
                                ClipboardEntry::ExternalPaths(paths) => files.extend(paths.paths().iter().cloned()),
                                _ => {}
                            }
                        }
                        if files.is_empty() {
                            return false;
                        }
                        _ = weak.update(cx, |this, cx| this.agent_attach(files, cx));
                        true
                    }),
                ),
            )
            .child(
                h_flex()
                    .h(theme::u(34.))
                    .px_1p5()
                    .gap_0p5()
                    .items_center()
                    .child(
                        icon_btn("ag-attach", Icon::new(gpui_kit::assets::IconName::Paperclip), tr("Attach files: images, PDF, Word, Excel, PowerPoint, code… (or drop / paste them)"))
                            .on_click(cx.listener(|this, _, w, cx| {
                                cx.stop_propagation();
                                this.agent_pick_files(w, cx)
                            })),
                    )
                    .child(
                        icon_btn("ag-at", Icon::new(gpui_kit::assets::IconName::AtSign), tr("Mention a file in the project"))
                            .on_click(cx.listener(|this, _, w, cx| {
                                cx.stop_propagation();
                                this.agent.input.update(cx, |s, cx| {
                                    let v = s.value().to_string();
                                    let new = if v.is_empty() || v.ends_with(char::is_whitespace) { format!("{v}@") } else { format!("{v} @") };
                                    let end = new.len();
                                    s.set_value(new, w, cx);
                                    s.set_selected_range(end..end, cx);
                                });
                                let h = this.agent.input.focus_handle(cx);
                                h.focus(w, cx);
                                this.agent_update_mention(cx);
                            })),
                    )
                    .when_some(a.config.iter().find(|o| o.category == "model"), |h, m| {
                        // "provider/Model name" reads as just the model name here, plus a non-default effort.
                        let current = m.current_name();
                        let mut short = current.rsplit('/').next().unwrap_or(current).to_string();
                        // "Free" is said by the group in the settings list; here it only takes room.
                        if let Some(s) = short.strip_suffix(" Free") {
                            short = s.to_string();
                        }
                        if let Some(e) = a.config.iter().find(|o| o.category == "thought_level").filter(|e| e.current != "default") {
                            short = format!("{short} · {}", e.current_name());
                        }
                        h.child(
                            h_flex()
                                .id("ag-model-btn")
                                .flex_none()
                                .max_w(theme::u(170.))
                                .h(theme::u(24.))
                                .px_1p5()
                                .gap_1()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(c(theme::FG_MUTED))
                                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG)))
                                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Model, effort and mode")).build(w, cx))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.agent_open_options(cx)
                                }))
                                .child(div().min_w_0().truncate().child(short))
                                .child(Icon::new(IconName::ChevronDown).xsmall()),
                        )
                    })
                    .child({
                        let level = self.settings.access().to_string();
                        let (icon, label) = match level.as_str() {
                            "read" => (Icon::new(IconName::Eye), crate::i18n::tr("Read only")),
                            "full" => (Icon::new(gpui_kit::assets::IconName::ShieldAlert), crate::i18n::tr("Full access")),
                            _ => (Icon::new(gpui_kit::assets::IconName::Pencil), crate::i18n::tr("Read & write")),
                        };
                        // Icon only (eye / pencil / shield): the row stays one line in a narrow panel.
                        Button::new("ag-access")
                            .icon(icon)
                            .ghost()
                            .xsmall()
                            .flex_none()
                            .tooltip(crate::trf!("Access: {level}. What the agent may do without asking"; level => label))
                            .dropdown_menu(move |m: gpui_kit::component::menu::PopupMenu, _, _| {
                                m.min_w(px(230.))
                                    .menu_with_check(crate::i18n::tr("Read only: reads and explains"), level == "read", Box::new(crate::workbench::AgentAccessRead))
                                    .menu_with_check(crate::i18n::tr("Read & write: edits files, commands ask"), level == "write", Box::new(crate::workbench::AgentAccessWrite))
                                    .menu_with_check(crate::i18n::tr("Full access: nothing asks"), level == "full", Box::new(crate::workbench::AgentAccessFull))
                            })
                    })
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .justify_end()
                            .gap_1p5()
                            .px_1()
                            .overflow_hidden()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .when_some(self.agent_status_line(), |h, (dot, label)| {
                                h.child(div().flex_none().size(px(6.)).rounded_full().bg(c(dot))).child(div().min_w_0().truncate().child(label))
                            }),
                    )
                    .child(if busy && !has_text {
                        // A solid square in the accent: the one action while the agent works.
                        send_btn("ag-stop", true, tr("Stop the answer"))
                            .child(div().size(theme::u(8.)).rounded(px(1.5)).bg(c(theme::ON_ACCENT)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.agent_cancel(cx)
                            }))
                            .into_any_element()
                    } else {
                        send_btn("ag-send", has_text, if busy { crate::i18n::tr("Queue it (Enter): sent when the answer ends · Shift+Enter new line") } else { crate::i18n::tr("Send (Enter) · Shift+Enter new line") })
                            .child(Icon::new(IconName::ArrowUp).size(theme::u(14.)).text_color(c(if has_text { theme::ON_ACCENT } else { theme::FG_DIM })))
                            .on_click(cx.listener(|this, _, w, cx| {
                                cx.stop_propagation();
                                this.agent_send(w, cx)
                            }))
                            .into_any_element()
                    }),
            )
    }

    /// The harness's own settings for this session, compact enough to see at once:
    /// one row per setting (label left, control right, wrapping in a narrow panel).
    /// Few choices are a row of buttons; long lists (models) are one dropdown, free
    /// models grouped first. Nothing here runs until a choice is made.
    fn render_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let name = agent_name();
        let weak = cx.entity().downgrade();
        let header = h_flex()
            .px_3()
            .h(theme::u(32.))
            .gap_2()
            .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("{name} settings"; name => name)))
            .child(Button::new("ag-opt-back").label(crate::i18n::tr("Back to the chat")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| {
                this.agent.show_options = false;
                cx.notify();
            })));
        // label | control, the control column wraps on its own when the panel is narrow
        let row = |label: SharedString, control: AnyElement| {
            h_flex()
                .px_3()
                .py_1p5()
                .gap_3()
                .items_start()
                .child(div().w(theme::u(64.)).flex_none().pt_0p5().text_xs().text_color(c(theme::FG_MUTED)).child(label))
                .child(div().flex_1().min_w_0().child(control))
                .into_any_element()
        };
        let buttons = |opt: &ConfigOpt| {
            let mut choices = opt.choices.clone();
            if opt.category == "thought_level" {
                choices.sort_by_key(|c| if c.0 == "default" { 0 } else { 1 });
            }
            h_flex()
                .gap_1()
                .flex_wrap()
                .children(choices.into_iter().map(|(v, n, _)| {
                    let id = opt.id.clone();
                    let on = opt.current == v;
                    Button::new(SharedString::from(format!("ag-opt-{id}-{v}")))
                        .label(n)
                        .xsmall()
                        .when(on, |b| b.primary())
                        .on_click(cx.listener(move |this, _, _, cx| this.agent_set_option(&id, v.clone(), true, cx)))
                }))
                .into_any_element()
        };
        let dropdown = |opt: &ConfigOpt| {
            let current = opt.current_name().rsplit('/').next().unwrap_or("").to_string();
            let groups: Vec<(String, Vec<(String, String)>)> = if opt.category == "model" {
                model_groups(&opt.choices)
            } else {
                vec![(String::new(), opt.choices.iter().map(|c| (c.0.clone(), c.1.clone())).collect())]
            };
            let (id, cur, weak) = (opt.id.clone(), opt.current.clone(), weak.clone());
            Button::new(SharedString::from(format!("ag-opt-{}", opt.id)))
                .label(current)
                .xsmall()
                .dropdown_caret(true)
                .dropdown_menu(move |mut m: gpui_kit::component::menu::PopupMenu, _, _| {
                    m = m.min_w(px(240.)).max_h(px(380.)).scrollable(true);
                    for (gi, (group, models)) in groups.iter().enumerate() {
                        if gi > 0 {
                            m = m.separator();
                        }
                        let title = match group.as_str() {
                            "free" => crate::i18n::tr("Free (OpenCode Zen)").to_string(),
                            g => g.to_string(),
                        };
                        if !title.is_empty() {
                            m = m.item(gpui_kit::component::menu::PopupMenuItem::label(title));
                        }
                        for (v, n) in models {
                            let (id, v, weak) = (id.clone(), v.clone(), weak.clone());
                            m = m.item(gpui_kit::component::menu::PopupMenuItem::new(n.clone()).checked(*v == cur).on_click(move |_, _, cx| {
                                _ = weak.update(cx, |this, cx| this.agent_set_option(&id, v.clone(), true, cx));
                            }));
                        }
                    }
                    m
                })
                .into_any_element()
        };
        let mut body = v_flex().id("ag-options").flex_1().min_h_0().pt_1().pb_2().overflow_y_scroll();
        if a.config.is_empty() {
            let on = matches!(a.status, Status::Starting | Status::Ready | Status::Busy);
            body = body.child(
                v_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(if on {
                        crate::trf!("{name} is starting. Its models and settings show here in a moment."; name => name)
                    } else {
                        crate::trf!("{name} lists its models and settings once it is on."; name => name)
                    })
                    .when(!on, |v| {
                        v.child(h_flex().child(Button::new("ag-opt-on").label(crate::trf!("Turn {name} on"; name => name)).xsmall().primary().on_click(cx.listener(|this, _, _, cx| this.agent_set_on(true, cx)))))
                    }),
            );
        }
        // Mode first (it changes what the agent does), then model, then effort, then the rest.
        let order = |o: &ConfigOpt| match o.category.as_str() {
            "mode" => 0,
            "model" => 1,
            "thought_level" => 2,
            _ => 3,
        };
        let mut opts: Vec<&ConfigOpt> = a.config.iter().collect();
        opts.sort_by_key(|o| order(o));
        for o in opts {
            let label: SharedString = match o.category.as_str() {
                "mode" => crate::i18n::tr("Mode").into(),
                "model" => crate::i18n::tr("Model").into(),
                "thought_level" => crate::i18n::tr("Effort").into(),
                _ => o.name.clone().into(),
            };
            let control = if o.category == "model" || o.choices.len() > 6 {
                dropdown(o)
            } else if o.category == "mode" {
                // The chosen mode's description in one dim line (the full text in its tooltip).
                let desc = o.choices.iter().find(|c| c.0 == o.current).map(|c| c.2.clone()).unwrap_or_default();
                let full: SharedString = desc.clone().into();
                v_flex()
                    .gap_1()
                    .child(buttons(o))
                    .when(!desc.is_empty(), |v| {
                        v.child(
                            div()
                                .id("ag-opt-mode-desc")
                                .truncate()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx))
                                .child(desc),
                        )
                    })
                    .into_any_element()
            } else {
                buttons(o)
            };
            body = body.child(row(label, control));
        }
        // FluxCode's own side of the agent.
        let s = &self.settings;
        let access = s.access().to_string();
        body = body
            .child(div().mx_3().my_1().h(px(1.)).bg(c(theme::BORDER)))
            .child(row(
                crate::i18n::tr("Access").into(),
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children([("read", crate::i18n::tr("Read only")), ("write", crate::i18n::tr("Read & write")), ("full", crate::i18n::tr("Full access"))].into_iter().map(|(id, n)| {
                        Button::new(SharedString::from(format!("ag-opt-access-{id}")))
                            .label(n)
                            .xsmall()
                            .when(access == id, |b| b.primary())
                            .on_click(cx.listener(move |this, _, _, cx| this.agent_set_access(id, cx)))
                    }))
                    .into_any_element(),
            ))
            .child(row(
                crate::i18n::tr("Idle").into(),
                h_flex()
                    .items_start()
                    .gap_2()
                    .child(
                        Button::new("ag-opt-idle")
                            .label(if s.agent_idle_min > 0 { crate::i18n::tr("On") } else { crate::i18n::tr("Off") })
                            .xsmall()
                            .when(s.agent_idle_min > 0, |b| b.primary())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.agent_idle_min = if this.settings.agent_idle_min == 0 { 10 } else { 0 };
                                this.settings.save();
                                cx.notify();
                            })),
                    )
                    .child(div().min_w_0().text_xs().text_color(c(theme::FG_DIM)).child(crate::i18n::tr("Turns off after 10 quiet minutes to free memory")))
                    .into_any_element(),
            ));
        if name == "OpenCode" {
            if let Some(cfg) = opencode_config_file() {
                body = body.child(
                    h_flex().px_3().pt_1().child(
                        Button::new("ag-opt-cfg")
                            .label(crate::i18n::tr("Open OpenCode's config file"))
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(move |this, _, w, cx| this.open_file(cfg.clone(), w, cx))),
                    ),
                );
            }
        }
        v_flex().size_full().child(header).child(body).into_any_element()
    }

    /// While an earlier message is being edited: what Enter will do, and a way out.
    fn render_edit_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.agent.editing?;
        Some(
            h_flex()
                .mx_3()
                .mb_1()
                .px_2()
                .py_1()
                .gap_2()
                .rounded(px(4.))
                .bg(c(theme::BG_ELEVATED))
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(Icon::new(gpui_kit::assets::IconName::Pencil).xsmall().text_color(c(theme::ACCENT_TEXT)))
                .child(div().flex_1().min_w_0().child(crate::i18n::tr("Editing your message. Enter sends it and the answer starts over from there. Esc cancels.")))
                .child(Button::new("ag-edit-cancel").icon(IconName::Close).xsmall().ghost().on_click(cx.listener(|this, _, w, cx| this.agent_cancel_edit(w, cx))))
                .into_any_element(),
        )
    }

    /// The memories used in this turn helped (the user kept the changes).
    fn agent_reinforce_turn(&mut self) {
        let store = memory::Store::default();
        for id in std::mem::take(&mut self.agent.turn_memories) {
            let _ = store.reinforce(&id);
        }
    }
}

/// A 12 px icon in a box one text line (18 px) tall, for rows whose text may wrap.
fn line_icon(icon: Icon) -> Div {
    div().flex_none().h(theme::u(18.)).flex().items_center().child(icon.size(theme::u(12.)))
}

/// One dim, single-line row about memory: brain icon, a label, then the note cut to fit.
fn memory_line(id: impl Into<ElementId>, label: &'static str, rest: String) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(theme::u(20.))
        .gap_1p5()
        .min_w_0()
        .cursor_pointer()
        .text_xs()
        .text_color(c(theme::FG_DIM))
        .hover(|s| s.text_color(c(theme::FG_MUTED)))
        .child(Icon::new(gpui_kit::assets::IconName::Brain).size(theme::u(12.)).flex_none())
        .when(!label.is_empty(), |h| h.child(div().flex_none().child(label)))
        .when(!rest.is_empty(), |h| h.child(div().flex_1().min_w_0().truncate().child(rest)))
}

/// The composer's icon buttons: one 24 px square each, icon centered.
fn icon_btn(id: &'static str, icon: Icon, tip: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(theme::u(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(c(theme::FG_MUTED))
        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
        .child(icon.size(theme::u(14.)))
}

/// Send / stop: the same 24 px square, filled with the accent when it does something.
fn send_btn(id: &'static str, active: bool, tip: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(theme::u(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .cursor_pointer()
        .when(active, |d| d.bg(c(theme::ACCENT)).hover(|s| s.opacity(0.9)))
        .when(!active, |d| d.border_1().border_color(c(theme::BORDER_STRONG)).hover(|s| s.bg(c(theme::BG_HOVER))))
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
}

/// A chip above the input: 22 px tall, the remove button first, then the name.
fn chip_shell(id: SharedString, on: bool) -> Stateful<Div> {
    h_flex()
        .id(id)
        .max_w(theme::u(210.))
        .h(theme::u(22.))
        .gap_0p5()
        .rounded(px(4.))
        .border_1()
        .text_xs()
        .border_color(c(if on { theme::BORDER_STRONG } else { theme::BORDER }))
        .bg(if on { c(theme::BG_EDITOR) } else { gpui_kit::transparent_black() })
        .text_color(c(if on { theme::FG } else { theme::FG_DIM }))
}

fn chip_x(id: SharedString, icon: IconName, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(theme::u(18.))
        .ml_px()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .cursor_pointer()
        .text_color(c(theme::FG_DIM))
        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
        .on_click(f)
        .child(Icon::new(icon).size(theme::u(11.)))
}

/// A 22 px icon button of the Changes card.
fn change_btn(id: &'static str, icon: impl Into<Icon>, tip: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(theme::u(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(c(theme::FG_DIM))
        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
        .child(icon.into().size(theme::u(13.)))
}

/// Smooth-scroll key of the chat log (editors use their entity id).
const CHAT_SCROLL_KEY: u64 = u64::MAX - 1;

/// OpenCode's user config (opencode.jsonc or opencode.json), if it exists.
fn opencode_config_file() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let dir = PathBuf::from(home).join(".config").join("opencode");
    ["opencode.jsonc", "opencode.json"].iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

/// Code from an answer's Insert button, handed to `on_agent_insert_code` (the
/// button's closure can't hold the workbench, so it goes through an action).
static PENDING_INSERT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
mod tests {
    use super::model_groups;

    #[test]
    fn groups_free_models_first() {
        let c = |v: &str, n: &str| (v.to_string(), n.to_string(), String::new());
        let g = model_groups(&[
            c("claude-agent/sonnet", "claude-agent/Sonnet 5.5"),
            c("my-ai-gweh/SigmaBoy", "my-ai-gweh/SigmaBoy"),
            c("opencode/big-pickle", "opencode/Big Pickle"),
            c("opencode/glm-5.3-flash-free", "opencode/GLM-5.3-Flash Free"),
        ]);
        assert_eq!(g[0].0, "free");
        assert_eq!(g[0].1, vec![("opencode/glm-5.3-flash-free".to_string(), "GLM-5.3-Flash".to_string())]);
        assert_eq!(g.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["free", "claude-agent", "my-ai-gweh", "opencode"]);
        assert_eq!(g[1].1[0].1, "Sonnet 5.5");
    }
}
