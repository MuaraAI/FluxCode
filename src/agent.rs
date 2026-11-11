//! The agent panel, over the Agent Client Protocol (ACP): newline-delimited
//! JSON-RPC on the agent's stdio. Which agents exist and how each one starts is
//! `crate::acp`; this file is the protocol and the panel.

use crate::i18n::tr;
use crate::agent_ctx::{self, RemoteSession};
use crate::core;
use crate::theme::{self, c};
use crate::workbench::{tool_button, SideView, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    switch::Switch,
    text::TextView,
    v_flex, Icon, IconName, Selectable, Sizable,
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
    /// A command run quietly for the usage panel (`/usage`, `/context`): its reply
    /// goes there, not into the chat.
    Probe,
}

mod activity;
mod follow;
mod tidy;
pub(crate) mod worktree;
mod catalog;
mod menu;
mod settings_modal;
mod usage;
mod web;
pub(crate) use menu::Menu;

/// Stands for the older ACP session modes (`modes`, `session/set_mode`) among the config options.
const MODE_ID: &str = "__mode";
/// Settings FluxCode passes on the agent's command line (`acp::flag_opts`) have ids
/// starting with this; changing one restarts the agent.
const FLAG_PREFIX: &str = "__flag:";

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
        let provider = value.split_once('/').or_else(|| name.split_once('/')).map(|(p, _)| p).unwrap_or("").to_string();
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
    /// The chat continues with another harness from here (a branch, not a new chat).
    Branch(String),
    /// A web search or a page read, with its sources.
    Web(web::Web),
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
    /// ACP's tool kind: execute, edit, read, fetch, ...
    kind: String,
    /// The command or path, when the title doesn't already show it.
    detail: Option<String>,
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
/// How far back a tool's update is looked for (an agent runs a few tools at a time).
const RECENT: usize = 4096;
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
    /// The row of attachment and context chips above the message, when it is wider than the box.
    chips_scroll: ScrollHandle,
    /// Thinking blocks the user expanded.
    open_thoughts: HashSet<usize>,
    /// The agent accepts image blocks (ACP promptCapabilities.image).
    can_image: bool,
    pub items: Vec<Item>,
    permission: Option<Permission>,
    /// Requests that came while one was on screen (an agent running two tools at once):
    /// each is shown in turn, none is dropped.
    perm_queue: std::collections::VecDeque<Permission>,
    pub changes: Vec<Change>,
    /// Open editors to reload from disk after the agent wrote them (needs a Window).
    pub reloads: Vec<PathBuf>,
    /// Files the agent finished editing, for the editor to follow (follow.rs).
    pub follow_q: Vec<PathBuf>,
    /// The files each edit tool touched, so its "done" update (which often names no
    /// file) still gets its lines counted.
    tool_paths: HashMap<String, Vec<PathBuf>>,
    /// When the user last picked a tab or file themselves: the editor doesn't jump to the
    /// agent's edits for a moment after (follow.rs).
    pub(crate) user_nav: Option<std::time::Instant>,
    /// The replay on screen: its file and final text; the counter stops older ones.
    pub replay: Option<(PathBuf, String)>,
    pub replay_gen: u64,
    /// The Changes card shows only its header.
    changes_folded: bool,
    /// Groups of steps the user opened (by the index of their first item).
    open_groups: HashSet<usize>,
    /// The chat's own git worktree, when it has one (worktree.rs).
    pub worktree: Option<worktree::Worktree>,
    pub worktree_busy: bool,
    pub(crate) input: Entity<TextareaState>,
    /// The chat log as a virtual list: only the messages on screen are laid out, so a
    /// chat of a million items costs a frame what one of twenty does. It follows the
    /// newest text (tail mode) until the reader scrolls up, and again once they scroll
    /// back to the bottom.
    pub(crate) list: ListState,
    /// What the list was last told (see `agent_sync_list`).
    list_seen: std::cell::RefCell<ListSeen>,
    /// The first item in the list. Older ones are not rendered at all: while the reader
    /// follows the newest text they leave on their own, and "Show earlier" pages them
    /// back in, PAGE at a time.
    view_start: std::cell::Cell<usize>,
    /// Items whose height changed by a click (a thought or a group opened).
    remeasure: std::cell::RefCell<Vec<std::ops::Range<usize>>>,
    /// The chat keeps the newest text in view: off once the reader scrolls up, on again
    /// when they scroll back to the bottom or send a message.
    pub(crate) follow: bool,
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
    /// The language note went out in this session.
    hinted: bool,
    /// Signature of the workspace overview the agent has (0: none yet this session).
    ws_sig: u64,
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
    /// An agent being installed: its id and what the installer is doing.
    pub installing: Option<(String, crate::acp::Tracker, Entity<crate::flux_anim::InstallAnim>)>,
    /// A question the agent asked (elicitation), waiting for the user.
    pub ask: Option<crate::agent_ask::Ask>,
    /// Text boxes for typed answers, one per question of a form.
    pub ask_inputs: Vec<Entity<InputState>>,
    /// Boxes to empty before the next frame (a new form arrived).
    pub ask_clear: Vec<usize>,
    /// The first required question still unanswered, after Send.
    pub ask_missing: Option<String>,
    /// Removing an agent: its id, while the user types its name twice.
    pub uninstall: Option<String>,
    pub uninstall_inputs: [Entity<InputState>; 2],
    /// The last install that failed: the agent's id, why, and its stopped animation.
    pub install_err: Option<(String, String, Option<Entity<crate::flux_anim::InstallAnim>>)>,
    /// The agent, model, mode or permission menu that is open.
    pub menu: menu::MenuState,
    /// A command-line setting changed while the agent answered: restart it after the turn.
    relaunch: bool,
    /// Usage and limits: live numbers and the usage panel.
    pub usage: usage::UsageState,
    /// Web searches whose every source is shown.
    pub web_open: HashSet<String>,
    /// When each step (tool call, thought) started and how long it took.
    pub times: activity::Times,
    /// The thought being written now (its key in `times`).
    open_thought: Option<String>,
    /// Where the effort slider's handle was before the last change (it slides from there).
    pub effort_from: std::cell::Cell<Option<f32>>,
    /// The effort handle is held: where it is on the rail (0..1).
    pub effort_drag: std::cell::Cell<Option<f32>>,
    /// Bumped on each pick, so the handle glides even onto the level it was on.
    pub effort_gen: std::cell::Cell<u32>,
    /// The panel's top and the message box's top, in window pixels, as last drawn:
    /// the menus above the box get exactly the room between them.
    pub geom: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    /// The message box's width as last drawn: below `NARROW` the bottom row compacts.
    pub compose_w: std::rc::Rc<std::cell::Cell<f32>>,
    /// The settings dialog's open section, and its focus (for Esc and Tab).
    pub settings_tab: settings_modal::Tab,
    pub settings_focus: FocusHandle,
    _sub: Subscription,
    /// Redraws the removal page as its two name boxes change.
    _sub_un: Vec<Subscription>,
}

/// MuaraAI's Mura harness (`mura`, one Rust binary, github.com/MuaraAI/Mura-Harness),
/// installed on its own next to FluxCode (its README puts it in ~/.local/bin).
pub(crate) fn find_mura() -> Option<PathBuf> {
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

/// The agent picked in the panel's title menu: "auto" or an agent id (`crate::acp`).
static CHOICE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn set_choice(cli: &str) {
    if let Ok(mut g) = CHOICE.lock() {
        *g = cli.to_string();
    }
}

/// The agent the panel runs: the one picked, or with "auto" the first installed one
/// in the known list (Mura, then OpenCode, ...); OpenCode when none is installed.
pub fn agent_id() -> String {
    let c = CHOICE.lock().map(|g| g.clone()).unwrap_or_default();
    if !c.is_empty() && c != "auto" {
        return c;
    }
    crate::acp::installed().into_iter().next().unwrap_or_else(|| "opencode-v2".into())
}

/// A `/command`: FluxCode's own, or one the agent announced (ACP available_commands_update).
#[derive(Clone)]
pub struct SlashCmd {
    pub name: String,
    pub desc: String,
    pub builtin: bool,
    /// Needs text after the name: Enter fills it in instead of sending.
    pub takes_input: bool,
}

fn builtin_commands() -> Vec<SlashCmd> {
    let c = |name: &str, desc: &str, takes_input: bool| SlashCmd { name: name.into(), desc: desc.into(), builtin: true, takes_input };
    vec![
        c("new", crate::i18n::tr("Start a new chat"), false),
        c("history", crate::i18n::tr("Saved chats and agent sessions for this folder"), false),
        c("stop", crate::i18n::tr("Turn the agent off"), false),
    ]
}

/// The running agent's display name. Names are kept for the whole run (a handful of
/// short strings), so call sites can hold a `&'static str`.
pub fn agent_name() -> &'static str {
    static NAMES: std::sync::Mutex<Option<HashMap<String, &'static str>>> = std::sync::Mutex::new(None);
    let id = agent_id();
    let Ok(mut g) = NAMES.lock() else { return "Agent" };
    let map = g.get_or_insert_with(HashMap::new);
    if let Some(n) = map.get(&id) {
        return n;
    }
    let name: &'static str = Box::leak(crate::acp::display_name(&id).into_boxed_str());
    map.insert(id, name);
    name
}

