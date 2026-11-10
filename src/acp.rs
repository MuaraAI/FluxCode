//! The ACP agents the panel can talk to, and how each one is found, installed and started.
//!
//! Every agent speaks the Agent Client Protocol over stdio, so FluxCode only needs a
//! command line per agent. Three places can supply it, checked in this order:
//!
//! 1. An install FluxCode made itself, under `~/.fluxcode/agents/<id>/launch.json`.
//! 2. A copy the user installed on PATH (the names in `KNOWN`).
//! 3. Nothing yet: the picker shows a download button, which installs from the
//!    official ACP registry (cdn.agentclientprotocol.com). Native binaries are
//!    preferred; npm packages go into the agent's own folder (not the global npm),
//!    and are started with `node` directly, without npx in between.
//!
//! The registry is one JSON file, fetched with the system curl at most once a day and
//! only when the picker needs it. Nothing here runs at start-up.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const REGISTRY_URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
const REGISTRY_FILE: &str = "acp-registry.json";
const REGISTRY_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// An agent FluxCode lists even before the registry is read.
pub struct Known {
    /// The registry id ("mura" is not in the registry; it is MuaraAI's own harness).
    pub id: &'static str,
    pub name: &'static str,
    /// Who makes it, in a few words, for the picker's second line.
    pub by: &'static str,
    /// Executables a user install puts on PATH, with the arguments that start ACP.
    pub path: &'static [(&'static str, &'static [&'static str])],
}

/// The picker's main list, roughly by how many people use each one. Every other
/// registry agent shows under "More agents".
pub const KNOWN: &[Known] = &[
    Known { id: "mura", name: "Mura", by: "MuaraAI", path: &[("mura", &["acp"])] },
    Known { id: "opencode", name: "OpenCode", by: "open source", path: &[("opencode", &["acp"])] },
    Known { id: "claude-acp", name: "Claude Agent", by: "Anthropic", path: &[("claude-agent-acp", &[]), ("claude-code-acp", &[])] },
    Known { id: "codex-acp", name: "Codex", by: "OpenAI", path: &[("codex-acp", &[])] },
    Known { id: "gemini", name: "Gemini CLI", by: "Google", path: &[("gemini", &["--acp"])] },
    Known { id: "github-copilot-cli", name: "GitHub Copilot", by: "GitHub", path: &[("copilot", &["--acp"])] },
    Known { id: "cursor", name: "Cursor", by: "Anysphere", path: &[("cursor-agent", &["acp"])] },
    Known { id: "qwen-code", name: "Qwen Code", by: "Alibaba", path: &[("qwen", &["--acp"])] },
    Known { id: "kimi-code", name: "Kimi Code", by: "Moonshot AI", path: &[] },
    Known { id: "goose", name: "goose", by: "Block", path: &[("goose", &["acp"])] },
    Known { id: "mistral-vibe", name: "Mistral Vibe", by: "Mistral AI", path: &[("vibe-acp", &[])] },
    Known { id: "cline", name: "Cline", by: "Cline", path: &[("cline", &["--acp"])] },
    Known { id: "kilo", name: "Kilo", by: "Kilo Code", path: &[("kilo", &["acp"])] },
    Known { id: "auggie", name: "Auggie CLI", by: "Augment Code", path: &[("auggie", &["--acp"])] },
    Known { id: "factory-droid", name: "Factory Droid", by: "Factory", path: &[("droid", &["exec", "--output-format", "acp-daemon"])] },
    Known { id: "junie", name: "Junie", by: "JetBrains", path: &[("junie", &["--acp=true"])] },
];

/// Where an agent's instruction file lives: in the home folder, or in the project.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Home(&'static str),
    Project(&'static str),
}

/// The files each agent reads for its standing instructions (its "soul"), as each
/// project documents them. Mura's own character is built into the program; it reads
/// the project's MURA.md / AGENTS.md / CLAUDE.md.
pub fn instruction_files(id: &str) -> &'static [Place] {
    use Place::*;
    match id {
        "mura" => &[Project("MURA.md"), Project("AGENTS.md")],
        "opencode" => &[Home(".config/opencode/AGENTS.md"), Project("AGENTS.md")],
        "claude-acp" => &[Home(".claude/CLAUDE.md"), Project("CLAUDE.md")],
        "codex-acp" => &[Home(".codex/AGENTS.md"), Project("AGENTS.md")],
        "gemini" => &[Home(".gemini/GEMINI.md"), Project("GEMINI.md")],
        "qwen-code" => &[Home(".qwen/QWEN.md"), Project("QWEN.md")],
        "github-copilot-cli" => &[Project(".github/copilot-instructions.md"), Project("AGENTS.md")],
        "cursor" => &[Project("AGENTS.md")],
        "goose" => &[Home(".config/goose/.goosehints"), Project(".goosehints")],
        _ => &[Project("AGENTS.md")],
    }
}

pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// How to start an installed agent.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Launch {
    pub cmd: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// The registry version that was installed (empty for PATH copies).
    #[serde(default)]
    pub version: String,
}

