//! Ways for guests outside the host's network to reach the host's PC, with no server of
//! ours. All are tried together when hosting starts; each one that works becomes an
//! address in the invite code, and guests race them all (`net::join`).
//!
//! - **IPv6**: when the ISP gives the PC a public IPv6 address, guests with IPv6
//!   connect straight to it.
//! - **Router port (UPnP)**: the PC asks the home router to forward the session port,
//!   the way game servers open theirs, and reads the router's public address. That
//!   can't help behind carrier-grade NAT (the router's own address is then private),
//!   which is detected and said.
//! - **Relay (bore.pub)**: a free public TCP tunnel (github.com/ekzhang/bore). The PC
//!   keeps one connection to bore.pub open; bore.pub gives it a public port and passes
//!   each guest's bytes through. Session traffic is encrypted with the invite code's
//!   secret before it leaves either PC, so the relay only ever sees ciphertext.
//!
//! All of it is plain std networking on a few threads, and it stops with the session.

use crate::i18n::tr;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One relay we can use: our own first, `bore.pub` as a last resort.
#[derive(Clone, Debug, PartialEq)]
pub struct RelayEndpoint {
    pub host: String,
    pub port: u16,
    /// Needed to talk to a relay started with `--secret`.
    pub secret: Option<String>,
}

const PRIMARY_RELAY_HOST: &str = "live.muaraai.com";
const FALLBACK_RELAY_HOST: &str = "bore.pub";
const RELAY_CONTROL_PORT: u16 = 7835;

/// Where to try, in order. A build without a secret cannot complete the
/// handshake a protected relay asks for, so the primary is left out entirely
/// instead of failing on every session and filling the relay's log.
pub fn endpoints() -> Vec<RelayEndpoint> {
    endpoints_from(&|k| std::env::var(k).ok(), option_env!("FLUXCODE_RELAY_SECRET"))
}

fn endpoints_from(get: &dyn Fn(&str) -> Option<String>, baked: Option<&'static str>) -> Vec<RelayEndpoint> {
    let secret = get("FLUXCODE_RELAY_SECRET").filter(|s| !s.is_empty()).or_else(|| baked.map(str::to_string).filter(|s| !s.is_empty()));
    let mut out = Vec::new();
    if let Some(secret) = secret {
        let host = get("FLUXCODE_RELAY_HOST").filter(|h| !h.is_empty()).unwrap_or_else(|| PRIMARY_RELAY_HOST.to_string());
        let port = get("FLUXCODE_RELAY_PORT").and_then(|p| p.parse().ok()).unwrap_or(RELAY_CONTROL_PORT);
        out.push(RelayEndpoint { host, port, secret: Some(secret) });
    }
    if get("FLUXCODE_RELAY_FALLBACK").as_deref() != Some("0") {
        out.push(RelayEndpoint { host: FALLBACK_RELAY_HOST.to_string(), port: RELAY_CONTROL_PORT, secret: None });
    }
    out
}

/// The name to show in messages: our relay when this build can use it.
pub fn relay_host_label() -> String {
    endpoints().first().map(|e| e.host.clone()).unwrap_or_else(|| FALLBACK_RELAY_HOST.to_string())
}

/// Is `addr` (`host:port`) one of the relays we hand out?
pub fn is_relay_addr(addr: &str) -> bool {
    let Some((host, _)) = addr.rsplit_once(':') else { return false };
    endpoints().iter().any(|e| e.host == host)
}

/// Where one way in stands.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Trying,
    /// Guests can use it: the address that goes in the invite code.
    Ready(String),
    /// Not available here, and why (shown to the host).
    No(String),
    /// Not tried (the host chose same-network only).
    Off,
}

/// What the host's panel shows and the invite code is made from.
#[derive(Clone, Debug, PartialEq)]
pub struct Reach {
    pub lan: Option<String>,
    pub ipv6: State,
    pub upnp: State,
    pub relay: State,
}

impl Reach {
    pub fn local_only(lan: Option<String>) -> Reach {
        Reach { lan, ipv6: State::Off, upnp: State::Off, relay: State::Off }
    }