/// Where the OpenCode CLI lives (same search as the Tauri app).
pub(crate) fn find_opencode() -> Option<PathBuf> {
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

/// The command line that starts the running agent in ACP mode.
fn agent_cmd() -> Option<std::process::Command> {
    crate::acp::locate(&agent_id()).map(|l| l.command())
}

/// Mura and OpenCode list their saved sessions (`<cli> session list`); others don't.
fn sessions_cmd() -> Option<std::process::Command> {
    let id = agent_id();
    if id != "mura" && !crate::acp::is_opencode(&id) {
        return None;
    }
    let mut l = crate::acp::locate(&id)?;
    l.args.clear();
    Some(l.command())
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
        let uninstall_inputs = [
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("Type the name"))),
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("Type it again"))),
        ];
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
            chips_scroll: ScrollHandle::new(),
            open_thoughts: HashSet::new(),
            can_image: false,
            items: Vec::new(),
            permission: None,
            perm_queue: Default::default(),
            changes: Vec::new(),
            reloads: Vec::new(),
            follow_q: Vec::new(),
            tool_paths: HashMap::new(),
            user_nav: None,
            replay: None,
            replay_gen: 0,
            changes_folded: false,
            open_groups: HashSet::new(),
            worktree: None,
            worktree_busy: false,
            input,
            list: {
                let l = ListState::new(0, ListAlignment::Top, px(600.));
                l.set_follow_mode(FollowMode::Tail);
                l
            },
            list_seen: Default::default(),
            view_start: Default::default(),
            remeasure: Default::default(),
            follow: true,
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
            hinted: false,
            ws_sig: 0,
            slash: None,
            commands: Vec::new(),
            config: Vec::new(),
            show_options: false,
            editing: None,
            quiet_stop: false,
            last_active: None,
            silent_load: false,
            resume_id: None,
            installing: None,
            install_err: None,
            menu: menu::MenuState::new(window, cx),
            relaunch: false,
            usage: Default::default(),
            web_open: HashSet::new(),
            times: Default::default(),
            open_thought: None,
            effort_from: std::cell::Cell::new(None),
            effort_drag: std::cell::Cell::new(None),
            effort_gen: std::cell::Cell::new(0),
            geom: std::rc::Rc::new(std::cell::Cell::new((0., 0.))),
            compose_w: std::rc::Rc::new(std::cell::Cell::new(0.)),
            settings_tab: Default::default(),
            settings_focus: cx.focus_handle(),
            ask: None,
            ask_inputs: crate::agent_ask::make_inputs(window, cx),
            ask_clear: Vec::new(),
            ask_missing: None,
            uninstall: None,
            _sub_un: uninstall_inputs.iter().map(|i| cx.subscribe(i, |_: &mut Workbench, _, _: &InputEvent, cx| cx.notify())).collect(),
            uninstall_inputs,
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
    pub(crate) fn agent_write(&mut self, msg: Value) {
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
        if let Some(flag) = id.strip_prefix(FLAG_PREFIX) {
            // Always kept: the agent reads it from its command line at the next start.
            let key = format!("{}:{FLAG_PREFIX}{flag}", self.settings.agent_cli);
            self.settings.agent_options.insert(key, value);
            self.settings.save();
            self.agent_relaunch(cx);
            return cx.notify();
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

    /// The session's settings as the panel shows them: the agent's own, plus those
    /// FluxCode passes on its command line when the agent doesn't offer them over ACP.
    pub(crate) fn agent_config_view(&self) -> Vec<ConfigOpt> {
        let cli = self.settings.agent_cli.clone();
        let agent = agent_id();
        let mut v = self.agent.config.clone();
        for f in crate::acp::flag_opts(&agent) {
            if v.iter().any(|o| o.category == f.category) {
                continue;
            }
            let current = self.settings.agent_options.get(&format!("{cli}:{FLAG_PREFIX}{}", f.id)).cloned().unwrap_or_else(|| crate::acp::FLAG_DEFAULT.into());
            let mut choices = crate::acp::flag_choices(&agent, f.id);
            if !choices.iter().any(|c| c.0 == current) {
                choices.push((current.clone(), crate::acp::model_label(&current), String::new()));
            }
            v.push(ConfigOpt { id: format!("{FLAG_PREFIX}{}", f.id), name: f.name.into(), category: f.category.into(), current, choices });
        }
        v
    }

    /// The command-line flags for the saved command-line settings.
    fn agent_flag_args(&self) -> Vec<String> {
        let cli = &self.settings.agent_cli;
        crate::acp::flag_opts(&agent_id())
            .iter()
            .filter_map(|f| {
                let v = self.settings.agent_options.get(&format!("{cli}:{FLAG_PREFIX}{}", f.id))?;
                (v != crate::acp::FLAG_DEFAULT).then(|| [f.flag.to_string(), v.clone()])
            })
            .flatten()
            .collect()
    }

    /// Restart the agent so a command-line setting takes effect, in the same chat:
    /// the session is resumed when the agent can, else it gets the chat as context.
    fn agent_relaunch(&mut self, cx: &mut Context<Self>) {
        match self.agent.status {
            Status::Busy => {
                self.agent.relaunch = true;
                return;
            }
            Status::Ready | Status::Starting => {}
            _ => return,
        }
        self.agent.relaunch = false;
        let session = self.agent.session.clone();
        let chatted = self.agent.items.iter().any(|i| matches!(i, Item::User(_)));
        let can_load = self.agent.can_load;
        self.agent.save_chat();
        self.agent_stop(cx);
        if chatted {
            match session {
                Some(id) if can_load => {
                    self.agent.pending_load = Some(id);
                    self.agent.silent_load = true;
                }
                _ => self.agent.carry = Some(transcript(&self.agent.items)),
            }
        }
        self.agent_start(cx);
    }

    pub(crate) fn agent_set_access(&mut self, level: &str, cx: &mut Context<Self>) {
        if level != "full" {
            self.settings.agent_access_off = level.into();
        }
        self.settings.agent_access = level.into();
        self.settings.save();
        // A request waiting now is answered by the new level when it covers it.
        if level == "full" {
            while let Some(p) = &self.agent.permission {
                let Some(oid) = p.options.iter().find(|o| o.2.starts_with("allow_once")).or_else(|| p.options.iter().find(|o| o.2.starts_with("allow"))).map(|o| o.0.clone()) else { break };
                self.agent_answer_permission(Some(oid), cx);
            }
        }
        cx.notify();
    }

    /// The Full access switch: on, or back to the level it was switched on from.
    pub(crate) fn agent_toggle_full(&mut self, on: bool, cx: &mut Context<Self>) {
        let level = if on { "full" } else { self.settings.access_off() };
        self.agent_set_access(level, cx);
    }

    pub(crate) fn on_agent_show_options(&mut self, _: &crate::workbench::AgentShowOptions, window: &mut Window, cx: &mut Context<Self>) {
        self.agent_open_options(window, cx);
    }

    /// The agent's settings: a dialog over the window, so the chat stays where it was.
    pub(crate) fn agent_open_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.agent.show_options = true;
        self.agent.menu.open = None;
        self.agent.usage.open = false;
        gpui_kit::base::TextSelection::clear(window, cx);
        let h = self.agent.settings_focus.clone();
        h.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn agent_close_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.agent.show_options = false;
        let h = self.agent.input.focus_handle(cx);
        h.focus(window, cx);
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
        self.agent.open_thoughts.retain(|k| *k < ix);
        self.agent.open_groups.retain(|k| *k < ix);
        self.agent.items.push(Item::Note(crate::i18n::tr("Edited: the answer starts over from this message.").into(), false));
        self.agent.carry = self.agent.items.iter().any(|i| matches!(i, Item::User(_) | Item::Text(_))).then(|| transcript(&self.agent.items));
        self.agent.queue = vec![Queued { text, files }];
        self.agent.changes.clear();
        self.agent.permission = None;
        self.agent.perm_queue.clear();
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
                let cwd = self.agent_cwd();
                self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": self.agent_mcp_servers() }));
            }
            Status::Starting => {}
            _ => self.agent_start(cx),
        }
        self.agent.follow = true;
        self.agent_to_end();
        cx.notify();
    }

    fn agent_request(&mut self, kind: Req, method: &str, params: Value) {
        self.agent.next_id += 1;
        let id = self.agent.next_id;
        self.agent.pending.insert(id, kind);
        self.agent_write(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
    }

    /// MCP servers every agent session gets: the browser, when it is set up.
    fn agent_mcp_servers(&self) -> Value {
        let s = &self.settings;
        Value::Array(crate::browser::mcp_server(&s.agent_browser, s.agent_browser_headless, self.root.as_deref()).into_iter().collect())
    }

    /// Set up the agents' browser tools in the background (once), when a browser is
    /// chosen and Node.js is there; sessions after that get the browser.
    pub(crate) fn agent_browser_setup(&mut self, cx: &mut Context<Self>) {
        if !matches!(crate::browser::state(&self.settings.agent_browser), crate::browser::State::NotInstalled(_)) {
            return;
        }
        let task = cx.background_spawn(async move { crate::browser::install() });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            _ = this.update(cx, |this, cx| {
                this.status = match res {
                    Ok(()) => crate::i18n::tr("Browser tools are ready: agents can open and use web pages from their next session.").into(),
                    Err(e) => crate::trf!("Browser tools could not be installed: {e}"; e => e).into(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn agent_start(&mut self, cx: &mut Context<Self>) {
        if matches!(self.agent.status, Status::Starting | Status::Ready | Status::Busy) {
            return;
        }
        self.agent_browser_setup(cx);
        let Some(mut cmd) = agent_cmd() else {
            self.agent.status = Status::Error(if agent_id() == "mura" {
                crate::i18n::tr("Mura isn't installed on this PC (FluxCode looks on PATH and in ~/.local/bin). Build it from github.com/MuaraAI/Mura-Harness, or pick another agent in the menu above.").into()
            } else {
                crate::trf!("{name} isn't installed yet. Download it from the agent menu above, or pick another agent."; name => agent_name()).into()
            });
            cx.notify();
            return;
        };
        cmd.args(self.agent_flag_args());
        cmd.env("NO_COLOR", "1").env("FORCE_COLOR", "0");
        crate::github::apply_env(&mut cmd);
        // Another account: the CLI reads its sign-in from that account's folder.
        if let Some(a) = self.agent_account() {
            let (k, v) = worktree::account_env(&a);
            cmd.env(k, v);
        }
        self.agent.status = Status::Starting;
        self.agent.buf.clear();
        self.agent.log_tail.clear();
        match self.procs.spawn(cmd, Some(self.agent_cwd().as_path())) {
            Ok(id) => {
                self.agent.proc_id = Some(id);
                self.agent_request(
                    Req::Initialize,
                    "initialize",
                    json!({
                        "protocolVersion": 1,
                        // Elicitation: the agent may ask the user questions (agent_ask.rs).
                        "clientCapabilities": { "fs": { "readTextFile": true, "writeTextFile": true }, "terminal": false, "elicitation": { "form": {}, "url": {} } },
                        "clientInfo": { "name": "FluxCode", "version": crate::update::BUILD }
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
        self.agent.perm_queue.clear();
        self.agent.ask = None;
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
        match text.as_str() {
            "/new" => return self.agent_new_chat(cx),
            "/history" => {
                self.agent.show_history = true;
                        self.agent_load_remote_list(cx);
                return cx.notify();
            }
            "/stop" => return self.agent_set_on(false, cx),
            "/usage" | "/context" if self.agent_has_usage() => return self.agent_usage_open(cx),
            _ => {}
        }
        if let Some(ix) = self.agent.editing.take() {
            return self.agent_send_edit(ix, text, cx);
        }
        let files = std::mem::take(&mut self.agent.attachments);
        self.agent.queue.push(Queued { text, files });
        self.agent.follow = true;
        self.agent_to_end();
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
        prompt.extend(self.agent_context_blocks(cx));
        self.agent.status = Status::Busy;
        self.agent_request(Req::Prompt, "session/prompt", json!({ "sessionId": session, "prompt": prompt }));
    }

    /// The thought being written ended (something else came, or the turn did).
    fn agent_close_thought(&mut self) {
        if let Some(k) = self.agent.open_thought.take() {
            if let Some((start, end @ None)) = self.agent.times.get_mut(&k) {
                *end = Some(start.elapsed());
            }
        }
    }

    /// Run one of the agent's own commands without showing it in the chat.
    fn agent_send_probe(&mut self, command: &str) {
        let Some(session) = self.agent.session.clone() else { return };
        self.agent.status = Status::Busy;
        self.agent_request(Req::Probe, "session/prompt", json!({ "sessionId": session, "prompt": [{ "type": "text", "text": command }] }));
    }

    fn agent_cancel(&mut self, cx: &mut Context<Self>) {
        if let (Status::Busy, Some(s)) = (&self.agent.status, self.agent.session.clone()) {
            self.agent_write(json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": s } }));
            let waiting: Vec<Permission> = self.agent.permission.take().into_iter().chain(self.agent.perm_queue.drain(..)).collect();
            for p in waiting {
                self.agent_write(json!({ "jsonrpc": "2.0", "id": p.rpc_id, "result": { "outcome": { "outcome": "cancelled" } } }));
            }
            if self.agent.ask.is_some() {
                self.agent_answer_ask(None, cx);
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
        self.agent_follow();
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
                Req::Probe => {
                    self.agent.status = Status::Ready;
                    self.agent_probe_done(Some(m), cx);
                    self.agent_flush_queue(cx);
                    self.agent_probe_next(cx);
                }
                Req::LoadSession if self.agent.silent_load => {
                    self.agent.silent_load = false;
                    self.agent.loading_id = None;
                    self.agent.carry = Some(transcript(&self.agent.items));
                    let cwd = self.agent_cwd();
                    self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": self.agent_mcp_servers() }));
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
                let cwd = self.agent_cwd();
                match self.agent.pending_load.take() {
                    Some(id) if self.agent.can_load => self.agent_load_session(id),
                    _ => {
                        if std::mem::take(&mut self.agent.silent_load) {
                            self.agent.carry = Some(transcript(&self.agent.items));
                        }
                        self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": self.agent_mcp_servers() }))
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
                // Effort starts at medium: good results with fewer tokens. A level the
                // user picked once is kept instead (above).
                let effort = self
                    .agent
                    .config
                    .iter()
                    .find(|o| o.category == "thought_level")
                    .filter(|o| !self.settings.agent_options.contains_key(&format!("{harness}:{}", o.id)) && o.current != "medium" && o.choices.iter().any(|c| c.0 == "medium"))
                    .map(|o| o.id.clone());
                if let Some(id) = effort {
                    self.agent_set_option(&id, "medium".into(), false, cx);
                }
                self.agent.status = Status::Ready;
                self.agent_schedule_idle(cx);
                self.agent_flush_queue(cx);
                self.agent_probe_next(cx);
            }
            Req::SetOption => self.agent_take_config(&result),
            Req::Probe => {
                self.agent.status = Status::Ready;
                self.agent_probe_done(None, cx);
                self.agent_flush_queue(cx);
                self.agent_probe_next(cx);
            }
            Req::Prompt => {
                self.agent_close_thought();
                self.agent_recount();
                let stop = result.get("stopReason").and_then(Value::as_str).unwrap_or("end_turn");
                let quiet = std::mem::take(&mut self.agent.quiet_stop);
                if stop != "end_turn" && !quiet {
                    self.agent.items.push(Item::Note(stop.replace('_', " "), false));
                }
                self.agent.permission = None;
                self.agent.perm_queue.clear();
                self.agent.status = Status::Ready;
                self.agent.save_chat();
                self.agent_schedule_idle(cx);
                self.refresh_tree(cx);
                if std::mem::take(&mut self.agent.relaunch) {
                    return self.agent_relaunch(cx);
                }
                self.agent_flush_queue(cx);
                self.agent_probe_next(cx);
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
        if u.get("sessionUpdate").and_then(Value::as_str) == Some("usage_update") {
            self.agent.usage.live.get_or_insert_with(Default::default).take(u);
            return;
        }
        // A quiet command's reply goes to the usage panel.
        if let Some((_, buf)) = self.agent.usage.probe.as_mut() {
            if u.get("sessionUpdate").and_then(Value::as_str) == Some("agent_message_chunk") {
                buf.push_str(u.pointer("/content/text").and_then(Value::as_str).unwrap_or(""));
            }
            return;
        }
        if self.agent.silent_load {
            return;
        }
        let text = |u: &Value| u.pointer("/content/text").and_then(Value::as_str).unwrap_or("").to_string();
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        let kind_now = u.get("sessionUpdate").and_then(Value::as_str).unwrap_or("");
        if kind_now != "agent_thought_chunk" {
            self.agent_close_thought();
        }
        match kind_now {
            "tool_call" => {
                if let Some(id) = s(u, "toolCallId") {
                    self.agent.times.entry(id).or_insert((std::time::Instant::now(), None));
                }
            }
            "tool_call_update" if matches!(s(u, "status").as_deref(), Some("completed" | "failed")) => {
                if let Some((start, end @ None)) = s(u, "toolCallId").and_then(|id| self.agent.times.get_mut(&id)) {
                    *end = Some(start.elapsed());
                }
            }
            _ => {}
        }
        match u.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
            "agent_message_chunk" => self.agent.append(false, &text(u)),
            // Only while an old session replays; live prompts are shown when sent.
            "user_message_chunk" if self.agent.loading_id.is_some() => self.agent.append_user(&text(u)),
            "agent_thought_chunk" => {
                self.agent.append(true, &text(u));
                let key = format!("think-{}", self.agent.items.len().saturating_sub(1));
                self.agent.times.entry(key.clone()).or_insert((std::time::Instant::now(), None));
                self.agent.open_thought = Some(key);
            }
            "tool_call" if web::hidden_tool(u) => {}
            "tool_call" if web::is_web(u) => self.agent.items.push(Item::Web(web::Web::new(u))),
            // Updates go to recent items: the search stops RECENT items back, so a tool
            // update costs the same in a chat of any length.
            "tool_call_update" if self.agent.items.iter().rev().take(RECENT).any(|i| matches!(i, Item::Web(w) if Some(w.id.as_str()) == u.get("toolCallId").and_then(Value::as_str))) => {
                let id = u.get("toolCallId").and_then(Value::as_str).unwrap_or("");
                if let Some(Item::Web(w)) = self.agent.items.iter_mut().rev().take(RECENT).find(|i| matches!(i, Item::Web(w) if w.id == id)) {
                    w.take(u);
                }
            }
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
                let known = self.agent.items.iter().rev().take(RECENT).find_map(|i| match i {
                    Item::Tool { id: tid, kind, .. } if *tid == id => Some(kind.clone()),
                    _ => None,
                });
                let kind = s(u, "kind").or(known).unwrap_or_default();
                self.agent_track_edit(u, &kind, s(u, "status").as_deref() == Some("completed"));
                for item in self.agent.items.iter_mut().rev().take(RECENT) {
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
                // What exactly runs: the command or the path, when the agent says it.
                let detail = ["/toolCall/rawInput/command", "/toolCall/rawInput/cmd", "/toolCall/rawInput/filePath", "/toolCall/rawInput/file_path", "/toolCall/rawInput/path", "/toolCall/locations/0/path"]
                    .iter()
                    .find_map(|p| params.pointer(p).and_then(Value::as_str))
                    .filter(|d| !d.trim().is_empty() && !title.contains(d.trim()))
                    .map(str::to_string);
                let req = Permission { rpc_id: id.clone(), title, options, kind: kind.to_string(), detail };
                if self.agent.permission.is_some() {
                    self.agent.perm_queue.push_back(req);
                } else {
                    self.agent.permission = Some(req);
                }
                // The agent waits for this: make sure it is seen.
                if self.settings.agent_right {
                    self.agent_open = true;
                } else if !(self.sidebar_open && self.view == SideView::Agent) {
                    self.view = SideView::Agent;
                    self.sidebar_open = true;
                }
                self.agent.follow = true;
                self.agent_to_end();
                if self.settings.agent_notify {
                    crate::wintitle::flash_attention();
                }
                cx.notify();
            }
            ("elicitation/create", _) => self.agent_on_elicitation(id.clone(), &params, cx),
            ("fs/read_text_file", Some(p)) => {
                // Unsaved editor text wins over the file on disk.
                let text = match self.docs.iter().find(|d| d.path == p) {
                    Some(d) => Ok(d.text(cx)),
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
                    self.agent_follow_edit(p.clone());
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
        self.agent_fresh_view();
        self.agent.chat_id = None;
        self.agent.carry = None;
        self.agent.show_history = false;
        if self.agent.status == Status::Ready && self.agent.proc_id.is_some() {
            self.agent.session = None;
            self.agent.status = Status::Starting;
            let cwd = self.agent_cwd();
            self.agent_request(Req::NewSession, "session/new", json!({ "cwd": cwd, "mcpServers": self.agent_mcp_servers() }));
        }
        cx.notify();
    }

    /// A chat starts clean: what was opened, timed or changed belonged to the last one
    /// (its changes count as kept, as with Keep all).
    fn agent_fresh_view(&mut self) {
        self.agent.open_thoughts.clear();
        self.agent.open_groups.clear();
        self.agent.times.clear();
        self.agent.changes.clear();
        self.agent.changes_folded = false;
    }

    /// Reopen a saved chat. OpenCode can't resume the old session, so the next
    /// message carries the earlier ones as context.
    fn agent_open_chat(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(chat) = self.agent.history.iter().find(|c| c.id == id).cloned() else { return };
        self.agent_fresh_view();
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
        self.agent.follow = true;
        self.agent_to_end();
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
                    .child(self.render_agent_picker(cx))
                    .child(self.render_agent_switch(cx))
                    .child(div().flex_1())
                    .child(self.render_agent_isolation(cx))
                    .child(self.render_agent_tools(cx))
                    .child(tool_button("ag-dock-close", IconName::Close, crate::i18n::tr("Hide (Ctrl+L)"), cx, |this, _, cx| {
                        this.agent_open = false;
                        cx.notify();
                    })),
            )
            .children(self.render_worktree_strip(cx))
            // Filling the window, the chat keeps a readable width in the middle.
            .child(h_flex().flex_1().min_h_0().justify_center().child(div().h_full().w_full().when(self.agent_full && self.settings.agent_right, |d| d.max_w(theme::u(860.))).child(self.render_agent(window, cx))))
            .into_any_element()
    }

    /// Switch the harness from the title menu. A running agent is stopped first.
    fn agent_use(&mut self, cli: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.agent_cli == cli {
            return;
        }
        let old = agent_name();
        // The chat under the pointer changes: nothing in it stays selected.
        gpui_kit::base::TextSelection::clear(window, cx);
        self.agent.save_chat();
        self.agent_stop(cx);
        self.agent.install_err = None;
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
        self.agent.relaunch = false;
        self.agent.config.clear();
        // The context and limits were the old agent's.
        self.agent.usage = Default::default();
        self.agent.input.update(cx, |s, cx| s.set_placeholder(crate::trf!("Ask {name}…"; name => name), window, cx));
        if !self.agent.queue.is_empty() {
            self.agent_start(cx);
        }
        cx.notify();
    }

    pub(crate) fn on_agent_use_opencode(&mut self, _: &crate::workbench::AgentUseOpenCode, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_use("opencode-v2", w, cx);
    }

    pub(crate) fn on_agent_use_mura(&mut self, _: &crate::workbench::AgentUseMura, w: &mut Window, cx: &mut Context<Self>) {
        self.agent_use("mura", w, cx);
    }

    /// The panel title: the agent's icon and name, opening the agent menu (`menu.rs`):
    /// agents on this PC with whether each is on, then the ones to download.
    pub(crate) fn render_agent_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = agent_id();
        let open = self.agent.menu.open == Some(Menu::Agent);
        let mut button = Button::new("ag-pick").label(agent_name()).ghost().xsmall().dropdown_caret(true).selected(open).tooltip(tr("Choose the agent"));
        if let Some(path) = crate::acp::icon_asset(&current) {
            button = button.icon(Icon::new(IconName::Bot).path(path));
        }
        button.on_click(cx.listener(|this, _, w, cx| {
            crate::acp::refresh_registry_soon();
            this.agent_menu_toggle(Menu::Agent, w, cx)
        }))
    }

    /// Install an agent from the ACP registry, off the UI thread, then switch to it.
    /// The panel shows the install animation, which reads the same tracker.
    pub(crate) fn agent_install(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.agent.installing.is_some() {
            return;
        }
        if id == "mura" {
            // Mura is built from its repository; it is not in the registry.
            cx.open_url("https://github.com/MuaraAI/Mura-Harness");
            return;
        }
        let tracker = crate::acp::Tracker::default();
        let title: SharedString = crate::trf!("Installing {name}"; name => crate::acp::display_name(&id)).into();
        let anim = cx.new(|cx| crate::flux_anim::InstallAnim::new(tracker.clone(), title, cx));
        self.agent.installing = Some((id.clone(), tracker.clone(), anim));
        self.agent.install_err = None;
        cx.notify();
        let id2 = id.clone();
        let task = cx.background_spawn(async move {
            let find = |reg: &[crate::acp::Entry]| reg.iter().find(|e| e.id == id2).cloned();
            let entry = match find(&crate::acp::registry()) {
                Some(e) if !crate::acp::registry_stale() => Ok(e),
                _ => {
                    // Fresh versions when the saved list is old; the saved one when offline.
                    let fetched = crate::acp::refresh_registry();
                    crate::acp::fetch_icons_soon();
                    match (find(&crate::acp::registry()), fetched) {
                        (Some(e), _) => Ok(e),
                        (None, Err(e)) => Err(format!("{}: {e}", tr("Could not read the ACP registry"))),
                        (None, Ok(())) => Err(tr("This agent is no longer in the ACP registry.").to_string()),
                    }
                }
            };
            let res = entry.and_then(|e| crate::acp::install(&e, &tracker));
            tracker.phase(if res.is_ok() { crate::acp::Phase::Done } else { crate::acp::Phase::Failed });
            res
        });
        cx.spawn_in(window, async move |this, cx| {
            let res = task.await;
            if res.is_ok() {
                // Long enough for the check to draw in before the chat appears.
                cx.background_executor().timer(std::time::Duration::from_millis(800)).await;
            }
            _ = this.update_in(cx, |this, window, cx| {
                let anim = this.agent.installing.take().map(|(_, _, a)| a);
                match res {
                    Ok(_) => this.agent_use(&id, window, cx),
                    Err(e) => this.agent.install_err = Some((id.clone(), e, anim)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Which agent, model, effort and mode the panel runs now (for live share's AI labels).
    pub(crate) fn agent_attribution(&self) -> crate::collab::proto::AiInfo {
        let view = self.agent_config_view();
        let pick = |cat: &str| view.iter().find(|o| o.category == cat).map(|o| o.current_name().rsplit('/').next().unwrap_or("").to_string()).unwrap_or_default();
        crate::collab::proto::AiInfo { agent: agent_name().to_string(), model: pick("model"), effort: pick("thought_level"), mode: pick("mode") }
    }

    /// Ask before removing an agent: its name, typed twice.
    pub(crate) fn agent_ask_uninstall(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.agent.uninstall = Some(id.to_string());
        for i in &self.agent.uninstall_inputs {
            i.update(cx, |s, cx| s.set_value("", window, cx));
        }
        let h = self.agent.uninstall_inputs[0].focus_handle(cx);
        h.focus(window, cx);
        cx.notify();
    }

    fn render_uninstall(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let name = crate::acp::display_name(id);
        let typed: Vec<String> = self.agent.uninstall_inputs.iter().map(|i| i.read(cx).value().to_string()).collect();
        let ok = typed.iter().all(|t| t.trim() == name);
        let folder = crate::acp::agents_dir().join(id);
        let id2 = id.to_string();
        let mismatch = typed.iter().any(|t| !t.trim().is_empty() && !name.starts_with(t.trim()));
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(agent_glyph(id, 20.))
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("Remove {name}?"; name => name))),
            )
            .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(crate::trf!("This deletes {path}: the program and its files. Your chats stay. To be sure it is the right agent, type {name} twice."; path => folder.display(), name => name)))
            .child(v_flex().gap_2().children(self.agent.uninstall_inputs.iter().map(|i| Input::new(i).small())))
            .when(mismatch, |v| v.child(div().text_xs().text_color(c(theme::WARN)).child(crate::trf!("That is not “{name}”. Check the spelling (capital letters count)."; name => name))))
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("un-cancel").label(tr("Cancel")).small().ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.agent.uninstall = None;
                        cx.notify();
                    })))
                    .child(
                        Button::new("un-go")
                            .label(crate::trf!("Remove {name}"; name => name))
                            .small()
                            .danger()
                            .when(!ok, |b| b.ghost())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                // Checked again at the click: the name, exactly, in both boxes.
                                let name = crate::acp::display_name(&id2);
                                if !this.agent.uninstall_inputs.iter().all(|i| i.read(cx).value().trim() == name) {
                                    return;
                                }
                                this.agent.uninstall = None;
                                this.agent_uninstall(&id2, w, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Delete an agent FluxCode installed, and fall back to another one.
    fn agent_uninstall(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if agent_id() == id {
            self.agent_stop(cx);
        }
        if let Err(e) = crate::acp::uninstall(id) {
            self.agent.install_err = Some((id.to_string(), e, None));
        }
        if self.settings.agent_cli == id {
            self.agent_use("auto", window, cx);
        }
        cx.notify();
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
            .when(self.agent_has_usage(), |h| {
                h.child(
                    Button::new("ag-usage-btn")
                        .icon(Icon::new(gpui_kit::assets::IconName::Gauge))
                        .xsmall()
                        .ghost()
                        .selected(self.agent.usage.open)
                        .tooltip(if self.agent_model_free().is_some() { crate::i18n::tr("Usage: context (free model, no limits)") } else { crate::i18n::tr("Usage: limits, context and cost") })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.agent.usage.open {
                                this.agent.usage.open = false;
                                cx.notify();
                            } else {
                                this.agent_usage_open(cx);
                            }
                        })),
                )
            })
            .child(
                Button::new("ag-history-btn")
                    .icon(if self.agent.show_history { gpui_kit::component::Icon::new(gpui_kit::assets::IconName::MessageSquareCode) } else { gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Clock) })
                    .xsmall()
                    .ghost()
                    .tooltip(if self.agent.show_history { crate::i18n::tr("Back to the chat") } else { crate::i18n::tr("Saved chats") })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.show_history = !this.agent.show_history;
                        if this.agent.show_history {
                            this.agent_load_remote_list(cx);
                        }
                        cx.notify();
                    })),
            )
            .when(right, |h| {
                let full = self.agent_full;
                h.child(
                    Button::new("ag-full")
                        .icon(if full { IconName::Minimize } else { IconName::Maximize })
                        .xsmall()
                        .ghost()
                        .selected(full)
                        .tooltip(if full { crate::i18n::tr("Back beside the editor") } else { crate::i18n::tr("Fill the window") })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.agent_full = !this.agent_full;
                            cx.notify();
                        })),
                )
            })
            .child(
                Button::new("ag-dock")
                    .icon(if right { IconName::PanelLeft } else { IconName::PanelRight })
                    .xsmall()
                    .ghost()
                    .tooltip(if right { crate::i18n::tr("Move to the left side bar") } else { crate::i18n::tr("Move to the right side") })
                    .on_click(cx.listener(|this, _, w, cx| this.agent_move_dock(w, cx))),
            )
            .child(
                Button::new("ag-settings-btn")
                    .icon(Icon::new(gpui_kit::assets::IconName::Settings))
                    .xsmall()
                    .ghost()
                    .selected(self.agent.show_options)
                    .tooltip(crate::i18n::tr("Agent settings: model, permissions, browser"))
                    .on_click(cx.listener(|this, _, w, cx| {
                        if this.agent.show_options {
                            this.agent_close_options(w, cx);
                        } else {
                            this.agent_open_options(w, cx);
                        }
                    })),
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
        self.agent.permission = self.agent.perm_queue.pop_front();
        cx.notify();
    }

    /// Reload open editors the agent changed on disk (render has the Window).
    pub(crate) fn apply_agent_reloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for p in std::mem::take(&mut self.agent.reloads) {
            // Live share: this rewrite is the agent's, and is labelled so.
            self.collab_mark_ai(&p);
            if let (Some(d), Ok(f)) = (self.docs.iter_mut().find(|d| d.path == p), core::read_text(&p)) {
                d.crlf = f.crlf;
                d.editor.update(cx, |s, cx| s.set_value(f.text, window, cx));
                d.dirty = false;
            }
            // set_value fires no change event: send the agent's rewrite to the session now.
            self.collab_on_path(&p, cx);
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
        let cwd = self.agent_cwd();
        let abs = |p: &str| -> PathBuf {
            let p = PathBuf::from(p);
            if p.is_absolute() { p } else { cwd.join(p) }
        };
        let mut touched: Vec<PathBuf> = Vec::new();
        for c in u.get("content").and_then(Value::as_array).into_iter().flatten() {
            if c.get("type").and_then(Value::as_str) != Some("diff") {
                continue;
            }
            let Some(p) = c.get("path").and_then(Value::as_str).map(abs) else { continue };
            if !self.agent.changes.iter().any(|ch| ch.path == p) {
                let before = before_edit(std::fs::read_to_string(&p).ok(), c.get("oldText").and_then(Value::as_str), c.get("newText").and_then(Value::as_str).unwrap_or(""));
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
        let id = u.get("toolCallId").and_then(Value::as_str).unwrap_or("").to_string();
        if !done {
            let known = self.agent.tool_paths.entry(id).or_default();
            for p in touched {
                if !known.contains(&p) {
                    known.push(p);
                }
            }
            return;
        }
        for p in self.agent.tool_paths.remove(&id).unwrap_or_default() {
            if !touched.contains(&p) {
                touched.push(p);
            }
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
            if self.agent.changes.iter().any(|ch| ch.path == p) {
                self.agent_follow_edit(p.clone());
            }
            if self.docs.iter().any(|d| d.path == p) && !self.agent.reloads.contains(&p) {
                self.agent.reloads.push(p);
            }
        }
    }

    /// Count every changed file's lines again from the disk (end of a turn).
    fn agent_recount(&mut self) {
        self.agent.tool_paths.clear();
        self.agent.changes.retain_mut(|ch| {
            let after = std::fs::read_to_string(&ch.path).ok();
            (ch.added, ch.removed) = crate::diff::counts(ch.before.as_deref().unwrap_or(""), after.as_deref().unwrap_or(""));
            // Read but not changed after all.
            !(ch.added == 0 && ch.removed == 0 && ch.before.is_some() && after.is_some())
        });
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

    /// Context blocks for a prompt: the chips that are on, read-only mode, and a
    /// one-time note on the language to answer in.
    fn agent_context_blocks(&mut self, cx: &mut Context<Self>) -> Vec<Value> {
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
                    let mut text = crate::ws_context::overview(&inputs);
                    if let Some(w) = &self.agent.worktree {
                        text.push_str(&format!(
                            "\nYou work in a git worktree of this repository: {} (branch {}). Your working directory is there; \
                             read and edit the files in it, not in the main folder (the user merges the branch when the work is done).\n",
                            w.path.display(),
                            w.branch
                        ));
                    }
                    if let Some(notes) = self.ext_agent_notes() {
                        text.push_str(&notes);
                    }
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                "file" => {
                    if let Some(d) = self.active.map(|ix| &self.docs[ix]) {
                        let body = d.text(cx);
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
        if self.settings.access() == "read" {
            blocks.push(json!({ "type": "text", "text":
                "FluxCode is in read-only mode for this message: read files and explain, but do not edit files or run commands \
                 that change anything. If a change is needed, describe it or show it as a code block." }));
        }
        if !self.agent.hinted {
            self.agent.hinted = true;
            let lang = if crate::i18n::is_id() {
                "FluxCode's interface is in Indonesian: when the user's language is unclear, answer in Indonesian. "
            } else {
                "FluxCode's interface is in English. "
            };
            let text = "FluxCode note: answer in the language the user writes in.";
            blocks.push(json!({ "type": "text", "text": format!("{lang}{text}") }));
        }
        blocks
    }

    // ------------------------------------------------------------ OpenCode / Mura sessions

    fn agent_load_session(&mut self, id: String) {
        let cwd = self.agent_cwd();
        self.agent.loading_id = Some(id.clone());
        self.agent.status = Status::Starting;
        self.agent_request(Req::LoadSession, "session/load", json!({ "sessionId": id, "cwd": cwd, "mcpServers": self.agent_mcp_servers() }));
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
        let Some(root) = self.root.clone() else {
            self.agent.remote_state = 2;
            return;
        };
        let Some(mut cmd) = sessions_cmd() else {
            self.agent.remote_state = 2;
            self.agent.remote.clear();
            // Agents other than Mura and OpenCode keep no session list FluxCode can read.
            if crate::acp::locate(&agent_id()).is_none() {
                self.agent.remote_err = Some(format!("{} isn't installed, so its sessions can't be read.", agent_name()));
            }
            return;
        };
        let harness = agent_name();
        cmd.args(["session", "list", "--format", "json", "-n", "40"]).current_dir(&root).env("NO_COLOR", "1");
        if crate::acp::is_opencode(&agent_id()) {
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
                    .child(Icon::new(gpui_kit::assets::IconName::MessagesSquare).size(theme::u(14.)).text_color(c(theme::FG_DIM)))
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
        if self.agent.permission.is_some() {
            return Some((theme::WARN, crate::i18n::tr("Waiting for you").into()));
        }
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

    /// What goes with the message, one chip each. A closed chip goes away; when some
    /// are closed, a "+" at the end brings them back.
    fn render_context_chips(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let chips = self.agent_chips(cx);
        let hidden: Vec<(String, String)> = chips.iter().filter(|c| !c.3).map(|c| (c.0.clone(), c.1.clone())).collect();
        let mut out: Vec<AnyElement> = chips
            .into_iter()
            .filter(|c| c.3)
            .enumerate()
            .map(|(i, (key, label, icon, _))| {
                let close_key = key.clone();
                chip_shell(SharedString::from(format!("ag-chip-{i}")), true)
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("ag-chip-open-{i}")))
                            .flex_1()
                            .min_w_0()
                            .pl_1p5()
                            .gap_1()
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Go to it")).build(w, cx))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                cx.stop_propagation();
                                this.agent_chip_go(&key, w, cx);
                            }))
                            .child(icon.size(theme::u(12.)).flex_none())
                            .child(div().min_w_0().truncate().child(label)),
                    )
                    .child(
                        chip_x(SharedString::from(format!("ag-chip-x-{i}")), IconName::Close, cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.agent.off_ctx.insert(close_key.clone());
                            cx.notify();
                        }))
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Leave it out of the message")).build(w, cx)),
                    )
                    .into_any_element()
            })
            .collect();
        if !hidden.is_empty() {
            let names = hidden.iter().map(|h| h.1.as_str()).collect::<Vec<_>>().join(", ");
            let tip: SharedString = crate::trf!("Send again: {names}"; names => names).into();
            let keys: Vec<String> = hidden.into_iter().map(|h| h.0).collect();
            out.push(
                div()
                    .id("ag-chip-restore")
                    .flex_none()
                    .size(theme::u(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .cursor_pointer()
                    .text_color(c(theme::FG_DIM))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                    .child(Icon::new(IconName::Plus).size(theme::u(12.)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        for k in &keys {
                            this.agent.off_ctx.remove(k);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        out
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
            format!(" · {}", crate::trf!("{name}'s own commands appear once it is on"; name => name))
        } else {
            String::new()
        };
        Some(v.child(div().px_3().pt_1().text_xs().text_color(c(theme::FG_DIM)).child(format!("{}{more}", tr("↑↓ choose · Tab fills · Enter runs · Esc closes")))).into_any_element())
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

    pub(crate) fn render_agent(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let body = self.render_agent_body(window, cx);
        let geom = self.agent.geom.clone();
        let id = cx.entity_id();
        // The panel's top, for the room the menus get (see `geom`).
        let probe = canvas(
            move |b, _, cx| {
                let y = f32::from(b.origin.y);
                let (top, box_top) = geom.get();
                if (top - y).abs() > 0.5 {
                    geom.set((y, box_top));
                    cx.defer(move |cx| cx.notify(id));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size(px(0.));
        div()
            .relative()
            .size_full()
            .child(body)
            .child(probe)
            // The agent menu opens under the panel title, inside the panel.
            .children(self.render_agent_menu(true, window, cx).map(|m| div().absolute().top_1().left_2().right_2().child(m)))
            .children(self.render_usage_modal(cx))
            .into_any_element()
    }

    fn render_agent_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
        if a.show_history {
            return v_flex().size_full().child(header).child(self.render_history(cx)).into_any_element();
        }
        if let Some(id) = &a.uninstall {
            return self.render_uninstall(id, cx);
        }

        self.agent_sync_list();
        let error = match &a.status {
            Status::Error(e) => Some(div().mx_3().mb_2().p_2().rounded(px(4.)).border_1().border_color(c(theme::DANGER)).bg(c(theme::BG_ELEVATED)).text_xs().text_color(c(theme::DANGER)).child(e.clone())),
            _ => None,
        };
        let log: AnyElement = if a.items.is_empty() {
            v_flex()
                .id("ag-log")
                .size_full()
                .px_3()
                .pb_3()
                .overflow_y_scroll()
                .children(error.is_none().then(|| self.render_agent_welcome(on, cx)))
                .children(error)
                .into_any_element()
        } else {
            v_flex()
                .size_full()
                .children(error)
                .child(
                    list(
                        a.list.clone(),
                        cx.processor(|this, ix: usize, window, cx| {
                            let start = this.agent.view_start.get();
                            match (start > 0, ix) {
                                (true, 0) => this.render_agent_pager(start, cx),
                                (pager, _) => this.render_agent_item(start + ix - pager as usize, window, cx),
                            }
                        }),
                    )
                    .flex_1()
                    .min_h_0(),
                )
                .into_any_element()
        };
        let permission = a.permission.as_ref().map(|p| self.render_permission(p, cx));
        
        // What the agent changed this turn, like a worktree: each file with its
        // lines added / removed, a diff, and undo.
        let folded = a.changes_folded;
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
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.agent.user_nav = Some(std::time::Instant::now());
                                this.open_file(path.clone(), w, cx)
                            }))
                            .child(Icon::new(icon).flex_none().size(theme::u(13.)).text_color(theme::tint(tint)))
                            .child(div().min_w_0().truncate().text_color(c(theme::FG)).child(name))
                            .when(ch.before.is_none(), |h| h.child(div().flex_none().text_color(c(theme::GREEN)).child(crate::i18n::tr("new"))))
                            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(dir)),
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
                        .when(!folded, |h| h.border_b_1())
                        .border_color(c(theme::BORDER))
                        .child(
                            h_flex()
                                .id("ag-ch-fold")
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .gap_2()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.agent.changes_folded = !this.agent.changes_folded;
                                    cx.notify();
                                }))
                                .child(Icon::new(if folded { IconName::ChevronRight } else { IconName::ChevronDown }).flex_none().size(theme::u(12.)).text_color(c(theme::FG_DIM)))
                                .child(Icon::new(gpui_kit::assets::IconName::FileDiff).flex_none().size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
                                .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::i18n::tr("Changes")))
                                .child(div().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(files))
                                .child(counts(add, del))
                                .child(div().flex_1()),
                        )
                        .child(change_btn("ag-undo-all", gpui_kit::assets::IconName::Undo2, crate::i18n::tr("Undo all")).on_click(cx.listener(|this, _, _, cx| {
                            for i in (0..this.agent.changes.len()).rev() {
                                this.agent_undo(i, cx);
                            }
                        })))
                        .child(change_btn("ag-keep-all", IconName::Check, tr("Keep all")).on_click(cx.listener(|this, _, _, cx| {
                            this.agent.changes.clear();
                            cx.notify();
                        }))),
                )
                .when(!folded, |v| v.child(v_flex().id("ag-changes").max_h(theme::u(156.)).overflow_y_scroll().py_0p5().children(rows)))
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
                    .child(log),
            )
            .children(self.render_agent_ask(cx))
            .when_some(permission, |v, p| v.child(p))
            .when_some(changes, |v, ch| v.child(ch))
            .children(self.render_queue(cx))
            .children(self.render_slash_popup(cx))
            .children(self.render_edit_strip(cx))
            .children(self.render_mention_popup(cx))
            .child(self.render_composer(window, cx))
            .into_any_element()
    }


    /// One chat item, for the virtual list: the speaker line when an answer starts, the
    /// group line above a run of steps, then the item (or nothing, folded in a group).
    fn render_agent_item(&mut self, i: usize, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.agent;
        let Some(item) = a.items.get(i) else { return div().into_any_element() };
        let name = agent_name();
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
        let busy = a.status == Status::Busy;
        let run_of = |k: usize| self.agent_run_of(k);
        // The group header sits above the first step of a group, so the rail joins them.
        let joined_up = |k: usize| is_step(k.checked_sub(1).and_then(|j| a.items.get(j))) || run_of(k).is_some_and(|r| r.0 == k);
        let last_item = i + 1 == a.items.len();
        // Rows get the list's width: relative widths inside (the user bubble) are of the panel.
        let mut col = v_flex().w_full().min_w_0().px_3().pt_2().when(last_item, |v| v.pb_3());
        // An answer starts after the user's message (or a branch), notes aside.
        let answer = !matches!(item, Item::User(_) | Item::Note(..) | Item::Branch(_));
        let prev = a.items[..i].iter().rev().find(|x| !matches!(x, Item::Note(..)));
        if answer && matches!(prev, None | Some(Item::User(_) | Item::Branch(_))) {
            col = col.child(speaker(name, Icon::new(gpui_kit::assets::IconName::MessageSquareCode)));
        }
        let top = a.view_start.get();
        if let Some((first, last)) = run_of(i) {
            let head = first.max(top);
            let open = a.open_groups.contains(&first);
            // While the agent works on the last group, its newest step stays in view.
            let live_tail = busy && last + 1 == a.items.len();
            if i == head {
                let members = &a.items[first..=last];
                let thoughts = members.iter().filter(|m| matches!(m, Item::Thought(_))).count();
                let tools = members.len() - thoughts;
                let total: std::time::Duration = (first..=last)
                    .filter_map(|k| {
                        let key = match &a.items[k] {
                            Item::Tool { id, .. } => id.clone(),
                            _ => format!("think-{k}"),
                        };
                        a.times.get(&key).and_then(|t| t.1)
                    })
                    .sum();
                col = col.child(self.render_step_group(activity::Group {
                    first,
                    last,
                    thoughts,
                    tools,
                    running: live_tail,
                    open,
                    total: (!total.is_zero()).then_some(total),
                    joined_down: open || live_tail,
                }, cx));
            }
            if !open && !(live_tail && i == last) {
                return if i == head { col.into_any_element() } else { div().into_any_element() };
            }
        }
    col.child(match item {
            Item::User(t) => {
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
            Item::Text(t) => TextView::markdown(("ag-md", i), tidy::answer(t).into_owned())
                .text_sm()
                .style(chat_text_style())
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
                let step = activity::Step {
                    key: format!("think-{i}"),
                    kind: "think",
                    title: "",
                    status: if a.open_thought.as_deref() == Some(format!("think-{i}").as_str()) && a.status == Status::Busy { "in_progress" } else { "completed" },
                    joined_up: joined_up(i),
                    joined_down: is_step(a.items.get(i + 1)),
                    live: a.status == Status::Busy,
                    note: (!open).then_some(last),
                    open,
                    detail: open.then(|| tidy::thought(t)),
                };
                let toggle: Box<dyn Fn(&mut Workbench, &mut Context<Workbench>)> = Box::new(move |this, cx| {
                    if !this.agent.open_thoughts.remove(&i) {
                        this.agent.open_thoughts.insert(i);
                    }
                    this.agent.remeasure.borrow_mut().push(i..i + 1);
                    cx.notify();
                });
                self.render_step(step, Some(toggle), cx)
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
            Item::Web(w) => self.render_web_item(i, w, cx),
            Item::Tool { id, title, status, kind } => self.render_step(
                activity::Step {
                    key: id.clone(),
                    kind,
                    title,
                    status,
                    joined_up: joined_up(i),
                    joined_down: is_step(a.items.get(i + 1)),
                    live: a.status == Status::Busy,
                    note: None,
                    open: false,
                    detail: None,
                },
                None,
                cx,
            ),
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
        })
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
    fn render_composer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.agent;
        let view = self.agent_config_view();
        let busy = a.status == Status::Busy;
        let has_text = !a.input.read(cx).value().trim().is_empty() || !a.attachments.is_empty();
        let weak = cx.entity().downgrade();
        let attachments = a.attachments.iter().enumerate().map(|(i, p)| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let (icon, tint) = crate::files::icon_for(&name);
            let path = p.clone();
            let image = crate::files::is_image(p) && !name.to_lowercase().ends_with(".svg");
            let name_tip: SharedString = crate::trf!("{name} · click to open"; name => name.clone()).into();
            let preview = p.clone();
            let lead = if image {
                img(p.clone()).flex_none().size(theme::u(16.)).rounded(px(3.)).object_fit(ObjectFit::Cover).into_any_element()
            } else {
                Icon::new(icon).size(theme::u(12.)).text_color(theme::tint(tint)).flex_none().into_any_element()
            };
            chip_shell(SharedString::from(format!("ag-att-{i}")), true)
                .child(
                    h_flex()
                        .id(SharedString::from(format!("ag-att-open-{i}")))
                        .flex_1()
                        .min_w_0()
                        .pl_1p5()
                        .gap_1()
                        .cursor_pointer()
                        .hover(|s| s.text_color(c(theme::FG_STRONG)))
                        .tooltip(move |w, cx| {
                            if image {
                                let (p, label) = (preview.clone(), name_tip.clone());
                                gpui_kit::component::tooltip::Tooltip::element(move |_, _| {
                                    v_flex()
                                        .gap_1()
                                        .child(img(p.clone()).max_w(px(260.)).max_h(px(200.)).rounded(px(4.)).object_fit(ObjectFit::Contain))
                                        .child(div().text_xs().child(label.clone()))
                                })
                                .build(w, cx)
                            } else {
                                gpui_kit::component::tooltip::Tooltip::new(crate::i18n::tr("Open the file")).build(w, cx)
                            }
                        })
                        .on_click(cx.listener(move |this, _, w, cx| {
                            cx.stop_propagation();
                            if path.is_file() {
                                this.open_file(path.clone(), w, cx);
                            }
                        }))
                        .child(lead)
                        .child(div().min_w_0().truncate().child(name)),
                )
                .child(chip_x(SharedString::from(format!("ag-att-x-{i}")), IconName::Close, cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if i < this.agent.attachments.len() {
                        this.agent.attachments.remove(i);
                    }
                    cx.notify();
                })))
                .into_any_element()
        });
        let mut chips: Vec<AnyElement> = attachments.collect();
        chips.extend(self.render_context_chips(cx));
        let menu = self.render_agent_menu(false, window, cx);
        let geom = self.agent.geom.clone();
        let width = self.agent.compose_w.clone();
        let eid = cx.entity_id();
        // The box's top (the menus above it get the room up to the panel's top) and its
        // width (a narrow box compacts its bottom row), as drawn.
        let box_probe = canvas(
            move |b, _, cx| {
                let y = f32::from(b.origin.y);
                let w = f32::from(b.size.width);
                let (top, box_top) = geom.get();
                let moved = (box_top - y).abs() > 0.5;
                let resized = (width.get() - w).abs() > 0.5;
                if moved {
                    geom.set((top, y));
                }
                if resized {
                    width.set(w);
                }
                if moved || resized {
                    cx.defer(move |cx| cx.notify(eid));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let w = a.compose_w.get();
        // Below NARROW the labels on the right turn into icons, below TIGHT the share
        // beside the context ring goes too.
        let narrow = w > 0. && w < NARROW;
        let tight = w > 0. && w < TIGHT;
        let boxed = v_flex()
            .id("ag-composer")
            .relative()
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
            .when(!chips.is_empty(), |v| {
                let h = a.chips_scroll.clone();
                v.child(
                    h_flex()
                        .id("ag-chips")
                        .gap_1()
                        .mx_2()
                        .mt_2()
                        .min_w_0()
                        .overflow_x_scroll()
                        .track_scroll(&a.chips_scroll)
                        // A wheel turned over the row moves it left and right.
                        .on_scroll(move |ev: &ScrollEvent, _, cx| {
                            let d = ev.delta.pixel_delta(px(24.));
                            if d.x != px(0.) || d.y == px(0.) {
                                return;
                            }
                            let (off, max) = (h.offset(), h.max_offset());
                            if max.x <= px(0.) {
                                return;
                            }
                            h.set_offset(point((off.x + d.y).clamp(-max.x, px(0.)), off.y));
                            cx.stop_propagation();
                        })
                        .children(chips),
                )
            })
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
                    .h(theme::u(36.))
                    .px_1p5()
                    .gap_0p5()
                    .items_center()
                    .min_w_0()
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
                    // The model and mode take what is left of the row; the model's name is
                    // what gives way (cut at its end), after the labels on the right have
                    // turned into icons. Send never leaves the box.
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .when_some(view.iter().find(|o| o.category == "model"), |h, m| h.child(self.render_model_menu(m, narrow, cx)))
                            .when_some(view.iter().find(|o| o.category == "mode").filter(|o| o.choices.len() > 1), |h, m| h.child(self.render_mode_menu(m, tight, cx))),
                    )
                    .when_some(self.agent_status_line(), |h, (dot, label)| {
                        let tip: SharedString = label.clone().into();
                        h.child(
                            h_flex()
                                .id("ag-status")
                                .flex_none()
                                .px_1()
                                .gap_1p5()
                                .text_xs()
                                .text_color(c(if dot == theme::WARN { theme::WARN } else { theme::FG_DIM }))
                                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                                .child(div().flex_none().size(px(6.)).rounded_full().bg(c(dot)))
                                .when(!narrow, |h| h.child(div().whitespace_nowrap().child(label))),
                        )
                    })
                    .child(self.render_access_menu(narrow, cx))
                    .children(self.render_context_meter(tight, cx))
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
            );
        // The menus open right above the box, as wide as it, over the chat.
        div()
            .relative()
            .mx_3()
            .mb_3()
            .child(boxed.child(box_probe))
            .when_some(menu, |d, m| d.child(div().absolute().left_0().right_0().bottom_full().mb_1p5().child(m)))
    }

    /// Which browser the agents drive, and where its tools stand.
    fn render_browser_setting(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::browser::State;
        let setting = self.settings.agent_browser.clone();
        let state = crate::browser::state(&setting);
        let current = match &state {
            State::Off => "off",
            _ => crate::browser::chosen(&setting).map(|b| b.id).unwrap_or("off"),
        };
        let mut choices: Vec<(&'static str, String)> = crate::browser::browsers().iter().map(|b| (b.id, b.name.to_string())).collect();
        choices.push(("off", crate::i18n::tr("Off").to_string()));
        let buttons = h_flex().gap_1().flex_wrap().children(choices.into_iter().map(|(id, name)| {
            let label = if id == "chrome" { format!("{name} ({})", crate::i18n::tr("recommended")) } else { name };
            Button::new(SharedString::from(format!("ag-opt-browser-{id}")))
                .label(label)
                .xsmall()
                .when(current == id, |b| b.primary())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings.agent_browser = id.to_string();
                    this.settings.save();
                    this.agent_browser_setup(cx);
                    cx.notify();
                }))
        }));
        let (note, install): (String, bool) = match &state {
            State::Off => (crate::i18n::tr("Agents have no browser.").into(), false),
            State::NoBrowser => (crate::i18n::tr("No Chrome, Brave, Edge or Chromium found on this PC.").into(), false),
            State::NoNode(e) => (e.clone(), false),
            State::NotInstalled(b) => (crate::trf!("{name} is found; its tools for agents ({pkg}, about 15 MB) aren't installed yet."; name => b.name, pkg => crate::browser::PACKAGE), true),
            State::Installing(b) => (crate::trf!("Installing the tools for {name}…"; name => b.name), false),
            State::Ready(b, v) => (crate::trf!("Agents open pages in {name} (its own profile, not yours) through {pkg} {v}. Applies from the next session."; name => b.name, pkg => crate::browser::PACKAGE, v => v), false),
        };
        let headless = self.settings.agent_browser_headless;
        v_flex()
            .gap_1()
            .child(buttons)
            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(note))
            .child(
                h_flex()
                    .gap_2()
                    .when(install, |h| {
                        h.child(Button::new("ag-opt-browser-install").label(crate::i18n::tr("Install")).xsmall().primary().on_click(cx.listener(|this, _, _, cx| this.agent_browser_setup(cx))))
                    })
                    .when(current != "off", |h| {
                        h.child(
                            Button::new("ag-opt-browser-headless")
                                .label(if headless { crate::i18n::tr("No window") } else { crate::i18n::tr("With a window") })
                                .xsmall()
                                .ghost()
                                .tooltip(crate::i18n::tr("Show the browser while the agent uses it, or run it out of sight"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings.agent_browser_headless = !this.settings.agent_browser_headless;
                                    this.settings.save();
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .into_any_element()
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

    /// The agent is waiting for a yes or no: what it wants to do, in its own words and
    /// (for a command or a file) exactly, then the choices. The one warm-colored card in
    /// the panel, since nothing moves until it is answered.
    fn render_permission(&self, p: &Permission, cx: &mut Context<Self>) -> AnyElement {
        let name = agent_name();
        let lu = gpui_kit::assets::IconName::ShieldAlert;
        let what: &'static str = match p.kind.as_str() {
            "execute" => "Run a command",
            "edit" => "Change a file",
            "delete" => "Delete a file",
            "move" => "Move or rename a file",
            "read" | "search" => "Read files",
            "fetch" => "Open a web page",
            _ => "Use a tool",
        };
        let (kind_icon, mono) = match p.kind.as_str() {
            "execute" => (gpui_kit::assets::IconName::SquareTerminal, true),
            "edit" | "delete" | "move" => (gpui_kit::assets::IconName::FilePen, true),
            "read" | "search" => (gpui_kit::assets::IconName::FolderOpen, true),
            "fetch" => (gpui_kit::assets::IconName::Globe, false),
            _ => (gpui_kit::assets::IconName::Wrench, false),
        };
        let exact = p.detail.clone();
        let mut options: Vec<(usize, String, String, String)> = p.options.iter().enumerate().map(|(i, (o, n, k))| (i, o.clone(), n.clone(), k.clone())).collect();
        // Allow once first (the default answer), then the lasting allow, then the rejects.
        options.sort_by_key(|o| match o.3.as_str() {
            "allow_once" => 0,
            "allow_always" => 1,
            "reject_once" => 2,
            _ => 3,
        });
        let buttons = options.into_iter().map(|(i, oid, label, kind)| {
            let label: String = match kind.as_str() {
                "allow_once" => tr("Allow once").into(),
                "allow_always" => tr("Always allow").into(),
                "reject_once" => tr("Reject").into(),
                "reject_always" => tr("Always reject").into(),
                _ => label,
            };
            let b = Button::new(("perm", i)).label(label).small();
            let b = match kind.as_str() {
                "allow_once" => b.primary(),
                k if k.starts_with("reject") => b.ghost(),
                _ => b,
            };
            b.on_click(cx.listener(move |this, _, _, cx| this.agent_answer_permission(Some(oid.clone()), cx)))
        });
        v_flex()
            .id("ag-permission")
            .mx_3()
            .mb_2()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::WARN).opacity(0.55))
            .bg(c(theme::BG_ELEVATED))
            .overflow_hidden()
            .child(
                h_flex()
                    .px_3()
                    .pt_2p5()
                    .gap_2()
                    .child(Icon::new(lu).size(theme::u(15.)).text_color(c(theme::WARN)).flex_none())
                    .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("{name} is waiting for you"; name => name)))
                    .child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(match self.agent.perm_queue.len() {
                        0 => tr("Paused").to_string(),
                        n => crate::trf!("Paused · {n} more after this"; n => n),
                    })),
            )
            .child(
                v_flex()
                    .px_3()
                    .pt_2()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .text_xs()
                            .text_color(c(theme::FG_MUTED))
                            .child(Icon::new(kind_icon).size(theme::u(13.)).flex_none())
                            .child(div().min_w_0().child(tr(what))),
                    )
                    .child(div().text_sm().text_color(c(theme::FG)).child(p.title.clone()))
                    .when_some(exact, |v, d| {
                        v.child(
                            div()
                                .id("ag-perm-exact")
                                .px_2()
                                .py_1p5()
                                .rounded(px(5.))
                                .bg(c(theme::BG_EDITOR))
                                .border_1()
                                .border_color(c(theme::BORDER))
                                .text_xs()
                                .text_color(c(theme::FG_STRONG))
                                .when(mono, |d| d.font_family("Cascadia Code"))
                                .max_h(theme::u(96.))
                                .overflow_y_scroll()
                                .child(d),
                        )
                    }),
            )
            .child(h_flex().px_3().pt_2p5().gap_1p5().flex_wrap().children(buttons))
            .child(
                h_flex()
                    .mt_2p5()
                    .px_2()
                    .py_1()
                    .gap_1()
                    .border_t_1()
                    .border_color(c(theme::BORDER))
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(
                        Button::new("ag-perm-full")
                            .icon(Icon::new(gpui_kit::assets::IconName::ShieldCheck))
                            .label(tr("Turn on full access"))
                            .xsmall()
                            .ghost()
                            .flex_none()
                            .tooltip(tr("Allows this request and every next one: commands and any file on this PC, without asking. Switch it off in the agent's settings."))
                            .on_click(cx.listener(|this, _, _, cx| this.agent_toggle_full(true, cx))),
                    )

            )
            .into_any_element()
    }
}

/// A 12 px icon in a box one text line (18 px) tall, for rows whose text may wrap.
impl Workbench {
    /// The composer's model chip: the model's short name, then the effort when one is
    /// set. The name is what gets cut when the box is narrow.
    fn render_model_menu(&self, m: &ConfigOpt, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let current = m.current_name();
        let mut short = current.rsplit('/').next().unwrap_or(current).to_string();
        // "Free" is said by the group in the menu; here it only takes room.
        if let Some(s) = short.strip_suffix(" Free") {
            short = s.to_string();
        }
        if m.current == crate::acp::FLAG_DEFAULT && m.id.starts_with(FLAG_PREFIX) {
            short = tr("Model").to_string();
        }
        // "Default (recommended)": the menu says why; the chip only needs the name.
        if let Some(s) = short.strip_suffix(" (recommended)") {
            short = s.to_string();
        }
        let effort = self.agent_config_view().into_iter().find(|o| o.category == "thought_level").filter(|e| e.current != "default").map(|e| e.current_name().to_string());
        let tip: SharedString = match &effort {
            Some(e) => crate::trf!("Model: {m} · effort {e}"; m => current, e => e).into(),
            None => crate::trf!("Model: {m}"; m => current).into(),
        };
        chip_button("ag-model-btn", self.agent.menu.open == Some(Menu::Model), tip)
            // The one chip that shrinks: everything else keeps its size.
            .flex_shrink(1.)
            .min_w(theme::u(48.))
            .child(div().min_w_0().truncate().text_color(c(theme::FG)).child(short))
            .when_some(effort.filter(|_| !narrow), |h, e| h.child(div().flex_none().text_color(c(theme::FG_DIM)).child(e)))
            .child(chevron())
            .on_click(cx.listener(|this, _, w, cx| {
                cx.stop_propagation();
                this.agent_menu_toggle(Menu::Model, w, cx)
            }))
    }

    /// The agent's modes (build, plan, ...): the current one, opening the mode menu.
    fn render_mode_menu(&self, m: &ConfigOpt, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let name = m.current_name().to_string();
        let tip: SharedString = crate::trf!("Mode: {m}"; m => name).into();
        let mut label = name;
        let max = if narrow { 6 } else { 11 };
        if label.chars().count() > max {
            label = format!("{}…", label.chars().take(max - 1).collect::<String>());
        }
        chip_button("ag-mode-btn", self.agent.menu.open == Some(Menu::Mode), tip)
            .flex_none()
            .child(div().whitespace_nowrap().text_color(c(theme::FG_MUTED)).child(label))
            .child(chevron())
            .on_click(cx.listener(|this, _, w, cx| {
                cx.stop_propagation();
                this.agent_menu_toggle(Menu::Mode, w, cx)
            }))
    }

    /// What the agent may do without asking: an icon, and its name when there's room.
    /// Full access is the one shown in the warning color, so it is never on unnoticed.
    fn render_access_menu(&self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let level = self.settings.access().to_string();
        let (icon, label) = menu::ACCESS.iter().find(|l| l.0 == level).map(|l| (l.1, l.2)).unwrap_or((gpui_kit::assets::IconName::PencilLine, "Edit files"));
        let short = match level.as_str() {
            "read" => tr("Read only"),
            "full" => tr("Full access"),
            _ => tr("Asks first"),
        };
        let full = level == "full";
        let tip: SharedString = crate::trf!("Permissions: {level}"; level => tr(label)).into();
        chip_button("ag-access", self.agent.menu.open == Some(Menu::Access), tip)
            .flex_none()
            .child(Icon::new(icon).size(theme::u(13.)).flex_none().text_color(c(if full { theme::WARN } else { theme::FG_MUTED })))
            .when(!narrow, |h| h.child(div().whitespace_nowrap().text_color(c(if full { theme::WARN } else { theme::FG_MUTED })).child(short)))
            .on_click(cx.listener(|this, _, w, cx| {
                cx.stop_propagation();
                this.agent_menu_toggle(Menu::Access, w, cx)
            }))
    }

    /// The chat before the first message: the agent and what to type, or, when the
    /// agent isn't on this PC, a way to get it (or any other agent).
    fn render_agent_welcome(&self, on: bool, cx: &mut Context<Self>) -> AnyElement {
        let name = agent_name();
        let id = agent_id();
        let installed = crate::acp::locate(&id).is_some();
        let any = !crate::acp::installed().is_empty();
        let mut col = v_flex().size_full().min_h(theme::u(240.)).items_center().justify_center().gap_2().px_4();
        if let Some((_, _, anim)) = &self.agent.installing {
            return col.child(anim.clone()).into_any_element();
        }
        if let Some((who, why, Some(anim))) = &self.agent.install_err {
            let retry = who.clone();
            return col
                .child(anim.clone())
                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("{name} wasn't installed"; name => crate::acp::display_name(who))))
                .child(div().max_w(theme::u(320.)).text_xs().text_center().text_color(c(theme::DANGER)).child(why.clone()))
                .child(
                    h_flex()
                        .pt_2()
                        .gap_2()
                        .child(Button::new("ag-retry").label(tr("Try again")).small().primary().on_click(cx.listener(move |this, _, w, cx| this.agent_install(retry.clone(), w, cx))))
                        .child(Button::new("ag-retry-no").label(tr("Back")).small().ghost().on_click(cx.listener(|this, _, _, cx| {
                            this.agent.install_err = None;
                            cx.notify();
                        }))),
                )
                .into_any_element();
        }
        col = col.child(crate::ui::meander(36., c(theme::FG_DIM), c(theme::ACCENT_TEXT)).mb_2());
        let err_box = self.agent.install_err.as_ref().map(|(i, e, _)| {
            div()
                .w_full()
                .p_2()
                .rounded(px(4.))
                .border_1()
                .border_color(c(theme::DANGER))
                .text_xs()
                .text_color(c(theme::DANGER))
                .child(crate::trf!("{name} wasn't installed. {why}"; name => crate::acp::display_name(i), why => e))
        });
        if installed {
            let hint = if on {
                tr("Type @ to add a file. Your open file, selection and last error go along when they help.")
            } else {
                tr("Off for now. Your first message turns it on. Type @ to add a file.")
            };
            let skills: Vec<SlashCmd> = self.agent.commands.iter().filter(|c| !c.builtin).take(12).cloned().collect();
            let chips = (!skills.is_empty()).then(|| {
                v_flex()
                    .w_full()
                    .max_w(theme::u(340.))
                    .pt_3()
                    .gap_1p5()
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(crate::trf!("Skills and commands from {name}"; name => name)))
                    .child(h_flex().flex_wrap().gap_1().children(skills.into_iter().enumerate().map(|(i, s)| {
                        let fill = format!("/{} ", s.name);
                        let desc: SharedString = s.desc.clone().into();
                        div()
                            .id(("ag-skill", i))
                            .px_2()
                            .py_0p5()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(theme::FG))
                            .hover(|st| st.border_color(c(theme::BORDER_STRONG)).bg(c(theme::BG_HOVER)))
                            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(desc.clone()).build(w, cx))
                            .child(format!("/{}", s.name))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                let f = fill.clone();
                                this.agent.input.update(cx, |st, cx| {
                                    let end = f.len();
                                    st.set_value(f.clone(), w, cx);
                                    st.set_selected_range(end..end, cx);
                                });
                                let h = this.agent.input.focus_handle(cx);
                                h.focus(w, cx);
                            }))
                    })))
            });
            return col
                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("Ask {name}"; name => name)))
                .child(div().text_xs().text_center().text_color(c(theme::FG_DIM)).child(hint))
                .children(chips)
                .children(err_box)
                .into_any_element();
        }
        // Not on this PC: the chosen agent first, then the most used ones.
        let mut ids: Vec<&'static str> = Vec::new();
        if let Some(k) = crate::acp::known(&id) {
            ids.push(k.id);
        }
        ids.extend(crate::acp::KNOWN.iter().map(|k| k.id).filter(|k| *k != id && crate::acp::locate(k).is_none()).take(if any { 0 } else { 3 }));
        let title = if any { crate::trf!("{name} isn't installed"; name => name) } else { tr("Pick an agent to start").into() };
        let note = if any {
            tr("Download it, or pick an agent you have in the menu above.")
        } else {
            tr("Every agent here speaks the Agent Client Protocol. FluxCode downloads it into its own folder and starts it only while you use it.")
        };
        let list = v_flex().w_full().max_w(theme::u(300.)).pt_2().gap_1().children(ids.into_iter().map(|k| {
            let known = crate::acp::known(k).expect("a known agent");
            let kid = k.to_string();
            h_flex()
                .id(SharedString::from(format!("ag-get-{k}")))
                .w_full()
                .px_2()
                .py_1()
                .gap_2p5()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::BORDER))
                .cursor_pointer()
                .hover(|s| s.bg(c(theme::BG_HOVER)).border_color(c(theme::BORDER_STRONG)))
                .child(agent_glyph(k, 20.))
                .child(two_line(known.name, known.by))
                .child(
                    h_flex()
                        .gap_1()
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .child(Icon::new(gpui_kit::assets::IconName::Download).size(theme::u(14.)))
                        .child(if k == "mura" { tr("Get") } else { tr("Download") }),
                )
                .on_click(cx.listener(move |this, _, w, cx| this.agent_install(kid.clone(), w, cx)))
        }));
        col.child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
            .child(div().text_xs().text_center().text_color(c(theme::FG_DIM)).child(note))
            .child(list)
            .when(!any, |v| v.child(div().pt_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("The menu above lists every agent in the ACP registry."))))
            .children(err_box)
            .into_any_element()
    }
}

