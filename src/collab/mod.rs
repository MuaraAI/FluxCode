//! Live share: code together in real time, with the host's PC as the server.
//!
//! - The host shares the open folder and gets an invite code. Whoever has the code
//!   joins straight to the host's PC (`net`): no server of ours, encrypted. From
//!   outside the host's network they come in over public IPv6, a port the router
//!   opened (UPnP), or a free public relay that only sees ciphertext (`reach`).
//! - Guests join into their own clone of the same repository (the open folder) or
//!   into a new folder; files come from the host as they are opened.
//! - Every open shared file is synced keystroke by keystroke with operational
//!   transformation (`ot`): the host orders edits, so everyone ends with the same text.
//! - Everyone sees who is in the session, which file and line each is on, and an
//!   activity list of edits. An edit made by an AI agent says so, with the agent,
//!   model, effort and mode, and every entry opens as a diff.
//!
//! Only files someone has open travel, so a huge project costs no more than the
//! files being worked on.

pub mod net;
pub mod ot;
pub mod page;
pub mod proto;
pub mod ratelimit;
pub mod reach;
pub mod share;

use crate::i18n::tr;
use crate::workbench::Workbench;
use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::*;
use net::{Invite, NetEvent};
use ot::Op;
use proto::{AiInfo, Msg, Peer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Instant;

/// Files at most this big are shared (larger ones open read-only for guests).
const MAX_SHARED_FILE: usize = 16 << 20;
/// File names sent to guests for their file list.
const MAX_LISTED: usize = 20_000;

pub enum Role {
    Host { server: net::Host, guests: HashMap<u64, Guest> },
    Guest { tx: Sender<Msg>, addr: String },
}

pub struct Guest {
    tx: Sender<Msg>,
    peer: Option<Peer>,
    opens: ratelimit::Bucket,
    edits: ratelimit::Bucket,
    cursors: ratelimit::Bucket,
}

/// Which of a guest's buckets a message draws from.
#[derive(Clone, Copy)]
enum Limb {
    Open,
    Edit,
    Cursor,
}

/// One shared file on this PC.
pub struct SharedDoc {
    pub path: PathBuf,
    /// The text as last synced (the editor holds the same, or one unsent local edit more).
    shadow: String,
    /// Host: the one true copy and its revisions.
    server: Option<ot::Server>,
    /// Guest: the edit in flight and the ones behind it, and the host's revision.
    client: ot::Client,
    rev: u64,
    /// The next change comes from an AI agent.
    ai_next: bool,
    /// Guest: the attribution of the edit in flight / buffered.
    pending_ai: Option<AiInfo>,
}

pub struct FeedItem {
    pub who: String,
    pub rel: String,
    pub ai: Option<AiInfo>,
    pub before: String,
    pub after: String,
    pub at: Instant,
    pub lines: (usize, usize),
}

pub struct Collab {
    pub role: Option<Role>,
    pub invite: Option<Invite>,
    pub me: u64,
    pub host_name: String,
    pub folder: String,
    /// Where the shared files are on this PC.
    pub root: Option<PathBuf>,
    pub peers: Vec<Peer>,
    /// Guest: every file in the host's folder (relative paths).
    pub files: Vec<String>,
    pub docs: HashMap<String, SharedDoc>,
    pub feed: Vec<FeedItem>,
    pub error: Option<String>,
    pub connecting: bool,
    pub join_input: Entity<InputState>,
    pub filter_input: Entity<InputState>,
    /// Join into the open folder (the guest's own clone of the same repository).
    pub into_open_folder: bool,
    /// Host: also open ways in from the internet (router port, IPv6, relay).
    pub internet: bool,
    /// Host: the probes opening those ways in, and where they stand.
    pub probes: Option<reach::Probes>,
    pub reach: Option<reach::Reach>,
    /// When the invite code was last copied (the button says so for a moment), and
    /// what was copied (the ways in can change it afterwards).
    pub copied: Option<Instant>,
    pub copied_code: Option<String>,
    /// The Live share page is showing in the editor area.
    pub page: bool,
    last_cursor: (String, u32),
    /// Re-render as the invite code or the file filter is typed.
    _subs: Vec<Subscription>,
}

impl Collab {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let join_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Paste an invite code (FLUX1-…)")));
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Find a shared file")));
        let mut _subs: Vec<Subscription> = [&join_input, &filter_input].into_iter().map(|i| cx.subscribe(i, |_: &mut Workbench, _, _: &InputEvent, cx| cx.notify())).collect();
        // Enter in the invite box joins.
        _subs.push(cx.subscribe_in(&join_input, window, |this: &mut Workbench, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) && !this.collab.connecting && !this.collab.active() {
                this.collab_join(window, cx);
            }
        }));
        Self {
            role: None,
            invite: None,
            me: 0,
            host_name: String::new(),
            folder: String::new(),
            root: None,
            peers: Vec::new(),
            files: Vec::new(),
            docs: HashMap::new(),
            feed: Vec::new(),
            error: None,
            connecting: false,
            join_input,
            filter_input,
            into_open_folder: false,
            internet: true,
            probes: None,
            reach: None,
            copied: None,
            copied_code: None,
            page: false,
            last_cursor: (String::new(), 0),
            _subs,
        }
    }

    pub fn active(&self) -> bool {
        self.role.is_some()
    }

    fn is_host(&self) -> bool {
        matches!(self.role, Some(Role::Host { .. }))
    }

    /// This file's path in the session, when it is in the shared folder.
    pub fn rel_of(&self, path: &Path) -> Option<String> {
        let root = self.root.as_ref()?;
        let rel = path.strip_prefix(root).ok()?;
        let s = rel.to_string_lossy().replace('\\', "/");
        (!s.is_empty()).then_some(s)
    }

    /// The path of `rel` on this PC; `None` for paths that would leave the folder.
    fn abs_of(&self, rel: &str) -> Option<PathBuf> {
        if rel.split('/').any(|p| p == ".." || p.is_empty()) || rel.contains(':') || rel.starts_with('/') {
            return None;
        }
        Some(rel.split('/').fold(self.root.clone()?, |p, s| p.join(s)))
    }

    fn send_all(&self, msg: &Msg, except: Option<u64>) {
        if let Some(Role::Host { guests, .. }) = &self.role {
            for (id, g) in guests {
                if Some(*id) != except && g.peer.is_some() {
                    let _ = g.tx.send(msg.clone());
                }
            }
        }
    }

    fn send_host(&self, msg: Msg) {
        if let Some(Role::Guest { tx, .. }) = &self.role {
            let _ = tx.send(msg);
        }
    }

    fn name_of(&self, id: u64) -> String {
        self.peers.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| tr("Someone").into())
    }

    /// Add an edit to the activity list: one entry per person, file and kind
    /// (AI or typed) while they keep going, a new one after a pause.
    fn note_edit(&mut self, who: String, rel: &str, ai: Option<AiInfo>, before: &str, after: &str) {
        let lines = |b: &str, a: &str| if b.len() + a.len() < 600_000 { crate::diff::counts(b, a) } else { (0, 0) };
        if let Some(last) = self.feed.first_mut() {
            if last.who == who && last.rel == rel && last.ai.is_some() == ai.is_some() && last.at.elapsed().as_secs() < 30 {
                last.after = after.to_string();
                last.at = Instant::now();
                last.lines = lines(&last.before, after);
                if ai.is_some() {
                    last.ai = ai;
                }
                return;
            }
        }
        self.feed.insert(0, FeedItem { who, rel: rel.to_string(), ai, before: before.to_string(), after: after.to_string(), at: Instant::now(), lines: lines(before, after) });
        self.feed.truncate(60);
    }
}

