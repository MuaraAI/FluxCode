use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const REGISTRY_URL: &str =
    "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
const REGISTRY_FILE: &str = "acp-registry.json";
const REGISTRY_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

pub struct Known {
    pub id: &'static str,
    pub name: &'static str,
    pub by: &'static str,
    pub path: &'static [(&'static str, &'static [&'static str])],
}

pub const KNOWN: &[Known] = &[
    Known {
        id: "opencode-v2",
        name: "OpenCode 2",
        by: "Anomaly · open source",
        path: &[],
    },
    Known {
        id: "claude-acp",
        name: "Claude Agent",
        by: "Anthropic",
        path: &[("claude-agent-acp", &[]), ("claude-code-acp", &[])],
    },
    Known {
        id: "codex-acp",
        name: "Codex",
        by: "OpenAI",
        path: &[("codex-acp", &[])],
    },
    Known {
        id: "gemini",
        name: "Gemini CLI",
        by: "Google",
        path: &[("gemini", &["--acp"])],
    },
    Known {
        id: "antigravity-acp",
        name: "Google Antigravity",
        by: "Google",
        path: &[
            ("agy_acp_server", &[]),
            ("agy-acp-server", &[]),
            ("agy", &["acp"]),
        ],
    },
    Known {
        id: "github-copilot-cli",
        name: "GitHub Copilot",
        by: "GitHub",
        path: &[("copilot", &["--acp"])],
    },
    Known {
        id: "cursor",
        name: "Cursor",
        by: "Anysphere",
        path: &[("cursor-agent", &["acp"])],
    },
    Known {
        id: "qwen-code",
        name: "Qwen Code",
        by: "Alibaba",
        path: &[("qwen", &["--acp"])],
    },
    Known {
        id: "kimi-code",
        name: "Kimi Code",
        by: "Moonshot AI",
        path: &[],
    },
    Known {
        id: "commandcode",
        name: "Command Code",
        by: "Command Code · open models",
        path: &[("commandcode", &["acp"]), ("cmdc", &["acp"])],
    },
    Known {
        id: "goose",
        name: "goose",
        by: "Block",
        path: &[("goose", &["acp"])],
    },
    Known {
        id: "mistral-vibe",
        name: "Mistral Vibe",
        by: "Mistral AI",
        path: &[("vibe-acp", &[])],
    },
    Known {
        id: "cline",
        name: "Cline",
        by: "Cline",
        path: &[("cline", &["--acp"])],
    },
    Known {
        id: "kilo",
        name: "Kilo",
        by: "Kilo Code",
        path: &[("kilo", &["acp"])],
    },
    Known {
        id: "auggie",
        name: "Auggie CLI",
        by: "Augment Code",
        path: &[("auggie", &["--acp"])],
    },
    Known {
        id: "factory-droid",
        name: "Factory Droid",
        by: "Factory",
        path: &[("droid", &["exec", "--output-format", "acp-daemon"])],
    },
    Known {
        id: "junie",
        name: "Junie",
        by: "JetBrains",
        path: &[("junie", &["--acp=true"])],
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Home(&'static str),
    Project(&'static str),
}

pub fn instruction_files(id: &str) -> &'static [Place] {
    use Place::*;
    match id {
        "opencode" | "opencode-v2" => &[Home(".config/opencode/AGENTS.md"), Project("AGENTS.md")],
        "claude-acp" => &[Home(".claude/CLAUDE.md"), Project("CLAUDE.md")],
        "codex-acp" => &[Home(".codex/AGENTS.md"), Project("AGENTS.md")],
        "gemini" => &[Home(".gemini/GEMINI.md"), Project("GEMINI.md")],
        "antigravity-acp" => &[
            Home(".gemini/GEMINI.md"),
            Project("AGENTS.md"),
            Project("GEMINI.md"),
        ],
        "qwen-code" => &[Home(".qwen/QWEN.md"), Project("QWEN.md")],
        "github-copilot-cli" => &[
            Project(".github/copilot-instructions.md"),
            Project("AGENTS.md"),
        ],
        "cursor" => &[Project("AGENTS.md")],
        "goose" => &[Home(".config/goose/.goosehints"), Project(".goosehints")],
        _ => &[Project("AGENTS.md")],
    }
}

pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

pub fn recommended(id: &str) -> bool {
    matches!(
        id,
        "opencode-v2" | "opencode" | "commandcode" | "claude-acp"
    )
}

pub struct FlagOpt {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub flag: &'static str,
}

const COPILOT_FLAGS: &[FlagOpt] = &[FlagOpt {
    id: "model",
    name: "Model",
    category: "model",
    flag: "--model",
}];

pub fn flag_opts(agent: &str) -> &'static [FlagOpt] {
    match agent {
        "github-copilot-cli" => COPILOT_FLAGS,
        _ => &[],
    }
}

pub const FLAG_DEFAULT: &str = "default";

pub fn flag_choices(agent: &str, opt: &str) -> Vec<(String, String, String)> {
    static COPILOT: Mutex<Option<Vec<String>>> = Mutex::new(None);
    static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut out = vec![(
        FLAG_DEFAULT.to_string(),
        "Default/Your Copilot setting".to_string(),
        String::new(),
    )];
    if agent != "github-copilot-cli" || opt != "model" {
        return out;
    }
    if !ASKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        if let Some(l) = locate(agent) {
            std::thread::spawn(move || {
                let mut c = crate::proc::command(&l.cmd.to_string_lossy());
                c.args(["help", "config"]);
                if let Ok(o) = c.output() {
                    let list = copilot_models(&String::from_utf8_lossy(&o.stdout));
                    if !list.is_empty() {
                        *COPILOT.lock().unwrap() = Some(list);
                    }
                }
            });
        }
    }
    if let Some(list) = COPILOT.lock().unwrap().as_ref() {
        out.extend(
            list.iter()
                .map(|m| (m.clone(), model_label(m), String::new())),
        );
    }
    out
}

pub fn copilot_models(help: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in help.lines() {
        let t = line.trim();
        if t.starts_with("`model`:") {
            inside = true;
            continue;
        }
        if inside {
            match t.strip_prefix("- \"").and_then(|r| r.strip_suffix('"')) {
                Some(m) => out.push(m.to_string()),
                None => break,
            }
        }
    }
    out
}

pub fn model_label(id: &str) -> String {
    let mut parts = id.split('-').peekable();
    let first = parts.peek().copied().unwrap_or("");
    let provider = match first {
        "claude" => "Anthropic",
        "gpt" | "o1" | "o3" | "o4" => "OpenAI",
        "gemini" => "Google",
        "grok" => "xAI",
        "mai" => "Microsoft",
        "kimi" => "Moonshot",
        "qwen" => "Alibaba",
        "deepseek" => "DeepSeek",
        _ => "Other",
    };
    let mut words: Vec<String> = Vec::new();
    while let Some(p) = parts.next() {
        let w = match p {
            "gpt" => match parts.next() {
                Some(v) => format!("GPT-{v}"),
                None => "GPT".into(),
            },
            "mai" => "MAI".into(),
            p if p.chars().next().is_some_and(|c| c.is_ascii_digit()) => p.to_string(),
            p => {
                let mut c = p.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            }
        };
        words.push(w);
    }
    format!("{provider}/{}", words.join(" "))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Launch {
    pub cmd: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub version: String,
}

impl Launch {
    pub fn command(&self) -> std::process::Command {
        let s = self.cmd.to_string_lossy();
        let mut c = if cfg!(windows) && (s.ends_with(".cmd") || s.ends_with(".bat")) {
            let mut c = crate::proc::command("cmd.exe");
            c.arg("/d").arg("/c").arg(self.cmd.as_os_str());
            c
        } else {
            crate::proc::command(&s)
        };
        c.args(&self.args);
        c.env("PATH", fresh_path());
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c
    }
}

pub fn agents_dir() -> PathBuf {
    fluxcode_home().join("agents")
}

pub fn runtime_dir() -> PathBuf {
    fluxcode_home().join("runtime")
}

fn fluxcode_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".fluxcode")
}

