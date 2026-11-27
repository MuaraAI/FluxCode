use crate::theme::Palette;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

const MAX_ACTIVE_CONNS: usize = 32;
const MAX_POST_BYTES: usize = 2 * 1024 * 1024;
const MAX_SSE_QUEUE: usize = 512;

#[derive(Clone)]
enum BridgeEvent {
    Message(String),
    Reload(u64),
    ThemeCss(String),
}

type MsgCallback = Arc<dyn Fn(String, String) + Send + Sync>;
type OutboundQueue = (Mutex<(u64, VecDeque<(u64, BridgeEvent)>)>, Condvar);

struct WebviewSlot {
    html: Mutex<String>,
    html_rev: AtomicU64,
    ext_folder: PathBuf,
    local_roots: Mutex<Vec<PathBuf>>,
    outbound: OutboundQueue,
    sse_gen: AtomicU64,
    active_sse: AtomicUsize,
    closed: AtomicBool,
}

impl WebviewSlot {
    fn push_event(&self, ev: BridgeEvent) {
        let (lock, cv) = &self.outbound;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        guard.0 += 1;
        let seq = guard.0;
        if guard.1.len() >= MAX_SSE_QUEUE {
            guard.1.pop_front();
        }
        guard.1.push_back((seq, ev));
        cv.notify_all();
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        *self.html.lock().unwrap_or_else(|e| e.into_inner()) = String::new();
        let (lock, cv) = &self.outbound;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.clear();
        cv.notify_all();
    }
}

struct BridgeState {
    port: u16,
    token: String,
    theme_css: Mutex<String>,
    slots: Mutex<HashMap<String, Arc<WebviewSlot>>>,
    on_msg: MsgCallback,
    active_conns: AtomicUsize,
    shutdown: AtomicBool,
}

pub struct WebviewBridge {
    state: Arc<BridgeState>,
}

impl WebviewBridge {
    pub fn start(theme_css: String, on_msg: MsgCallback) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let mut raw = [0u8; 16];
        getrandom::getrandom(&mut raw).map_err(|e| e.to_string())?;
        let token = hex::encode(raw);
        let state = Arc::new(BridgeState {
            port,
            token,
            theme_css: Mutex::new(theme_css),
            slots: Mutex::new(HashMap::new()),
            on_msg,
            active_conns: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
        });
        let bg = Arc::clone(&state);
        thread::Builder::new()
            .name("flux-webview-bridge".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    if bg.shutdown.load(Ordering::Acquire) {
                        drop(stream);
                        break;
                    }
                    if bg.active_conns.load(Ordering::Relaxed) >= MAX_ACTIVE_CONNS {
                        drop(stream);
                        continue;
                    }
                    bg.active_conns.fetch_add(1, Ordering::Relaxed);
                    let st = Arc::clone(&bg);
                    thread::spawn(move || {
                        let _ = handle_conn(stream, &st);
                        st.active_conns.fetch_sub(1, Ordering::Relaxed);
                    });
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { state })
    }

    pub fn port(&self) -> u16 {
        self.state.port
    }

    pub fn token(&self) -> &str {
        &self.state.token
    }

    pub fn update_theme_css(&self, css: String) {
        *self
            .state
            .theme_css
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = css.clone();
        let slots: Vec<Arc<WebviewSlot>> = self
            .state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for slot in slots {
            if !slot.closed.load(Ordering::Acquire) {
                slot.push_event(BridgeEvent::ThemeCss(css.clone()));
            }
        }
    }

    pub fn register_webview(&self, handle: &str, ext_folder: PathBuf, local_roots: Vec<PathBuf>) {
        let mut roots = Vec::new();
        if let Ok(canon) = ext_folder.canonicalize() {
            roots.push(canon);
        }
        for r in local_roots {
            if let Ok(canon) = r.canonicalize() {
                if !roots.contains(&canon) {
                    roots.push(canon);
                }
            }
        }
        let slot = Arc::new(WebviewSlot {
            html: Mutex::new(String::new()),
            html_rev: AtomicU64::new(0),
            ext_folder,
            local_roots: Mutex::new(roots),
            outbound: (Mutex::new((0, VecDeque::new())), Condvar::new()),
            sse_gen: AtomicU64::new(0),
            active_sse: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
        });
        if let Some(prev) = self
            .state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle.to_string(), slot)
        {
            prev.close();
        }
    }

    pub fn remove_webview(&self, handle: &str) {
        if let Some(slot) = self
            .state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(handle)
        {
            slot.close();
        }
    }

    pub fn has_sse_client(&self, handle: &str) -> bool {
        self.state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .is_some_and(|s| s.active_sse.load(Ordering::Acquire) > 0)
    }

    pub fn set_html(&self, handle: &str, html: String) -> u64 {
        let slot = self
            .state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .cloned();
        if let Some(s) = slot {
            *s.html.lock().unwrap_or_else(|e| e.into_inner()) = html;
            let rev = s.html_rev.fetch_add(1, Ordering::AcqRel) + 1;
            s.push_event(BridgeEvent::Reload(rev));
            rev
        } else {
            0
        }
    }

    pub fn update_roots(&self, handle: &str, local_roots: Vec<PathBuf>) {
        let slot = self
            .state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .cloned();
        if let Some(s) = slot {
            let mut roots = Vec::new();
            if let Ok(c) = s.ext_folder.canonicalize() {
                roots.push(c);
            }
            for r in local_roots {
                if let Ok(c) = r.canonicalize() {
                    if !roots.contains(&c) {
                        roots.push(c);
                    }
                }
            }
            *s.local_roots.lock().unwrap_or_else(|e| e.into_inner()) = roots;
        }
    }

    pub fn post_message(&self, handle: &str, json_msg: String) {
        let slot = self
            .state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .cloned();
        if let Some(s) = slot {
            s.push_event(BridgeEvent::Message(json_msg));
        }
    }

    pub fn url_for(&self, handle: &str) -> String {
        format!(
            "http://127.0.0.1:{}/webview/{}?t={}",
            self.state.port, handle, self.state.token
        )
    }

    #[allow(dead_code)]
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.state.port)
    }
}

