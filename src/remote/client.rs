use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use super::proto::{
    read_frame, write_frame, DirEntry, Hello, Request, Response, SearchWire, FRAME_BYTES, FRAME_JSON, PROTOCOL_VERSION,
};
use super::WslTarget;

type FrameMsg = Result<Option<(u8, Vec<u8>)>, String>;

const UI_RPC_TIMEOUT: Duration = Duration::from_secs(2);
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const IO_RPC_TIMEOUT: Duration = Duration::from_secs(20);
const HEAVY_RPC_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_BACKOFF_SECS: u64 = 60;

fn helper_dir() -> String {
    format!(".fluxcode/helper/{}", crate::update::BUILD)
}

fn spawn_frame_reader(mut stdout: impl Read + Send + 'static) -> mpsc::Receiver<FrameMsg> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("wsl-helper-rx".into())
        .spawn(move || loop {
            match read_frame(&mut stdout) {
                Ok(Some(frame)) => {
                    if tx.send(Ok(Some(frame))).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = tx.send(Ok(None));
                    break;
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    break;
                }
            }
        });
    rx
}

fn recv_frame_from(rx: &mpsc::Receiver<FrameMsg>, timeout: Duration) -> Result<(u8, Vec<u8>), String> {
    match rx.recv_timeout(timeout) {
        Ok(Ok(Some(frame))) => Ok(frame),
        Ok(Ok(None)) | Err(mpsc::RecvTimeoutError::Disconnected) => Err("the helper closed the connection".into()),
        Ok(Err(e)) => Err(e),
        Err(mpsc::RecvTimeoutError::Timeout) => Err("the helper timed out".into()),
    }
}

fn timeout_for(req: &Request, on_ui: bool) -> Duration {
    if on_ui {
        return UI_RPC_TIMEOUT;
    }
    match req {
        Request::Hello | Request::Ping => HELLO_TIMEOUT,
        Request::List { .. } | Request::Read { .. } | Request::Write { .. } => IO_RPC_TIMEOUT,
        Request::Scan { .. } | Request::Search { .. } | Request::Git { .. } => HEAVY_RPC_TIMEOUT,
    }
}

pub struct RemoteClient {
    child: Child,
    stdin: ChildStdin,
    rx: mpsc::Receiver<FrameMsg>,
    broken: bool,
    pub build: String,
    pub platform: String,
    pub home: String,
    pub distro: String,
}