    /// Addresses for the invite code, best first.
    pub fn addresses(&self, port: u16) -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        if let Some(l) = &self.lan {
            a.push(format!("{l}:{port}"));
        }
        for s in [&self.upnp, &self.ipv6, &self.relay] {
            if let State::Ready(addr) = s {
                a.push(addr.clone());
            }
        }
        a.push(format!("127.0.0.1:{port}"));
        a
    }

    pub fn checking(&self) -> bool {
        [&self.ipv6, &self.upnp, &self.relay].iter().any(|s| matches!(s, State::Trying))
    }

    /// Anyone outside this network can join: through the router's port or the relay.
    /// (A public IPv6 address only helps guests with IPv6, and the router may still
    /// block it, so it doesn't count here.)
    pub fn internet(&self) -> bool {
        [&self.upnp, &self.relay].iter().any(|s| matches!(s, State::Ready(_)))
    }
}

/// Probes running for one session. Dropping it closes the relay and removes the router
/// mapping.
pub struct Probes {
    pub state: Arc<Mutex<Reach>>,
    stop: Arc<AtomicBool>,
    relay_control: Arc<Mutex<Option<TcpStream>>>,
    upnp_undo: Arc<Mutex<Option<(String, String, u16)>>>,
}

impl Probes {
    pub fn snapshot(&self) -> Reach {
        self.state.lock().map(|g| g.clone()).unwrap_or_else(|_| Reach::local_only(None))
    }
}

impl Drop for Probes {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(s) = self.relay_control.lock().ok().and_then(|mut c| c.take()) {
            let _ = s.shutdown(Shutdown::Both);
        }
        if let Some((control, service, port)) = self.upnp_undo.lock().ok().and_then(|mut u| u.take()) {
            std::thread::spawn(move || {
                let _ = soap(&control, &service, "DeletePortMapping", &format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol>"));
            });
        }
    }
}

/// Start every probe for a host listening on `port` (`ipv6`: it listens on IPv6 too).
/// The receiver gets a message whenever a probe's state changes.
pub fn start(port: u16, ipv6: bool) -> (Probes, UnboundedReceiver<()>) {
    let lan = crate::collab::net::lan_ip();
    let state = Arc::new(Mutex::new(Reach { lan: lan.clone(), ipv6: State::Trying, upnp: State::Trying, relay: State::Trying }));
    let stop = Arc::new(AtomicBool::new(false));
    let relay_control = Arc::new(Mutex::new(None));
    let upnp_undo = Arc::new(Mutex::new(None));
    let (tx, rx) = unbounded();
    fn set(st: &Mutex<Reach>, tx: &UnboundedSender<()>, f: impl FnOnce(&mut Reach)) {
        if let Ok(mut g) = st.lock() {
            f(&mut g);
        }
        let _ = tx.unbounded_send(());
    }
    {
        let (st, tx) = (state.clone(), tx.clone());
        std::thread::spawn(move || {
            let r = match public_ipv6() {
                Some(ip) if ipv6 => State::Ready(format!("[{ip}]:{port}")),
                Some(_) => State::No(tr("This PC has a public IPv6 address, but the session could not listen on it.").into()),
                None => State::No(tr("Your internet provider gives this PC no public IPv6 address.").into()),
            };
            set(&st, &tx, |g| g.ipv6 = r);
        });
    }
    {
        let (st, tx, undo, lan) = (state.clone(), tx.clone(), upnp_undo.clone(), lan.clone());
        std::thread::spawn(move || {
            let r = match lan {
                Some(lan) => upnp_map(port, &lan, &undo),
                None => State::No(tr("No network connection.").into()),
            };
            set(&st, &tx, |g| g.upnp = r);
        });
    }
    {
        let (st, stop2, ctl) = (state.clone(), stop.clone(), relay_control.clone());
        std::thread::spawn(move || loop {
            // Guests already through keep their connection; a new tunnel gets a new
            // port, so the invite code changes with it.
            let r = relay(port, &stop2, &ctl, &|addr| set(&st, &tx, |g| g.relay = State::Ready(addr)));
            if stop2.load(Ordering::Relaxed) {
                break;
            }
            let why = match r {
                Ok(label) => crate::trf!("{relay} closed the tunnel. Opening a new one; new guests need a fresh code."; relay => label),
                Err(e) => crate::trf!("{why} Trying again in a few seconds."; why => e),
            };
            set(&st, &tx, |g| g.relay = State::No(why));
            for _ in 0..10 {
                std::thread::sleep(Duration::from_secs(1));
                if stop2.load(Ordering::Relaxed) {
                    return;
                }
            }
        });
    }
    (Probes { state, stop, relay_control, upnp_undo }, rx)
}