/// An agent's icon from the ACP registry, drawn in the theme's text color; agents
/// without one (Mura, or before the icons arrive) get their initial in a square.
fn agent_glyph(id: &str, size: f32) -> AnyElement {
    let s = theme::u(size);
    match crate::acp::icon_asset(id) {
        Some(path) => svg().path(SharedString::from(path)).flex_none().size(s).text_color(c(theme::FG)).into_any_element(),
        None => {
            let letter: String = crate::acp::display_name(id).chars().next().unwrap_or('?').to_uppercase().collect();
            div()
                .flex_none()
                .size(s)
                .rounded(px(4.))
                .border_1()
                .border_color(c(theme::BORDER_STRONG))
                .flex()
                .items_center()
                .justify_center()
                .text_size(s * 0.6)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_MUTED))
                .child(letter)
                .into_any_element()
        }
    }
}

/// A title over one dim line, for menu rows and agent cards.
/// A composer button that opens one of the menus: its label and a caret.
/// A thought or a tool call: the rows that share the activity rail.
fn is_step(item: Option<&Item>) -> bool {
    matches!(item, Some(Item::Thought(_) | Item::Tool { .. }))
}

/// Below this width the composer's bottom row shows icons instead of labels.
const NARROW: f32 = 500.;
const TIGHT: f32 = 360.;

