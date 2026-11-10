//! Live share: code together in real time, with the host's PC as the server.
//!
//! - The host shares the open folder and gets an invite code. Whoever has the code
//!   joins straight to the host's PC (`net`): no server of ours, encrypted.
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
pub mod proto;

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{EditorState, Input, InputState},
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
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
    last_cursor: (String, u32),
}

impl Collab {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
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
            join_input: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Paste an invite code (FLUX1-…)"))),
            filter_input: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Find a shared file"))),
            into_open_folder: false,
            last_cursor: (String::new(), 0),
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
        let mut addrs: Vec<String> = net::lan_ip().map(|ip| format!("{ip}:{port}")).into_iter().collect();
        addrs.push(format!("127.0.0.1:{port}"));
        let (name, github) = my_name();
        let folder = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let c = &mut self.collab;
        c.invite = Some(Invite { a: addrs, k: base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, secret), h: name.clone(), n: folder.clone() });
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
        cx.notify();
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
                    guests.insert(conn, Guest { tx, peer: None });
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
                let files: Vec<String> = self.all_files.iter().filter_map(|f| root.as_ref().and_then(|r| Path::new(f).strip_prefix(r).ok()).map(|p| p.to_string_lossy().replace('\\', "/"))).take(MAX_LISTED).collect();
                reply(self, Msg::Welcome { you: conn, host: self.collab.host_name.clone(), folder: self.collab.folder.clone(), files, peers: self.collab.peers.clone() });
                let peers = self.collab.peers.clone();
                self.collab.send_all(&Msg::Peers { peers }, Some(conn));
                self.status = crate::trf!("{name} joined the session"; name => name).into();
            }
            Msg::Open { path } => match self.collab_host_open(&path, window, cx) {
                Ok((rev, text)) => reply(self, Msg::Snapshot { path, rev, text }),
                Err(why) => reply(self, Msg::Refused { why }),
            },
            Msg::Edit { path, rev, op, ai } => {
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

    // ---------------------------------------------------------------- panel

    pub(crate) fn render_collab(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let section = |t: SharedString| div().pt_4().pb_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(t);
        let note = |t: SharedString| div().text_xs().text_color(c(theme::FG_DIM)).child(t);
        let mut v = v_flex().id("collab").size_full().px_3().pb_4().overflow_y_scroll().text_sm();
        if let Some(e) = &co.error {
            v = v.child(div().mt_2().p_2().rounded(px(4.)).border_1().border_color(c(theme::DANGER)).text_xs().text_color(c(theme::DANGER)).child(e.clone()));
        }
        let (me, github) = my_name();
        match &co.role {
            None => {
                let folder = self.root.as_ref().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().into_owned());
                v = v
                    .child(section(tr("Host a session").into()))
                    .child(note(match &folder {
                        Some(f) => crate::trf!("Share {folder} with the people you invite. Your PC is the server: edits go straight between your PC and theirs, encrypted."; folder => f).into(),
                        None => tr("Open a folder to share it.").into(),
                    }))
                    .when(folder.is_some(), |v| v.child(h_flex().pt_2().child(Button::new("co-host").label(tr("Start hosting")).small().primary().on_click(cx.listener(|this, _, w, cx| this.collab_host(w, cx))))))
                    .child(section(tr("Join a session").into()))
                    .child(Input::new(&co.join_input).small())
                    .child(
                        h_flex()
                            .pt_2()
                            .gap_2()
                            .child(
                                Button::new("co-into")
                                    .label(if co.into_open_folder { tr("Into my open folder") } else { tr("Into a new folder") })
                                    .xsmall()
                                    .ghost()
                                    .dropdown_caret(true)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.collab.into_open_folder = !this.collab.into_open_folder;
                                        cx.notify();
                                    })),
                            )
                            .child(div().flex_1())
                            .child(Button::new("co-join").label(if co.connecting { tr("Connecting…") } else { tr("Join") }).small().primary().on_click(cx.listener(|this, _, w, cx| this.collab_join(w, cx)))),
                    )
                    .child(div().pt_1().child(note(if co.into_open_folder {
                        tr("Your open folder should be your clone of the same repository; the host's version of each file you open replaces yours.").into()
                    } else {
                        tr("Files arrive from the host as you open them, into a folder of their own.").into()
                    })))
                    .child(div().pt_4().child(note(if github {
                        crate::trf!("Others see you as @{name} (GitHub)."; name => me).into()
                    } else {
                        crate::trf!("Others see you as {name}. Sign in to GitHub in Settings so they see your GitHub name."; name => me).into()
                    })));
            }
            Some(Role::Host { server, .. }) => {
                let code = co.invite.as_ref().map(|i| i.code()).unwrap_or_default();
                let ip = co.invite.as_ref().and_then(|i| i.a.first().cloned()).unwrap_or_default();
                let code2 = code.clone();
                v = v
                    .child(section(crate::trf!("Hosting {folder}"; folder => co.folder).into()))
                    .child(
                        div()
                            .p_2()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(theme::BORDER_STRONG))
                            .bg(c(theme::BG_ELEVATED))
                            .font_family("Cascadia Code")
                            .text_xs()
                            .text_color(c(theme::FG))
                            .child(format!("{}…{}", &code[..code.len().min(24)], &code[code.len().saturating_sub(10)..])),
                    )
                    .child(
                        h_flex()
                            .pt_2()
                            .gap_2()
                            .child(Button::new("co-copy").label(tr("Copy invite code")).small().primary().on_click(cx.listener(move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(code2.clone()));
                                this.status = tr("Invite code copied").into();
                                cx.notify();
                            })))
                            .child(Button::new("co-end").label(tr("End session")).small().ghost().on_click(cx.listener(|this, _, _, cx| this.collab_stop("The host ended the session.", cx)))),
                    )
                    .child(div().pt_2().child(note(crate::trf!("Anyone with this code can join and edit {folder}. Same network: it works as is. Over the internet: forward TCP port {port} on your router to this PC ({ip})."; folder => co.folder, port => server.port, ip => ip).into())));
            }
            Some(Role::Guest { addr, .. }) => {
                v = v
                    .child(section({
                        let gh = co.peers.iter().any(|p| p.host && p.github);
                        let host = if gh { format!("@{}", co.host_name) } else { co.host_name.clone() };
                        crate::trf!("In {host}'s {folder}"; host => host, folder => co.folder).into()
                    }))
                    .child(note(crate::trf!("Connected to {addr}, encrypted."; addr => addr).into()))
                    .child(h_flex().pt_2().child(Button::new("co-leave").label(tr("Leave")).small().ghost().on_click(cx.listener(|this, _, _, cx| this.collab_stop("", cx)))));
            }
        }
        if co.active() {
            // People.
            v = v.child(section(crate::trf!("In the session ({n})"; n => co.peers.len()).into()));
            for p in &co.peers {
                let initial: String = p.name.chars().next().unwrap_or('?').to_uppercase().collect();
                let place = if p.file.is_empty() { String::new() } else { format!("{}:{}", p.file, p.line) };
                v = v.child(
                    h_flex()
                        .py_1()
                        .gap_2()
                        .child(div().flex_none().size(theme::u(22.)).rounded_full().flex().items_center().justify_center().bg(c(theme::BG_HOVER)).text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(initial))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .child(div().truncate().text_color(c(theme::FG_STRONG)).child(if p.github { format!("@{}", p.name) } else { p.name.clone() }))
                                        .when(p.host, |h| h.child(div().text_xs().text_color(c(theme::ACCENT_TEXT)).child(tr("host"))))
                                        .when(p.id == co.me, |h| h.child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("you")))),
                                )
                                .when(!place.is_empty(), |v| v.child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(place))),
                        ),
                );
            }
            // Guest: the host's files.
            if !co.is_host() {
                let q = co.filter_input.read(cx).value().to_lowercase();
                v = v.child(section(crate::trf!("Shared files ({n})"; n => co.files.len()).into())).child(Input::new(&co.filter_input).small());
                let hits: Vec<&String> = co.files.iter().filter(|f| q.is_empty() || f.to_lowercase().contains(&q)).take(80).collect();
                for (i, f) in hits.into_iter().enumerate() {
                    let rel = f.clone();
                    let (icon, tint) = crate::files::icon_for(f.rsplit('/').next().unwrap_or(f));
                    v = v.child(
                        h_flex()
                            .id(("co-file", i))
                            .h(theme::u(24.))
                            .gap_1p5()
                            .px_1()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(Icon::new(icon).size(theme::u(13.)).text_color(theme::tint(tint)))
                            .child(div().truncate().text_xs().text_color(c(if co.docs.contains_key(f) { theme::FG_STRONG } else { theme::FG })).child(f.clone()))
                            .on_click(cx.listener(move |this, _, w, cx| this.collab_open(rel.clone(), w, cx))),
                    );
                }
            }
            // Activity.
            v = v.child(section(tr("Activity").into()));
            if co.feed.is_empty() {
                v = v.child(note(tr("Edits show here as people make them. An edit made by an AI agent says which agent, model and effort.").into()));
            }
            for (i, f) in co.feed.iter().enumerate() {
                let secs = f.at.elapsed().as_secs();
                let ago = if secs < 60 { tr("just now").to_string() } else { crate::trf!("{m} min ago"; m => secs / 60) };
                v = v.child(
                    v_flex()
                        .id(("co-feed", i))
                        .py_1p5()
                        .gap_0p5()
                        .border_b_1()
                        .border_color(c(theme::BORDER))
                        .child(
                            h_flex()
                                .gap_1()
                                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(f.who.clone()))
                                .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_MUTED)).child(f.rel.clone()))
                                .child(div().flex_none().text_xs().text_color(c(theme::GREEN)).child(format!("+{}", f.lines.0)))
                                .child(div().flex_none().text_xs().text_color(c(theme::DANGER)).child(format!("-{}", f.lines.1))),
                        )
                        .when_some(f.ai.as_ref(), |v, ai| {
                            v.child(
                                h_flex()
                                    .gap_1()
                                    .child(div().flex_none().px_1().rounded(px(3.)).bg(c(theme::ACCENT)).text_size(theme::u(10.)).font_weight(FontWeight::BOLD).text_color(c(theme::ON_ACCENT)).child("AI"))
                                    .child(div().min_w_0().truncate().text_xs().text_color(c(theme::ACCENT_TEXT)).child(ai.line())),
                            )
                        })
                        .child(
                            h_flex()
                                .gap_2()
                                .child(div().flex_1().text_xs().text_color(c(theme::FG_DIM)).child(ago))
                                .child(Button::new(("co-diff", i)).label(tr("Diff")).xsmall().ghost().on_click(cx.listener(move |this, _, w, cx| this.collab_show_diff(i, w, cx)))),
                        ),
                );
            }
        }
        v.into_any_element()
    }
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