impl Launch {
    pub fn command(&self) -> std::process::Command {
        let s = self.cmd.to_string_lossy();
        // A `.cmd`/`.bat` shim (npm on Windows, Cursor's launcher) needs cmd.exe to run.
        let mut c = if cfg!(windows) && (s.ends_with(".cmd") || s.ends_with(".bat")) {
            let mut c = crate::proc::command("cmd.exe");
            c.arg("/d").arg("/c").arg(self.cmd.as_os_str());
            c
        } else {
            crate::proc::command(&s)
        };
        c.args(&self.args);
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c
    }
}

/// `~/.fluxcode/agents`, next to the FluxCode install.
pub fn agents_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    home.join(".fluxcode").join("agents")
}

fn launch_file(id: &str) -> PathBuf {
    agents_dir().join(id).join("launch.json")
}

/// The full path of `exe` on PATH (with Windows' executable extensions).
///
/// Looking up 16 agents by probing every PATH folder for every extension cost about
/// 50 ms of start-up; instead each PATH folder is listed once (per `forget`) into a
/// name index, and lookups are hash hits. The first folder on PATH wins, as in a shell.
pub fn which(exe: &str) -> Option<PathBuf> {
    static INDEX: Mutex<(u64, Option<std::sync::Arc<HashMap<String, PathBuf>>>)> = Mutex::new((0, None));
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    let index = {
        let mut g = INDEX.lock().ok()?;
        if g.0 != gen || g.1.is_none() {
            *g = (gen, Some(std::sync::Arc::new(path_index())));
        }
        g.1.clone()?
    };
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ".com", ""] } else { &[""] };
    let key = |e: &str| if cfg!(windows) { format!("{exe}{e}").to_ascii_lowercase() } else { format!("{exe}{e}") };
    exts.iter().find_map(|e| index.get(&key(e)).filter(|p| p.is_file()).cloned())
}

/// Every file in the PATH folders, by name (lower-case on Windows), first folder first.
fn path_index() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let Some(path) = std::env::var_os("PATH") else { return map };
    // Windows' own folders hold thousands of files and never an agent: skip them.
    let system = std::env::var("SystemRoot").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        if cfg!(windows) && !system.is_empty() && dir.to_string_lossy().to_ascii_lowercase().starts_with(&system) {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let name = if cfg!(windows) { name.to_ascii_lowercase() } else { name };
            map.entry(name).or_insert_with(|| e.path());
        }
    }
    map
}

/// Bumped by `forget`: cached lookups older than this are redone.
static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Where an agent can be started from, if anywhere. Results are cached until
/// `forget` (an install or uninstall), so the panel can ask on every frame.
pub fn locate(id: &str) -> Option<Launch> {
    static CACHE: Mutex<(u64, Option<HashMap<String, Option<Launch>>>)> = Mutex::new((0, None));
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    let mut g = CACHE.lock().ok()?;
    if g.0 != gen {
        *g = (gen, Some(HashMap::new()));
    }
    let map = g.1.get_or_insert_with(HashMap::new);
    map.entry(id.to_string()).or_insert_with(|| find(id)).clone()
}

/// Ids of the agents that can start now: the known ones found on this PC (in the
/// known list's order), then registry agents FluxCode installed. Cached like `locate`.
pub fn installed() -> Vec<String> {
    static CACHE: Mutex<(u64, Vec<String>)> = Mutex::new((0, Vec::new()));
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    if let Ok(g) = CACHE.lock() {
        if g.0 == gen {
            return g.1.clone();
        }
    }
    let mut ids: Vec<String> = KNOWN.iter().filter(|k| locate(k.id).is_some()).map(|k| k.id.to_string()).collect();
    if let Ok(rd) = std::fs::read_dir(agents_dir()) {
        let mut extra: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|id| known(id).is_none() && locate(id).is_some())
            .collect();
        extra.sort();
        ids.extend(extra);
    }
    if let Ok(mut g) = CACHE.lock() {
        *g = (gen, ids.clone());
    }
    ids
}

/// Look again on the next `locate` (after an install, or when the user may have installed one).
pub fn forget() {
    GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn find(id: &str) -> Option<Launch> {
    if let Some(l) = std::fs::read_to_string(launch_file(id)).ok().and_then(|s| serde_json::from_str::<Launch>(&s).ok()) {
        let local = l.cmd.is_absolute();
        if !local || l.cmd.is_file() {
            return Some(l);
        }
    }
    let k = known(id)?;
    if id == "mura" {
        return crate::agent::find_mura().map(|cmd| Launch { cmd, args: vec!["acp".into()], ..Default::default() });
    }
    if id == "opencode" {
        return crate::agent::find_opencode().map(|cmd| Launch { cmd, args: vec!["acp".into()], ..Default::default() });
    }
    k.path.iter().find_map(|(exe, args)| which(exe).map(|cmd| Launch { cmd, args: args.iter().map(|a| a.to_string()).collect(), ..Default::default() }))
}

// ---- the registry ----

/// One agent from the ACP registry.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// The registry's icon: a 16x16 SVG drawn in `currentColor`.
    pub icon: String,
    dist: Value,
}