fn launch_file(id: &str) -> PathBuf {
    agents_dir().join(id).join("launch.json")
}

pub fn which(exe: &str) -> Option<PathBuf> {
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    let index = {
        let mut g = INDEX.lock().ok()?;
        if g.0 != gen || g.1.is_none() {
            *g = (gen, Some(std::sync::Arc::new(path_index())));
        }
        g.1.clone()?
    };
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ".com", ""]
    } else {
        &[""]
    };
    let key = |e: &str| {
        if cfg!(windows) {
            format!("{exe}{e}").to_ascii_lowercase()
        } else {
            format!("{exe}{e}")
        }
    };
    exts.iter()
        .find_map(|e| index.get(&key(e)).filter(|p| p.is_file()).cloned())
}

fn sanitize_path_dir(d: PathBuf) -> Option<PathBuf> {
    let s = d.to_string_lossy();
    // Registry Path entries occasionally retain embedded NULs, which make Command::env fail.
    let clean = if s.contains('\0') {
        s.replace('\0', "")
    } else {
        return (!s.trim().is_empty()).then_some(d);
    };
    let trimmed = clean.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

pub fn fresh_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    #[cfg(windows)]
    {
        use crate::shell::reg::{get, HKCU, HKLM};
        let user = get(HKCU, "Environment", Some("Path"));
        let machine = get(
            HKLM,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            Some("Path"),
        );
        for p in [user, machine].into_iter().flatten() {
            dirs.extend(std::env::split_paths(&p));
        }
    }
    dirs.extend(install_dirs());
    let mut seen = std::collections::HashSet::new();
    let dirs: Vec<PathBuf> = dirs
        .into_iter()
        .filter_map(sanitize_path_dir)
        .filter(|d| {
            seen.insert(if cfg!(windows) {
                d.to_string_lossy()
                    .trim_end_matches(['\\', '/'])
                    .to_ascii_lowercase()
            } else {
                d.to_string_lossy().into_owned()
            })
        })
        .collect();
    std::env::join_paths(dirs).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}

fn install_dirs() -> Vec<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let mut out = Vec::new();
    let rt = runtime_dir();
    if cfg!(windows) {
        out.push(rt.join("node").join("current"));
    } else {
        out.push(rt.join("node").join("current").join("bin"));
    }
    out.push(rt.join("uv").join("current"));
    if let Some(home) = var("USERPROFILE").or_else(|| var("HOME")) {
        for rel in [
            ".local/bin",
            ".opencode/bin",
            ".bun/bin",
            ".cargo/bin",
            ".volta/bin",
            "scoop/shims",
            ".deno/bin",
            ".npm-global/bin",
            ".gemini/antigravity/bin",
            ".antigravity/bin",
        ] {
            out.push(home.join(rel));
        }
    }
    if let Some(app) = var("APPDATA") {
        out.push(app.join("npm"));
        out.push(app.join("Antigravity").join("bin"));
    }
    if let Some(local) = var("LOCALAPPDATA") {
        for rel in [
            "pnpm",
            "Microsoft/WinGet/Links",
            "GitHubCopilotCLI",
            "Programs/opencode",
            "agy/bin",
            "Programs/antigravity",
            "Programs/antigravity/resources/bin",
            "cursor-agent",
            "Volta/bin",
        ] {
            out.push(local.join(rel));
        }
    }
    if let Some(pf) = var("ProgramFiles") {
        out.push(pf.join("nodejs"));
    }
    out
}

fn shim_target(cmd: &Path) -> Option<PathBuf> {
    let ext = cmd.extension()?.to_str()?.to_ascii_lowercase();
    if ext != "cmd" {
        return None;
    }
    let text = std::fs::read_to_string(cmd).ok()?;
    let dir = cmd.parent()?;
    text.lines().rev().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("\"%dp0%\\")?;
        let (rel, _) = rest.split_once('"')?;
        rel.to_ascii_lowercase()
            .ends_with(".exe")
            .then(|| dir.join(rel))
            .filter(|p| p.is_file())
    })
}

fn path_index() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let path = fresh_path();
    let system = std::env::var("SystemRoot")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        if cfg!(windows)
            && !system.is_empty()
            && dir
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with(&system)
        {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let name = if cfg!(windows) {
                name.to_ascii_lowercase()
            } else {
                name
            };
            map.entry(name).or_insert_with(|| e.path());
        }
    }
    map
}

static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

static INDEX: Mutex<(u64, Option<std::sync::Arc<HashMap<String, PathBuf>>>)> =
    Mutex::new((0, None));

pub fn rescan() {
    let index = std::sync::Arc::new(path_index());
    let next = GEN.load(std::sync::atomic::Ordering::Relaxed) + 1;
    if let Ok(mut g) = INDEX.lock() {
        *g = (next, Some(index));
    }
    GEN.store(next, std::sync::atomic::Ordering::Relaxed);
    let _ = installed();
}

pub fn locate(id: &str) -> Option<Launch> {
    static CACHE: Mutex<(u64, Option<HashMap<String, Option<Launch>>>)> = Mutex::new((0, None));
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    let mut g = CACHE.lock().ok()?;
    if g.0 != gen {
        *g = (gen, Some(HashMap::new()));
    }
    let map = g.1.get_or_insert_with(HashMap::new);
    map.entry(id.to_string())
        .or_insert_with(|| find(id))
        .clone()
}

pub fn installed() -> Vec<String> {
    static CACHE: Mutex<(u64, Vec<String>)> = Mutex::new((0, Vec::new()));
    let gen = GEN.load(std::sync::atomic::Ordering::Relaxed);
    if let Ok(g) = CACHE.lock() {
        if g.0 == gen {
            return g.1.clone();
        }
    }
    let mut ids: Vec<String> = KNOWN
        .iter()
        .filter(|k| locate(k.id).is_some())
        .map(|k| k.id.to_string())
        .collect();
    if let Ok(rd) = std::fs::read_dir(agents_dir()) {
        let mut extra: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter(|id| !hidden(id) && known(id).is_none() && locate(id).is_some())
            .collect();
        extra.sort();
        ids.extend(extra);
    }
    if let Ok(mut g) = CACHE.lock() {
        *g = (gen, ids.clone());
    }
    ids
}

