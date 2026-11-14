//! The editor's side of the helper inside a distro: installing it, starting it,
//! and one request at a time over its stdin/stdout.
//!
//! The helper is the same program as the editor, built for Linux. It comes from
//! the release of the build that is running, so the two are always the same
//! version; a development build can point at a local binary instead with
//! `FLUXCODE_HELPER`.

use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::{Mutex, OnceLock};

use super::proto::{
    read_frame, write_frame, DirEntry, Hello, Request, Response, SearchWire, Stat, FRAME_BYTES, FRAME_JSON, PROTOCOL_VERSION,
};
use super::WslTarget;

/// Where the helper lives inside the distro, per build.
fn helper_dir() -> String {
    format!(".fluxcode/helper/{}", crate::update::BUILD)
}

pub struct RemoteClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pub build: String,
    pub platform: String,
    pub home: String,
    pub distro: String,
}

impl RemoteClient {
    /// Start the helper in `distro`, installing it first if it is not there.
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
        let mut client = Self {
            child,
            stdin,
            stdout,
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

    /// One request, one JSON answer.
    pub fn json<T: serde::de::DeserializeOwned>(&mut self, req: Request) -> Result<T, String> {
        let body = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        write_frame(&mut self.stdin, FRAME_JSON, &body).map_err(|e| e.to_string())?;
        let value = self.read_json()?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    /// A request whose answer is a JSON header followed by the bytes.
    pub fn bytes(&mut self, req: Request, body: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        write_frame(&mut self.stdin, FRAME_JSON, &payload).map_err(|e| e.to_string())?;
        if let Some(b) = body {
            write_frame(&mut self.stdin, FRAME_BYTES, b).map_err(|e| e.to_string())?;
        }
        // The header says whether it worked; the bytes follow only when it did.
        let _header = self.read_json()?;
        match read_frame(&mut self.stdout).map_err(|e| e.to_string())? {
            Some((FRAME_BYTES, bytes)) => Ok(bytes),
            Some(_) => Err("the helper sent something other than bytes".into()),
            None => Err("the helper closed the connection".into()),
        }
    }

    fn read_json(&mut self) -> Result<serde_json::Value, String> {
        let (kind, payload) = match read_frame(&mut self.stdout).map_err(|e| e.to_string())? {
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

    // ---- the calls the editor makes -------------------------------------

    pub fn list(&mut self, path: &str) -> Result<Vec<DirEntry>, String> {
        self.json(Request::List { path: path.to_string() })
    }

    pub fn stat(&mut self, path: &str) -> Result<Stat, String> {
        self.json(Request::Stat { path: path.to_string() })
    }

    pub fn read(&mut self, path: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.bytes(Request::Read { path: path.to_string(), limit }, None)
    }

    pub fn write(&mut self, path: &str, bytes: &[u8], crlf: bool) -> Result<(), String> {
        self.bytes(Request::Write { path: path.to_string(), crlf }, Some(bytes)).map(|_| ())
    }

    pub fn search(&mut self, root: &str, query: &str, match_case: bool, whole_word: bool) -> Result<SearchWire, String> {
        self.json(Request::Search { root: root.to_string(), query: query.to_string(), match_case, whole_word })
    }

    /// Every file under `root`, read inside the distro. This is the list behind
    /// Ctrl+P, so the 9p share is not walked for it.
    pub fn scan(&mut self, root: &str, max: usize) -> Result<Vec<String>, String> {
        self.json(Request::Scan { root: root.to_string(), max })
    }

    pub fn git(&mut self, cwd: &str, args: &[String], input: Option<&str>) -> Result<super::proto::GitWire, String> {
        self.json(Request::Git { cwd: cwd.to_string(), args: args.to_vec(), input: input.map(str::to_string) })
    }

    /// Whether the helper process has gone away (a distro shutdown, a crash).
    pub fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)) | Err(_))
    }
}