impl Entry {
    /// How this platform installs it: a native binary when there is one, else npm, else uv.
    pub fn method(&self) -> Option<Method> {
        let plat = platform();
        if let Some(b) = self.dist.get("binary").and_then(|b| b.get(plat)) {
            return Some(Method::Binary {
                url: b.get("archive")?.as_str()?.to_string(),
                cmd: b.get("cmd")?.as_str()?.to_string(),
                args: strings(b.get("args")),
                env: env_map(b.get("env")),
                sha256: b.get("sha256").and_then(Value::as_str).map(str::to_ascii_lowercase),
            });
        }
        if let Some(n) = self.dist.get("npx") {
            return Some(Method::Npm { package: n.get("package")?.as_str()?.to_string(), args: strings(n.get("args")), env: env_map(n.get("env")) });
        }
        if let Some(u) = self.dist.get("uvx") {
            return Some(Method::Uv { package: u.get("package")?.as_str()?.to_string(), args: strings(u.get("args")), env: env_map(u.get("env")) });
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Method {
    Binary { url: String, cmd: String, args: Vec<String>, env: HashMap<String, String>, sha256: Option<String> },
    Npm { package: String, args: Vec<String>, env: HashMap<String, String> },
    Uv { package: String, args: Vec<String>, env: HashMap<String, String> },
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn env_map(v: Option<&Value>) -> HashMap<String, String> {
    v.and_then(Value::as_object).map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()).unwrap_or_default()
}

/// The registry's platform key for this build.
pub fn platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "windows-aarch64",
        ("windows", _) => "windows-x86_64",
        ("macos", "aarch64") => "darwin-aarch64",
        ("macos", _) => "darwin-x86_64",
        (_, "aarch64") => "linux-aarch64",
        _ => "linux-x86_64",
    }
}

pub fn parse_registry(json: &str) -> Vec<Entry> {
    let Ok(v) = serde_json::from_str::<Value>(json.trim_start_matches('\u{feff}')) else { return Vec::new() };
    let s = |a: &Value, k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    v.get("agents")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let id = a.get("id")?.as_str()?.to_string();
                    Some(Entry { name: s(a, "name"), description: s(a, "description"), version: s(a, "version"), icon: s(a, "icon"), dist: a.get("distribution")?.clone(), id })
                })
                .collect()
        })
        .unwrap_or_default()
}

static REGISTRY: Mutex<Option<std::sync::Arc<Vec<Entry>>>> = Mutex::new(None);

/// The registry as last saved on disk (empty before the first fetch).
pub fn registry() -> std::sync::Arc<Vec<Entry>> {
    let Ok(mut g) = REGISTRY.lock() else { return Default::default() };
    g.get_or_insert_with(|| {
        let text = std::fs::read_to_string(crate::settings::config_dir().join(REGISTRY_FILE)).unwrap_or_default();
        std::sync::Arc::new(parse_registry(&text))
    })
    .clone()
}

/// Fetch the registry in the background when the saved copy is old (once at a time).
/// The picker shows the fresh list the next time it opens.
pub fn refresh_registry_soon() {
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !registry_stale() {
        fetch_icons_soon();
        return;
    }
    if RUNNING.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(|| {
        let _ = refresh_registry();
        fetch_icons();
        // An agent may have been installed outside FluxCode in the meantime.
        forget();
        RUNNING.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}

/// True when the saved registry is missing or older than a day.
pub fn registry_stale() -> bool {
    std::fs::metadata(crate::settings::config_dir().join(REGISTRY_FILE))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age > REGISTRY_MAX_AGE)
}

/// What a curl error means, in a sentence: curl's own message names exit codes
/// ("curl: (7) Failed to connect to ... port 443 after 2017 ms").
pub fn curl_error(stderr: &str) -> String {
    let e = stderr.trim();
    let code = e.split_once("curl: (").and_then(|(_, r)| r.split_once(')')).and_then(|(n, _)| n.parse::<u32>().ok());
    match code {
        Some(5 | 6) => "The server's name could not be found. Check the internet connection, then try again.".into(),
        Some(7) => "Could not connect to the server. Check the internet connection or proxy, then try again.".into(),
        Some(28) => "The connection timed out. Try again on a steadier connection.".into(),
        Some(35 | 60) => "The secure connection to the server failed (TLS). A proxy or antivirus may be in the way.".into(),
        Some(22) if e.contains("404") => "The file is no longer on the server (404). The registry may list a newer version tomorrow.".into(),
        Some(22) => format!("The server refused the request ({}).", e.rsplit(':').next().unwrap_or(e).trim()),
        Some(23) => "The file could not be written to disk. Check the free space.".into(),
        Some(37) => "The file is not there.".into(),
        _ if e.is_empty() => "The server did not send the file.".into(),
        _ => e.trim_start_matches("curl: ").to_string(),
    }
}

fn curl() -> std::process::Command {
    crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
}

/// Download the registry and keep it (blocking; run it off the UI thread).
pub fn refresh_registry() -> Result<(), String> {
    let out = curl()
        .args(["-s", "-S", "-f", "-L", "--max-time", "20", "-H", "User-Agent: FluxCode", REGISTRY_URL])
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        return Err(curl_error(&String::from_utf8_lossy(&out.stderr)));
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let list = parse_registry(&text);
    if list.is_empty() {
        return Err("the ACP registry answered with no agents".into());
    }
    let dir = crate::settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join(REGISTRY_FILE), &text).map_err(|e| e.to_string())?;
    if let Ok(mut g) = REGISTRY.lock() {
        *g = Some(std::sync::Arc::new(list));
    }
    Ok(())
}

