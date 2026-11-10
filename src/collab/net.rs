//! The wire: the host's PC listens on TCP (IPv4 and IPv6); each guest connects to it
//! directly, or through the public relay when the host opened one (`reach`). No server
//! of ours is involved.
//!
//! Every frame is encrypted and authenticated with ChaCha20-Poly1305. The key is the
//! 32-byte secret inside the invite code, so only someone holding the code can join
//! or read the traffic. Each connection derives its own key from that secret and a
//! random salt from both sides, and each direction counts its own nonces, so no
//! nonce is ever used twice under one key. The host's first frame proves it holds the
//! same secret, so a guest knows it reached the right session before it says hello.
//!
//! Frames are `u32` length + ciphertext of one JSON message. Reading and writing run
//! on two threads per connection; the UI thread only sees whole messages.

use super::proto::Msg;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 47615;
/// Version 2 adds the host's proof frame; a version 1 peer is turned away at once.
const MAGIC: &[u8; 4] = b"FXC2";
const PROOF: &[u8] = b"fluxcode-live-share";
/// Larger frames are refused (a file snapshot above this is not shared).
pub const MAX_FRAME: usize = 24 << 20;

/// What the invite code carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Invite {
    /// Addresses to try (all at once, earlier ones a moment sooner): the host's LAN
    /// address first, then the ways in from the internet.
    pub a: Vec<String>,
    /// The session secret (base64url, 32 bytes).
    pub k: String,
    /// Who hosts, and the shared folder's name, for the join screen.
    pub h: String,
    pub n: String,
}

impl Invite {
    pub fn code(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        format!("FLUX1-{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
    }

    pub fn parse(code: &str) -> Result<Invite, String> {
        let body = code.trim().strip_prefix("FLUX1-").ok_or("That is not a FluxCode invite code (they start with FLUX1-).")?;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body.trim()).map_err(|_| "The invite code is incomplete. Copy it again.".to_string())?;
        let inv: Invite = serde_json::from_slice(&bytes).map_err(|_| "The invite code is damaged. Copy it again.".to_string())?;
        if inv.secret().is_none() || inv.a.is_empty() {
            return Err("The invite code is damaged. Copy it again.".into());
        }
        Ok(inv)
    }

    pub fn secret(&self) -> Option<[u8; 32]> {
        let v = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&self.k).ok()?;
        v.try_into().ok()
    }
}

pub fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("the system's random source is unavailable");
    b
}

#[cfg(test)]
pub fn new_secret() -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random::<32>())
}

/// This PC's address on its network (the route to the internet), without sending
/// anything: connecting a UDP socket only picks the interface.
pub fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// One connection's cipher: its own key, and a nonce counter per direction.
struct Cipher {
    aead: ChaCha20Poly1305,
    /// 1 host -> guest, 2 guest -> host.
    send_dir: u8,
    sent: u64,
    received: u64,
}

impl Cipher {
    fn new(secret: &[u8; 32], host_salt: &[u8; 6], guest_salt: &[u8; 6], is_host: bool) -> Cipher {
        // The connection key: the secret's keystream at a nonce made of both salts.
        let master = ChaCha20Poly1305::new(Key::from_slice(secret));
        let mut n = [0u8; 12];
        n[..6].copy_from_slice(host_salt);
        n[6..].copy_from_slice(guest_salt);
        let ks = master.encrypt(Nonce::from_slice(&n), [0u8; 32].as_slice()).expect("key derivation");
        let aead = ChaCha20Poly1305::new(Key::from_slice(&ks[..32]));
        Cipher { aead, send_dir: if is_host { 1 } else { 2 }, sent: 0, received: 0 }
    }

    fn nonce(dir: u8, counter: u64) -> [u8; 12] {
        let mut n = [0u8; 12];
        n[0] = dir;
        n[4..].copy_from_slice(&counter.to_be_bytes());
        n
    }

    fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let n = Self::nonce(self.send_dir, self.sent);
        self.sent += 1;
        self.aead.encrypt(Nonce::from_slice(&n), plain).expect("encrypt")
    }

    fn open(&mut self, sealed: &[u8]) -> Option<Vec<u8>> {
        let dir = if self.send_dir == 1 { 2 } else { 1 };
        let n = Self::nonce(dir, self.received);
        self.received += 1;
        self.aead.decrypt(Nonce::from_slice(&n), sealed).ok()
    }
}

