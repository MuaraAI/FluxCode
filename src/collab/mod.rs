
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

const MAX_SHARED_FILE: usize = 16 << 20;
const MAX_LISTED: usize = 20_000;
const MAX_GUESTS: usize = 16;
const MAX_CURSOR_PATH: usize = 1024;
const MAX_PEER_NAME: usize = 64;

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

#[derive(Clone, Copy)]
enum Limb {
    Open,
    Edit,
    Cursor,
}

pub struct SharedDoc {
    pub path: PathBuf,
    shadow: String,
    server: Option<ot::Server>,
    client: ot::Client,
    rev: u64,
    ai_next: bool,
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
    pub root: Option<PathBuf>,
    pub peers: Vec<Peer>,
    pub files: Vec<String>,
    pub docs: HashMap<String, SharedDoc>,
    pub feed: Vec<FeedItem>,
    pub error: Option<String>,
    pub connecting: bool,
    pub join_input: Entity<InputState>,
    pub filter_input: Entity<InputState>,
    pub into_open_folder: bool,
    pub internet: bool,
    pub probes: Option<reach::Probes>,
    pub reach: Option<reach::Reach>,
    pub copied: Option<Instant>,
    pub copied_code: Option<String>,
    pub ways_open: bool,
    pub page: bool,
    last_cursor: (String, u32),
    host_opens: ratelimit::Bucket,
    _subs: Vec<Subscription>,
}

impl Collab {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let join_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Paste an invite code (FLUX1-…)")));
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Find a shared file")));
        let mut _subs: Vec<Subscription> = [&join_input, &filter_input].into_iter().map(|i| cx.subscribe(i, |_: &mut Workbench, _, _: &InputEvent, cx| cx.notify())).collect();
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
            ways_open: false,
            page: false,
            last_cursor: (String::new(), 0),
            host_opens: ratelimit::Bucket::new(120, 40),
            _subs,
        }
    }

    pub fn active(&self) -> bool {
        self.role.is_some()
    }

    fn is_host(&self) -> bool {
        matches!(self.role, Some(Role::Host { .. }))
    }

    pub fn rel_of(&self, path: &Path) -> Option<String> {
        let root = self.root.as_ref()?;
        let rel = path.strip_prefix(root).ok()?;
        let s = rel.to_string_lossy().replace('\\', "/");
        (!s.is_empty()).then_some(s)
    }

    fn abs_of(&self, rel: &str) -> Option<PathBuf> {
        let parts = share::safe_components(rel)?;
        Some(parts.iter().fold(self.root.clone()?, |p, s| p.join(s)))
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

fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map_or(s.len(), |(b, _)| b)
}

fn char_at(s: &str, bi: usize) -> usize {
    s[..bi.min(s.len())].chars().count()
}