/// An agent's display name: the known list, then the registry, then the id itself.
pub fn display_name(id: &str) -> String {
    if let Some(k) = known(id) {
        return k.name.to_string();
    }
    registry().iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_else(|| id.to_string())
}

// ---- icons ----
//
// The registry gives every agent a 16x16 SVG in `currentColor`. FluxCode keeps a copy
// per agent version under `<config>/acp-icons`, fetched in one curl call for all
// missing ones, so a logo that changes upstream arrives with the agent's next version.
// The SVGs are drawn as masks in the theme's text color (`svg()`), so they cost one
// small cached texture each.

/// The asset path prefix `main.rs` serves from the icon folder.
pub const ICON_ASSETS: &str = "acp-icons/";

pub fn icons_dir() -> PathBuf {
    crate::settings::config_dir().join("acp-icons")
}

fn icon_file_name(e: &Entry) -> String {
    let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect::<String>();
    format!("{}@{}.svg", clean(&e.id), clean(&e.version))
}

/// The asset path of an agent's icon, when it is on disk. Cached like `locate`.
pub fn icon_asset(id: &str) -> Option<String> {
    static CACHE: Mutex<(u64, Option<HashMap<String, Option<String>>>)> = Mutex::new((0, None));
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    let mut g = CACHE.lock().ok()?;
    if g.0 != gen {
        *g = (gen, Some(HashMap::new()));
    }
    let map = g.1.get_or_insert_with(HashMap::new);
    map.entry(id.to_string())
        .or_insert_with(|| {
            let e = registry().iter().find(|e| e.id == id).cloned()?;
            let name = icon_file_name(&e);
            icons_dir().join(&name).is_file().then(|| format!("{ICON_ASSETS}{name}"))
        })
        .clone()
}

/// The bytes of an icon asset (`acp-icons/<file>`), for the app's asset source.
pub fn load_icon(path: &str) -> Option<Vec<u8>> {
    let name = path.strip_prefix(ICON_ASSETS)?;
    if name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    std::fs::read(icons_dir().join(name)).ok()
}

/// Fetch the icons that are missing, in the background, once per run.
pub fn fetch_icons_soon() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(fetch_icons);
}

/// Download every registry icon not on disk yet (one curl process, in parallel), keep
/// the ones that are SVG, and drop copies of older versions. Blocking.
pub fn fetch_icons() {
    let reg = registry();
    let dir = icons_dir();
    let _ = std::fs::create_dir_all(&dir);
    let want: Vec<(String, String)> = reg
        .iter()
        .filter(|e| e.icon.starts_with("https://"))
        .map(|e| (icon_file_name(e), e.icon.clone()))
        .filter(|(name, _)| !dir.join(name).is_file())
        .collect();
    if !want.is_empty() {
        let mut c = curl();
        c.args(["-s", "-L", "--max-time", "30", "--max-filesize", "65536", "-H", "User-Agent: FluxCode", "--parallel", "--parallel-max", "8"]);
        for (name, url) in &want {
            c.arg("-o").arg(dir.join(format!("{name}.part"))).arg(url);
        }
        let _ = c.output();
        for (name, _) in &want {
            let part = dir.join(format!("{name}.part"));
            match std::fs::read(&part) {
                Ok(b) if is_svg(&b) => {
                    let _ = std::fs::rename(&part, dir.join(name));
                }
                _ => {
                    let _ = std::fs::remove_file(&part);
                }
            }
        }
    }
    // Icons of versions the registry no longer lists.
    let keep: std::collections::HashSet<String> = reg.iter().map(icon_file_name).collect();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for f in rd.flatten() {
            let n = f.file_name().to_string_lossy().into_owned();
            if n.ends_with(".svg") && !keep.contains(&n) && !reg.is_empty() {
                let _ = std::fs::remove_file(f.path());
            }
        }
    }
    forget();
}

fn is_svg(b: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&b[..b.len().min(512)]).to_ascii_lowercase();
    b.len() < 65536 && head.contains("<svg")
}

// ---- installing ----

/// What an install is doing, shared with the panel's progress animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Registry,
    Download,
    Verify,
    Unpack,
    Npm,
    Done,
    Failed,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Registry => "Reading the ACP registry",
            Phase::Download => "Downloading",
            Phase::Verify => "Checking the download",
            Phase::Unpack => "Unpacking",
            Phase::Npm => "Installing with npm",
            Phase::Done => "Installed",
            Phase::Failed => "Not installed",
        }
    }
}