/// What the network threads tell the UI.
pub enum NetEvent {
    /// A guest finished the handshake (host side).
    Connected { conn: u64, tx: Sender<Msg> },
    Message { conn: u64, msg: Msg },
    Closed { conn: u64, why: String },
}

fn write_frame(s: &mut TcpStream, c: &mut Cipher, msg: &Msg) -> std::io::Result<()> {
    let plain = serde_json::to_vec(msg).unwrap_or_default();
    let sealed = c.seal(&plain);
    s.write_all(&(sealed.len() as u32).to_be_bytes())?;
    s.write_all(&sealed)?;
    s.flush()
}

fn read_frame(s: &mut TcpStream, c: &mut Cipher) -> Result<Msg, String> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len).map_err(|e| e.to_string())?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_FRAME + 64 {
        return Err("a message was too large".into());
    }
    let mut buf = vec![0u8; n];
    s.read_exact(&mut buf).map_err(|e| e.to_string())?;
    let plain = c.open(&buf).ok_or("the connection was not from this session (wrong invite code)")?;
    serde_json::from_slice(&plain).map_err(|e| format!("unreadable message: {e}"))
}

/// Run one connection: a reader on this thread, a writer on another.
fn run_conn(conn: u64, stream: TcpStream, cipher: Cipher, events: UnboundedSender<NetEvent>, rx: std::sync::mpsc::Receiver<Msg>) {
    let _ = stream.set_nodelay(true);
    let Ok(mut ws) = stream.try_clone() else { return };
    let mut rs = stream;
    // Both directions share the cipher's key but not its counters.
    let Cipher { aead, send_dir, sent, received } = cipher;
    let mut wc = Cipher { aead: aead.clone(), send_dir, sent, received: 0 };
    let mut rc = Cipher { aead, send_dir, sent: 0, received };
    std::thread::spawn(move || {
        while let Ok(m) = rx.recv() {
            let end = matches!(m, Msg::Bye { .. });
            if write_frame(&mut ws, &mut wc, &m).is_err() || end {
                let _ = ws.shutdown(std::net::Shutdown::Both);
                break;
            }
        }
    });
    let why = loop {
        match read_frame(&mut rs, &mut rc) {
            Ok(msg) => {
                if events.unbounded_send(NetEvent::Message { conn, msg }).is_err() {
                    break "closed".to_string();
                }
            }
            Err(e) => break e,
        }
    };
    // Close both halves, so the other side's reader ends too.
    let _ = rs.shutdown(std::net::Shutdown::Both);
    let _ = events.unbounded_send(NetEvent::Closed { conn, why });
}

/// A listening host. Dropping it stops taking new guests (existing ones stay until
/// they are sent `Bye`).
pub struct Host {
    pub port: u16,
    /// Also listening on IPv6.
    pub ipv6: bool,
    stop: Arc<AtomicBool>,
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wake the accept loops.
        let _ = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], self.port)), Duration::from_millis(200));
        if self.ipv6 {
            let _ = TcpStream::connect_timeout(&SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, self.port)), Duration::from_millis(200));
        }
    }
}

/// Listen on the default port (or any free one) for guests holding `secret`, on IPv4
/// and, when the PC has it, IPv6 on the same port.
pub fn host(secret: [u8; 32]) -> Result<(Host, UnboundedReceiver<NetEvent>), String> {
    let listener = TcpListener::bind(("0.0.0.0", DEFAULT_PORT)).or_else(|_| TcpListener::bind(("0.0.0.0", 0))).map_err(|e| format!("Could not open a port for the session: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let v6 = bind_v6_only(port);
    let (tx, rx) = unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let next = Arc::new(AtomicU64::new(1));
    let ipv6 = v6.is_some();
    for l in std::iter::once(listener).chain(v6) {
        let (tx, stop, next) = (tx.clone(), stop.clone(), next.clone());
        std::thread::spawn(move || accept_loop(l, secret, tx, stop, next));
    }
    Ok((Host { port, ipv6, stop }, rx))
}

/// An IPv6 listener that leaves IPv4 to the other one (on Linux an IPv6 socket would
/// otherwise claim the IPv4 port too, and fail to bind next to it).
fn bind_v6_only(port: u16) -> Option<TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};
    let s = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP)).ok()?;
    s.set_only_v6(true).ok()?;
    // What std's TcpListener::bind does on Unix, so a restarted session can reuse the port.
    #[cfg(unix)]
    let _ = s.set_reuse_address(true);
    s.bind(&SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, port)).into()).ok()?;
    s.listen(128).ok()?;
    Some(s.into())
}