impl Drop for WebviewBridge {
    fn drop(&mut self) {
        self.state.shutdown.store(true, Ordering::Release);
        let slots: Vec<Arc<WebviewSlot>> = {
            let mut map = self
                .state
                .slots
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            map.drain().map(|(_, s)| s).collect()
        };
        for slot in slots {
            slot.close();
        }
        let addr = SocketAddr::from(([127, 0, 0, 1], self.state.port));
        let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(100));
    }
}

pub fn theme_css(p: &Palette) -> String {
    let kind = if p.dark {
        "vscode-dark"
    } else {
        "vscode-light"
    };
    let color_scheme = if p.dark { "dark" } else { "light" };
    format!(
        ":root {{
  color-scheme: {color_scheme};
  --vscode-font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  --vscode-font-size: 13px;
  --vscode-editor-font-family: 'Cascadia Code', Consolas, monospace;
  --vscode-foreground: #{fg:06x};
  --vscode-disabledForeground: #{dim:06x};
  --vscode-descriptionForeground: #{muted:06x};
  --vscode-errorForeground: #{danger:06x};
  --vscode-focusBorder: #{accent:06x};
  --vscode-selection-background: #{select:06x};
  --vscode-editor-background: #{editor:06x};
  --vscode-editor-foreground: #{fg:06x};
  --vscode-sideBar-background: #{side:06x};
  --vscode-sideBar-foreground: #{fg:06x};
  --vscode-sideBar-border: #{border:06x};
  --vscode-panel-background: #{side:06x};
  --vscode-panel-border: #{border:06x};
  --vscode-widget-border: #{border:06x};
  --vscode-input-background: #{elevated:06x};
  --vscode-input-foreground: #{strong:06x};
  --vscode-input-border: #{border_strong:06x};
  --vscode-input-placeholderForeground: #{dim:06x};
  --vscode-button-background: #{accent:06x};
  --vscode-button-foreground: #{on_accent:06x};
  --vscode-button-hoverBackground: #{hover:06x};
  --vscode-button-secondaryBackground: #{elevated:06x};
  --vscode-button-secondaryForeground: #{strong:06x};
  --vscode-badge-background: #{accent:06x};
  --vscode-badge-foreground: #{on_accent:06x};
  --vscode-list-hoverBackground: #{hover:06x};
  --vscode-list-activeSelectionBackground: #{select:06x};
  --vscode-list-activeSelectionForeground: #{strong:06x};
  --vscode-textLink-foreground: #{accent_text:06x};
  --vscode-textLink-activeForeground: #{blue:06x};
  --vscode-charts-green: #{green:06x};
  --vscode-charts-yellow: #{warn:06x};
  --vscode-charts-red: #{danger:06x};
  --vscode-charts-blue: #{blue:06x};
}}
html, body {{
  background: var(--vscode-sideBar-background);
  color: var(--vscode-foreground);
  font-family: var(--vscode-font-family);
  font-size: var(--vscode-font-size);
}}
body[data-vscode-theme-kind='{kind}'] {{ margin: 0; }}",
        fg = p.fg,
        dim = p.dim,
        muted = p.muted,
        danger = p.danger,
        accent = p.accent,
        select = p.select,
        editor = p.editor,
        side = p.side,
        border = p.border,
        elevated = p.elevated,
        strong = p.strong,
        border_strong = p.border_strong,
        on_accent = p.on_accent,
        hover = p.hover,
        accent_text = p.accent_text,
        blue = p.blue,
        green = p.green,
        warn = p.warn,
    )
}

