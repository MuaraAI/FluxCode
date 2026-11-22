
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

const FILE: &str = "instance.json";
const MAX_MESSAGE: u64 = 256 * 1024;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Info {
    port: u16,
    token: String,
    pid: u32,
}

pub(crate) fn forward(files: &[PathBuf]) -> bool {
    let info: Info = crate::settings::load_json(FILE);
    if info.port == 0 || info.token.is_empty() || info.pid == std::process::id() || !alive(info.pid) {
        return false;
    }
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, info.port));
    let Ok(mut conn) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        return false;
    };
    let _ = conn.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = conn.set_write_timeout(Some(Duration::from_secs(3)));
    allow_foreground(info.pid);
    if conn.write_all(message(&info.token, files).as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(conn).read_line(&mut reply).is_ok() && reply.trim_end() == "ok"
}

fn message(token: &str, files: &[PathBuf]) -> String {
    let mut msg = format!("{token}\n");
    for f in files {
        msg.push_str(&f.to_string_lossy());
        msg.push('\n');
    }
    msg.push('\n');
    msg
}

fn parse(input: impl Read, token: &str) -> Option<Vec<PathBuf>> {
    let mut lines = BufReader::new(input.take(MAX_MESSAGE)).lines();
    if lines.next()?.ok()? != token {
        return None;
    }
    let mut paths = Vec::new();
    for line in lines {
        let line = line.ok()?;
        if line.is_empty() {
            return Some(paths);
        }
        paths.push(PathBuf::from(line));
    }
    None
}

pub(crate) fn listen() -> Option<UnboundedReceiver<Vec<PathBuf>>> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).ok()?;
    let token = hex::encode(bytes);
    crate::settings::save_json(FILE, &Info { port, token: token.clone(), pid: std::process::id() });
    let (tx, rx) = unbounded();
    std::thread::Builder::new()
        .name("instance".into())
        .spawn(move || {
            for conn in listener.incoming() {
                if tx.is_closed() {
                    break;
                }
                if let Ok(conn) = conn {
                    serve(conn, &token, &tx);
                }
            }
        })
        .ok()?;
    Some(rx)
}

fn serve(mut conn: TcpStream, token: &str, tx: &UnboundedSender<Vec<PathBuf>>) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
    let Ok(reader) = conn.try_clone() else { return };
    let Some(paths) = parse(reader, token) else { return };
    if tx.unbounded_send(paths).is_ok() {
        let _ = conn.write_all(b"ok\n");
    }
}

#[cfg(windows)]
fn allow_foreground(pid: u32) {
    #[link(name = "user32")]
    extern "system" {
        fn AllowSetForegroundWindow(pid: u32) -> i32;
    }
    unsafe { AllowSetForegroundWindow(pid) };
}

#[cfg(not(windows))]
fn allow_foreground(_pid: u32) {}

#[cfg(windows)]
fn alive(pid: u32) -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetExitCodeProcess(process: isize, code: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        ok && code == STILL_ACTIVE
    }
}

#[cfg(not(windows))]
fn alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists() || !cfg!(target_os = "linux")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let files = vec![PathBuf::from(r"C:\a b\x.ts"), PathBuf::from("/home/u/y.rs")];
        let msg = message("t0k", &files);
        assert_eq!(parse(msg.as_bytes(), "t0k"), Some(files));
    }

    #[test]
    fn wrong_token_or_cut_short_is_refused() {
        let msg = message("t0k", &[PathBuf::from("x")]);
        assert_eq!(parse(msg.as_bytes(), "other"), None);
        assert_eq!(parse("t0k\nx\n".as_bytes(), "t0k"), None);
    }

    #[test]
    fn forwards_to_a_listener() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, mut rx) = unbounded();
        std::thread::spawn(move || {
            let (conn, _) = listener.accept().unwrap();
            serve(conn, "abc", &tx);
        });
        let mut conn = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        conn.write_all(message("abc", &[PathBuf::from("f.txt")]).as_bytes()).unwrap();
        let mut reply = String::new();
        BufReader::new(conn).read_line(&mut reply).unwrap();
        assert_eq!(reply, "ok\n");
        assert_eq!(rx.try_recv().unwrap(), vec![PathBuf::from("f.txt")]);
    }
}