/// One moment of an install: the phase, and how far it is when that can be measured
/// (bytes downloaded, files unpacked). `total` 0 means unknown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub done: u64,
    pub total: u64,
}

impl Snapshot {
    /// 0..1 when the total is known.
    pub fn fraction(&self) -> Option<f32> {
        (self.total > 0).then(|| (self.done as f64 / self.total as f64).clamp(0., 1.) as f32)
    }
}

/// The installer writes, the animation reads.
#[derive(Clone)]
pub struct Tracker(std::sync::Arc<Mutex<Snapshot>>);

impl Default for Tracker {
    fn default() -> Self {
        Tracker(std::sync::Arc::new(Mutex::new(Snapshot { phase: Phase::Registry, done: 0, total: 0 })))
    }
}

impl Tracker {
    pub fn get(&self) -> Snapshot {
        self.0.lock().map(|g| *g).unwrap_or(Snapshot { phase: Phase::Failed, done: 0, total: 0 })
    }
    pub fn phase(&self, phase: Phase) {
        if let Ok(mut g) = self.0.lock() {
            // Done and Failed are final: a late update from the installer can't undo them.
            if !matches!(g.phase, Phase::Done | Phase::Failed) {
                *g = Snapshot { phase, done: 0, total: 0 };
            }
        }
    }
    pub fn amount(&self, done: u64, total: u64) {
        if let Ok(mut g) = self.0.lock() {
            if !matches!(g.phase, Phase::Done | Phase::Failed) {
                g.done = done;
                g.total = total;
            }
        }
    }
}

/// Install an agent from the registry into `~/.fluxcode/agents/<id>` and return how
/// to start it. Blocking: downloads and unpacks, or runs npm, reporting to `t`. A
/// failed install leaves nothing behind.
pub fn install(entry: &Entry, t: &Tracker) -> Result<Launch, String> {
    let method = entry.method().ok_or_else(|| format!("{} has no download for this system ({}).", entry.name, platform()))?;
    let root = agents_dir().join(&entry.id);
    let dir = root.join(if entry.version.is_empty() { "current" } else { entry.version.as_str() });
    let had = managed(&entry.id);
    let res = install_into(&method, entry, &dir, t);
    if res.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        if !had {
            let _ = std::fs::remove_dir_all(&root);
        }
        return res;
    }
    let launch = res?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::write(launch_file(&entry.id), serde_json::to_string_pretty(&launch).unwrap_or_default()).map_err(|e| e.to_string())?;
    // Older versions of this agent are no longer started.
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            if e.path().is_dir() && e.path() != dir {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    forget();
    Ok(launch)
}

fn install_into(method: &Method, entry: &Entry, dir: &Path, t: &Tracker) -> Result<Launch, String> {
    Ok(match method.clone() {
        Method::Binary { url, cmd, args, env, sha256 } => {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let file_name = url.rsplit('/').next().unwrap_or("download").split('?').next().unwrap_or("download").to_string();
            let archive = dir.join(&file_name);
            t.phase(Phase::Download);
            download(&url, &archive, t)?;
            if let Some(want) = sha256 {
                t.phase(Phase::Verify);
                if sha256_of(&archive)? != want {
                    return Err("The download does not match the checksum the registry lists, so it was deleted.".into());
                }
            }
            let lower = file_name.to_ascii_lowercase();
            let is_archive = [".zip", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar"].iter().any(|e| lower.ends_with(e));
            if is_archive {
                t.phase(Phase::Unpack);
                unpack(&archive, dir, t)?;
                let _ = std::fs::remove_file(&archive);
            }
            let rel = cmd.trim_start_matches("./");
            let exe = rel_path(dir, rel);
            if !exe.is_file() {
                return Err(format!("{rel} was not in the download."));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
            }
            Launch { cmd: exe, args, env, version: entry.version.clone() }
        }
        Method::Npm { package, args, env } => {
            let npm = which("npm").ok_or("This agent is an npm package, and Node.js (npm) isn't installed. Get it from nodejs.org, then try again.")?;
            let node = which("node").ok_or("Node.js isn't on PATH.")?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            t.phase(Phase::Npm);
            let mut c = Launch { cmd: npm, ..Default::default() }.command();
            c.args(["install", "--no-audit", "--no-fund", "--omit=dev", "--loglevel=error", "--prefix"]).arg(dir).arg(&package);
            let out = c.output().map_err(|e| format!("npm could not start ({e})"))?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(format!("npm could not install {package}: {}", last_lines(&err, 3)));
            }
            let name = package_name(&package);
            let pkg_dir = dir.join("node_modules").join(name);
            let script = npm_bin(&pkg_dir, name).ok_or_else(|| format!("{name} has no program to start."))?;
            let mut all = vec![script.to_string_lossy().into_owned()];
            all.extend(args);
            Launch { cmd: node, args: all, env, version: entry.version.clone() }
        }
        Method::Uv { package, args, env } => {
            let uvx = which("uvx").ok_or("This agent is a Python package started with uv, and uv isn't installed. Get it from docs.astral.sh/uv, then try again.")?;
            let mut all = vec![package];
            all.extend(args);
            Launch { cmd: uvx, args: all, env, version: entry.version.clone() }
        }
    })
}