fn handle_conn(mut stream: TcpStream, state: &BridgeState) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let raw_target = parts.next().unwrap_or("");

    let mut host_val: Option<String> = None;
    let mut token_hdr: Option<String> = None;
    let mut content_len: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.eq_ignore_ascii_case("host") {
                host_val = Some(v.trim().to_string());
            } else if k.eq_ignore_ascii_case("x-flux-token") {
                token_hdr = Some(v.trim().to_string());
            } else if k.eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse().unwrap_or(0);
            }
        }
    }

    let host_ok = host_val.as_deref().is_some_and(|h| {
        h == format!("127.0.0.1:{}", state.port) || h == format!("localhost:{}", state.port)
    });
    if !host_ok {
        return respond(
            &mut stream,
            "403 Forbidden",
            "text/plain",
            b"Forbidden host",
        );
    }

    let (path, query) = raw_target.split_once('?').unwrap_or((raw_target, ""));
    let query_token = query.split('&').find_map(|kv| {
        kv.split_once('=')
            .filter(|(k, _)| *k == "t")
            .map(|(_, v)| v)
    });
    let token_ok = query_token == Some(state.token.as_str())
        || token_hdr.as_deref() == Some(state.token.as_str());
    if !token_ok {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"Invalid token");
    }

    if method == "OPTIONS" {
        return write!(
            stream,
            "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: http://127.0.0.1:{}\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, X-Flux-Token\r\nConnection: close\r\n\r\n",
            state.port
        );
    }

    if let Some(handle) = path.strip_prefix("/webview/") {
        if method != "GET" {
            return respond(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"Method not allowed",
            );
        }
        let slot = state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .cloned();
        let Some(slot) = slot else {
            return respond(
                &mut stream,
                "404 Not Found",
                "text/plain",
                b"Unknown webview",
            );
        };
        let raw_html = slot.html.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let html_rev = slot.html_rev.load(Ordering::Acquire);
        let css = state
            .theme_css
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let full = render_document(&raw_html, handle, html_rev, state.port, &state.token, &css);
        return respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            full.as_bytes(),
        );
    }

    if let Some(rest) = path.strip_prefix("/res/") {
        if method != "GET" {
            return respond(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"Method not allowed",
            );
        }
        let Some((handle, enc_rel)) = rest.split_once('/') else {
            return respond(&mut stream, "404 Not Found", "text/plain", b"Not found");
        };
        let slot = state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .cloned();
        let Some(slot) = slot else {
            return respond(
                &mut stream,
                "404 Not Found",
                "text/plain",
                b"Unknown webview",
            );
        };
        let roots = slot
            .local_roots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(file) = resolve_resource(&slot.ext_folder, &roots, enc_rel) else {
            return respond(&mut stream, "403 Forbidden", "text/plain", b"Access denied");
        };
        let Ok(bytes) = std::fs::read(&file) else {
            return respond(&mut stream, "404 Not Found", "text/plain", b"Missing file");
        };
        return respond(&mut stream, "200 OK", content_type(&file), &bytes);
    }

    if let Some(handle) = path.strip_prefix("/events/") {
        if method != "GET" {
            return respond(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"Method not allowed",
            );
        }
        let slot = state
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .cloned();
        let Some(slot) = slot else {
            return respond(
                &mut stream,
                "404 Not Found",
                "text/plain",
                b"Unknown webview",
            );
        };
        return serve_sse(stream, &slot, state);
    }

    if let Some(handle) = path.strip_prefix("/msg/") {
        if method != "POST" {
            return respond(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"Method not allowed",
            );
        }
        if content_len > MAX_POST_BYTES {
            return respond(
                &mut stream,
                "413 Payload Too Large",
                "text/plain",
                b"Too large",
            );
        }
        let mut body = vec![0u8; content_len];
        if content_len > 0 {
            reader.read_exact(&mut body)?;
        }
        let json_msg = String::from_utf8_lossy(&body).into_owned();
        (state.on_msg)(handle.to_string(), json_msg);
        return write!(
            stream,
            "HTTP/1.1 204 No Content\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
        );
    }

    respond(&mut stream, "404 Not Found", "text/plain", b"Not found")
}