pub fn forget() {
    GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn find(id: &str) -> Option<Launch> {
    if let Some(l) = std::fs::read_to_string(launch_file(id))
        .ok()
        .and_then(|s| serde_json::from_str::<Launch>(&s).ok())
    {
        let local = l.cmd.is_absolute();
        if !local || l.cmd.is_file() {
            return Some(l);
        }
    }
    let k = known(id)?;
    if id == "opencode" {
        return crate::agent::find_opencode().map(|cmd| Launch {
            cmd,
            args: vec!["acp".into()],
            ..Default::default()
        });
    }
    if id == "opencode-v2" {
        return opencode_v2().map(|(cmd, _)| Launch {
            cmd,
            args: vec!["acp".into()],
            ..Default::default()
        });
    }
    if id == "antigravity-acp" {
        return find_antigravity();
    }
    k.path.iter().find_map(|(exe, args)| {
        which(exe).map(|cmd| Launch {
            cmd: shim_target(&cmd).unwrap_or(cmd),
            args: args.iter().map(|a| a.to_string()).collect(),
            ..Default::default()
        })
    })
}

fn opencode_v2() -> Option<(PathBuf, String)> {
    let mut cands: Vec<PathBuf> = Vec::new();
    for name in ["opencode2", "opencode"] {
        if let Some(p) = which(name) {
            cands.push(shim_target(&p).unwrap_or(p));
        }
    }
    let exe = if cfg!(windows) {
        "opencode.exe"
    } else {
        "opencode"
    };
    cands.extend(install_dirs().into_iter().map(|d| d.join(exe)));
    let mut seen = std::collections::HashSet::new();
    cands
        .into_iter()
        .filter(|p| p.is_file() && seen.insert(p.clone()))
        .filter(|p| {
            !matches!(
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("cmd" | "ps1" | "bat")
            )
        })
        .find_map(|p| {
            let v = program_version(&p)?;
            (major(&v) >= 2).then_some((p, v))
        })
}

fn major(v: &str) -> u32 {
    v.trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|m| m.trim().parse().ok())
        .unwrap_or(0)
}

fn program_version(exe: &Path) -> Option<String> {
    for up in exe.ancestors().skip(1).take(3) {
        if let Ok(text) = std::fs::read_to_string(up.join("package.json")) {
            if let Some(v) = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|j| j["version"].as_str().map(String::from))
            {
                return Some(v);
            }
        }
    }
    static ASKED: Mutex<Option<HashMap<(PathBuf, u64), Option<String>>>> = Mutex::new(None);
    let key = (
        exe.to_path_buf(),
        std::fs::metadata(exe).map(|m| m.len()).unwrap_or(0),
    );
    if let Some(v) = ASKED
        .lock()
        .ok()?
        .get_or_insert_with(HashMap::new)
        .get(&key)
    {
        return v.clone();
    }
    let v = (|| {
        let mut c = crate::proc::command(&exe.to_string_lossy());
        c.arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut child = c.spawn().ok()?;
        let start = std::time::Instant::now();
        loop {
            match child.try_wait().ok()? {
                Some(_) => break,
                None if start.elapsed() > std::time::Duration::from_secs(4) => {
                    let _ = child.kill();
                    return None;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        let out = child.wait_with_output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        text.split_whitespace()
            .map(|w| w.trim_start_matches('v'))
            .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) && w.contains('.'))
            .map(String::from)
    })();
    ASKED
        .lock()
        .ok()?
        .get_or_insert_with(HashMap::new)
        .insert(key, v.clone());
    v
}

fn antigravity_server_args() -> Vec<String> {
    if cfg!(target_os = "linux") {
        vec!["--uid=".into()]
    } else {
        Vec::new()
    }
}

fn find_antigravity_in(roots: &[PathBuf]) -> Option<Launch> {
    let mut dirs: Vec<PathBuf> = Vec::with_capacity(roots.len() + 4);
    for r in roots {
        dirs.push(r.clone());
        if r.file_name().and_then(|n| n.to_str()) == Some("antigravity-acp") {
            if let Ok(rd) = std::fs::read_dir(r) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        dirs.push(p);
                    }
                }
            }
        }
    }
    let server_names: &[&str] = if cfg!(windows) {
        &["agy_acp_server.exe", "agy-acp-server.exe"]
    } else {
        &["agy_acp_server", "agy-acp-server", "agy_acp_server.par"]
    };
    for d in &dirs {
        for name in server_names {
            let p = d.join(name);
            if p.is_file() {
                return Some(Launch {
                    cmd: p,
                    args: antigravity_server_args(),
                    ..Default::default()
                });
            }
        }
    }
    let cli_names: &[&str] = if cfg!(windows) {
        &["agy.exe", "agy.cmd"]
    } else {
        &["agy"]
    };
    for d in &dirs {
        for name in cli_names {
            let p = d.join(name);
            if p.is_file() {
                return Some(Launch {
                    cmd: shim_target(&p).unwrap_or(p),
                    args: vec!["acp".into()],
                    ..Default::default()
                });
            }
        }
    }
    None
}

fn find_antigravity() -> Option<Launch> {
    let server_which: &[&str] = if cfg!(windows) {
        &["agy_acp_server", "agy-acp-server"]
    } else {
        &["agy_acp_server", "agy-acp-server", "agy_acp_server.par"]
    };
    for exe in server_which {
        if let Some(p) = which(exe) {
            return Some(Launch {
                cmd: shim_target(&p).unwrap_or(p),
                args: antigravity_server_args(),
                ..Default::default()
            });
        }
    }
    if let Some(p) = which("agy") {
        return Some(Launch {
            cmd: shim_target(&p).unwrap_or(p),
            args: vec!["acp".into()],
            ..Default::default()
        });
    }
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let mut roots = Vec::new();
    if let Some(local) = var("LOCALAPPDATA") {
        roots.push(local.join("agy").join("bin"));
        roots.push(local.join("Programs").join("antigravity"));
        roots.push(
            local
                .join("Programs")
                .join("antigravity")
                .join("resources")
                .join("bin"),
        );
        roots.push(
            local
                .join("Zed")
                .join("external_agents")
                .join("antigravity-acp"),
        );
    }
    if let Some(home) = var("USERPROFILE").or_else(|| var("HOME")) {
        roots.push(home.join(".gemini").join("bin"));
        roots.push(home.join(".gemini").join("antigravity").join("bin"));
        roots.push(home.join(".antigravity").join("bin"));
        roots.push(home.join(".local").join("bin"));
        roots.push(
            home.join(".local")
                .join("share")
                .join("zed")
                .join("external_agents")
                .join("antigravity-acp"),
        );
    }
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from(
            "/Applications/Antigravity.app/Contents/Resources/app/bin",
        ));
    }
    if cfg!(unix) {
        roots.push(PathBuf::from("/usr/local/bin"));
    }
    find_antigravity_in(&roots)
}

fn antigravity_desktop_in(
    home: Option<&Path>,
    local: Option<&Path>,
    appdata: Option<&Path>,
    pf: Option<&Path>,
) -> bool {
    if cfg!(windows) {
        let mut cands = Vec::new();
        if let Some(l) = local {
            cands.push(
                l.join("Programs")
                    .join("antigravity")
                    .join("Antigravity.exe"),
            );
            cands.push(
                l.join("Programs")
                    .join("antigravity")
                    .join("resources")
                    .join("bin")
                    .join("language_server.exe"),
            );
        }
        if let Some(p) = pf {
            cands.push(p.join("Antigravity").join("Antigravity.exe"));
        }
        if let Some(h) = home {
            cands.push(
                h.join(".gemini")
                    .join("antigravity")
                    .join("bin")
                    .join("agentapi.bat"),
            );
        }
        if let Some(a) = appdata {
            cands.push(a.join("Antigravity").join("bin").join("agy-node.cmd"));
        }
        cands.into_iter().any(|p| p.is_file())
    } else {
        let mut cands = vec![
            PathBuf::from("/Applications/Antigravity.app"),
            PathBuf::from("/opt/Antigravity/antigravity"),
            PathBuf::from("/usr/bin/antigravity"),
        ];
        if let Some(h) = home {
            cands.push(h.join("Applications").join("Antigravity.app"));
            cands.push(
                h.join(".gemini")
                    .join("antigravity")
                    .join("bin")
                    .join("agentapi"),
            );
        }
        cands.into_iter().any(|p| p.exists())
    }
}