/// The size the server reports for `url` after redirects (0 when it doesn't say).
fn remote_size(url: &str) -> u64 {
    let Ok(out) = curl().args(["-s", "-I", "-L", "--max-time", "15", "-H", "User-Agent: FluxCode", url]).output() else { return 0 };
    content_length(&String::from_utf8_lossy(&out.stdout))
}

/// The last `Content-Length` in a chain of response heads (`curl -I -L` prints one per hop).
pub fn content_length(heads: &str) -> u64 {
    let mut last = 0;
    for line in heads.lines() {
        let l = line.trim().to_ascii_lowercase();
        if l.starts_with("http/") {
            last = 0;
        } else if let Some(v) = l.strip_prefix("content-length:") {
            last = v.trim().parse().unwrap_or(0);
        }
    }
    last
}

/// Download with curl, reporting the bytes on disk as they arrive.
fn download(url: &str, to: &Path, t: &Tracker) -> Result<(), String> {
    let total = remote_size(url);
    t.amount(0, total);
    let mut child = curl()
        .args(["-s", "-S", "-f", "-L", "--max-time", "1800", "-H", "User-Agent: FluxCode", "-o"])
        .arg(to)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl could not start ({e})"))?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
        let n = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
        t.amount(n, total.max(n));
    };
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut err);
    }
    if !status.success() {
        return Err(format!("Download failed. {}", curl_error(&err)));
    }
    let n = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    if total > 0 && n != total {
        return Err(format!("Download incomplete ({n} of {total} bytes)."));
    }
    t.amount(n, n);
    Ok(())
}

/// Remove an agent FluxCode installed (a PATH copy is the user's and stays).
pub fn uninstall(id: &str) -> Result<(), String> {
    let root = agents_dir().join(id);
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    }
    forget();
    Ok(())
}

/// Installed by FluxCode (so it can be removed from the picker).
pub fn managed(id: &str) -> bool {
    launch_file(id).is_file()
}

fn last_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join(" ")
}

/// "@scope/name@1.2.3" -> "@scope/name"; "name@1" -> "name".
pub fn package_name(spec: &str) -> &str {
    let at = spec.get(1..).and_then(|rest| rest.find('@')).map(|i| i + 1);
    match at {
        Some(i) => &spec[..i],
        None => spec,
    }
}

/// The script a package's `bin` points at: the one named like the package when there
/// are several.
fn npm_bin(pkg_dir: &Path, name: &str) -> Option<PathBuf> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(pkg_dir.join("package.json")).ok()?).ok()?;
    let rel = match v.get("bin")? {
        Value::String(s) => s.clone(),
        Value::Object(m) => {
            let short = name.rsplit('/').next().unwrap_or(name);
            m.get(short).or_else(|| m.values().next())?.as_str()?.to_string()
        }
        _ => return None,
    };
    let p = rel_path(pkg_dir, &rel);
    p.is_file().then_some(p)
}

/// `base` joined with a relative path written with `/` or `\`, in this system's separators.
fn rel_path(base: &Path, rel: &str) -> PathBuf {
    rel.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".").fold(base.to_path_buf(), |acc, p| acc.join(p))
}

/// How many entries a ZIP holds, from its end record (read from the file's tail only).
pub fn zip_count(archive: &Path) -> u64 {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(archive) else { return 0 };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let tail = len.min(66 * 1024);
    let mut buf = vec![0u8; tail as usize];
    if f.seek(SeekFrom::Start(len - tail)).is_err() || f.read_exact(&mut buf).is_err() {
        return 0;
    }
    zip_count_in(&buf)
}