struct SseGuard<'a> {
    counter: &'a AtomicUsize,
}

impl Drop for SseGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

fn serve_sse(
    mut stream: TcpStream,
    slot: &WebviewSlot,
    state: &BridgeState,
) -> std::io::Result<()> {
    let my_gen = slot.sse_gen.fetch_add(1, Ordering::AcqRel) + 1;
    slot.active_sse.fetch_add(1, Ordering::AcqRel);
    let _guard = SseGuard {
        counter: &slot.active_sse,
    };
    // Wake any superseded SSE handler on this slot so it exits immediately.
    slot.outbound.1.notify_all();

    write!(
        stream,
        concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: text/event-stream\r\n",
            "Cache-Control: no-cache\r\n",
            "Connection: keep-alive\r\n\r\n",
            ": connected\n\n"
        )
    )?;
    stream.flush()?;

    let (lock, cv) = &slot.outbound;
    let mut last_seq = 0u64;
    loop {
        if state.shutdown.load(Ordering::Acquire)
            || slot.closed.load(Ordering::Acquire)
            || slot.sse_gen.load(Ordering::Acquire) != my_gen
        {
            break;
        }
        let mut items: Vec<(u64, BridgeEvent)> = Vec::new();
        {
            let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            while !guard.1.iter().any(|(s, _)| *s > last_seq) {
                if state.shutdown.load(Ordering::Acquire)
                    || slot.closed.load(Ordering::Acquire)
                    || slot.sse_gen.load(Ordering::Acquire) != my_gen
                {
                    return Ok(());
                }
                let (next_guard, res) = cv
                    .wait_timeout(guard, Duration::from_secs(15))
                    .unwrap_or_else(|e| e.into_inner());
                guard = next_guard;
                if res.timed_out() && !guard.1.iter().any(|(s, _)| *s > last_seq) {
                    break;
                }
            }
            for (seq, ev) in &guard.1 {
                if *seq > last_seq {
                    items.push((*seq, ev.clone()));
                }
            }
        }
        if items.is_empty() {
            if write!(stream, ": keepalive\n\n").is_err() || stream.flush().is_err() {
                break;
            }
            continue;
        }
        for (seq, ev) in items {
            last_seq = seq;
            let write_res = match ev {
                BridgeEvent::Message(msg) => {
                    let payload = msg.replace(['\r', '\n'], " ");
                    write!(stream, "data: {payload}\n\n")
                }
                BridgeEvent::Reload(rev) => {
                    write!(stream, "event: flux-reload\ndata: {rev}\n\n")
                }
                BridgeEvent::ThemeCss(css) => {
                    let payload = serde_json::to_string(&css).unwrap_or_else(|_| "\"\"".into());
                    write!(stream, "event: flux-theme\ndata: {payload}\n\n")
                }
            };
            if write_res.is_err() {
                return Ok(());
            }
        }
        if stream.flush().is_err() {
            break;
        }
    }
    Ok(())
}