impl RemoteClient {
    pub fn connect(distro: &str) -> Result<Self, String> {
        let path = install(distro)?;
        let mut cmd = crate::proc::command("wsl.exe");
        cmd.args(["-d", distro, "--exec", &path, "--remote-server"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| format!("the helper could not start: {e}"))?;
        let stdin = child.stdin.take().ok_or("the helper has no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("the helper has no stdout")?);
        let rx = spawn_frame_reader(stdout);
        let mut client = Self {
            child,
            stdin,
            rx,
            broken: false,
            build: String::new(),
            platform: String::new(),
            home: String::new(),
            distro: distro.to_string(),
        };
        let hello: Hello = client.json(Request::Hello)?;
        if hello.protocol != PROTOCOL_VERSION {
            return Err(format!(
                "the helper speaks protocol {}, this build speaks {PROTOCOL_VERSION}",
                hello.protocol
            ));
        }
        client.build = hello.build;
        client.platform = hello.platform;
        client.home = hello.home;
        Ok(client)
    }

    fn send_frame(&mut self, kind: u8, payload: &[u8]) -> Result<(), String> {
        write_frame(&mut self.stdin, kind, payload).map_err(|e| {
            self.broken = true;
            e.to_string()
        })
    }

    fn recv_frame(&mut self, timeout: Duration) -> Result<(u8, Vec<u8>), String> {
        recv_frame_from(&self.rx, timeout).inspect_err(|_| {
            self.broken = true;
        })
    }

    fn read_json_timeout(&mut self, timeout: Duration) -> Result<serde_json::Value, String> {
        let (kind, payload) = self.recv_frame(timeout)?;
        if kind != FRAME_JSON {
            self.broken = true;
            return Err("the helper sent something other than JSON".into());
        }
        let r: Response = serde_json::from_slice(&payload).map_err(|e| {
            self.broken = true;
            e.to_string()
        })?;
        if !r.ok {
            return Err(r.error.unwrap_or_else(|| "the helper refused the request".into()));
        }
        Ok(r.json.unwrap_or(serde_json::Value::Null))
    }

    pub fn json<T: serde::de::DeserializeOwned>(&mut self, req: Request) -> Result<T, String> {
        let timeout = timeout_for(&req, on_ui_thread());
        let body = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        self.send_frame(FRAME_JSON, &body)?;
        let value = self.read_json_timeout(timeout)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn bytes(&mut self, req: Request, body: Option<&[u8]>) -> Result<Vec<u8>, String> {
        read_path_accepts(&req)?;
        let timeout = timeout_for(&req, on_ui_thread());
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        self.send_frame(FRAME_JSON, &payload)?;
        if let Some(b) = body {
            self.send_frame(FRAME_BYTES, b)?;
        }
        let _header = self.read_json_timeout(timeout)?;
        match self.recv_frame(timeout)? {
            (FRAME_BYTES, bytes) => Ok(bytes),
            _ => {
                self.broken = true;
                Err("the helper sent something other than bytes".into())
            }
        }
    }

    pub fn list(&mut self, path: &str) -> Result<Vec<DirEntry>, String> {
        self.json(Request::List { path: path.to_string() })
    }

    pub fn read(&mut self, path: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.bytes(Request::Read { path: path.to_string(), limit }, None)
    }

    pub fn write(&mut self, path: &str, bytes: &[u8], crlf: bool) -> Result<(), String> {
        let req = Request::Write {
            path: path.to_string(),
            crlf,
        };
        write_path_accepts(&req)?;
        let timeout = timeout_for(&req, on_ui_thread());
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        self.send_frame(FRAME_JSON, &payload)?;
        self.send_frame(FRAME_BYTES, bytes)?;
        self.read_json_timeout(timeout)?;
        Ok(())
    }

    pub fn search(&mut self, root: &str, query: &str, match_case: bool, whole_word: bool) -> Result<SearchWire, String> {
        self.json(Request::Search { root: root.to_string(), query: query.to_string(), match_case, whole_word })
    }

    pub fn scan(&mut self, root: &str, max: usize) -> Result<Vec<String>, String> {
        self.json(Request::Scan { root: root.to_string(), max })
    }

    pub fn git(&mut self, cwd: &str, args: &[String], input: Option<&str>) -> Result<super::proto::GitWire, String> {
        self.json(Request::Git { cwd: cwd.to_string(), args: args.to_vec(), input: input.map(str::to_string) })
    }

    pub fn exited(&mut self) -> bool {
        self.broken || matches!(self.child.try_wait(), Ok(Some(_)) | Err(_))
    }
}

impl Drop for RemoteClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn drop_in_background(client: RemoteClient) {
    let _ = std::thread::Builder::new()
        .name("wsl-helper-drop".into())
        .spawn(move || drop(client));
}

fn read_path_accepts(req: &Request) -> Result<(), String> {
    if matches!(req, Request::Write { .. }) {
        return Err("a write is answered with one JSON frame; use write()".into());
    }
    Ok(())
}

fn write_path_accepts(req: &Request) -> Result<(), String> {
    if !matches!(req, Request::Write { .. }) {
        return Err("only a write sends a body frame; use write()".into());
    }
    Ok(())
}

#[cfg(test)]
fn read_json_from(stdout: &mut impl Read) -> Result<serde_json::Value, String> {
    let (kind, payload) = match read_frame(stdout).map_err(|e| e.to_string())? {
        Some(frame) => frame,
        None => return Err("the helper closed the connection".into()),
    };
    if kind != FRAME_JSON {
        return Err("the helper sent something other than JSON".into());
    }
    let r: Response = serde_json::from_slice(&payload).map_err(|e| e.to_string())?;
    if !r.ok {
        return Err(r.error.unwrap_or_else(|| "the helper refused the request".into()));
    }
    Ok(r.json.unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
fn write_request(
    stdin: &mut impl Write,
    stdout: &mut impl Read,
    req: &Request,
    body: &[u8],
) -> Result<(), String> {
    write_path_accepts(req)?;
    let payload = serde_json::to_vec(req).map_err(|e| e.to_string())?;
    write_frame(stdin, FRAME_JSON, &payload).map_err(|e| e.to_string())?;
    write_frame(stdin, FRAME_BYTES, body).map_err(|e| e.to_string())?;
    read_json_from(stdout)?;
    Ok(())
}

fn install(distro: &str) -> Result<String, String> {
    let dir = helper_dir();
    let path = format!("{dir}/fluxcode");
    if runs(distro, &path) {
        return helper_exe(distro);
    }
    let tarball = fetch(distro)?;
    push(distro, &tarball, &dir)?;
    if !runs(distro, &path) {
        return Err("the helper was unpacked but will not run".into());
    }
    helper_exe(distro)
}

fn helper_exe(distro: &str) -> Result<String, String> {
    let home = super::home_dir(distro).ok_or("the distro's home folder could not be read")?;
    Ok(format!("{home}/{}/fluxcode", helper_dir()))
}

fn runs(distro: &str, path: &str) -> bool {
    super::program_command(&WslTarget::new(distro, "/"), "/", "sh", &["-c", r#"test -x "$HOME/$1""#, "--", path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn curl_download_args() -> &'static [&'static str] {
    &["-sSL", "--fail", "--connect-timeout", "8", "--max-time", "300", "-o"]
}

fn curl_digest_args() -> &'static [&'static str] {
    &["-sSL", "--fail", "--connect-timeout", "8", "--max-time", "25", "-H", "Accept: application/vnd.github+json"]
}

fn fetch(distro: &str) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir().join(format!("fluxcode-helper-{}.tar.gz", crate::update::BUILD));
    if let Ok(local) = std::env::var("FLUXCODE_HELPER") {
        let src = PathBuf::from(&local);
        if src.is_file() {
            let bytes = std::fs::read(&src).map_err(|e| e.to_string())?;
            pack_single(&tmp, &bytes)?;
            return Ok(tmp);
        }
    }
    let asset = format!("FluxCode-{}-linux-x64.tar.gz", crate::update::BUILD);
    let url = format!(
        "https://github.com/MuaraAI/FluxCode/releases/download/{build}/{asset}",
        build = crate::update::BUILD
    );
    let mut cmd = crate::proc::command("curl");
    cmd.args(curl_download_args()).arg(&tmp).arg(&url);
    let out = cmd.output().map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() || !tmp.is_file() {
        return Err(format!("the Linux helper for {} could not be downloaded", crate::update::BUILD));
    }
    let want = release_digest(&asset)?;
    let got = sha256_of(&tmp)?;
    if got != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("the Linux helper for {} does not match the release's SHA-256", crate::update::BUILD));
    }
    let _ = distro;
    Ok(tmp)
}

fn release_digest(asset: &str) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/MuaraAI/FluxCode/releases/tags/{}", crate::update::BUILD);
    let out = crate::proc::command("curl")
        .args(curl_digest_args())
        .arg(&url)
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        return Err("the release could not be read to check the helper".into());
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let digest = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(asset)))
        .and_then(|x| x["digest"].as_str())
        .ok_or("the release has no SHA-256 for the Linux helper")?;
    let hex = digest.strip_prefix("sha256:").ok_or("the release's helper digest is not SHA-256")?;
    Ok(hex.to_ascii_lowercase())
}