fn apply_to_editor(editor: &Entity<EditorState>, op: &Op, window: &mut Window, cx: &mut App) {
    editor.update(cx, |s, cx| {
        let text = s.value().to_string();
        let sel = s.selected_range();
        let (a, b) = (char_at(&text, sel.start), char_at(&text, sel.end));
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
                let full = if let Some(Role::Host { guests, .. }) = &self.collab.role { guests.len() >= MAX_GUESTS } else { false };
                if full {
                    let _ = tx.send(Msg::Refused { why: tr("This session is full.").to_string() });
                    let _ = tx.send(Msg::Bye { why: tr("This session is full.").to_string() });
                    return;
                }
                if let Some(Role::Host { guests, .. }) = &mut self.collab.role {
                    guests.insert(conn, Guest { tx, peer: None, opens: ratelimit::Bucket::new(10, 1), edits: ratelimit::Bucket::new(60, 60), cursors: ratelimit::Bucket::new(20, 20) });
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

    fn guest_take(&mut self, conn: u64, limb: Limb, weight: f64) -> bool {
        if matches!(limb, Limb::Open) && !self.collab.host_opens.take_weight(1.0) {
            return false;
        }
        let Some(Role::Host { guests, .. }) = &mut self.collab.role else { return true };
        let Some(g) = guests.get_mut(&conn) else { return true };
        let bucket = match limb {
            Limb::Open => &mut g.opens,
            Limb::Edit => &mut g.edits,
            Limb::Cursor => &mut g.cursors,
        };
        bucket.take_weight(weight)
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
                if !self.guest_take(conn, Limb::Open, 1.0) {
                    reply(self, Msg::Refused { why: tr("Too many requests, slow down.").to_string() });
                    return;
                }
                let name: String = name.trim().chars().take(MAX_PEER_NAME).collect();
                let peer = Peer { id: conn, name: name.clone(), github, host: false, file: String::new(), line: 0 };
                if let Some(Role::Host { guests, .. }) = &mut self.collab.role {
                    if let Some(g) = guests.get_mut(&conn) {
                        g.peer = Some(peer.clone());
                    }
                }
                match self.collab.peers.iter_mut().find(|p| p.id == conn) {
                    Some(slot) => *slot = peer,
                    None => self.collab.peers.push(peer),
                }
                let root = self.collab.root.clone();
                let files: Vec<String> = self.all_files.iter().filter_map(|f| root.as_ref().and_then(|r| Path::new(f).strip_prefix(r).ok()).map(|p| p.to_string_lossy().replace('\\', "/"))).filter(|p| share::is_shareable(p)).take(MAX_LISTED).collect();
                reply(self, Msg::Welcome { you: conn, host: self.collab.host_name.clone(), folder: self.collab.folder.clone(), files, peers: self.collab.peers.clone() });
                let peers = self.collab.peers.clone();
                self.collab.send_all(&Msg::Peers { peers }, Some(conn));
                self.status = crate::trf!("{name} joined the session"; name => name).into();
            }
            Msg::Open { path } => {
                if !self.guest_take(conn, Limb::Open, 1.0) {
                    reply(self, Msg::Refused { why: tr("Too many requests, slow down.").to_string() });
                    return;
                }
                match self.collab_host_open(&path, window, cx) {
                    Ok((rev, text)) => reply(self, Msg::Snapshot { path, rev, text }),
                    Err(why) => reply(self, Msg::Refused { why }),
                }
            }
            Msg::Edit { path, rev, op, ai } => {
                let behind = self.collab.docs.get(&path).and_then(|d| d.server.as_ref()).map(|s| s.rev.saturating_sub(rev) as f64).unwrap_or(0.0);
                let weight = 1.0 + op.changed_bytes() as f64 / 1024.0 + behind / 64.0;
                if !self.guest_take(conn, Limb::Edit, weight) {
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
                        if let Some(ix) = self.docs.iter().position(|d| d.path == path_abs) {
                            if let Some(ed) = self.ensure_doc_editor(ix, window, cx) {
                                apply_to_editor(&ed, &applied, window, cx);
                            }
                        }
                        reply(self, Msg::Ack { path: path.clone(), rev: new_rev });
                        self.collab.send_all(&Msg::Applied { path: path.clone(), rev: new_rev, op: applied, author: conn, ai: ai.clone() }, Some(conn));
                        self.collab.note_edit(who, &path, ai, &before, &text);
                    }
                    Err(_) => {
                        let (rev, text) = (server.rev, server.text.clone());
                        reply(self, Msg::Snapshot { path, rev, text });
                    }
                }
            }
            Msg::Cursor { path, line } => {
                if !self.guest_take(conn, Limb::Cursor, 1.0) || path.len() > MAX_CURSOR_PATH {
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

    fn collab_host_open(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) -> Result<(u64, String), String> {
        share::ensure_shareable(rel)?;
        let abs = self.collab.abs_of(rel).ok_or_else(|| tr("That file is outside the shared folder.").to_string())?;
        let resolved = std::fs::canonicalize(&abs).map_err(|_| tr("That file is not there any more.").to_string())?;
        let root = self.collab.root.as_ref().and_then(|r| std::fs::canonicalize(r).ok()).ok_or_else(|| tr("That file is outside the shared folder.").to_string())?;
        if !resolved.starts_with(&root) {
            return Err(tr("That file is outside the shared folder.").to_string());
        }
        let canon_rel = resolved
            .strip_prefix(&root)
            .map_err(|_| tr("That file is outside the shared folder.").to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        share::ensure_shareable(&canon_rel)?;
        if !resolved.is_file() {
            return Err(tr("That file is not there any more.").to_string());
        }
        if let Some(d) = self.collab.docs.get(rel) {
            if let Some(s) = &d.server {
                return Ok((s.rev, s.text.clone()));
            }
        }
        if !self.docs.iter().any(|d| d.path == abs) {
            let keep = self.active;
            self.open_file_now(abs.clone(), window, cx);
            if let Some(k) = keep {
                self.activate(k, window, cx);
            }
        }
        if let Some(ix) = self.docs.iter().position(|d| d.path == abs) {
            self.ensure_doc_editor(ix, window, cx);
            self.flux_load_source(ix, window, cx);
        }
        let d = self.docs.iter().find(|d| d.path == abs).ok_or_else(|| tr("That file can't be opened as text.").to_string())?;
        if d.read_only {
            return Err(tr("That file can't be shared as text.").into());
        }
        let text = d.text(cx);
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
                        if let Some(ed) = self.ensure_doc_editor(ix, window, cx) {
                            let t = text.clone();
                            ed.update(cx, |s, cx| s.set_value(t, window, cx));
                        }
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
                if let Some(ix) = self.docs.iter().position(|d| d.path == abs) {
                    if let Some(ed) = self.ensure_doc_editor(ix, window, cx) {
                        apply_to_editor(&ed, &mine, window, cx);
                    }
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
                let _ = why;
                self.collab.error = Some(tr("The host ended the session.").into());
                self.collab_stop("", cx);
            }
            _ => {}
        }
    }


    pub(crate) fn collab_on_change(&mut self, editor: &Entity<EditorState>, cx: &mut Context<Self>) {
        if !self.collab.active() {
            return;
        }
        let Some(d) = self.docs.iter().find(|d| d.editor.as_ref() == Some(editor)) else { return };
        let Some(rel) = self.collab.rel_of(&d.path) else { return };
        self.collab_flush_local(&rel, cx);
    }

    pub(crate) fn collab_on_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Some(rel) = self.collab.rel_of(path) {
            self.collab_flush_local(&rel, cx);
        }
    }

    fn collab_flush_local(&mut self, rel: &str, cx: &mut Context<Self>) {
        let ai_info = self.agent_attribution();
        let me = self.collab.peers.iter().find(|p| p.id == self.collab.me).map(|p| p.name.clone()).unwrap_or_default();
        let host = self.collab.is_host();
        let Some(doc) = self.collab.docs.get_mut(rel) else { return };
        let Some(d) = self.docs.iter().find(|d| d.path == doc.path) else { return };
        let Some(ed) = d.editor.as_ref() else { return };
        let now = ed.read(cx).value().to_string();
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

    pub(crate) fn collab_mark_ai(&mut self, path: &Path) {
        if let Some(rel) = self.collab.rel_of(path) {
            if let Some(doc) = self.collab.docs.get_mut(&rel) {
                doc.ai_next = true;
            }
        }
    }

    pub(crate) fn collab_cursor(&mut self, cx: &mut Context<Self>) {
        if !self.collab.active() {
            return;
        }
        let Some(ix) = self.active else { return };
        let Some(d) = self.docs.get(ix) else { return };
        let Some(rel) = self.collab.rel_of(&d.path) else { return };
        let line = d.cursor_position(cx).line + 1;
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
    RelayPublic,
    Internet,
}

fn path_kind(addr: &str) -> Via {
    if reach::is_primary_relay(addr) {
        return Via::Relay;
    }
    if reach::is_relay_addr(addr) {
        return Via::RelayPublic;
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