fn render_document(
    raw_html: &str,
    handle: &str,
    html_rev: u64,
    port: u16,
    token: &str,
    theme_css: &str,
) -> String {
    let origin = format!("http://127.0.0.1:{port}");
    let sanitized = rewrite_csp(raw_html, &origin);
    let injected = format!(
        concat!(
            "<style id=\"fluxcode-vscode-theme\">\n{theme_css}\n</style>\n",
            "<script>\n",
            "(function() {{\n",
            "  var HANDLE = {h_json};\n",
            "  var TOKEN = {t_json};\n",
            "  var INITIAL_REV = {html_rev};\n",
            "  var STATE_KEY = 'flux_webview_state_' + HANDLE;\n",
            "  window.acquireVsCodeApi = function() {{\n",
            "    if (window.__fluxVsCodeApi) return window.__fluxVsCodeApi;\n",
            "    window.__fluxVsCodeApi = {{\n",
            "      postMessage: function(msg) {{\n",
            "        var u = '/msg/' + encodeURIComponent(HANDLE)",
            " + '?t=' + encodeURIComponent(TOKEN);\n",
            "        fetch(u, {{\n",
            "          method: 'POST',\n",
            "          headers: {{ 'Content-Type': 'application/json', 'X-Flux-Token': TOKEN }},\n",
            "          body: JSON.stringify(msg === undefined ? null : msg)\n",
            "        }}).catch(function(){{}});\n",
            "      }},\n",
            "      getState: function() {{\n",
            "        try {{\n",
            "          var r = sessionStorage.getItem(STATE_KEY);\n",
            "          return r ? JSON.parse(r) : undefined;\n",
            "        }} catch (e) {{ return undefined; }}\n",
            "      }},\n",
            "      setState: function(st) {{\n",
            "        try {{ sessionStorage.setItem(STATE_KEY, JSON.stringify(st)); }} catch (e) {{}}\n",
            "        return st;\n",
            "      }}\n",
            "    }};\n",
            "    return window.__fluxVsCodeApi;\n",
            "  }};\n",
            "  try {{\n",
            "    var eu = '/events/' + encodeURIComponent(HANDLE)",
            " + '?t=' + encodeURIComponent(TOKEN);\n",
            "    var es = new EventSource(eu);\n",
            "    es.onmessage = function(ev) {{\n",
            "      try {{\n",
            "        var parsed = JSON.parse(ev.data);\n",
            "        window.dispatchEvent(new MessageEvent('message', {{ data: parsed }}));\n",
            "      }} catch (e) {{}}\n",
            "    }};\n",
            "    es.addEventListener('flux-reload', function(ev) {{\n",
            "      if (Number(ev.data) > INITIAL_REV) {{\n",
            "        try {{ es.close(); }} catch (e) {{}}\n",
            "        location.reload();\n",
            "      }}\n",
            "    }});\n",
            "    es.addEventListener('flux-theme', function(ev) {{\n",
            "      try {{\n",
            "        var st = document.getElementById('fluxcode-vscode-theme');\n",
            "        if (st) st.textContent = JSON.parse(ev.data);\n",
            "      }} catch (e) {{}}\n",
            "    }});\n",
            "    window.addEventListener('beforeunload', function() {{\n",
            "      try {{ es.close(); }} catch (e) {{}}\n",
            "    }});\n",
            "  }} catch (e) {{}}\n",
            "}})();\n",
            "</script>",
        ),
        theme_css = theme_css,
        h_json = serde_json::to_string(handle).unwrap_or_else(|_| "\"\"".into()),
        t_json = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".into()),
        html_rev = html_rev,
    );
    if sanitized.trim().is_empty() {
        return format!(
            concat!(
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\">{injected}</head>",
                "<body><div style=\"padding:16px;opacity:0.7;\">",
                "Waiting for extension view content...</div></body></html>"
            ),
            injected = injected
        );
    }
    let lower = sanitized.to_ascii_lowercase();
    if let Some(pos) = lower.find("<head") {
        if let Some(gt) = lower[pos..].find('>') {
            let at = pos + gt + 1;
            let mut out = String::with_capacity(sanitized.len() + injected.len() + 4);
            out.push_str(&sanitized[..at]);
            out.push_str(&injected);
            out.push_str(&sanitized[at..]);
            return out;
        }
    }
    format!("{injected}\n{sanitized}")
}