/// Byte offset of char index `ci` in `s`.
fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map_or(s.len(), |(b, _)| b)
}

fn char_at(s: &str, bi: usize) -> usize {
    s[..bi.min(s.len())].chars().count()
}

/// Apply a remote edit to an editor span by span, keeping the local caret where
/// its text went.
fn apply_to_editor(editor: &Entity<EditorState>, op: &Op, window: &mut Window, cx: &mut App) {
    editor.update(cx, |s, cx| {
        let text = s.value().to_string();
        let sel = s.selected_range();
        let (a, b) = (char_at(&text, sel.start), char_at(&text, sel.end));
        // Byte offsets on the original text; applied last to first so earlier ones hold.
        let spans: Vec<(usize, usize, String)> = op
            .spans()
            .into_iter()
            .map(|(start, del, ins)| {
                let b0 = byte_at(&text, start);
                let b1 = byte_at(&text, start + del);
                (b0, b1, ins)
            })
            .collect();
        for (b0, b1, ins) in spans.into_iter().rev() {
            s.set_selected_range(b0..b1, cx);
            s.replace(ins, window, cx);
        }
        let now = s.value().to_string();
        let (na, nb) = (op.transform_index(a), op.transform_index(b));
        s.set_selected_range(byte_at(&now, na)..byte_at(&now, nb), cx);
    });
}