fn antigravity_desktop_found() -> bool {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let home = var("USERPROFILE").or_else(|| var("HOME"));
    let local = var("LOCALAPPDATA");
    let appdata = var("APPDATA");
    let pf = var("ProgramFiles");
    antigravity_desktop_in(
        home.as_deref(),
        local.as_deref(),
        appdata.as_deref(),
        pf.as_deref(),
    ) || which("Antigravity").is_some()
        || which("antigravity").is_some()
}

pub fn cli_without_bridge(id: &str) -> Option<&'static str> {
    match id {
        "claude-acp" => which("claude").map(|_| "Claude Code"),
        "codex-acp" => which("codex").map(|_| "Codex CLI"),
        "antigravity-acp" => antigravity_desktop_found().then_some("Google Antigravity"),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub icon: String,
    dist: Value,
}

impl Entry {
    pub fn method(&self) -> Option<Method> {
        let plat = platform();
        if let Some(b) = self.dist.get("binary").and_then(|b| b.get(plat)) {
            return Some(Method::Binary {
                url: b.get("archive")?.as_str()?.to_string(),
                cmd: b.get("cmd")?.as_str()?.to_string(),
                args: strings(b.get("args")),
                env: env_map(b.get("env")),
                sha256: b
                    .get("sha256")
                    .and_then(Value::as_str)
                    .map(str::to_ascii_lowercase),
            });
        }
        if let Some(n) = self.dist.get("npx") {
            return Some(Method::Npm {
                package: n.get("package")?.as_str()?.to_string(),
                args: strings(n.get("args")),
                env: env_map(n.get("env")),
            });
        }
        if let Some(u) = self.dist.get("uvx") {
            return Some(Method::Uv {
                package: u.get("package")?.as_str()?.to_string(),
                args: strings(u.get("args")),
                env: env_map(u.get("env")),
            });
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Method {
    Binary {
        url: String,
        cmd: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        sha256: Option<String>,
    },
    Npm {
        package: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Uv {
        package: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn env_map(v: Option<&Value>) -> HashMap<String, String> {
    v.and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

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
    let Ok(v) = serde_json::from_str::<Value>(json.trim_start_matches('\u{feff}')) else {
        return Vec::new();
    };
    let s = |a: &Value, k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    v.get("agents")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let id = a.get("id")?.as_str()?.to_string();
                    Some(Entry {
                        name: s(a, "name"),
                        description: s(a, "description"),
                        version: s(a, "version"),
                        icon: s(a, "icon"),
                        dist: a.get("distribution")?.clone(),
                        id,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

static REGISTRY: Mutex<Option<std::sync::Arc<Vec<Entry>>>> = Mutex::new(None);

pub fn registry() -> std::sync::Arc<Vec<Entry>> {
    let Ok(mut g) = REGISTRY.lock() else {
        return Default::default();
    };
    g.get_or_insert_with(|| {
        let text = std::fs::read_to_string(crate::settings::config_dir().join(REGISTRY_FILE))
            .unwrap_or_default();
        std::sync::Arc::new(parse_registry(&text))
    })
    .clone()
}

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
        forget();
        RUNNING.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}

pub fn registry_stale() -> bool {
    std::fs::metadata(crate::settings::config_dir().join(REGISTRY_FILE))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age > REGISTRY_MAX_AGE)
}

pub fn curl_error(stderr: &str) -> String {
    let e = stderr.trim();
    let code = e
        .split_once("curl: (")
        .and_then(|(_, r)| r.split_once(')'))
        .and_then(|(n, _)| n.parse::<u32>().ok());
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

pub fn refresh_registry() -> Result<(), String> {
    let out = curl()
        .args([
            "-s",
            "-S",
            "-f",
            "-L",
            "--max-time",
            "20",
            "-H",
            "User-Agent: FluxCode",
            REGISTRY_URL,
        ])
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

pub fn is_opencode(id: &str) -> bool {
    id == "opencode" || id == "opencode-v2"
}

pub fn hidden(id: &str) -> bool {
    matches!(id, "opencode" | "mura")
}

pub fn display_name(id: &str) -> String {
    if let Some(k) = known(id) {
        return k.name.to_string();
    }
    registry()
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| id.to_string())
}

pub const ICON_ASSETS: &str = "acp-icons/";

const COMMANDCODE_ICON: &str = include_str!("../assets/acp-icons/commandcode.svg");

pub fn icons_dir() -> PathBuf {
    crate::settings::config_dir().join("acp-icons")
}

fn icon_file_name(e: &Entry) -> String {
    let clean = |s: &str| {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
    };
    format!("{}@{}.svg", clean(&e.id), clean(&e.version))
}

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
            if id == "commandcode" {
                return Some(format!("{ICON_ASSETS}commandcode.svg"));
            }
            let e = registry().iter().find(|e| e.id == id).cloned()?;
            let name = icon_file_name(&e);
            icons_dir()
                .join(&name)
                .is_file()
                .then(|| format!("{ICON_ASSETS}{name}"))
        })
        .clone()
}

pub fn load_icon(path: &str) -> Option<Vec<u8>> {
    let name = path.strip_prefix(ICON_ASSETS)?;
    if name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    if name == "commandcode.svg" {
        return Some(COMMANDCODE_ICON.as_bytes().to_vec());
    }
    std::fs::read(icons_dir().join(name)).ok()
}

pub fn fetch_icons_soon() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(fetch_icons);
}

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
        c.args([
            "-s",
            "-L",
            "--max-time",
            "30",
            "--max-filesize",
            "65536",
            "-H",
            "User-Agent: FluxCode",
            "--parallel",
            "--parallel-max",
            "8",
        ]);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    Node,
    Uv,
}

impl RuntimeKind {
    pub fn name(self) -> &'static str {
        match self {
            RuntimeKind::Node => "Node.js & npm",
            RuntimeKind::Uv => "uv (Python runtime)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            RuntimeKind::Node => "Node.js LTS",
            RuntimeKind::Uv => "uv",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Registry,
    EnvCheck,
    RuntimeDownload,
    RuntimeVerify,
    RuntimeUnpack,
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
            Phase::EnvCheck => "Checking environment",
            Phase::RuntimeDownload => "Installing runtime",
            Phase::RuntimeVerify => "Verifying runtime integrity",
            Phase::RuntimeUnpack => "Setting up runtime",
            Phase::Download => "Downloading",
            Phase::Verify => "Checking the download",
            Phase::Unpack => "Unpacking",
            Phase::Npm => "Installing with npm",
            Phase::Done => "Installed",
            Phase::Failed => "Not installed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub runtime: Option<RuntimeKind>,
    pub done: u64,
    pub total: u64,
}

impl Snapshot {
    pub fn fraction(&self) -> Option<f32> {
        (self.total > 0).then(|| (self.done as f64 / self.total as f64).clamp(0., 1.) as f32)
    }
}

#[derive(Clone)]
pub struct Tracker(std::sync::Arc<Mutex<Snapshot>>);

impl Default for Tracker {
    fn default() -> Self {
        Tracker(std::sync::Arc::new(Mutex::new(Snapshot {
            phase: Phase::Registry,
            runtime: None,
            done: 0,
            total: 0,
        })))
    }
}

impl Tracker {
    pub fn get(&self) -> Snapshot {
        self.0.lock().map(|g| *g).unwrap_or(Snapshot {
            phase: Phase::Failed,
            runtime: None,
            done: 0,
            total: 0,
        })
    }
    pub fn phase(&self, phase: Phase) {
        if let Ok(mut g) = self.0.lock() {
            if !matches!(g.phase, Phase::Done | Phase::Failed) {
                *g = Snapshot {
                    phase,
                    runtime: None,
                    done: 0,
                    total: 0,
                };
            }
        }
    }
    pub fn runtime(&self, phase: Phase, runtime: RuntimeKind) {
        if let Ok(mut g) = self.0.lock() {
            if !matches!(g.phase, Phase::Done | Phase::Failed) {
                *g = Snapshot {
                    phase,
                    runtime: Some(runtime),
                    done: 0,
                    total: 0,
                };
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

pub fn missing_runtime_for(id: &str) -> Option<RuntimeKind> {
    let entry = registry().iter().find(|e| e.id == id)?.clone();
    match entry.method()? {
        Method::Npm { .. } if which("node").is_none() || which("npm").is_none() => {
            Some(RuntimeKind::Node)
        }
        Method::Uv { .. } if which("uvx").is_none() => Some(RuntimeKind::Uv),
        _ => None,
    }
}

fn atomic_swap_dir(staging: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let prev = dest.with_extension("prev");
    if prev.exists() {
        let _ = std::fs::remove_dir_all(&prev);
    }
    let had_dest = dest.exists();
    if had_dest {
        std::fs::rename(dest, &prev)
            .map_err(|e| format!("Could not replace previous installation ({e})."))?;
    }
    if let Err(e) = std::fs::rename(staging, dest) {
        if had_dest && prev.exists() {
            let _ = std::fs::rename(&prev, dest);
        }
        return Err(format!("Could not finalize installation ({e})."));
    }
    if prev.exists() {
        let _ = std::fs::remove_dir_all(&prev);
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn runtime_ok(exe: &Path) -> bool {
    if !exe.is_file() {
        return false;
    }
    let mut c = Launch {
        cmd: exe.to_path_buf(),
        args: vec!["--version".into()],
        ..Default::default()
    }
    .command();
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let Ok(mut child) = c.spawn() else {
        return false;
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return st.success(),
            Ok(None) if start.elapsed() > std::time::Duration::from_secs(5) => {
                let _ = child.kill();
                return false;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(_) => return false,
        }
    }
}

fn fetch_text(url: &str) -> Result<String, String> {
    let out = curl()
        .args([
            "-s",
            "-S",
            "-f",
            "-L",
            "--max-time",
            "25",
            "-H",
            "User-Agent: FluxCode",
            url,
        ])
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        return Err(curl_error(&String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn parse_node_shasums(text: &str, plat: &str) -> Option<(String, String)> {
    let suffix = match plat {
        "windows-x86_64" => "-win-x64.zip",
        "windows-aarch64" => "-win-arm64.zip",
        "darwin-aarch64" => "-darwin-arm64.tar.gz",
        "darwin-x86_64" => "-darwin-x64.tar.gz",
        "linux-aarch64" => "-linux-arm64.tar.gz",
        _ => "-linux-x64.tar.gz",
    };
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let sum = parts.next()?.to_ascii_lowercase();
        let file = parts.next()?.trim_start_matches('*');
        let ok_sum = sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit());
        (ok_sum && file.starts_with("node-") && file.ends_with(suffix))
            .then(|| (file.to_string(), sum))
    })
}

pub fn parse_uv_sha256(text: &str) -> Option<String> {
    text.split_whitespace()
        .next()
        .map(str::to_ascii_lowercase)
        .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
}

fn uv_asset(plat: &str) -> &'static str {
    match plat {
        "windows-x86_64" => "uv-x86_64-pc-windows-msvc.zip",
        "windows-aarch64" => "uv-aarch64-pc-windows-msvc.zip",
        "darwin-aarch64" => "uv-aarch64-apple-darwin.tar.gz",
        "darwin-x86_64" => "uv-x86_64-apple-darwin.tar.gz",
        "linux-aarch64" => "uv-aarch64-unknown-linux-gnu.tar.gz",
        _ => "uv-x86_64-unknown-linux-gnu.tar.gz",
    }
}

fn node_bins_in(root: &Path) -> (PathBuf, PathBuf) {
    if cfg!(windows) {
        (root.join("node.exe"), root.join("npm.cmd"))
    } else {
        (root.join("bin").join("node"), root.join("bin").join("npm"))
    }
}

fn find_extracted_root(extract: &Path, probe: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if probe(extract) {
        return Some(extract.to_path_buf());
    }
    let rd = std::fs::read_dir(extract).ok()?;
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() && probe(&p) {
            return Some(p);
        }
    }
    None
}

fn ensure_node(t: &Tracker) -> Result<(PathBuf, PathBuf), String> {
    if let (Some(node), Some(npm)) = (which("node"), which("npm")) {
        if runtime_ok(&node) {
            return Ok((node, npm));
        }
    }
    let plat = platform();
    const BASE: &str = "https://nodejs.org/dist/latest-v22.x";
    t.runtime(Phase::RuntimeDownload, RuntimeKind::Node);
    let sums = fetch_text(&format!("{BASE}/SHASUMS256.txt")).map_err(|e| {
        format!("Node.js & npm are missing on this PC and could not be downloaded: {e}")
    })?;
    let (file, want_sha) = parse_node_shasums(&sums, plat)
        .ok_or_else(|| format!("Node.js LTS archive was not listed for this platform ({plat})."))?;
    let root = runtime_dir().join("node");
    let staging = root.join(format!(".staging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let res: Result<(PathBuf, PathBuf), String> = (|| {
        let archive = staging.join(&file);
        download(&format!("{BASE}/{file}"), &archive, t)
            .map_err(|e| format!("Could not download Node.js LTS: {e}"))?;
        t.runtime(Phase::RuntimeVerify, RuntimeKind::Node);
        if sha256_of(&archive)? != want_sha {
            return Err(
                "The Node.js LTS download failed SHA-256 verification and was discarded.".into(),
            );
        }
        let extract = staging.join("extract");
        std::fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
        t.runtime(Phase::RuntimeUnpack, RuntimeKind::Node);
        unpack(&archive, &extract, t)?;
        let _ = std::fs::remove_file(&archive);
        let unpacked = find_extracted_root(&extract, |dir| {
            let (n, m) = node_bins_in(dir);
            n.is_file() && m.is_file()
        })
        .ok_or_else(|| "Node.js archive unpacked without node and npm.".to_string())?;
        let (staged_node, _) = node_bins_in(&unpacked);
        if !runtime_ok(&staged_node) {
            return Err("Downloaded Node.js could not start cleanly, so it was removed.".into());
        }
        let dest = root.join("current");
        atomic_swap_dir(&unpacked, &dest)?;
        let (node, npm) = node_bins_in(&dest);
        Ok((node, npm))
    })();
    let _ = std::fs::remove_dir_all(&staging);
    let (node, npm) = res?;
    rescan();
    Ok((node, npm))
}

fn ensure_uv(t: &Tracker) -> Result<PathBuf, String> {
    if let Some(uvx) = which("uvx") {
        if runtime_ok(&uvx) {
            return Ok(uvx);
        }
    }
    let plat = platform();
    let asset = uv_asset(plat);
    const BASE: &str = "https://github.com/astral-sh/uv/releases/latest/download";
    t.runtime(Phase::RuntimeDownload, RuntimeKind::Uv);
    let sum_text = fetch_text(&format!("{BASE}/{asset}.sha256")).map_err(|e| {
        format!("uv (Python runtime) is missing on this PC and could not be downloaded: {e}")
    })?;
    let want_sha = parse_uv_sha256(&sum_text)
        .ok_or_else(|| "Could not read the official SHA-256 checksum for uv.".to_string())?;
    let root = runtime_dir().join("uv");
    let staging = root.join(format!(".staging-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let uvx_name = if cfg!(windows) { "uvx.exe" } else { "uvx" };
    let res: Result<PathBuf, String> = (|| {
        let archive = staging.join(asset);
        download(&format!("{BASE}/{asset}"), &archive, t)
            .map_err(|e| format!("Could not download uv: {e}"))?;
        t.runtime(Phase::RuntimeVerify, RuntimeKind::Uv);
        if sha256_of(&archive)? != want_sha {
            return Err("The uv download failed SHA-256 verification and was discarded.".into());
        }
        let extract = staging.join("extract");
        std::fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
        t.runtime(Phase::RuntimeUnpack, RuntimeKind::Uv);
        unpack(&archive, &extract, t)?;
        let _ = std::fs::remove_file(&archive);
        let unpacked = find_extracted_root(&extract, |dir| dir.join(uvx_name).is_file())
            .ok_or_else(|| "uv archive unpacked without uvx.".to_string())?;
        let staged_uvx = unpacked.join(uvx_name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&staged_uvx, std::fs::Permissions::from_mode(0o755));
            let uv_bin = unpacked.join("uv");
            if uv_bin.is_file() {
                let _ = std::fs::set_permissions(&uv_bin, std::fs::Permissions::from_mode(0o755));
            }
        }
        if !runtime_ok(&staged_uvx) {
            return Err("Downloaded uv could not start cleanly, so it was removed.".into());
        }
        let dest = root.join("current");
        atomic_swap_dir(&unpacked, &dest)?;
        Ok(dest.join(uvx_name))
    })();
    let _ = std::fs::remove_dir_all(&staging);
    let uvx = res?;
    rescan();
    Ok(uvx)
}

pub fn install(entry: &Entry, t: &Tracker) -> Result<Launch, String> {
    let method = entry.method().ok_or_else(|| {
        format!(
            "{} has no download for this system ({}).",
            entry.name,
            platform()
        )
    })?;
    let root = agents_dir().join(&entry.id);
    let dir = root.join(if entry.version.is_empty() {
        "current"
    } else {
        entry.version.as_str()
    });
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
    let json = serde_json::to_string_pretty(&launch).unwrap_or_default();
    write_atomic(&launch_file(&entry.id), json.as_bytes())?;
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
        Method::Binary {
            url,
            cmd,
            args,
            env,
            sha256,
        } => {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let file_name = url
                .rsplit('/')
                .next()
                .unwrap_or("download")
                .split('?')
                .next()
                .unwrap_or("download")
                .to_string();
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
            let is_archive = [
                ".zip", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar",
            ]
            .iter()
            .any(|e| lower.ends_with(e));
            if is_archive {
                t.phase(Phase::Unpack);
                unpack(&archive, dir, t)?;
                let _ = std::fs::remove_file(&archive);
            }
            let rel = cmd.trim_start_matches("./");
            let mut exe = rel_path(dir, rel);
            if !exe.is_file() {
                if let Some(root) = find_extracted_root(dir, |d| rel_path(d, rel).is_file()) {
                    exe = rel_path(&root, rel);
                }
            }
            if !exe.is_file() {
                return Err(format!("{rel} was not in the download."));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
            }
            Launch {
                cmd: exe,
                args,
                env,
                version: entry.version.clone(),
            }
        }
        Method::Npm { package, args, env } => {
            t.phase(Phase::EnvCheck);
            let (node, npm) = ensure_node(t)?;
            let staging = dir.with_extension(format!("staging-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&staging);
            std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
            t.phase(Phase::Npm);
            let mut c = Launch {
                cmd: npm,
                ..Default::default()
            }
            .command();
            c.args([
                "install",
                "--no-audit",
                "--no-fund",
                "--omit=dev",
                "--loglevel=error",
                "--prefix",
            ])
            .arg(&staging)
            .arg(&package);
            let out = match c.output() {
                Ok(out) => out,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&staging);
                    return Err(format!("npm could not start ({e})"));
                }
            };
            if !out.status.success() {
                let _ = std::fs::remove_dir_all(&staging);
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(format!(
                    "npm could not install {package}: {}",
                    last_lines(&err, 3)
                ));
            }
            let name = package_name(&package);
            if npm_bin(&staging.join("node_modules").join(name), name).is_none() {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(format!("{name} has no program to start."));
            }
            if let Err(e) = atomic_swap_dir(&staging, dir) {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e);
            }
            let pkg_dir = dir.join("node_modules").join(name);
            let script = npm_bin(&pkg_dir, name)
                .ok_or_else(|| format!("{name} has no program to start."))?;
            let mut all = vec![script.to_string_lossy().into_owned()];
            all.extend(args);
            Launch {
                cmd: node,
                args: all,
                env,
                version: entry.version.clone(),
            }
        }
        Method::Uv { package, args, env } => {
            t.phase(Phase::EnvCheck);
            let uvx = ensure_uv(t)?;
            let mut all = vec![package];
            all.extend(args);
            Launch {
                cmd: uvx,
                args: all,
                env,
                version: entry.version.clone(),
            }
        }
    })
}

fn remote_size(url: &str) -> u64 {
    let Ok(out) = curl()
        .args([
            "-s",
            "-I",
            "-L",
            "--max-time",
            "15",
            "-H",
            "User-Agent: FluxCode",
            url,
        ])
        .output()
    else {
        return 0;
    };
    content_length(&String::from_utf8_lossy(&out.stdout))
}

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

fn download(url: &str, to: &Path, t: &Tracker) -> Result<(), String> {
    let total = remote_size(url);
    t.amount(0, total);
    let mut child = curl()
        .args([
            "-s",
            "-S",
            "-f",
            "-L",
            "--max-time",
            "1800",
            "-H",
            "User-Agent: FluxCode",
            "-o",
        ])
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

pub fn uninstall(id: &str) -> Result<(), String> {
    let root = agents_dir().join(id);
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    }
    forget();
    Ok(())
}

pub fn managed(id: &str) -> bool {
    launch_file(id).is_file()
}

fn last_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join(" ")
}

pub fn package_name(spec: &str) -> &str {
    let at = spec.get(1..).and_then(|rest| rest.find('@')).map(|i| i + 1);
    match at {
        Some(i) => &spec[..i],
        None => spec,
    }
}

fn npm_bin(pkg_dir: &Path, name: &str) -> Option<PathBuf> {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(pkg_dir.join("package.json")).ok()?).ok()?;
    let rel = match v.get("bin")? {
        Value::String(s) => s.clone(),
        Value::Object(m) => {
            let short = name.rsplit('/').next().unwrap_or(name);
            m.get(short)
                .or_else(|| m.values().next())?
                .as_str()?
                .to_string()
        }
        _ => return None,
    };
    let p = rel_path(pkg_dir, &rel);
    p.is_file().then_some(p)
}

fn rel_path(base: &Path, rel: &str) -> PathBuf {
    rel.split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .fold(base.to_path_buf(), |acc, p| acc.join(p))
}

pub fn zip_count(archive: &Path) -> u64 {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(archive) else {
        return 0;
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let tail = len.min(66 * 1024);
    let mut buf = vec![0u8; tail as usize];
    if f.seek(SeekFrom::Start(len - tail)).is_err() || f.read_exact(&mut buf).is_err() {
        return 0;
    }
    zip_count_in(&buf)
}

pub fn zip_count_in(tail: &[u8]) -> u64 {
    (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .map(|i| u16::from_le_bytes([tail[i + 10], tail[i + 11]]))
        .filter(|&n| n != 0xffff)
        .map_or(0, u64::from)
}

fn unpack(archive: &Path, into: &Path, t: &Tracker) -> Result<(), String> {
    use std::io::BufRead;
    let tar = if cfg!(windows) {
        let sys = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        sys.join("System32")
            .join("tar.exe")
            .to_string_lossy()
            .into_owned()
    } else {
        "tar".to_string()
    };
    let total = if archive
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(".zip")
    {
        zip_count(archive)
    } else {
        0
    };
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
        return Err(format!(
            "Unpacking failed: {}",
            last_lines(&errors.join("\n"), 2)
        ));
    }
    let n = counted.load(std::sync::atomic::Ordering::Relaxed);
    t.amount(n, n.max(1));
    Ok(())
}

fn sha256_of(file: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f =
        std::fs::File::open(file).map_err(|e| format!("could not check the download ({e})"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("could not check the download ({e})"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn npm_shims_lead_to_their_native_program() {
        let dir = std::env::temp_dir().join(format!("fluxcode-shim-{}", std::process::id()));
        let bin = dir
            .join("node_modules")
            .join("@opencode")
            .join("cli")
            .join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("opencode.exe"), b"").unwrap();
        std::fs::write(
            dir.join("node_modules")
                .join("@opencode")
                .join("cli")
                .join("package.json"),
            r#"{"version":"2.3.0"}"#,
        )
        .unwrap();
        let native = "@ECHO off\r\nGOTO start\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\"%dp0%\\node_modules\\@opencode\\cli\\bin\\opencode.exe\"   %*\r\n";
        std::fs::write(dir.join("opencode.cmd"), native).unwrap();
        let node = "@ECHO off\r\n\"%_prog%\"  \"%dp0%\\node_modules\\x\\cli.js\" %*\r\n";
        std::fs::write(dir.join("x.cmd"), node).unwrap();
        let exe = shim_target(&dir.join("opencode.cmd"));
        assert_eq!(exe.as_deref(), Some(bin.join("opencode.exe").as_path()));
        assert_eq!(shim_target(&dir.join("x.cmd")), None);
        assert_eq!(
            program_version(&bin.join("opencode.exe")).as_deref(),
            Some("2.3.0")
        );
        assert_eq!((major("2.3.0"), major("v1.19.5"), major("")), (2, 1, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commandcode_is_known_and_avoids_windows_cmd() {
        let k = known("commandcode").expect("Command Code is in the known list");
        assert!(k.path.iter().all(|(exe, _)| *exe != "cmd"));
        assert!(k
            .path
            .iter()
            .any(|(exe, args)| *exe == "commandcode" && *args == ["acp"]));
        assert!(k
            .path
            .iter()
            .any(|(exe, args)| *exe == "cmdc" && *args == ["acp"]));
    }
    #[test]
    fn commandcode_icon_is_bundled() {
        let path = icon_asset("commandcode").expect("the bundled Command Code icon");
        assert_eq!(path, format!("{ICON_ASSETS}commandcode.svg"));
        let bytes = load_icon(&path).expect("the bundled icon's bytes");
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
        assert!(head.contains("<svg"), "the icon is an SVG");
    }

    #[test]
    fn copilot_models_from_its_help() {
        let help = "  `mouse`: whatever\n\n  `model`: AI model to use for Copilot CLI; can be changed with /model.\n    - \"claude-sonnet-5.5\"\n    - \"gpt-6.1-sol\"\n\n  `contextTier`: tiers\n    - \"x\"\n";
        assert_eq!(copilot_models(help), ["claude-sonnet-5.5", "gpt-6.1-sol"]);
        assert_eq!(
            model_label("claude-sonnet-5.5"),
            "Anthropic/Claude Sonnet 5.5"
        );
        assert_eq!(model_label("gpt-6.1-sol"), "OpenAI/GPT-6.1 Sol");
        assert_eq!(
            model_label("mai-code-1.1-flash"),
            "Microsoft/MAI Code 1.1 Flash"
        );
        assert_eq!(model_label("kimi-k3"), "Moonshot/Kimi K3");
    }

    #[test]
    fn package_names_drop_the_version() {
        assert_eq!(
            package_name("@google/gemini-cli@0.67.0"),
            "@google/gemini-cli"
        );
        assert_eq!(package_name("cline@3.0.74"), "cline");
        assert_eq!(package_name("pi-acp"), "pi-acp");
        assert_eq!(package_name("@scope/x"), "@scope/x");
    }

    #[test]
    fn prefers_a_native_binary_over_npm() {
        let json = r#"{"agents":[{"id":"kilo","name":"Kilo","version":"7","distribution":{
            "npx":{"package":"@kilocode/cli@7","args":["acp"]},
            "binary":{"windows-x86_64":{"archive":"https://x/kilo.zip","cmd":"./kilo.exe","args":["acp"],"sha256":"AB"},
                      "linux-x86_64":{"archive":"https://x/kilo.tar.gz","cmd":"./kilo","args":["acp"]},
                      "darwin-aarch64":{"archive":"https://x/kilo-arm.tar.gz","cmd":"./kilo","args":["acp"]},
                      "darwin-x86_64":{"archive":"https://x/kilo-x64.tar.gz","cmd":"./kilo","args":["acp"]}}}},
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
        assert!(
            curl_error("curl: (6) Could not resolve host: x").contains("name could not be found")
        );
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
        assert_eq!(
            content_length("HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nHTTP/1.1 200 OK\r\n\r\n"),
            0
        );
    }

    #[test]
    fn zip_entry_count_from_the_end_record() {
        let mut tail = vec![0u8; 40];
        tail.extend_from_slice(&[
            0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 7, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
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
        assert_eq!(
            Snapshot {
                phase: Phase::Download,
                runtime: None,
                done: 5,
                total: 0
            }
            .fraction(),
            None
        );
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
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("fluxcode-acp-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn url(p: &Path) -> String {
        crate::docs::file_uri(p)
    }

    fn archive(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("bin")).unwrap();
        std::fs::write(src.join("bin").join("agent"), b"#!/bin/sh\n").unwrap();
        let out = dir.join("agent.tar.gz");
        let tar = if cfg!(windows) {
            std::env::var("SystemRoot")
                .map(|r| format!("{r}\\System32\\tar.exe"))
                .unwrap()
        } else {
            "tar".into()
        };
        let st = std::process::Command::new(tar)
            .arg("-czf")
            .arg(&out)
            .arg("-C")
            .arg(&src)
            .arg("bin")
            .status()
            .unwrap();
        assert!(st.success());
        out
    }

    fn entry() -> Entry {
        Entry {
            id: "t".into(),
            name: "T".into(),
            description: String::new(),
            version: "1".into(),
            icon: String::new(),
            dist: Value::Null,
        }
    }

    fn binary(url: String, cmd: &str, sha256: Option<String>) -> Method {
        Method::Binary {
            url,
            cmd: cmd.into(),
            args: vec!["acp".into()],
            env: HashMap::new(),
            sha256,
        }
    }

    #[test]
    fn a_good_archive_installs_and_reports_each_step() {
        let d = scratch("ok");
        let a = archive(&d);
        let sum = sha256_of(&a).unwrap();
        let t = Tracker::default();
        let l = install_into(
            &binary(url(&a), "./bin/agent", Some(sum)),
            &entry(),
            &d.join("install"),
            &t,
        )
        .unwrap();
        assert!(l.cmd.is_file(), "{:?}", l.cmd);
        let snap = t.get();
        assert_eq!(snap.phase, Phase::Unpack);
        assert!(snap.done >= 1 && snap.fraction() == Some(1.0), "{snap:?}");
    }

    #[test]
    fn a_wrong_checksum_is_refused() {
        let d = scratch("sum");
        let a = archive(&d);
        let e = install_into(
            &binary(url(&a), "./bin/agent", Some("0".repeat(64))),
            &entry(),
            &d.join("install"),
            &Tracker::default(),
        )
        .unwrap_err();
        assert!(e.contains("checksum"), "{e}");
    }

    #[test]
    fn a_missing_download_says_so() {
        let d = scratch("missing");
        let e = install_into(
            &binary(url(&d.join("nope.tar.gz")), "./bin/agent", None),
            &entry(),
            &d.join("install"),
            &Tracker::default(),
        )
        .unwrap_err();
        assert!(
            e.starts_with("Download failed. The file is not there"),
            "{e}"
        );
    }

    #[test]
    fn a_program_missing_from_the_archive_says_so() {
        let d = scratch("nocmd");
        let a = archive(&d);
        let e = install_into(
            &binary(url(&a), "./bin/other", None),
            &entry(),
            &d.join("install"),
            &Tracker::default(),
        )
        .unwrap_err();
        assert!(e.contains("was not in the download"), "{e}");
    }

    #[test]
    fn a_broken_archive_fails_to_unpack() {
        let d = scratch("broken");
        let a = d.join("agent.tar.gz");
        std::fs::write(&a, b"this is not a tarball at all, just text").unwrap();
        let e = install_into(
            &binary(url(&a), "./bin/agent", None),
            &entry(),
            &d.join("install"),
            &Tracker::default(),
        )
        .unwrap_err();
        assert!(e.starts_with("Unpacking failed"), "{e}");
    }

    #[test]
    fn fresh_path_sanitizes_embedded_nul_bytes() {
        assert_eq!(
            sanitize_path_dir(PathBuf::from("C:\\Tools\0")),
            Some(PathBuf::from("C:\\Tools"))
        );
        assert_eq!(sanitize_path_dir(PathBuf::from("\0\0")), None);
        assert!(!fresh_path().to_string_lossy().contains('\0'));
    }

    #[test]
    fn parses_node_shasums_and_uv_checksums() {
        let sha_a = "a".repeat(64);
        let sha_b = "B".repeat(64);
        let txt = format!(
            "{sha_a}  node-v22.14.0-win-x64.zip\n{sha_b}  node-v22.14.0-linux-x64.tar.gz\n"
        );
        assert_eq!(
            parse_node_shasums(&txt, "windows-x86_64"),
            Some(("node-v22.14.0-win-x64.zip".into(), sha_a.clone()))
        );
        assert_eq!(
            parse_node_shasums(&txt, "linux-x86_64"),
            Some(("node-v22.14.0-linux-x64.tar.gz".into(), "b".repeat(64)))
        );
        assert_eq!(
            parse_uv_sha256(&format!("{sha_b}  uv-x86_64.zip\n")),
            Some("b".repeat(64))
        );
        assert_eq!(parse_uv_sha256("short"), None);
    }

    #[test]
    fn atomic_swap_dir_replaces_without_leaving_partial_state() {
        let d = scratch("swap");
        let old = d.join("current");
        let stage = d.join("staging");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&stage).unwrap();
        std::fs::write(old.join("ver.txt"), b"v1").unwrap();
        std::fs::write(stage.join("ver.txt"), b"v2").unwrap();
        atomic_swap_dir(&stage, &old).unwrap();
        assert_eq!(std::fs::read_to_string(old.join("ver.txt")).unwrap(), "v2");
        assert!(!stage.exists());
        assert!(!old.with_extension("prev").exists());
    }
    #[test]
    fn antigravity_is_known_and_detects_cli_acp_and_desktop_installs() {
        let k = known("antigravity-acp").expect("Google Antigravity is in the known list");
        assert_eq!(k.name, "Google Antigravity");
        assert!(instruction_files("antigravity-acp").contains(&Place::Home(".gemini/GEMINI.md")));

        let root = std::env::temp_dir().join(format!("fluxcode-agy-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let agy_dir = root.join("agy").join("bin");
        let zed_dir = root
            .join("Zed")
            .join("external_agents")
            .join("antigravity-acp")
            .join("1.5.2");
        let app_dir = root.join("Programs").join("antigravity");
        let gemini_bin = root.join(".gemini").join("antigravity").join("bin");
        std::fs::create_dir_all(&agy_dir).unwrap();
        std::fs::create_dir_all(&zed_dir).unwrap();
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::create_dir_all(&gemini_bin).unwrap();

        let cli_name = if cfg!(windows) { "agy.exe" } else { "agy" };
        std::fs::write(agy_dir.join(cli_name), b"").unwrap();
        let l_cli = find_antigravity_in(std::slice::from_ref(&agy_dir)).expect("finds agy CLI");
        assert_eq!(l_cli.cmd, agy_dir.join(cli_name));
        assert_eq!(l_cli.args, vec!["acp".to_string()]);

        let home_gemini_bin = root.join(".gemini").join("bin");
        std::fs::create_dir_all(&home_gemini_bin).unwrap();
        std::fs::write(home_gemini_bin.join(cli_name), b"").unwrap();
        let l_home = find_antigravity_in(std::slice::from_ref(&home_gemini_bin))
            .expect("finds ~/.gemini/bin/agy");
        assert_eq!(l_home.cmd, home_gemini_bin.join(cli_name));
        assert_eq!(l_home.args, vec!["acp".to_string()]);

        let srv_name = if cfg!(windows) {
            "agy_acp_server.exe"
        } else {
            "agy_acp_server"
        };
        std::fs::write(zed_dir.join(srv_name), b"").unwrap();
        let zed_parent = zed_dir.parent().unwrap().to_path_buf();
        let l_srv = find_antigravity_in(&[agy_dir.clone(), zed_parent])
            .expect("prefers agy_acp_server over agy");
        assert_eq!(l_srv.cmd, zed_dir.join(srv_name));
        assert_eq!(l_srv.args, antigravity_server_args());

        if cfg!(windows) {
            assert!(!antigravity_desktop_in(
                Some(&root),
                Some(&root.join("missing")),
                None,
                None
            ));
            std::fs::write(app_dir.join("Antigravity.exe"), b"").unwrap();
            assert!(antigravity_desktop_in(None, Some(&root), None, None));
            std::fs::write(gemini_bin.join("agentapi.bat"), b"").unwrap();
            assert!(antigravity_desktop_in(Some(&root), None, None, None));
        } else {
            std::fs::write(gemini_bin.join("agentapi"), b"").unwrap();
            assert!(antigravity_desktop_in(Some(&root), None, None, None));
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn install_from_the_registry() {
        super::refresh_registry().unwrap();
        for id in std::env::var("ACP_IDS").unwrap().split(',') {
            let e = super::registry()
                .iter()
                .find(|e| e.id == id)
                .cloned()
                .unwrap();
            let l = super::install(&e, &super::Tracker::default()).unwrap();
            eprintln!("{id}: {l:?}");
            assert_eq!(super::locate(id), Some(l.clone()));
            use std::io::{BufRead, Write};
            let mut c = l.command();
            let mut ch = c
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let mut inp = ch.stdin.take().unwrap();
            writeln!(inp, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":1,"clientCapabilities":{{}}}}}}"#).unwrap();
            let mut line = String::new();
            std::io::BufReader::new(ch.stdout.take().unwrap())
                .read_line(&mut line)
                .unwrap();
            eprintln!("{id} answered: {}", &line[..line.len().min(200)]);
            assert!(line.contains("\"result\""), "{line}");
            let _ = ch.kill();
        }
    }
}