impl Drop for RemoteClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Make sure the helper for this build is in the distro, and return its path.
fn install(distro: &str) -> Result<String, String> {
    let dir = helper_dir();
    let path = format!("{dir}/fluxcode");
    // `test -x` is the whole version check: the directory carries the build.
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

/// The helper's absolute path inside the distro: its home plus the relative dir.
fn helper_exe(distro: &str) -> Result<String, String> {
    let home = super::home_dir(distro).ok_or("the distro's home folder could not be read")?;
    Ok(format!("{home}/{}/fluxcode", helper_dir()))
}

fn runs(distro: &str, path: &str) -> bool {
    // The path is a positional parameter, never spliced into the script.
    super::program_command(&WslTarget::new(distro, "/"), "/", "sh", &["-c", r#"test -x "$HOME/$1""#, "--", path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A local Linux binary when one is supplied (a development build), else the
/// Linux tarball of this build's release.
fn fetch(distro: &str) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir().join(format!("fluxcode-helper-{}.tar.gz", crate::update::BUILD));
    if let Ok(local) = std::env::var("FLUXCODE_HELPER") {
        let src = PathBuf::from(&local);
        // A bare binary is packed here so `push` has one thing to send.
        if src.is_file() {
            let bytes = std::fs::read(&src).map_err(|e| e.to_string())?;
            pack_single(&tmp, &bytes)?;
            return Ok(tmp);
        }
    }
    // The release that carries this build, in the same repository updates come from.
    let asset = format!("FluxCode-{}-linux-x64.tar.gz", crate::update::BUILD);
    let url = format!(
        "https://github.com/MuaraAI/FluxCode/releases/download/{build}/{asset}",
        build = crate::update::BUILD
    );
    let mut cmd = crate::proc::command("curl");
    cmd.args(["-sSL", "--fail", "-o"]).arg(&tmp).arg(&url);
    let out = cmd.output().map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() || !tmp.is_file() {
        return Err(format!("the Linux helper for {} could not be downloaded", crate::update::BUILD));
    }
    // The release records a SHA-256 for every asset. Refuse to run a tarball
    // that does not match it: a truncated or swapped download must not execute.
    let want = release_digest(&asset)?;
    let got = sha256_of(&tmp)?;
    if got != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("the Linux helper for {} does not match the release's SHA-256", crate::update::BUILD));
    }
    let _ = distro;
    Ok(tmp)
}

/// The SHA-256 the release records for `asset`, from the GitHub API.
fn release_digest(asset: &str) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/MuaraAI/FluxCode/releases/tags/{}", crate::update::BUILD);
    let out = crate::proc::command("curl")
        .args(["-sSL", "--fail", "-H", "Accept: application/vnd.github+json", &url])
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

/// SHA-256 of a file, in process: no system tool to find.
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

/// Wrap one binary in a tar.gz laid out like the release archive (one folder,
/// `fluxcode` inside it), so `push` treats the two the same way.
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

/// Send the tarball into the distro and unpack it, with no `/mnt` copy: the
/// share is slow and permissions are unreliable there.
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

// ---- one connection per distro, shared by every caller -------------------

/// Whether the helper may be used at all. Off by default: it installs and runs
/// a program inside the distro, so the user turns it on in Settings.
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_helper_enabled(on: bool) {
    ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn helper_enabled() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

static CLIENT: OnceLock<Mutex<Option<RemoteClient>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<RemoteClient>> {
    CLIENT.get_or_init(|| Mutex::new(None))
}

/// Run `f` against the helper for `distro`, connecting it on first use.
/// `None` means the helper is not available and the caller should use the UNC
/// path instead; the editor works either way.
pub fn with_client<T>(distro: &str, f: impl FnOnce(&mut RemoteClient) -> T) -> Option<T> {
    if !helper_enabled() {
        return None;
    }
    let mut guard = slot().lock().ok()?;
    let stale = guard
        .as_mut()
        .map(|c| c.distro != *distro || c.exited())
        .unwrap_or(true);
    if stale {
        *guard = RemoteClient::connect(distro).ok();
    }
    let client = guard.as_mut()?;
    Some(f(client))
}

/// Forget the connection (a distro shut down, or a call failed).
pub fn drop_client() {
    if let Ok(mut guard) = slot().lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_is_off_until_it_is_turned_on() {
        // Off by default: nothing is installed, started or read through it.
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
}