fn my_name() -> (String, bool) {
    match crate::github::load() {
        Some(a) => (a.login, true),
        None => (std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "Guest".into()), false),
    }
}

impl Workbench {
    // ---------------------------------------------------------------- starting

    pub(crate) fn collab_host(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            self.collab.error = Some(tr("Open a folder first: the session shares it.").into());
            cx.notify();
            return;
        };
        let secret = net::random::<32>();
        let (server, rx) = match net::host(secret) {
            Ok(v) => v,
            Err(e) => {
                self.collab.error = Some(e);
                cx.notify();
                return;
            }
        };
        let port = server.port;
        let (probes, changes) = if self.collab.internet {
            let (p, rx) = reach::start(port, server.ipv6);
            (Some(p), Some(rx))
        } else {
            (None, None)
        };
        let view = probes.as_ref().map(|p| p.snapshot()).unwrap_or_else(|| reach::Reach::local_only(net::lan_ip()));
        let (name, github) = my_name();
        let folder = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let c = &mut self.collab;
        c.invite = Some(Invite { a: view.addresses(port), k: base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, secret), h: name.clone(), n: folder.clone() });
        c.reach = Some(view);
        c.probes = probes;
        c.copied = None;
        c.copied_code = None;
        c.role = Some(Role::Host { server, guests: HashMap::new() });
        c.me = 0;
        c.host_name = name.clone();
        c.folder = folder;
        c.root = Some(root);
        c.peers = vec![Peer { id: 0, name, github, host: true, file: String::new(), line: 0 }];
        c.docs.clear();
        c.feed.clear();
        c.error = None;
        self.collab_listen(rx, window, cx);
        if let Some(changes) = changes {
            self.collab_watch_reach(changes, window, cx);
        }
        cx.notify();
    }

    /// Keep the invite code and the panel in step with the ways in as they open or fail.
    fn collab_watch_reach(&mut self, mut changes: futures::channel::mpsc::UnboundedReceiver<()>, window: &mut Window, cx: &mut Context<Self>) {
        use futures::StreamExt;
        cx.spawn_in(window, async move |this, cx| {
            while changes.next().await.is_some() {
                let alive = this.update(cx, |this, cx| {
                    let c = &mut this.collab;
                    let (Some(p), Some(Role::Host { server, .. })) = (&c.probes, &c.role) else { return false };
                    let view = p.snapshot();
                    if let Some(inv) = c.invite.as_mut() {
                        inv.a = view.addresses(server.port);
                    }
                    c.reach = Some(view);
                    cx.notify();
                    true
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn collab_copy_invite(&mut self, cx: &mut Context<Self>) {
        let Some(code) = self.collab.invite.as_ref().map(|i| i.code()) else { return };
        cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
        self.collab.copied = Some(Instant::now());
        self.collab.copied_code = Some(code);
        self.status = tr("Invite code copied").into();
        cx.notify();
        // Put the button's label back.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(1800)).await;
            _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub(crate) fn collab_join(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.collab.join_input.read(cx).value().to_string();
        let inv = match Invite::parse(&code) {
            Ok(i) => i,
            Err(e) => {
                self.collab.error = Some(e);
                cx.notify();
                return;
            }
        };
        let root = if self.collab.into_open_folder {
            match self.root.clone() {
                Some(r) => r,
                None => {
                    self.collab.error = Some(tr("Open your copy of the project first, or join into a new folder.").into());
                    cx.notify();
                    return;
                }
            }
        } else {
            let id: String = inv.k.chars().take(6).collect();
            crate::acp::agents_dir().parent().map(|p| p.join("collab")).unwrap_or_else(std::env::temp_dir).join(format!("{}-{}", sanitize(&inv.n), sanitize(&id)))
        };
        self.collab.connecting = true;
        self.collab.error = None;
        cx.notify();
        let inv2 = inv.clone();
        let task = cx.background_spawn(async move { net::join(&inv2) });
        cx.spawn_in(window, async move |this, cx| {
            let res = task.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.collab.connecting = false;
                match res {
                    Ok((tx, rx, addr)) => {
                        let (name, github) = my_name();
                        let _ = tx.send(Msg::Hello { name, github, build: crate::update::BUILD.into() });
                        let _ = std::fs::create_dir_all(&root);
                        let c = &mut this.collab;
                        c.role = Some(Role::Guest { tx, addr });
                        c.invite = Some(inv);
                        c.root = Some(root.clone());
                        c.docs.clear();
                        c.feed.clear();
                        c.files.clear();
                        // A new folder: show it as the workspace, so the explorer and the agent work in it.
                        if this.root.as_ref() != Some(&root) {
                            this.workspace.extra.clear();
                            this.open_folder(root, cx);
                        }
                        this.collab_listen(rx, window, cx);
                    }
                    Err(e) => this.collab.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// End hosting, or leave the session.
    pub(crate) fn collab_stop(&mut self, why: &str, cx: &mut Context<Self>) {
        let bye = Msg::Bye { why: why.to_string() };
        match &self.collab.role {
            Some(Role::Host { .. }) => self.collab.send_all(&bye, None),
            Some(Role::Guest { .. }) => self.collab.send_host(bye),
            None => {}
        }
        let c = &mut self.collab;
        c.role = None;
        c.invite = None;
        // Closes the relay and removes the router's port mapping.
        c.probes = None;
        c.reach = None;
        c.peers.clear();
        c.docs.clear();
        c.files.clear();
        cx.notify();
    }

    fn collab_listen(&mut self, rx: futures::channel::mpsc::UnboundedReceiver<NetEvent>, window: &mut Window, cx: &mut Context<Self>) {
        use futures::StreamExt;
        let mut rx = rx;
        cx.spawn_in(window, async move |this, cx| {
            while let Some(ev) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.collab_on_event(ev, window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    // ---------------------------------------------------------------- events

    fn collab_on_event(&mut self, ev: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.collab.is_host() {
            self.collab_host_event(ev, window, cx);
        } else if self.collab.active() {
            self.collab_guest_event(ev, window, cx);
        }
        cx.notify();
    }

    fn collab_host_event(&mut self, ev: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            NetEvent::Connected { conn, tx } => {
                if let Some(Role::Host { guests, .. }) = &mut self.collab.role {
                    guests.insert(conn, Guest { tx, peer: None, opens: ratelimit::Bucket::new(10, 1), edits: ratelimit::Bucket::new(200, 200), cursors: ratelimit::Bucket::new(50, 50) });
                }
            }
            NetEvent::Closed { conn, .. } | NetEvent::Message { conn, msg: Msg::Bye { .. } } => {
                let gone = if let Some(Role::Host { guests, .. }) = &mut self.collab.role { guests.remove(&conn).and_then(|g| g.peer) } else { None };
                if let Some(p) = gone {
                    self.collab.peers.retain(|x| x.id != conn);
                    self.status = crate::trf!("{name} left the session"; name => p.name).into();
                    let peers = self.collab.peers.clone();
                    self.collab.send_all(&Msg::Peers { peers }, None);
                }
            }
            NetEvent::Message { conn, msg } => self.collab_host_msg(conn, msg, window, cx),
        }
    }

    /// Spend one of `conn`'s tokens for this kind of message. A guest we don't
    /// know is allowed through: the connection is about to be dropped anyway.
    fn guest_take(&mut self, conn: u64, limb: Limb) -> bool {
        let Some(Role::Host { guests, .. }) = &mut self.collab.role else { return true };
        let Some(g) = guests.get_mut(&conn) else { return true };
        match limb {
            Limb::Open => g.opens.take(),
            Limb::Edit => g.edits.take(),
            Limb::Cursor => g.cursors.take(),
        }
    }

    fn collab_host_msg(&mut self, conn: u64, msg: Msg, window: &mut Window, cx: &mut Context<Self>) {
        let reply = |this: &Self, m: Msg| {
            if let Some(Role::Host { guests, .. }) = &this.collab.role {
                if let Some(g) = guests.get(&conn) {
                    let _ = g.tx.send(m);
                }
            }
        };
        match msg {
            Msg::Hello { name, github, .. } => {
                let peer = Peer { id: conn, name: name.clone(), github, host: false, file: String::new(), line: 0 };
                if let Some(Role::Host { guests, .. }) = &mut self.collab.role {
                    if let Some(g) = guests.get_mut(&conn) {
                        g.peer = Some(peer.clone());
                    }
                }
                self.collab.peers.push(peer);
                let root = self.collab.root.clone();
                let files: Vec<String> = self.all_files.iter().filter_map(|f| root.as_ref().and_then(|r| Path::new(f).strip_prefix(r).ok()).map(|p| p.to_string_lossy().replace('\\', "/"))).filter(|p| share::is_shareable(p)).take(MAX_LISTED).collect();
                reply(self, Msg::Welcome { you: conn, host: self.collab.host_name.clone(), folder: self.collab.folder.clone(), files, peers: self.collab.peers.clone() });
                let peers = self.collab.peers.clone();
                self.collab.send_all(&Msg::Peers { peers }, Some(conn));
                self.status = crate::trf!("{name} joined the session"; name => name).into();
            }
            Msg::Open { path } => {
                if !self.guest_take(conn, Limb::Open) {
                    reply(self, Msg::Refused { why: "Too many requests, slow down.".into() });
                    return;
                }
                match self.collab_host_open(&path, window, cx) {
                    Ok((rev, text)) => reply(self, Msg::Snapshot { path, rev, text }),
                    Err(why) => reply(self, Msg::Refused { why }),
                }
            }
            Msg::Edit { path, rev, op, ai } => {
                if !self.guest_take(conn, Limb::Edit) {
                    return;
                }
                let who = self.collab.name_of(conn);
                let Some(doc) = self.collab.docs.get_mut(&path) else { return };
                let Some(server) = doc.server.as_mut() else { return };
                let before = server.text.clone();
                match server.receive(rev, op) {
                    Ok(applied) => {
                        let (new_rev, text) = (server.rev, server.text.clone());
                        doc.shadow = text.clone();
                        let path_abs = doc.path.clone();
                        if let Some(d) = self.docs.iter().find(|d| d.path == path_abs) {
                            let ed = d.editor.clone();
                            apply_to_editor(&ed, &applied, window, cx);
                        }
                        reply(self, Msg::Ack { path: path.clone(), rev: new_rev });
                        self.collab.send_all(&Msg::Applied { path: path.clone(), rev: new_rev, op: applied, author: conn, ai: ai.clone() }, Some(conn));
                        self.collab.note_edit(who, &path, ai, &before, &text);
                    }
                    Err(_) => {
                        // Too far behind to merge: send the file again.
                        let (rev, text) = (server.rev, server.text.clone());
                        reply(self, Msg::Snapshot { path, rev, text });
                    }
                }
            }
            Msg::Cursor { path, line } => {
                if !self.guest_take(conn, Limb::Cursor) {
                    return;
                }
                if let Some(p) = self.collab.peers.iter_mut().find(|p| p.id == conn) {
                    p.file = path;
                    p.line = line;
                }
                let peers = self.collab.peers.clone();
                self.collab.send_all(&Msg::Peers { peers }, None);
            }
            _ => {}
        }
    }

    /// A guest opens `rel`: the host opens it too (in a tab, without moving away from
    /// what they are on) and serves its text.
    fn collab_host_open(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) -> Result<(u64, String), String> {
        share::ensure_shareable(rel)?;
        let abs = self.collab.abs_of(rel).ok_or_else(|| tr("That file is outside the shared folder.").to_string())?;
        if !abs.is_file() {
            return Err(tr("That file is not there any more.").into());
        }
        if let Some(d) = self.collab.docs.get(rel) {
            if let Some(s) = &d.server {
                return Ok((s.rev, s.text.clone()));
            }
        }
        if !self.docs.iter().any(|d| d.path == abs) {
            let keep = self.active;
            self.open_file(abs.clone(), window, cx);
            if let Some(k) = keep {
                self.activate(k, window, cx);
            }
        }
        // A large Flux document's editor fills now: guests edit through it.
        if let Some(ix) = self.docs.iter().position(|d| d.path == abs) {
            self.flux_load_source(ix, window, cx);
        }
        let d = self.docs.iter().find(|d| d.path == abs).ok_or_else(|| tr("That file can't be opened as text.").to_string())?;
        if d.read_only {
            return Err(tr("That file can't be shared as text.").into());
        }
        let text = d.editor.read(cx).value().to_string();
        if text.len() > MAX_SHARED_FILE {
            return Err(tr("That file is too large to share live (over 16 MB).").into());
        }
        self.collab.docs.insert(rel.to_string(), SharedDoc { path: abs, shadow: text.clone(), server: Some(ot::Server::new(text.clone())), client: Default::default(), rev: 0, ai_next: false, pending_ai: None });
        Ok((0, text))
    }

    fn collab_guest_event(&mut self, ev: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        let msg = match ev {
            NetEvent::Message { msg, .. } => msg,
            NetEvent::Closed { why, .. } => {
                if self.collab.active() {
                    self.collab.error = Some(crate::trf!("The session ended ({why})."; why => why).into());
                    self.collab_stop("", cx);
                }
                return;
            }
            NetEvent::Connected { .. } => return,
        };
        match msg {
            Msg::Welcome { you, host, folder, files, peers } => {
                let c = &mut self.collab;
                c.me = you;
                c.host_name = host;
                c.folder = folder;
                c.files = files;
                c.peers = peers;
            }
            Msg::Peers { peers } => self.collab.peers = peers,
            Msg::Snapshot { path, rev, text } => {
                let Some(abs) = self.collab.abs_of(&path) else { return };
                if let Some(dir) = abs.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                match self.docs.iter().position(|d| d.path == abs) {
                    Some(ix) => {
                        let ed = self.docs[ix].editor.clone();
                        let t = text.clone();
                        ed.update(cx, |s, cx| s.set_value(t, window, cx));
                        self.activate(ix, window, cx);
                    }
                    None => {
                        let _ = std::fs::write(&abs, &text);
                        self.open_file(abs.clone(), window, cx);
                    }
                }
                self.collab.docs.insert(path, SharedDoc { path: abs, shadow: text, server: None, client: Default::default(), rev, ai_next: false, pending_ai: None });
            }
            Msg::Applied { path, rev, op, author, ai } => {
                self.collab_flush_local(&path, cx);
                let who = self.collab.name_of(author);
                let Some(doc) = self.collab.docs.get_mut(&path) else { return };
                let mine = match doc.client.remote(op) {
                    Ok(o) => o,
                    Err(_) => {
                        // Out of step: ask for the file again.
                        self.collab.send_host(Msg::Open { path });
                        return;
                    }
                };
                let before = doc.shadow.clone();
                let Ok(after) = mine.apply(&doc.shadow) else {
                    self.collab.send_host(Msg::Open { path });
                    return;
                };
                doc.shadow = after.clone();
                doc.rev = rev;
                let abs = doc.path.clone();
                if let Some(d) = self.docs.iter().find(|d| d.path == abs) {
                    let ed = d.editor.clone();
                    apply_to_editor(&ed, &mine, window, cx);
                }
                self.collab.note_edit(who, &path, ai, &before, &after);
            }
            Msg::Ack { path, rev } => {
                let Some(doc) = self.collab.docs.get_mut(&path) else { return };
                doc.rev = rev;
                if let Some(next) = doc.client.ack() {
                    let ai = doc.pending_ai.take();
                    let rev = doc.rev;
                    self.collab.send_host(Msg::Edit { path, rev, op: next, ai });
                } else {
                    doc.pending_ai = None;
                }
            }
            Msg::Refused { why } => self.status = why.into(),
            Msg::Bye { why } => {
                // The host's reason is in the host's language; say it in ours.
                let _ = why;
                self.collab.error = Some(tr("The host ended the session.").into());
                self.collab_stop("", cx);
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- local edits

    /// Called on every change of an editor: a shared file's edit goes out.
    pub(crate) fn collab_on_change(&mut self, editor: &Entity<EditorState>, cx: &mut Context<Self>) {
        if !self.collab.active() {
            return;
        }
        let Some(d) = self.docs.iter().find(|d| &d.editor == editor) else { return };
        let Some(rel) = self.collab.rel_of(&d.path) else { return };
        self.collab_flush_local(&rel, cx);
    }

    /// A file's editor changed without a change event (a reload): sync it.
    pub(crate) fn collab_on_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Some(rel) = self.collab.rel_of(path) {
            self.collab_flush_local(&rel, cx);
        }
    }

    /// Send whatever the editor of `rel` has that the last sync doesn't.
    fn collab_flush_local(&mut self, rel: &str, cx: &mut Context<Self>) {
        let ai_info = self.agent_attribution();
        let me = self.collab.peers.iter().find(|p| p.id == self.collab.me).map(|p| p.name.clone()).unwrap_or_default();
        let host = self.collab.is_host();
        let Some(doc) = self.collab.docs.get_mut(rel) else { return };
        let Some(d) = self.docs.iter().find(|d| d.path == doc.path) else { return };
        let now = d.editor.read(cx).value().to_string();
        if now == doc.shadow {
            return;
        }
        let op = Op::diff(&doc.shadow, &now);
        let before = std::mem::replace(&mut doc.shadow, now.clone());
        let ai = if std::mem::take(&mut doc.ai_next) { Some(ai_info) } else { None };
        if host {
            let Some(server) = doc.server.as_mut() else { return };
            if server.local(op.clone()).is_err() {
                return;
            }
            let rev = server.rev;
            self.collab.send_all(&Msg::Applied { path: rel.to_string(), rev, op, author: 0, ai: ai.clone() }, None);
        } else {
            if ai.is_some() {
                doc.pending_ai = ai.clone();
            }
            match doc.client.local(op) {
                Ok(Some(send)) => {
                    let rev = doc.rev;
                    let a = doc.pending_ai.take().or(ai.clone());
                    self.collab.send_host(Msg::Edit { path: rel.to_string(), rev, op: send, ai: a });
                }
                Ok(None) => {}
                Err(_) => self.collab.send_host(Msg::Open { path: rel.to_string() }),
            }
        }
        self.collab.note_edit(me, rel, ai, &before, &now);
    }

    /// The agent is about to rewrite `path` in its editor: tag that change as AI.
    pub(crate) fn collab_mark_ai(&mut self, path: &Path) {
        if let Some(rel) = self.collab.rel_of(path) {
            if let Some(doc) = self.collab.docs.get_mut(&rel) {
                doc.ai_next = true;
            }
        }
    }

    /// The host shares a file the moment its tab opens, so guests can follow it.
    pub(crate) fn collab_cursor(&mut self, cx: &mut Context<Self>) {
        if !self.collab.active() {
            return;
        }
        let Some(ix) = self.active else { return };
        let Some(d) = self.docs.get(ix) else { return };
        let Some(rel) = self.collab.rel_of(&d.path) else { return };
        let line = d.editor.read(cx).cursor_position().line + 1;
        if self.collab.last_cursor == (rel.clone(), line) {
            return;
        }
        self.collab.last_cursor = (rel.clone(), line);
        if self.collab.is_host() {
            if let Some(p) = self.collab.peers.iter_mut().find(|p| p.id == 0) {
                p.file = rel;
                p.line = line;
            }
            let peers = self.collab.peers.clone();
            self.collab.send_all(&Msg::Peers { peers }, None);
        } else {
            self.collab.send_host(Msg::Cursor { path: rel, line });
        }
    }

    /// Open a shared file (guest): ask the host for it.
    fn collab_open(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = self.collab.docs.get(&rel) {
            let p = doc.path.clone();
            self.open_file(p, window, cx);
            return;
        }
        self.collab.send_host(Msg::Open { path: rel });
    }

    fn collab_show_diff(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.collab.feed.get(ix) else { return };
        let text = crate::diff::unified(&f.rel, &f.before, &f.after);
        let dir = std::env::temp_dir().join("fluxcode-diff");
        let name = f.rel.rsplit('/').next().unwrap_or("change").to_string();
        let file = dir.join(format!("{name}-{ix}.diff"));
        if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&file, text).is_ok() {
            if let Some(i) = self.docs.iter().position(|d| d.path == file) {
                self.close(i, window, cx);
            }
            self.open_file(file, window, cx);
        }
    }
}

enum Via {
    Lan,
    Relay,
    Internet,
}

/// How a guest reached the host, from the address it connected to.
fn path_kind(addr: &str) -> Via {
    if addr.starts_with(reach::RELAY_HOST) {
        return Via::Relay;
    }
    match addr.parse::<std::net::SocketAddr>().map(|s| s.ip()) {
        Ok(std::net::IpAddr::V4(ip)) if reach::is_private_v4(&ip) => Via::Lan,
        Ok(_) => Via::Internet,
        Err(_) => Via::Lan,
    }
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