fn accept_loop(listener: TcpListener, secret: [u8; 32], tx: UnboundedSender<NetEvent>, stop: Arc<AtomicBool>, next: Arc<AtomicU64>) {
    for stream in listener.incoming() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(mut stream) = stream else { continue };
        let events = tx.clone();
        let conn = next.fetch_add(1, Ordering::Relaxed);
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
            let mut hello = [0u8; 10];
            if stream.read_exact(&mut hello).is_err() || &hello[..4] != MAGIC {
                return;
            }
            let guest_salt: [u8; 6] = hello[4..].try_into().unwrap();
            let host_salt = random::<6>();
            let mut cipher = Cipher::new(&secret, &host_salt, &guest_salt, true);
            let proof = cipher.seal(PROOF);
            let mut out = host_salt.to_vec();
            out.extend_from_slice(&(proof.len() as u32).to_be_bytes());
            out.extend_from_slice(&proof);
            if stream.write_all(&out).is_err() {
                return;
            }
            let _ = stream.set_read_timeout(None);
            let (mtx, mrx) = channel();
            if events.unbounded_send(NetEvent::Connected { conn, tx: mtx }).is_err() {
                return;
            }
            run_conn(conn, stream, cipher, events, mrx);
        });
    }
}

/// Connect to one address and check that the host there holds the session's secret.
fn try_one(addr: &str, secret: &[u8; 32]) -> Result<(TcpStream, Cipher), String> {
    let sockets: Vec<SocketAddr> = addr.to_socket_addrs().map_err(|e| format!("{addr}: {e}"))?.collect();
    let mut last = format!("{addr}: no address");
    for sa in sockets {
        let mut stream = match TcpStream::connect_timeout(&sa, Duration::from_secs(5)) {
            Ok(s) => s,
            Err(e) => {
                last = format!("{addr}: {e}");
                continue;
            }
        };
        let guest_salt = random::<6>();
        let mut hello = MAGIC.to_vec();
        hello.extend_from_slice(&guest_salt);
        // Through the relay the host's answer takes a round trip more.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let mut host_salt = [0u8; 6];
        let mut len = [0u8; 4];
        if stream.write_all(&hello).is_err() || stream.read_exact(&mut host_salt).is_err() || stream.read_exact(&mut len).is_err() {
            last = format!("{addr} answered but is not a FluxCode host of this version (or the session ended)");
            continue;
        }
        let n = (u32::from_be_bytes(len) as usize).min(256);
        let mut proof = vec![0u8; n];
        if stream.read_exact(&mut proof).is_err() {
            last = format!("{addr} stopped answering");
            continue;
        }
        let mut cipher = Cipher::new(secret, &host_salt, &guest_salt, false);
        if cipher.open(&proof).as_deref() != Some(PROOF) {
            last = format!("{addr} is hosting a different session (the invite code is out of date)");
            continue;
        }
        let _ = stream.set_read_timeout(None);
        return Ok((stream, cipher));
    }
    Err(last)
}