/// A chip in the composer's bottom row that opens a menu: 24 px tall like the icon
/// buttons beside it, its text cut to fit, highlighted while its menu is open.
fn chip_button(id: &'static str, open: bool, tip: SharedString) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(theme::u(24.))
        .min_w_0()
        .px_1p5()
        .gap_1()
        .rounded(px(5.))
        .overflow_hidden()
        .cursor_pointer()
        .text_xs()
        .when(open, |d| d.bg(c(theme::BG_HOVER)))
        .hover(|s| s.bg(c(theme::BG_HOVER)))
        .active(|s| s.opacity(0.85))
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
}

fn chevron() -> Icon {
    Icon::new(IconName::ChevronDown).size(theme::u(12.)).flex_none().text_color(c(theme::FG_DIM))
}

fn two_line(title: impl Into<SharedString>, sub: impl Into<SharedString>) -> Div {
    let sub: SharedString = sub.into();
    v_flex()
        .flex_1()
        .min_w_0()
        .child(div().truncate().child(title.into()))
        .when(!sub.is_empty(), |v| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(sub)))
}

fn line_icon(icon: Icon) -> Div {
    div().flex_none().h(theme::u(18.)).flex().items_center().child(icon.size(theme::u(12.)))
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
        // In one row with the others, which scrolls sideways when they don't fit.
        .flex_none()
        .max_w(theme::u(200.))
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
pub(crate) const CHAT_SCROLL_KEY: u64 = u64::MAX - 1;

/// Answers read like a chat, not a document: headings a step or two above the 14 px
/// body (the default makes a `#` twice the body size), and paragraphs a little closer.
fn chat_text_style() -> gpui_kit::component::text::TextViewStyle {
    gpui_kit::component::text::TextViewStyle::default().paragraph_gap(rems(0.6)).heading_font_size(|level, _| match level {
        1 => px(17.),
        2 => px(16.),
        3 => px(15.),
        _ => px(14.),
    })
}

impl Workbench {
    /// The list follows the newest text by itself (tail mode) while the reader is at the
    /// bottom; nothing to do per update.
    fn agent_follow(&self) {}

    /// Jump to the newest message and follow it again (a message was sent).
    pub(crate) fn agent_to_end(&self) {
        self.agent.list.set_follow_mode(FollowMode::Tail);
        self.agent.list.scroll_to_end();
    }

    /// Tell the list what changed since the last frame. Streaming only ever changes the
    /// last few items, so only those are compared (by size, not by content); a chat that
    /// was replaced or cut short starts over. Costs the same for any length of chat.
    fn agent_sync_list(&self) {
        let a = &self.agent;
        let items = &a.items;
        let n = items.len();
        let busy = a.status == Status::Busy;
        let mut seen = a.list_seen.borrow_mut();
        let ptr = items.as_ptr() as usize;
        const TAIL: usize = 16;
        let tail: Vec<u64> = items[n.saturating_sub(TAIL)..].iter().map(item_size_sig).collect();
        let replaced = n < seen.len || (ptr != seen.ptr && seen.len > 0 && n > 0 && items.first().map(item_size_sig) != Some(seen.first));
        // The window: a chat opened (or cut short) shows its newest items; while the
        // reader follows the bottom, the oldest leave once PAGE more have come in.
        let mut start = a.view_start.get().min(n);
        if replaced || seen.len == 0 {
            start = n.saturating_sub(WINDOW);
        } else if a.list.is_following_tail() && n - start > WINDOW + PAGE {
            start = n - WINDOW;
        }
        a.view_start.set(start);
        let pager = (start > 0) as usize;
        let row = |abs: usize| abs.saturating_sub(start) + pager;
        let rows = n - start + pager;
        let mut from = usize::MAX;
        if replaced || start != seen.start || seen.len == 0 {
            a.list.reset(rows);
            if replaced {
                seen.run_start.clear();
                seen.run_end.clear();
                from = 0;
            } else {
                from = seen.len.saturating_sub(2);
            }
            if start < seen.start && !replaced {
                // Paged back: the item that was on top stays where the reader had it.
                a.list.scroll_to(ListOffset { item_ix: row(seen.start), offset_in_item: px(0.) });
            } else if a.list.is_following_tail() || replaced {
                a.list.scroll_to_end();
            }
        } else {
            if n > seen.len {
                let old_rows = row(seen.len);
                a.list.splice(old_rows..old_rows, n - seen.len);
                // The old last item(s) lose the bottom padding or fold into a group.
                from = seen.len.saturating_sub(2);
            }
            // The first of the last items whose size changed.
            let base = n.saturating_sub(TAIL);
            for (k, sig) in tail.iter().enumerate() {
                let ix = base + k;
                let old = ix.checked_sub(seen.len.saturating_sub(seen.tail.len())).and_then(|o| seen.tail.get(o));
                if ix < seen.len && old != Some(sig) {
                    from = from.min(ix);
                    break;
                }
            }
            // Working / done changes what the last group shows.
            if busy != seen.busy {
                from = from.min(n.saturating_sub(TAIL));
            }
            let upto = seen.len.min(n);
            if from < upto {
                a.list.remeasure_items(row(from.max(start))..row(upto));
            }
            for r in a.remeasure.borrow_mut().drain(..) {
                let (lo, hi) = (r.start.max(start).min(n), r.end.min(n));
                if lo < hi {
                    a.list.remeasure_items(row(lo)..row(hi));
                }
            }
        }
        a.remeasure.borrow_mut().clear();
        // Step runs, kept up to date from the first changed item only.
        let from = from.min(seen.run_start.len()).min(n);
        seen.run_start.truncate(from);
        seen.run_end.retain(|&f, _| f < from);
        if let Some(&f) = from.checked_sub(1).and_then(|k| seen.run_start.get(k)).filter(|f| **f != usize::MAX) {
            seen.run_end.insert(f, from - 1);
        }
        for i in from..n {
            let first = if !is_step(items.get(i)) {
                usize::MAX
            } else if i > 0 && is_step(items.get(i - 1)) {
                seen.run_start[i - 1]
            } else {
                i
            };
            seen.run_start.push(first);
            if first != usize::MAX {
                seen.run_end.insert(first, i);
            }
        }
        seen.len = n;
        seen.start = start;
        seen.ptr = ptr;
        seen.first = items.first().map(item_size_sig).unwrap_or(0);
        seen.tail = tail;
        seen.busy = busy;
    }

    /// The row above the window: how many earlier items are not shown, and a button that
    /// brings back PAGE more (the reader stays where they were).
    fn render_agent_pager(&mut self, start: usize, cx: &mut Context<Self>) -> AnyElement {
        let more = start.min(PAGE);
        h_flex()
            .w_full()
            .px_3()
            .pt_2()
            .justify_center()
            .child(
                h_flex()
                    .id("ag-pager")
                    .px_3()
                    .h(theme::u(26.))
                    .gap_1p5()
                    .rounded_full()
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_ELEVATED))
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .child(Icon::new(IconName::ChevronUp).size(theme::u(12.)))
                    .child(crate::trf!("Show {n} earlier"; n => more))
                    .child(div().text_color(c(theme::FG_DIM)).child(crate::trf!("· {n} hidden"; n => start)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Reading back: stop following the newest text until the reader
                        // returns to the bottom.
                        this.agent.list.pause_following_tail();
                        this.agent.view_start.set(this.agent.view_start.get().saturating_sub(PAGE));
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// The run of two or more steps `i` belongs to: (first, last).
    fn agent_run_of(&self, i: usize) -> Option<(usize, usize)> {
        let seen = self.agent.list_seen.borrow();
        let f = *seen.run_start.get(i)?;
        let l = *seen.run_end.get(&f)?;
        (f != usize::MAX && l > f).then_some((f, l))
    }
}

/// Items in the list while the reader follows the newest text, and how many more come
/// in before the oldest leave (and each "Show earlier" step).
const WINDOW: usize = 40;
const PAGE: usize = 20;

/// The file as it was before an edit tool ran, from what is on disk now and the tool's
/// diff. Agents send the diff of the edit (Claude: just the replaced snippet), and the
/// update can come before or after the file is written: if the new text is there and the
/// old is not, the edit already ran and is undone here; else the disk is the "before".
fn before_edit(disk: Option<String>, old: Option<&str>, new: &str) -> Option<String> {
    let d = disk?;
    match old {
        // A new file this tool wrote.
        None if d == new => None,
        Some(o) if !new.is_empty() && !o.is_empty() && d.contains(new) && !d.contains(o) => Some(d.replacen(new, o, 1)),
        _ => Some(d),
    }
}

/// What the virtual list was told last frame.
#[derive(Default)]
pub(crate) struct ListSeen {
    len: usize,
    /// The window's first item.
    start: usize,
    ptr: usize,
    first: u64,
    busy: bool,
    /// Size signatures of the last items.
    tail: Vec<u64>,
    /// For each item, the first item of its step run (usize::MAX: not a step); and each
    /// run's last item, by its first.
    run_start: Vec<usize>,
    run_end: HashMap<usize, usize>,
}

/// Changes when an item's height can: its kind and the lengths of what it shows.
fn item_size_sig(item: &Item) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(item).hash(&mut h);
    match item {
        Item::User(t) | Item::Text(t) | Item::Thought(t) | Item::Branch(t) => t.len().hash(&mut h),
        Item::Note(t, e) => (t.len(), *e).hash(&mut h),
        Item::Tool { title, status, kind, .. } => (title.len(), status, kind).hash(&mut h),
        Item::Plan(e) => (e.len(), e.iter().filter(|x| x.1 == "completed").count()).hash(&mut h),
        Item::Web(w) => (w.results.len(), &w.status, w.query.len()).hash(&mut h),
    }
    h.finish()
}

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
    fn before_an_edit() {
        use super::before_edit;
        // Not written yet: the disk is the "before".
        assert_eq!(before_edit(Some("a\nold\nb".into()), Some("old"), "new").as_deref(), Some("a\nold\nb"));
        // Already written: the snippet is undone.
        assert_eq!(before_edit(Some("a\nnew\nb".into()), Some("old"), "new").as_deref(), Some("a\nold\nb"));
        // A new file.
        assert_eq!(before_edit(Some("x".into()), None, "x"), None);
        assert_eq!(before_edit(None, None, "x"), None);
    }

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