fn rewrite_csp(html: &str, origin: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(pos) = lower.find("content-security-policy") else {
        return html.to_string();
    };
    let Some(tag_start) = lower[..pos].rfind('<') else {
        return html.to_string();
    };
    let Some(rel_end) = lower[pos..].find('>') else {
        return html.to_string();
    };
    let tag_end = pos + rel_end + 1;
    let tag = &html[tag_start..tag_end];
    let replacement = format!(
        concat!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"",
            "default-src 'self' {origin} http://127.0.0.1:* http://localhost:* data: blob: https:; ",
            "script-src 'self' {origin} http://127.0.0.1:* http://localhost:* ",
            "'unsafe-inline' 'unsafe-eval' blob: https:; ",
            "style-src 'self' {origin} http://127.0.0.1:* http://localhost:* ",
            "'unsafe-inline' data: https:; ",
            "img-src 'self' {origin} http://127.0.0.1:* http://localhost:* data: blob: https:; ",
            "font-src 'self' {origin} http://127.0.0.1:* http://localhost:* data: https:; ",
            "frame-src 'self' {origin} http://127.0.0.1:* http://localhost:* https: blob: data:; ",
            "connect-src 'self' {origin} http://127.0.0.1:* http://localhost:* ",
            "ws://127.0.0.1:* ws://localhost:* ws: wss: https:; ",
            "worker-src 'self' {origin} blob:; ",
            "media-src 'self' {origin} http://127.0.0.1:* http://localhost:* data: blob: https:;\"",
            ">"
        ),
        origin = origin
    );
    if tag.is_empty() {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len() + replacement.len());
    out.push_str(&html[..tag_start]);
    out.push_str(&replacement);
    out.push_str(&html[tag_end..]);
    out
}