fn sha256_of(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn pack_single(out: &std::path::Path, binary: &[u8]) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("fluxcode-helper-src-{}", std::process::id()));
    let inner = dir.join(crate::update::BUILD);
    std::fs::create_dir_all(&inner).map_err(|e| e.to_string())?;
    std::fs::write(inner.join("fluxcode"), binary).map_err(|e| e.to_string())?;
    let status = crate::proc::command("tar")
        .arg("-czf")
        .arg(out)
        .arg("-C")
        .arg(&dir)
        .arg(crate::update::BUILD)
        .status()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    if status.success() {
        Ok(())
    } else {
        Err("the helper could not be packed".into())
    }
}

fn push(distro: &str, tarball: &std::path::Path, dir: &str) -> Result<(), String> {
    let mut child = super::program_command(
        &WslTarget::new(distro, "/"),
        "/",
        "sh",
        &["-c", r#"mkdir -p "$HOME/$1" && tar -xzf - -C "$HOME/$1" --strip-components=1 && chmod 755 "$HOME/$1/fluxcode""#, "--", dir],
    )
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let mut f = std::fs::File::open(tarball).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            stdin.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("the helper could not be unpacked: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_helper_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn helper_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Recorded inside `Application::run` on GPUI's foreground UI thread so `with_client` never blocks UI frames.
static UI_THREAD: OnceLock<ThreadId> = OnceLock::new();

pub fn note_ui_thread() {
    let _ = UI_THREAD.set(std::thread::current().id());
}

fn on_ui_thread() -> bool {
    UI_THREAD.get() == Some(&std::thread::current().id())
}

#[derive(Default)]
struct DistroState {
    warming: bool,
    backoff_until: Option<Instant>,
}

static STATES: OnceLock<Mutex<HashMap<String, DistroState>>> = OnceLock::new();

fn states() -> &'static Mutex<HashMap<String, DistroState>> {
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cooling_down(distro: &str) -> bool {
    let guard = states().lock().unwrap_or_else(|p| p.into_inner());
    guard
        .get(distro)
        .and_then(|s| s.backoff_until)
        .is_some_and(|until| Instant::now() < until)
}

fn record_connect_failure(distro: &str) {
    let mut guard = states().lock().unwrap_or_else(|p| p.into_inner());
    guard.entry(distro.to_string()).or_default().backoff_until =
        Some(Instant::now() + Duration::from_secs(CONNECT_BACKOFF_SECS));
}

fn clear_connect_failure(distro: &str) {
    let mut guard = states().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = guard.get_mut(distro) {
        s.backoff_until = None;
    }
}

fn clear_all_backoffs() {
    let mut guard = states().lock().unwrap_or_else(|p| p.into_inner());
    for s in guard.values_mut() {
        s.backoff_until = None;
    }
}

fn begin_warming(distro: &str) -> bool {
    let mut guard = states().lock().unwrap_or_else(|p| p.into_inner());
    let entry = guard.entry(distro.to_string()).or_default();
    if entry.warming || entry.backoff_until.is_some_and(|until| Instant::now() < until) {
        return false;
    }
    entry.warming = true;
    true
}

fn finish_warming(distro: &str) {
    let mut guard = states().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = guard.get_mut(distro) {
        s.warming = false;
    }
}

struct WarmGuard(String);

impl Drop for WarmGuard {
    fn drop(&mut self) {
        finish_warming(&self.0);
    }
}

/// Generation counter bumped by `drop_client()` so in-flight connects or contended drops never retain stale helpers.
static CLIENT_GEN: AtomicU64 = AtomicU64::new(1);
static CLIENT: OnceLock<Mutex<Option<(u64, RemoteClient)>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<(u64, RemoteClient)>> {
    CLIENT.get_or_init(|| Mutex::new(None))
}

pub fn warm(distro: &str) {
    if !helper_enabled() {
        return;
    }
    let cur_gen = CLIENT_GEN.load(Ordering::SeqCst);
    if let Ok(mut guard) = slot().try_lock() {
        if guard
            .as_mut()
            .is_some_and(|(g, c)| *g == cur_gen && c.distro == distro && !c.exited())
        {
            return;
        }
    }
    if !begin_warming(distro) {
        return;
    }
    let distro_name = distro.to_string();
    let spawned = std::thread::Builder::new()
        .name("wsl-helper".into())
        .spawn(move || {
            let _guard = WarmGuard(distro_name.clone());
            with_client(&distro_name, |_| ());
        });
    if spawned.is_err() {
        finish_warming(distro);
    }
}

pub fn with_client<T>(distro: &str, f: impl FnOnce(&mut RemoteClient) -> T) -> Option<T> {
    if !helper_enabled() {
        return None;
    }
    let cur_gen = CLIENT_GEN.load(Ordering::SeqCst);
    if on_ui_thread() {
        // The UI thread never connects, installs, or blocks on a contended mutex.
        let mut guard = slot().try_lock().ok()?;
        let usable = guard
            .as_mut()
            .is_some_and(|(g, c)| *g == cur_gen && c.distro == distro && !c.exited());
        if !usable {
            if let Some((g, mut c)) = guard.take() {
                if g != cur_gen || c.exited() {
                    drop_in_background(c);
                } else {
                    *guard = Some((g, c));
                }
            }
            return None;
        }
        let out = {
            let (_, client) = guard.as_mut()?;
            f(client)
        };
        if guard.as_ref().is_some_and(|(_, c)| c.broken) {
            if let Some((_, old)) = guard.take() {
                drop_in_background(old);
            }
        }
        return Some(out);
    }

    if cooling_down(distro) {
        return None;
    }
    let mut guard = slot().lock().ok()?;
    let gen = CLIENT_GEN.load(Ordering::SeqCst);
    let stale = guard
        .as_mut()
        .map(|(g, c)| *g != gen || c.distro != distro || c.exited())
        .unwrap_or(true);
    if stale {
        if cooling_down(distro) {
            return None;
        }
        // Drop any old helper before starting a new connection (safe here on a background thread).
        guard.take();
        match RemoteClient::connect(distro) {
            Ok(client) => {
                clear_connect_failure(distro);
                // If drop_client() raced with connect(), discard this helper instead of storing a stale one.
                if CLIENT_GEN.load(Ordering::SeqCst) != gen {
                    drop(client);
                    return None;
                }
                *guard = Some((gen, client));
            }
            Err(_) => {
                record_connect_failure(distro);
                return None;
            }
        }
    }
    let out = {
        let (_, client) = guard.as_mut()?;
        f(client)
    };
    if guard.as_ref().is_some_and(|(_, c)| c.broken) {
        guard.take();
    }
    Some(out)
}

pub fn drop_client() {
    CLIENT_GEN.fetch_add(1, Ordering::SeqCst);
    clear_all_backoffs();
    if let Ok(mut guard) = slot().try_lock() {
        if let Some((_, old)) = guard.take() {
            drop_in_background(old);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_thread_detection_distinguishes_ui_and_worker_threads() {
        note_ui_thread();
        assert!(on_ui_thread());
        let worker_saw_ui = std::thread::spawn(on_ui_thread).join().unwrap();
        assert!(!worker_saw_ui);
    }

    #[test]
    fn backoff_and_warming_are_tracked_per_distro() {
        let a = "TestDistro-A-Backoff";
        let b = "TestDistro-B-Backoff";
        clear_connect_failure(a);
        clear_connect_failure(b);

        record_connect_failure(a);
        assert!(cooling_down(a));
        assert!(!cooling_down(b));

        // Failing distro B must not erase distro A's backoff.
        record_connect_failure(b);
        assert!(cooling_down(a));
        assert!(cooling_down(b));

        clear_connect_failure(a);
        assert!(!cooling_down(a));
        assert!(cooling_down(b));

        // Per-distro warming: warming A must not block warming B (when B is out of backoff).
        clear_connect_failure(b);
        assert!(begin_warming(a));
        assert!(!begin_warming(a), "duplicate warm for same distro must be deduplicated");
        assert!(begin_warming(b), "warming one distro must not block another distro");
        finish_warming(a);
        finish_warming(b);
        assert!(begin_warming(a));
        finish_warming(a);
    }

    #[test]
    fn drop_client_bumps_generation_and_clears_backoffs_even_under_lock() {
        let distro = "TestDistro-DropGen";
        record_connect_failure(distro);
        assert!(cooling_down(distro));

        let gen_before = CLIENT_GEN.load(Ordering::SeqCst);
        let held = slot().lock().unwrap();
        // Calling drop_client while slot() is locked must never block, must bump gen, and must clear backoff.
        let started = Instant::now();
        drop_client();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(CLIENT_GEN.load(Ordering::SeqCst) > gen_before);
        assert!(!cooling_down(distro));
        drop(held);
    }

    #[test]
    fn wedged_helper_times_out_instead_of_hanging_forever() {
        let (client_in, _server_out) = std::io::pipe().unwrap();
        let rx = spawn_frame_reader(client_in);
        let started = Instant::now();
        let err = recv_frame_from(&rx, Duration::from_millis(80)).unwrap_err();
        let elapsed = started.elapsed();
        assert!(err.contains("timed out"), "{err}");
        assert!(elapsed < Duration::from_secs(2), "timeout took {elapsed:?}");
    }

    #[test]
    fn curl_commands_enforce_connect_and_total_timeouts() {
        let dl = curl_download_args();
        assert!(dl.windows(2).any(|w| w == ["--connect-timeout", "8"]), "{dl:?}");
        assert!(dl.windows(2).any(|w| w == ["--max-time", "300"]), "{dl:?}");
        let dg = curl_digest_args();
        assert!(dg.windows(2).any(|w| w == ["--connect-timeout", "8"]), "{dg:?}");
        assert!(dg.windows(2).any(|w| w == ["--max-time", "25"]), "{dg:?}");
    }

    #[test]
    fn the_helper_is_off_until_it_is_turned_on() {
        assert!(!helper_enabled());
        assert!(with_client("Ubuntu", |_| ()).is_none());
    }

    #[test]
    fn a_file_hashes_to_its_sha256() {
        let dir = std::env::temp_dir().join(format!("fluxcode-sha-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(sha256_of(&f).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_returns_after_the_single_json_answer() {
        let (mut server_in, mut client_out) = std::io::pipe().unwrap();
        let (mut client_in, mut server_out) = std::io::pipe().unwrap();
        let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();

        let server = std::thread::spawn(move || {
            let (kind, payload) = read_frame(&mut server_in).unwrap().unwrap();
            assert_eq!(kind, FRAME_JSON);
            let req: Request = serde_json::from_slice(&payload).unwrap();
            assert!(matches!(req, Request::Write { .. }), "{req:?}");
            let (kind, body) = read_frame(&mut server_in).unwrap().unwrap();
            assert_eq!(kind, FRAME_BYTES);
            assert_eq!(body, b"hello");
            let answer = Response::json(serde_json::json!({"written": 5}));
            write_frame(
                &mut server_out,
                FRAME_JSON,
                &serde_json::to_vec(&answer).unwrap(),
            )
            .unwrap();
            let _ = hold_rx.recv_timeout(std::time::Duration::from_secs(5));
        });

        let started = std::time::Instant::now();
        let result = write_request(
            &mut client_out,
            &mut client_in,
            &Request::Write {
                path: "/w/a.txt".into(),
                crlf: false,
            },
            b"hello",
        );
        let waited = started.elapsed();
        let _ = hold_tx.send(());
        server.join().unwrap();

        assert!(result.is_ok(), "{result:?}");
        assert!(
            waited < std::time::Duration::from_secs(3),
            "write waited {waited:?} for a bytes frame"
        );
    }

    #[test]
    fn a_refused_write_returns_the_error_without_a_second_frame() {
        let (mut server_in, mut client_out) = std::io::pipe().unwrap();
        let (mut client_in, mut server_out) = std::io::pipe().unwrap();
        let server = std::thread::spawn(move || {
            let _ = read_frame(&mut server_in).unwrap().unwrap();
            let _ = read_frame(&mut server_in).unwrap().unwrap();
            let answer = Response::error("no such folder");
            write_frame(
                &mut server_out,
                FRAME_JSON,
                &serde_json::to_vec(&answer).unwrap(),
            )
            .unwrap();
        });
        let result = write_request(
            &mut client_out,
            &mut client_in,
            &Request::Write {
                path: "/nope/a.txt".into(),
                crlf: false,
            },
            b"x",
        );
        server.join().unwrap();
        assert_eq!(result.unwrap_err(), "no such folder");
    }

    #[test]
    fn the_read_path_refuses_a_write() {
        let err = read_path_accepts(&Request::Write {
            path: "/w/a.txt".into(),
            crlf: false,
        });
        assert_eq!(
            err.unwrap_err(),
            "a write is answered with one JSON frame; use write()"
        );
        assert!(read_path_accepts(&Request::Read {
            path: "/w/a.txt".into(),
            limit: 10
        })
        .is_ok());
    }

    #[test]
    fn the_write_path_refuses_a_read() {
        let err = write_path_accepts(&Request::Read {
            path: "/w/a.txt".into(),
            limit: 10,
        });
        assert_eq!(
            err.unwrap_err(),
            "only a write sends a body frame; use write()"
        );
        assert!(write_path_accepts(&Request::Write {
            path: "/w/a.txt".into(),
            crlf: false
        })
        .is_ok());
    }
}