// ---------------------------------------------------------------- IPv6

/// This PC's public IPv6 address, if it has one: the source address of the route to a
/// public IPv6 host (nothing is sent), when it is a global one.
pub fn public_ipv6() -> Option<Ipv6Addr> {
    let s = UdpSocket::bind("[::]:0").ok()?;
    s.connect("[2001:4860:4860::8888]:80").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V6(ip) if is_global_v6(&ip) => Some(ip),
        _ => None,
    }
}

/// In 2000::/3, the global unicast range (private fc00::/7 and link-local fe80::/10 are not).
pub fn is_global_v6(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xe000) == 0x2000
}

/// Private, carrier-grade NAT (100.64.0.0/10), or otherwise not reachable from the internet.
pub fn is_private_v4(ip: &Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || (o[0] == 100 && (o[1] & 0xc0) == 64)
}

// ---------------------------------------------------------------- UPnP

/// Ask the router (UPnP IGD) to forward `port` to this PC, and read its public address.
fn upnp_map(port: u16, lan: &str, undo: &Mutex<Option<(String, String, u16)>>) -> State {
    let Some(location) = ssdp_find() else {
        return State::No(tr("The router did not answer (UPnP is off in its settings, or it has none).").into());
    };
    let Ok(desc) = http("GET", &location, None, "") else {
        return State::No(tr("The router's UPnP description could not be read.").into());
    };
    let Some((service, control)) = control_url(&location, &desc) else {
        return State::No(tr("The router offers no port forwarding over UPnP.").into());
    };
    let add = |lease: u32| {
        soap(&control, &service, "AddPortMapping", &format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>{port}</NewInternalPort><NewInternalClient>{lan}</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>FluxCode Live share</NewPortMappingDescription><NewLeaseDuration>{lease}</NewLeaseDuration>"
        ))
    };
    // Some routers only take permanent mappings (lease 0); it is removed when the session ends.
    if add(7200).is_err() && add(0).is_err() {
        return State::No(crate::trf!("The router refused to open port {port} (another device may use it)."; port => port));
    }
    if let Ok(mut u) = undo.lock() {
        *u = Some((control.clone(), service.clone(), port));
    }
    let ip = soap(&control, &service, "GetExternalIPAddress", "").ok().and_then(|x| tag(&x, "NewExternalIPAddress")).and_then(|s| s.parse::<Ipv4Addr>().ok());
    match ip {
        Some(ip) if is_private_v4(&ip) => State::No(crate::trf!(
            "The router opened the port, but its own internet address ({ip}) is private: your provider shares one public address between customers (carrier-grade NAT). The relay works instead."; ip => ip
        )),
        Some(ip) => State::Ready(format!("{ip}:{port}")),
        None => State::No(tr("The router opened the port but did not give its public address.").into()),
    }
}

/// SSDP discovery of an Internet Gateway Device; its description URL.
fn ssdp_find() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.set_read_timeout(Some(Duration::from_millis(500))).ok()?;
    for st in ["urn:schemas-upnp-org:device:InternetGatewayDevice:1", "urn:schemas-upnp-org:service:WANIPConnection:1", "urn:schemas-upnp-org:service:WANPPPConnection:1"] {
        let msg = format!("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n");
        let _ = s.send_to(msg.as_bytes(), "239.255.255.250:1900");
    }
    let mut buf = [0u8; 2048];
    let until = Instant::now() + Duration::from_millis(3000);
    while Instant::now() < until {
        let Ok((n, _)) = s.recv_from(&mut buf) else { continue };
        let text = String::from_utf8_lossy(&buf[..n]);
        let loc = text.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("location").then(|| v.trim().to_string())
        });
        if loc.is_some() {
            return loc;
        }
    }
    None
}