/// Connect to a host from an invite. Every address is tried at once, each a moment
/// after the one before (the same network beats the internet), and the first that
/// proves it is this session's host is used.
pub fn join(inv: &Invite) -> Result<(Sender<Msg>, UnboundedReceiver<NetEvent>, String), String> {
    let secret = inv.secret().ok_or("The invite code is damaged.")?;
    let (rtx, rrx) = channel();
    for (i, a) in inv.a.iter().enumerate() {
        let (a, rtx) = (a.clone(), rtx.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150 * i as u64));
            let r = try_one(&a, &secret);
            let _ = rtx.send((a, r));
        });
    }
    drop(rtx);
    let mut errors = Vec::new();
    while let Ok((a, r)) = rrx.recv() {
        match r {
            Ok((stream, cipher)) => {
                let (tx, rx) = unbounded();
                let (mtx, mrx) = channel();
                std::thread::spawn(move || run_conn(0, stream, cipher, tx, mrx));
                return Ok((mtx, rx, a));
            }
            Err(e) => errors.push(e),
        }
    }
    Err(format!("Could not reach the host. Tried {}. Ask the host to send a fresh code once their panel shows a way in from the internet.", errors.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::{host, join, random, Cipher, Invite, NetEvent};
    use crate::collab::proto::Msg;
    use futures::StreamExt;

    #[test]
    fn invite_codes_round_trip_and_reject_garbage() {
        let inv = Invite { a: vec!["192.168.1.4:47615".into()], k: super::new_secret(), h: "octo".into(), n: "game".into() };
        let code = inv.code();
        assert!(code.starts_with("FLUX1-"));
        assert_eq!(Invite::parse(&format!("  {code}\n")).unwrap(), inv);
        assert!(Invite::parse("hello").is_err());
        assert!(Invite::parse("FLUX1-%%%").is_err());
    }

    #[test]
    fn frames_open_only_with_the_same_session_and_in_order() {
        let (secret, hs, gs) = (random::<32>(), random::<6>(), random::<6>());
        let mut h = Cipher::new(&secret, &hs, &gs, true);
        let mut g = Cipher::new(&secret, &hs, &gs, false);
        let a = h.seal(b"one");
        let b = h.seal(b"two");
        assert_eq!(g.open(&a).unwrap(), b"one");
        assert_eq!(g.open(&b).unwrap(), b"two");
        // A replayed frame fails (the counter moved on).
        assert!(g.open(&a).is_none());
        // Another secret can't read it.
        let mut other = Cipher::new(&random::<32>(), &hs, &gs, false);
        assert!(other.open(&h.seal(b"x")).is_none());
    }

    #[test]
    fn a_guest_connects_and_talks_over_loopback() {
        let secret = random::<32>();
        let (srv, mut hrx) = host(secret).unwrap();
        let inv = Invite { a: vec![format!("127.0.0.1:{}", srv.port)], k: base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, secret), h: "h".into(), n: "n".into() };
        let (gtx, mut grx, _) = join(&inv).unwrap();
        gtx.send(Msg::Cursor { path: "ping".into(), line: 1 }).ok();
        let got = futures::executor::block_on(async {
            let mut tx = None;
            let mut seen = None;
            while let Some(ev) = hrx.next().await {
                match ev {
                    NetEvent::Connected { tx: t, .. } => tx = Some(t),
                    NetEvent::Message { msg, .. } => {
                        seen = Some(msg);
                        break;
                    }
                    NetEvent::Closed { .. } => break,
                }
            }
            if let Some(t) = tx {
                t.send(Msg::Refused { why: "pong".into() }).ok();
            }
            seen
        });
        assert!(matches!(got, Some(Msg::Cursor { path, .. }) if path == "ping"));
        let back = futures::executor::block_on(async {
            while let Some(ev) = grx.next().await {
                if let NetEvent::Message { msg, .. } = ev {
                    return Some(msg);
                }
            }
            None
        });
        assert!(matches!(back, Some(Msg::Refused { why }) if why == "pong"));
        // A code with another secret is told so before it says anything.
        let bad = Invite { k: super::new_secret(), ..inv.clone() };
        let err = join(&bad).err().unwrap();
        assert!(err.contains("different session"), "{err}");
        // A dead address next to a live one: the live one wins, by host name too.
        let mixed = Invite { a: vec!["127.0.0.1:1".into(), format!("localhost:{}", srv.port)], ..inv };
        let (_, _, via) = join(&mixed).unwrap();
        assert_eq!(via, format!("localhost:{}", srv.port));
        if srv.ipv6 {
            let v6 = Invite { a: vec![format!("[::1]:{}", srv.port)], ..mixed };
            assert!(join(&v6).is_ok(), "IPv6 listener");
        }
    }
}