fn resolve_resource(ext_folder: &Path, roots: &[PathBuf], enc_rel: &str) -> Option<PathBuf> {
    let decoded = percent_decode(enc_rel);
    let clean = decoded.trim_start_matches(['/', '\\']);
    if clean.is_empty() {
        return None;
    }
    if Path::new(clean).components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }

    let try_root = |base: &Path, sub: &str| -> Option<PathBuf> {
        let joined = base.join(sub.replace('/', std::path::MAIN_SEPARATOR_STR));
        let canon = joined.canonicalize().ok()?;
        if roots.iter().any(|r| canon.starts_with(r)) && canon.is_file() {
            Some(canon)
        } else {
            None
        }
    };

    if let Some((prefix, rest)) = clean.split_once('/') {
        if let Some(idx_str) = prefix.strip_prefix('r') {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(root) = roots.get(idx) {
                    if let Some(p) = try_root(root, rest) {
                        return Some(p);
                    }
                }
            }
        }
    }

    if let Some(p) = try_root(ext_folder, clean) {
        return Some(p);
    }
    for r in roots {
        if let Some(p) = try_root(r, clean) {
            return Some(p);
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(n) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
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

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
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
        Some("wasm") => "application/wasm",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_http(port: u16, req: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }

    #[test]
    fn bridge_enforces_host_csrf_traversal_and_routes_messages() {
        let dir = std::env::temp_dir().join(format!("fx-wv-bridge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("media")).unwrap();
        std::fs::write(dir.join("media/ok.js"), "console.log('ok');").unwrap();
        std::fs::write(dir.parent().unwrap().join("secret.txt"), "nope").unwrap();

        let received: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let rec2 = Arc::clone(&received);
        let bridge = WebviewBridge::start(
            ":root{--vscode-foreground:#eee;}".into(),
            Arc::new(move |h, msg| {
                rec2.lock().unwrap().push((h, msg));
            }),
        )
        .unwrap();

        let port = bridge.port();
        let tok = bridge.token().to_string();
        bridge.register_webview("wv1", dir.clone(), vec![]);
        bridge.set_html(
            "wv1",
            "<html><head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none';\"></head><body>Hi</body></html>".into(),
        );

        let bad_host = raw_http(
            port,
            &format!(
                "GET /webview/wv1?t={tok} HTTP/1.1\r\nHost: evil.com:{port}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(bad_host.starts_with("HTTP/1.1 403"));

        let bad_tok = raw_http(
            port,
            &format!(
                "GET /webview/wv1?t=wrong HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(bad_tok.starts_with("HTTP/1.1 403"));

        let trav = raw_http(
            port,
            &format!(
                "GET /res/wv1/..%2Fsecret.txt?t={tok} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(trav.starts_with("HTTP/1.1 403"));

        let ok_res = raw_http(
            port,
            &format!(
                "GET /res/wv1/media/ok.js?t={tok} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(ok_res.starts_with("HTTP/1.1 200") && ok_res.contains("console.log('ok')"));

        let page = raw_http(
            port,
            &format!(
                "GET /webview/wv1?t={tok} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            ),
        );
        assert!(
            page.contains("--vscode-foreground:#eee;")
                && page.contains("window.acquireVsCodeApi")
                && page.contains(&format!("http://127.0.0.1:{port}"))
        );

        let post_resp = raw_http(
            port,
            &format!(
                "POST /msg/wv1?t={tok} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 10\r\nConnection: close\r\n\r\n{{\"hi\":123}}"
            ),
        );
        assert!(post_resp.starts_with("HTTP/1.1 204"));
        assert_eq!(
            received.lock().unwrap().as_slice(),
            &[("wv1".to_string(), "{\"hi\":123}".to_string())]
        );

        let mut sse = TcpStream::connect(("127.0.0.1", port)).unwrap();
        sse.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        write!(
            sse,
            "GET /events/wv1?t={tok} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
        )
        .unwrap();
        let mut sse_reader = BufReader::new(sse);
        let mut line = String::new();
        sse_reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("HTTP/1.1 200"));
        bridge.post_message("wv1", "{\"pong\":true}".into());
        let mut found_event = false;
        for _ in 0..12 {
            line.clear();
            if sse_reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line.contains("{\"pong\":true}") {
                found_event = true;
                break;
            }
        }
        assert!(found_event);

        // Second set_html emits flux-reload event and new CSP allows local iframes.
        let rev2 = bridge.set_html("wv1", "<html><body>Updated</body></html>".into());
        assert_eq!(rev2, 2);
        let mut found_reload = false;
        for _ in 0..12 {
            line.clear();
            if sse_reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line.contains("event: flux-reload") {
                found_reload = true;
                break;
            }
        }
        assert!(found_reload);
        assert!(
            page.contains("frame-src 'self'")
                && page.contains("http://127.0.0.1:*")
                && page.contains("http://localhost:*")
        );

        // Live theme update pushes flux-theme event without page reload.
        bridge.update_theme_css(":root{--vscode-foreground:#111;}".into());
        let mut found_theme = false;
        for _ in 0..12 {
            line.clear();
            if sse_reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line.contains("event: flux-theme") {
                found_theme = true;
                break;
            }
        }
        assert!(found_theme);

        // Removing the webview closes active SSE stream cleanly without leaking the thread.
        bridge.remove_webview("wv1");
        let mut eof = false;
        for _ in 0..12 {
            line.clear();
            if sse_reader.read_line(&mut line).unwrap_or(0) == 0 {
                eof = true;
                break;
            }
        }
        assert!(eof, "remove_webview must terminate active SSE loop");

        drop(bridge);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