/// The entry count in the last end-of-central-directory record of `tail` (0 if none,
/// or a ZIP64 archive, whose count lives elsewhere).
pub fn zip_count_in(tail: &[u8]) -> u64 {
    (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .map(|i| u16::from_le_bytes([tail[i + 10], tail[i + 11]]))
        .filter(|&n| n != 0xffff)
        .map_or(0, u64::from)
}

/// Unpack with the system tar (Windows 10+ ships bsdtar, which reads ZIP too). With
/// `-v`, tar names each entry as it writes it, which is the progress: entries
/// written out of the ZIP's count (unknown for tarballs).
fn unpack(archive: &Path, into: &Path, t: &Tracker) -> Result<(), String> {
    use std::io::BufRead;
    let tar = if cfg!(windows) {
        let sys = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        sys.join("System32").join("tar.exe").to_string_lossy().into_owned()
    } else {
        "tar".to_string()
    };
    let total = if archive.to_string_lossy().to_ascii_lowercase().ends_with(".zip") { zip_count(archive) } else { 0 };
    t.amount(0, total);
    let mut child = crate::proc::command(&tar)
        .arg("-xvf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("tar could not start ({e})"))?;
    // bsdtar lists to stderr, GNU tar to stdout: read stdout on a thread, stderr here.
    let out = child.stdout.take();
    let t2 = t.clone();
    let counted = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let c2 = counted.clone();
    let reader = std::thread::spawn(move || {
        if let Some(o) = out {
            for _ in std::io::BufReader::new(o).lines().map_while(Result::ok) {
                let n = c2.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                t2.amount(n, total.max(n));
            }
        }
    });
    let mut errors: Vec<String> = Vec::new();
    if let Some(e) = child.stderr.take() {
        for line in std::io::BufReader::new(e).lines().map_while(Result::ok) {
            if line.starts_with("x ") {
                let n = counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                t.amount(n, total.max(n));
            } else if !line.trim().is_empty() {
                errors.push(line);
            }
        }
    }
    let _ = reader.join();
    let st = child.wait().map_err(|e| e.to_string())?;
    if !st.success() {
        return Err(format!("Unpacking failed: {}", last_lines(&errors.join("\n"), 2)));
    }
    let n = counted.load(std::sync::atomic::Ordering::Relaxed);
    t.amount(n, n.max(1));
    Ok(())
}

/// SHA-256 with the system's own tool (certutil on Windows, sha256sum elsewhere).
fn sha256_of(file: &Path) -> Result<String, String> {
    let out = if cfg!(windows) {
        crate::proc::command("certutil").arg("-hashfile").arg(file).arg("SHA256").output()
    } else if which("sha256sum").is_some() {
        crate::proc::command("sha256sum").arg(file).output()
    } else {
        crate::proc::command("shasum").args(["-a", "256"]).arg(file).output()
    }
    .map_err(|e| format!("could not check the download ({e})"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .map(|l| l.split_whitespace().collect::<String>().to_ascii_lowercase())
        .find_map(|l| {
            let h: String = l.chars().take(64).collect();
            (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then_some(h)
        })
        .ok_or_else(|| "could not read the checksum".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_drop_the_version() {
        assert_eq!(package_name("@google/gemini-cli@0.67.0"), "@google/gemini-cli");
        assert_eq!(package_name("cline@3.0.74"), "cline");
        assert_eq!(package_name("pi-acp"), "pi-acp");
        assert_eq!(package_name("@scope/x"), "@scope/x");
    }

    #[test]
    fn prefers_a_native_binary_over_npm() {
        let json = r#"{"agents":[{"id":"kilo","name":"Kilo","version":"7","distribution":{
            "npx":{"package":"@kilocode/cli@7","args":["acp"]},
            "binary":{"windows-x86_64":{"archive":"https://x/kilo.zip","cmd":"./kilo.exe","args":["acp"],"sha256":"AB"},
                      "linux-x86_64":{"archive":"https://x/kilo.tar.gz","cmd":"./kilo","args":["acp"]}}}},
            {"id":"gem","name":"Gemini","version":"1","distribution":{"npx":{"package":"@google/gemini-cli@1","args":["--acp"]}}}]}"#;
        let list = parse_registry(json);
        assert_eq!(list.len(), 2);
        match list[0].method() {
            Some(Method::Binary { args, sha256, .. }) => {
                assert_eq!(args, vec!["acp".to_string()]);
                if cfg!(windows) {
                    assert_eq!(sha256.as_deref(), Some("ab"));
                }
            }
            other => panic!("expected a binary, got {other:?}"),
        }
        assert!(matches!(list[1].method(), Some(Method::Npm { .. })));
    }

    #[test]
    fn curl_errors_read_as_sentences() {
        assert!(curl_error("curl: (7) Failed to connect to 127.0.0.1 port 9 after 2017 ms: Could not connect to server").starts_with("Could not connect"));
        assert!(curl_error("curl: (6) Could not resolve host: x").contains("name could not be found"));
        assert!(curl_error("curl: (22) The requested URL returned error: 404").contains("404"));
        assert!(curl_error("curl: (22) The requested URL returned error: 503").contains("503"));
        assert_eq!(curl_error(""), "The server did not send the file.");
        assert_eq!(curl_error("curl: (99) odd"), "(99) odd");
    }

    #[test]
    fn content_length_is_the_last_hop() {
        let heads = "HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: x\r\n\r\nHTTP/2 200\r\ncontent-length: 48211\r\n\r\n";
        assert_eq!(content_length(heads), 48211);
        assert_eq!(content_length("HTTP/2 200\r\n\r\n"), 0);
        assert_eq!(content_length("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nHTTP/1.1 200 OK\r\n\r\n"), 0);
    }

    #[test]
    fn zip_entry_count_from_the_end_record() {
        let mut tail = vec![0u8; 40];
        tail.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 7, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(zip_count_in(&tail), 7);
        assert_eq!(zip_count_in(&[0u8; 30]), 0);
    }

    #[test]
    fn a_finished_install_stays_finished() {
        let t = Tracker::default();
        t.phase(Phase::Download);
        t.amount(50, 100);
        assert_eq!(t.get().fraction(), Some(0.5));
        t.phase(Phase::Failed);
        t.amount(100, 100);
        t.phase(Phase::Unpack);
        assert_eq!(t.get().phase, Phase::Failed);
        assert_eq!(Snapshot { phase: Phase::Download, done: 5, total: 0 }.fraction(), None);
    }

    #[test]
    fn every_known_agent_has_a_unique_id() {
        let mut ids: Vec<&str> = KNOWN.iter().map(|k| k.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), KNOWN.len());
    }
}

#[cfg(test)]
mod install_errors {
    //! Every way an install can go wrong, against local `file://` archives (curl and tar
    //! are the same programs a real install uses; no network).
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fluxcode-acp-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn url(p: &Path) -> String {
        format!("file:///{}", p.to_string_lossy().replace('\\', "/").trim_start_matches('/'))
    }

    /// A tarball holding `bin/agent`.
    fn archive(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("bin")).unwrap();
        std::fs::write(src.join("bin").join("agent"), b"#!/bin/sh\n").unwrap();
        let out = dir.join("agent.tar.gz");
        let tar = if cfg!(windows) { std::env::var("SystemRoot").map(|r| format!("{r}\\System32\\tar.exe")).unwrap() } else { "tar".into() };
        let st = std::process::Command::new(tar).arg("-czf").arg(&out).arg("-C").arg(&src).arg("bin").status().unwrap();
        assert!(st.success());
        out
    }

    fn entry() -> Entry {
        Entry { id: "t".into(), name: "T".into(), description: String::new(), version: "1".into(), icon: String::new(), dist: Value::Null }
    }

    fn binary(url: String, cmd: &str, sha256: Option<String>) -> Method {
        Method::Binary { url, cmd: cmd.into(), args: vec!["acp".into()], env: HashMap::new(), sha256 }
    }

    #[test]
    fn a_good_archive_installs_and_reports_each_step() {
        let d = scratch("ok");
        let a = archive(&d);
        let sum = sha256_of(&a).unwrap();
        let t = Tracker::default();
        let l = install_into(&binary(url(&a), "./bin/agent", Some(sum)), &entry(), &d.join("install"), &t).unwrap();
        assert!(l.cmd.is_file(), "{:?}", l.cmd);
        let snap = t.get();
        assert_eq!(snap.phase, Phase::Unpack);
        assert!(snap.done >= 1 && snap.fraction() == Some(1.0), "{snap:?}");
    }

    #[test]
    fn a_wrong_checksum_is_refused() {
        let d = scratch("sum");
        let a = archive(&d);
        let e = install_into(&binary(url(&a), "./bin/agent", Some("0".repeat(64))), &entry(), &d.join("install"), &Tracker::default()).unwrap_err();
        assert!(e.contains("checksum"), "{e}");
    }

    #[test]
    fn a_missing_download_says_so() {
        let d = scratch("missing");
        let e = install_into(&binary(url(&d.join("nope.tar.gz")), "./bin/agent", None), &entry(), &d.join("install"), &Tracker::default()).unwrap_err();
        assert!(e.starts_with("Download failed. The file is not there"), "{e}");
    }

    #[test]
    fn a_program_missing_from_the_archive_says_so() {
        let d = scratch("nocmd");
        let a = archive(&d);
        let e = install_into(&binary(url(&a), "./bin/other", None), &entry(), &d.join("install"), &Tracker::default()).unwrap_err();
        assert!(e.contains("was not in the download"), "{e}");
    }

    #[test]
    fn a_broken_archive_fails_to_unpack() {
        let d = scratch("broken");
        let a = d.join("agent.tar.gz");
        std::fs::write(&a, b"this is not a tarball at all, just text").unwrap();
        let e = install_into(&binary(url(&a), "./bin/agent", None), &entry(), &d.join("install"), &Tracker::default()).unwrap_err();
        assert!(e.starts_with("Unpacking failed"), "{e}");
    }
}

#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn install_from_the_registry() {
        super::refresh_registry().unwrap();
        for id in std::env::var("ACP_IDS").unwrap().split(',') {
            let e = super::registry().iter().find(|e| e.id == id).cloned().unwrap();
            let l = super::install(&e, &super::Tracker::default()).unwrap();
            eprintln!("{id}: {l:?}");
            assert_eq!(super::locate(id), Some(l.clone()));
            // ACP initialize over stdio: the agent must answer.
            use std::io::{BufRead, Write};
            let mut c = l.command();
            let mut ch = c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().unwrap();
            let mut inp = ch.stdin.take().unwrap();
            writeln!(inp, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":1,"clientCapabilities":{{}}}}}}"#).unwrap();
            let mut line = String::new();
            std::io::BufReader::new(ch.stdout.take().unwrap()).read_line(&mut line).unwrap();
            eprintln!("{id} answered: {}", &line[..line.len().min(200)]);
            assert!(line.contains("\"result\""), "{line}");
            let _ = ch.kill();
        }
    }
}