/// A plain-HTTP request to the router (`http://host:port/path`); the body of a 2xx reply.
fn http(method: &str, url: &str, soap_action: Option<&str>, body: &str) -> Result<String, String> {
    let rest = url.strip_prefix("http://").ok_or("not an http URL")?;
    let (hostport, path) = rest.find('/').map_or((rest, "/"), |i| (&rest[..i], &rest[i..]));
    let target = if hostport.contains(':') { hostport.to_string() } else { format!("{hostport}:80") };
    let sa = target.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut s = TcpStream::connect_timeout(&sa, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {hostport}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if let Some(a) = soap_action {
        req.push_str(&format!("Content-Type: text/xml; charset=\"utf-8\"\r\nSOAPAction: \"{a}\"\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").ok_or("no reply")?;
    let ok = head.split_whitespace().nth(1).is_some_and(|c| c.starts_with('2'));
    let body = if head.to_ascii_lowercase().contains("transfer-encoding: chunked") { dechunk(body) } else { body.to_string() };
    if ok {
        Ok(body)
    } else {
        Err(body)
    }
}

/// The body of a chunked HTTP reply.
fn dechunk(mut s: &str) -> String {
    let mut out = String::new();
    while let Some((size, rest)) = s.split_once("\r\n") {
        let n = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
        if n == 0 || rest.len() < n {
            break;
        }
        out.push_str(&rest[..n]);
        s = rest[n..].strip_prefix("\r\n").unwrap_or(&rest[n..]);
    }
    out
}

/// The first `<name>` text in `xml`.
fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let i = xml.find(&open)? + open.len();
    let j = xml[i..].find(&format!("</{name}>"))? + i;
    Some(xml[i..j].trim().to_string())
}

/// The WAN connection service and its absolute control URL, from a device description.
pub fn control_url(location: &str, desc: &str) -> Option<(String, String)> {
    let base = match tag(desc, "URLBase") {
        Some(b) if b.starts_with("http") => b.trim_end_matches('/').to_string(),
        _ => {
            let (scheme, rest) = location.split_once("://")?;
            format!("{scheme}://{}", rest.split('/').next()?)
        }
    };
    for kind in ["urn:schemas-upnp-org:service:WANIPConnection:2", "urn:schemas-upnp-org:service:WANIPConnection:1", "urn:schemas-upnp-org:service:WANPPPConnection:1"] {
        let Some(at) = desc.find(&format!("<serviceType>{kind}</serviceType>")) else { continue };
        let rest = &desc[at..];
        let c = tag(&rest[..rest.find("</service>").unwrap_or(rest.len())], "controlURL")?;
        let url = if c.starts_with("http") { c } else { format!("{base}/{}", c.trim_start_matches('/')) };
        return Some((kind.to_string(), url));
    }
    None
}

fn soap(control: &str, service: &str, action: &str, args: &str) -> Result<String, String> {
    let body = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{service}\">{args}</u:{action}></s:Body></s:Envelope>"
    );
    http("POST", control, Some(&format!("{service}#{action}")), &body)
}

// ---------------------------------------------------------------- relay

/// One null-terminated JSON frame from a bore stream.
fn read_frame(r: &mut BufReader<TcpStream>) -> std::io::Result<serde_json::Value> {
    let mut buf = Vec::new();
    r.read_until(0, &mut buf)?;
    if buf.last() == Some(&0) {
        buf.pop();
    }
    if buf.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the relay closed the connection"));
    }
    serde_json::from_slice(&buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn write_frame(w: &mut TcpStream, v: &serde_json::Value) -> std::io::Result<()> {
    let mut b = serde_json::to_vec(v)?;
    b.push(0);
    w.write_all(&b)
}

/// Answer to a relay's `Challenge`. `bore` keys its HMAC with SHA-256 of the
/// secret and signs the challenge's 16 raw UUID bytes, not its printed form.
fn challenge_tag(secret: &str, challenge: &str) -> Result<String, String> {
    use hmac::Mac;
    use sha2::Digest;
    let key = sha2::Sha256::digest(secret.as_bytes());
    let mut mac = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(&key).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::parse_str(challenge).map_err(|e| e.to_string())?;
    mac.update(id.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn relay_connect(ep: &RelayEndpoint) -> std::io::Result<TcpStream> {
    let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, "no address for the relay");
    for a in (ep.host.as_str(), ep.port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&a, Duration::from_secs(6)) {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Keep a public port open on a relay and pass each guest through to `port` on this
/// PC. Tries our relay first and falls back to the public one, so a session still
/// works when our own is down. `Ok(label)` means the tunnel closed after opening;
/// `Err` means nothing could be opened at all.
fn relay(port: u16, stop: &AtomicBool, ctl: &Mutex<Option<TcpStream>>, ready: &dyn Fn(String)) -> Result<String, String> {
    let eps = endpoints();
    if eps.is_empty() {
        return Err(tr("No relay is available in this build.").to_string());
    }
    let mut failures: Vec<String> = Vec::new();
    for ep in &eps {
        match relay_open(ep, port, stop, ctl, ready) {
            Ok(label) => return Ok(label),
            Err(e) => failures.push(e),
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
    }
    if failures.len() == 1 {
        return Err(failures.remove(0));
    }
    Err(failures.join(" "))
}

/// One attempt against a single relay.
fn relay_open(ep: &RelayEndpoint, port: u16, stop: &AtomicBool, ctl: &Mutex<Option<TcpStream>>, ready: &dyn Fn(String)) -> Result<String, String> {
    let mut control = relay_connect(ep).map_err(|e| crate::trf!("{relay} could not be reached ({e}). A firewall or the network may block it."; relay => ep.host.clone(), e => e))?;
    let mut reader = BufReader::new(control.try_clone().map_err(|e| e.to_string())?);
    let _ = reader.get_ref().set_read_timeout(Some(Duration::from_secs(10)));
    relay_auth(ep, &mut control, &mut reader)?;
    write_frame(&mut control, &serde_json::json!({ "Hello": 0 })).map_err(|e| e.to_string())?;
    if let Ok(mut c) = ctl.lock() {
        *c = control.try_clone().ok();
    }
    let hello = read_frame(&mut reader).map_err(|e| crate::trf!("{relay} did not answer ({e})."; relay => ep.host.clone(), e => e))?;
    let public = hello.get("Hello").and_then(|p| p.as_u64()).ok_or_else(|| crate::trf!("{relay} refused: {why}"; relay => ep.host.clone(), why => hello.get("Error").and_then(|e| e.as_str()).unwrap_or("unexpected reply")))?;
    ready(format!("{}:{public}", ep.host));
    // The relay sends a heartbeat every half second; a minute of silence means it's gone.
    let _ = reader.get_ref().set_read_timeout(Some(Duration::from_secs(60)));
    while !stop.load(Ordering::Relaxed) {
        let Ok(msg) = read_frame(&mut reader) else { break };
        if let Some(id) = msg.get("Connection").and_then(|v| v.as_str()).map(String::from) {
            let ep = ep.clone();
            std::thread::spawn(move || {
                let _ = relay_accept(&id, port, &ep);
            });
        }
    }
    Ok(ep.host.clone())
}

/// Prove we know the relay's secret, when it has one. A relay started with
/// `--secret` speaks first: it sends a UUID challenge and ignores everything
/// until we answer. A relay without one says nothing until we ask for a port.
fn relay_auth(ep: &RelayEndpoint, control: &mut TcpStream, reader: &mut BufReader<TcpStream>) -> Result<(), String> {
    let Some(secret) = ep.secret.as_deref() else { return Ok(()) };
    let relay = ep.host.clone();
    let frame = read_frame(reader).map_err(|e| crate::trf!("{relay} did not answer ({e})."; relay => relay.clone(), e => e))?;
    let Some(challenge) = frame.get("Challenge").and_then(|c| c.as_str()) else {
        return Err(crate::trf!("{relay} did not ask for a secret. Check `--secret` on the relay."; relay => relay));
    };
    let tag = challenge_tag(secret, challenge)?;
    write_frame(control, &serde_json::json!({ "Authenticate": tag })).map_err(|e| e.to_string())
}

/// One guest through the relay: claim its connection, then copy bytes both ways.
/// It must go back to the same relay that handed out the port, and prove the
/// secret again — every control connection is authenticated, not just the first.
fn relay_accept(id: &str, port: u16, ep: &RelayEndpoint) -> std::io::Result<()> {
    let mut remote = relay_connect(ep)?;
    let mut reader = BufReader::new(remote.try_clone()?);
    let _ = reader.get_ref().set_read_timeout(Some(Duration::from_secs(10)));
    relay_auth(ep, &mut remote, &mut reader).map_err(std::io::Error::other)?;
    write_frame(&mut remote, &serde_json::json!({ "Accept": id }))?;
    let local = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_secs(3))?;
    let _ = local.set_nodelay(true);
    pipe(remote, local)
}

/// Copy both ways until either side closes.
fn pipe(a: TcpStream, b: TcpStream) -> std::io::Result<()> {
    let (mut ar, mut bw) = (a.try_clone()?, b.try_clone()?);
    let (mut br, mut aw) = (b, a);
    let up = std::thread::spawn(move || {
        let _ = std::io::copy(&mut ar, &mut bw);
        let _ = bw.shutdown(Shutdown::Write);
    });
    let _ = std::io::copy(&mut br, &mut aw);
    let _ = aw.shutdown(Shutdown::Write);
    let _ = up.join();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{control_url, dechunk, is_global_v6, is_private_v4, Reach, State};

    /// Vector computed independently with Python's hmac over `uuid.bytes`.
    #[test]
    fn challenge_tag_matches_bore_scheme() {
        let tag = super::challenge_tag("MyKineSecret", "11111111-2222-3333-4444-555555555555").unwrap();
        assert_eq!(tag, "dd6715e066ac25007d7d425e59c0864f58517c8bd56319594cd1b473b8e696e1");
    }

    #[test]
    fn challenge_tag_differs_per_secret_and_challenge() {
        let a = super::challenge_tag("one", "11111111-2222-3333-4444-555555555555").unwrap();
        let b = super::challenge_tag("two", "11111111-2222-3333-4444-555555555555").unwrap();
        let c = super::challenge_tag("one", "11111111-2222-3333-4444-666666666666").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn challenge_tag_rejects_a_non_uuid() {
        assert!(super::challenge_tag("s", "not-a-uuid").is_err());
    }

    fn env_of(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(&'static str, String)> = pairs.iter().map(|(k, v)| (*k, v.to_string())).collect();
        move |key: &str| owned.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn endpoints_defaults_to_primary_then_borepub() {
        let eps = super::endpoints_from(&env_of(&[]), Some("baked"));
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].host, "live.muaraai.com");
        assert_eq!(eps[0].port, 7835);
        assert_eq!(eps[0].secret.as_deref(), Some("baked"));
        assert_eq!(eps[1].host, "bore.pub");
        assert_eq!(eps[1].secret, None);
    }

    #[test]
    fn endpoints_skips_primary_without_secret() {
        let eps = super::endpoints_from(&env_of(&[("FLUXCODE_RELAY_SECRET", "")]), None);
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].host, "bore.pub");
    }

    #[test]
    fn env_overrides_primary() {
        let eps = super::endpoints_from(
            &env_of(&[("FLUXCODE_RELAY_SECRET", "s"), ("FLUXCODE_RELAY_HOST", "relay.example"), ("FLUXCODE_RELAY_PORT", "9000")]),
            Some("baked"),
        );
        assert_eq!(eps[0].host, "relay.example");
        assert_eq!(eps[0].port, 9000);
        assert_eq!(eps[0].secret.as_deref(), Some("s"), "runtime secret wins over the baked one");
    }

    #[test]
    fn fallback_can_be_disabled() {
        let eps = super::endpoints_from(&env_of(&[("FLUXCODE_RELAY_SECRET", "s"), ("FLUXCODE_RELAY_FALLBACK", "0")]), None);
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].host, "live.muaraai.com");
    }

    #[test]
    fn relay_addr_does_not_match_a_lan_address() {
        assert!(!super::is_relay_addr("192.168.1.5:47615"));
        assert!(!super::is_relay_addr("10.0.0.2:47615"));
    }

    #[test]
    fn address_classes() {
        assert!(is_global_v6(&"2001:db8::1".parse().unwrap()));
        assert!(is_global_v6(&"2400:9800:1::5".parse().unwrap()));
        assert!(!is_global_v6(&"fe80::1".parse().unwrap()));
        assert!(!is_global_v6(&"fd00::1".parse().unwrap()));
        assert!(is_private_v4(&"100.72.3.4".parse().unwrap()), "carrier-grade NAT");
        assert!(is_private_v4(&"192.168.1.1".parse().unwrap()));
        assert!(!is_private_v4(&"100.128.0.1".parse().unwrap()));
        assert!(!is_private_v4(&"36.72.10.20".parse().unwrap()));
    }

    #[test]
    fn finds_the_wan_control_url() {
        let desc = "<root><device><serviceList><service><serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType><controlURL>/l3f</controlURL></service><service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/IPConn</controlURL></service></serviceList></device></root>";
        let (svc, url) = control_url("http://192.168.1.1:5000/rootDesc.xml", desc).unwrap();
        assert_eq!(svc, "urn:schemas-upnp-org:service:WANIPConnection:1");
        assert_eq!(url, "http://192.168.1.1:5000/ctl/IPConn");
        let with_base = format!("<URLBase>http://10.0.0.1:80/</URLBase>{desc}");
        assert_eq!(control_url("http://10.0.0.1:1900/d.xml", &with_base).unwrap().1, "http://10.0.0.1:80/ctl/IPConn");
    }

    #[test]
    fn reads_chunked_replies() {
        assert_eq!(dechunk("5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"), "hello world");
    }

    #[test]
    fn invite_addresses_put_the_network_first_and_skip_what_failed() {
        let r = Reach { lan: Some("192.168.1.5".into()), ipv6: State::No("none".into()), upnp: State::Ready("36.1.2.3:47615".into()), relay: State::Ready("bore.pub:41000".into()) };
        assert_eq!(r.addresses(47615), ["192.168.1.5:47615", "36.1.2.3:47615", "bore.pub:41000", "127.0.0.1:47615"]);
        assert!(r.internet() && !r.checking());
        assert!(!Reach::local_only(None).internet());
    }

    /// What this PC's network allows, as the host's panel would show it:
    /// `cargo test probe_this_network -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probe_this_network() {
        let (p, _rx) = super::start(47999, true);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while p.snapshot().checking() && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        println!("{:#?}", p.snapshot());
    }

    /// A whole session through bore.pub: a guest given only the relay's address joins
    /// the host, and the two exchange encrypted messages both ways.
    /// Needs the internet: `cargo test session_through_the_relay -- --ignored`.
    #[test]
    #[ignore]
    fn session_through_the_relay() {
        use crate::collab::net::{host, join, random, Invite, NetEvent};
        use crate::collab::proto::Msg;
        use futures::StreamExt;
        let secret = random::<32>();
        let (srv, mut hrx) = host(secret).unwrap();
        let (p, _rx) = super::start(srv.port, srv.ipv6);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let addr = loop {
            if let State::Ready(a) = p.snapshot().relay {
                break a;
            }
            assert!(std::time::Instant::now() < until, "relay never opened: {:?}", p.snapshot().relay);
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        let inv = Invite { a: vec![addr.clone()], k: base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, secret), h: "h".into(), n: "n".into() };
        let (gtx, mut grx, via) = join(&inv).unwrap();
        assert_eq!(via, addr);
        gtx.send(Msg::Cursor { path: "through the relay".into(), line: 7 }).unwrap();
        let got = futures::executor::block_on(async {
            let mut tx = None;
            while let Some(ev) = hrx.next().await {
                match ev {
                    NetEvent::Connected { tx: t, .. } => tx = Some(t),
                    NetEvent::Message { msg, .. } => {
                        tx.unwrap().send(Msg::Refused { why: "and back".into() }).unwrap();
                        return Some(msg);
                    }
                    NetEvent::Closed { .. } => {}
                }
            }
            None
        });
        assert!(matches!(got, Some(Msg::Cursor { path, line: 7 }) if path == "through the relay"));
        let back = futures::executor::block_on(async {
            while let Some(ev) = grx.next().await {
                if let NetEvent::Message { msg, .. } = ev {
                    return Some(msg);
                }
            }
            None
        });
        assert!(matches!(back, Some(Msg::Refused { why }) if why == "and back"));
    }

    /// The real relay, end to end: a guest reaches a local echo server through bore.pub.
    /// Needs the internet: `cargo test relay_round_trip -- --ignored`.
    #[test]
    #[ignore]
    fn relay_round_trip() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut b = [0u8; 5];
                    if s.read_exact(&mut b).is_ok() {
                        let _ = s.write_all(&b);
                    }
                });
            }
        });
        let stop = std::sync::atomic::AtomicBool::new(false);
        let ctl = std::sync::Mutex::new(None);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|sc| {
            sc.spawn(|| super::relay(port, &stop, &ctl, &|a| tx.send(a).unwrap()));
            let addr = rx.recv_timeout(std::time::Duration::from_secs(15)).expect("relay port");
            let mut g = std::net::TcpStream::connect(addr).unwrap();
            g.write_all(b"hello").unwrap();
            let mut back = [0u8; 5];
            g.read_exact(&mut back).unwrap();
            assert_eq!(&back, b"hello");
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(c) = ctl.lock().unwrap().take() {
                let _ = c.shutdown(std::net::Shutdown::Both);
            }
        });
    }

    /// A relay started with `--secret`, end to end: the handshake is what makes
    /// this work, so it proves the wire order against the real bore binary.
    /// Needs `bore` on PATH (or at /tmp/bore): `cargo test relay_round_trip_with_secret -- --ignored`.
    #[test]
    #[ignore]
    fn relay_round_trip_with_secret() {
        use std::io::{Read, Write};
        let bore = [std::path::PathBuf::from("/tmp/bore"), std::path::PathBuf::from("bore")]
            .into_iter()
            .find(|p| p.exists() || p.components().count() == 1)
            .unwrap();
        // bore's control port is fixed at 7835; --min-port/--max-port only bound tunnels.
        let port = 7835u16;
        let mut srv = std::process::Command::new(&bore)
            .args(["server", "--secret", "s3cret", "--min-port", "29800", "--max-port", "29810", "--bind-addr", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("start bore server");
        std::thread::sleep(std::time::Duration::from_millis(600));

        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let local = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut b = [0u8; 5];
                    if s.read_exact(&mut b).is_ok() {
                        let _ = s.write_all(&b);
                    }
                });
            }
        });

        let ep = super::RelayEndpoint { host: "127.0.0.1".into(), port, secret: Some("s3cret".into()) };
        let stop = std::sync::atomic::AtomicBool::new(false);
        let ctl = std::sync::Mutex::new(None);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|sc| {
            sc.spawn(|| {
                let _ = super::relay_open(&ep, local, &stop, &ctl, &|a| tx.send(a).unwrap());
            });
            let addr = rx.recv_timeout(std::time::Duration::from_secs(15)).expect("relay port");
            let mut g = std::net::TcpStream::connect(addr).unwrap();
            g.write_all(b"hello").unwrap();
            let mut back = [0u8; 5];
            g.read_exact(&mut back).unwrap();
            assert_eq!(&back, b"hello");
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(c) = ctl.lock().unwrap().take() {
                let _ = c.shutdown(std::net::Shutdown::Both);
            }
        });
        let _ = srv.kill();
    }
}
