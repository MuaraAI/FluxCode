//! Live Preview for HTML/CSS/JS: a tiny static file server bound to
//! 127.0.0.1 on a random port, serving only the workspace folder. HTML pages
//! get a small script that long-polls `/__flux/wait` and reloads the page
//! after a save, so there is no busy polling and no file watcher.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tauri::State;

#[derive(Default)]
pub struct Preview {
    server: Mutex<Option<(u16, Arc<Mutex<PathBuf>>)>>,
    version: Arc<(Mutex<u64>, Condvar)>,
}

/// Start (or retarget) the preview server; returns its port.
#[tauri::command]
pub fn preview_start(preview: State<'_, Preview>, root: String) -> Result<u16, String> {
    let mut server = preview.server.lock().unwrap();
    if let Some((port, dir)) = server.as_ref() {
        *dir.lock().unwrap() = PathBuf::from(&root);
        return Ok(*port);
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let dir = Arc::new(Mutex::new(PathBuf::from(&root)));
    let (d, v) = (dir.clone(), preview.version.clone());
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (d, v) = (d.clone(), v.clone());
            thread::spawn(move || {
                let _ = handle(stream, &d, &v);
            });
        }
    });
    *server = Some((port, dir));
    Ok(port)
}

/// A file was saved: reload open preview pages.
#[tauri::command]
pub fn preview_reload(preview: State<'_, Preview>) {
    let (lock, cv) = &*preview.version;
    *lock.lock().unwrap() += 1;
    cv.notify_all();
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
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let head = String::from_utf8_lossy(&buf[..n]);
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
    let mut file = root.lock().unwrap().join(rel_path);
    if file.is_dir() {
        file = file.join("index.html");
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
    use super::percent_decode;

    #[test]
    fn decodes_paths() {
        assert_eq!(percent_decode("my%20page.html"), "my page.html");
        assert_eq!(percent_decode("a%2Fb"), "a/b");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
