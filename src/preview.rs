//! Live Preview for HTML/CSS/JS, ported from `src-tauri/src/preview.rs`: a
//! tiny static file server bound to 127.0.0.1 on a random port, serving only
//! the workspace folder. HTML pages get a small script that long-polls
//! `/__flux/wait` and reloads after a save. The page is shown in a WebView2
//! (via gpui-webview) that exists only while the preview is open.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

pub struct PreviewServer {
    pub port: u16,
    root: Arc<Mutex<PathBuf>>,
    version: Arc<(Mutex<u64>, Condvar)>,
}

impl PreviewServer {
    pub fn start(root: &Path) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let root = Arc::new(Mutex::new(root.to_path_buf()));
        let version = Arc::new((Mutex::new(0u64), Condvar::new()));
        let (d, v) = (root.clone(), version.clone());
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (d, v) = (d.clone(), v.clone());
                thread::spawn(move || {
                    let _ = handle(stream, &d, &v);
                });
            }
        });
        Ok(Self { port, root, version })
    }
    pub fn is_root(&self, root: &Path) -> bool {
        *self.root.lock().unwrap() == root
    }

    pub fn set_root(&self, root: &Path) {
        *self.root.lock().unwrap() = root.to_path_buf();
    }

    /// A file was saved: reload open preview pages.
    pub fn reload(&self) {
        let (lock, cv) = &*self.version;
        *lock.lock().unwrap() += 1;
        cv.notify_all();
    }

    /// URL of `file` (inside the served root), or the root's index.
    pub fn url_for(&self, file: Option<&Path>) -> String {
        let root = self.root.lock().unwrap().clone();
        let rel = file
            .and_then(|f| f.strip_prefix(&root).ok())
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let rel: String = rel.split('/').map(|seg| seg.replace('%', "%25").replace(' ', "%20").replace('#', "%23")).collect::<Vec<_>>().join("/");
        format!("http://127.0.0.1:{}/{}", self.port, rel)
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs" | "cjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("wasm") => "application/wasm",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(n) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn handle(mut stream: TcpStream, root: &Mutex<PathBuf>, version: &(Mutex<u64>, Condvar)) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    // Read the whole request head: it can arrive in several pieces, and closing the
    // socket with unread bytes makes Linux reset the connection instead of closing it.
    let mut buf = [0u8; 8192];
    let mut n = 0;
    while n < buf.len() {
        let got = stream.read(&mut buf[n..])?;
        n += got;
        if got == 0 || buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    let host_ok = head
        .lines()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("host")).map(|(_, v)| v.trim()))
        .is_some_and(|h| {
            let host_only = h.split(':').next().unwrap_or("");
            host_only.eq_ignore_ascii_case("127.0.0.1") || host_only.eq_ignore_ascii_case("localhost")
        });
    if !host_ok {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"Forbidden");
    }
    let target = head.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == "/__flux/wait" {
        // Long-poll: answer as soon as the version moves past what the page has.
        let seen: u64 = query.strip_prefix("v=").and_then(|v| v.parse().ok()).unwrap_or(0);
        let (lock, cv) = version;
        let guard = lock.lock().unwrap();
        let (guard, _) = cv.wait_timeout_while(guard, Duration::from_secs(25), |v| *v <= seen).unwrap();
        let now = *guard;
        drop(guard);
        return respond(&mut stream, "200 OK", "text/plain", now.to_string().as_bytes());
    }

    let rel = percent_decode(path.trim_start_matches('/'));
    let rel_path = Path::new(&rel);
    // Only plain names inside the workspace: no "..", no drive or root jumps.
    if rel_path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"Forbidden");
    }
    let root_dir = root.lock().unwrap().clone();
    let mut file = root_dir.join(rel_path);
    if file.is_dir() {
        file = file.join("index.html");
    }
    let Ok(canon_root) = std::fs::canonicalize(&root_dir) else {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"Forbidden");
    };
    if let Ok(canon_file) = std::fs::canonicalize(&file) {
        if !canon_file.starts_with(&canon_root) {
            return respond(&mut stream, "403 Forbidden", "text/plain", b"Forbidden");
        }
    }
    match std::fs::read(&file) {
        Ok(mut body) => {
            let ctype = content_type(&file);
            if ctype.starts_with("text/html") {
                let v = *version.0.lock().unwrap();
                let script = format!(
                    "<script>(()=>{{let v={v};const w=()=>fetch('/__flux/wait?v='+v).then(r=>r.text()).then(t=>{{if(+t>v)location.reload();else w()}}).catch(()=>setTimeout(w,1500));w()}})();</script>"
                );
                let html = String::from_utf8_lossy(&body);
                let injected = match html.rfind("</body>") {
                    Some(i) => format!("{}{}{}", &html[..i], script, &html[i..]),
                    None => format!("{html}{script}"),
                };
                body = injected.into_bytes();
            }
            respond(&mut stream, "200 OK", ctype, &body)
        }
        Err(_) => respond(
            &mut stream,
            "404 Not Found",
            "text/html; charset=utf-8",
            format!("<body style=\"font:14px system-ui;background:#1e1f22;color:#cfd2d8;padding:24px\">Not found: {}</body>", rel.replace('<', "&lt;")).as_bytes(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{percent_decode, PreviewServer};
    use std::io::{Read, Write};

    fn get(port: u16, path: &str) -> String {
        get_with_host(port, path, &format!("127.0.0.1:{port}"))
    }

    fn get_with_host(port: u16, path: &str, host: &str) -> String {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_injects_and_reloads() {
        let dir = std::env::temp_dir().join("fluxcode-gpui-preview-test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("index.html"), "<html><body>hi</body></html>").unwrap();
        let server = PreviewServer::start(&dir).unwrap();
        let page = get(server.port, "/");
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(page.contains("/__flux/wait"));
        assert!(get(server.port, "/../secret").starts_with("HTTP/1.1 403"));
        let port = server.port;
        let waiter = std::thread::spawn(move || get(port, "/__flux/wait?v=0"));
        std::thread::sleep(std::time::Duration::from_millis(200));
        server.reload();
        assert!(waiter.join().unwrap().ends_with("\r\n\r\n1"));
        assert_eq!(server.url_for(Some(&dir.join("my page.html"))), format!("http://127.0.0.1:{port}/my%20page.html"));
        assert!(get_with_host(port, "/", "evil.example").starts_with("HTTP/1.1 403"));
    }

    #[test]
    fn decodes_paths() {
        assert_eq!(percent_decode("my%20page.html"), "my page.html");
        assert_eq!(percent_decode("a%2Fb"), "a/b");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
