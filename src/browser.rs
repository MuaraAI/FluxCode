//! A browser for the agents: the one already on this PC (Chrome first, then Brave,
//! Edge or Chromium), driven through the Chrome DevTools Protocol by Google's
//! `chrome-devtools-mcp`. Every agent FluxCode starts gets it as an MCP server, so it can
//! open pages, click, type, read the console and network, take screenshots and
//! accessibility snapshots, and record performance traces.
//!
//! Kept light: no browser is downloaded (the installed one is used), the MCP server is
//! 15 MB of JavaScript installed once into `~/.fluxcode/tools/browser` and run with the
//! Node.js already used for npm agents, and it starts only with an agent. The browser
//! gets a profile of its own (`~/.fluxcode/browser-profile`): Chrome ignores remote
//! debugging on the default profile since version 136, and the agent never sees the
//! user's own cookies or passwords. Usage statistics to Google are turned off.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

pub const PACKAGE: &str = "chrome-devtools-mcp";
/// Pinned: an update is a deliberate change here.
pub const VERSION: &str = "1.12.0";

#[derive(Clone, Debug, PartialEq)]
pub struct Browser {
    pub id: &'static str,
    pub name: &'static str,
    pub exe: PathBuf,
}

/// Browsers found on this PC, in the order `auto` picks them.
pub fn browsers() -> &'static [Browser] {
    static FOUND: OnceLock<Vec<Browser>> = OnceLock::new();
    FOUND.get_or_init(find)
}

fn find() -> Vec<Browser> {
    let kinds: [(&str, &str); 4] = [("chrome", "Chrome"), ("brave", "Brave"), ("edge", "Edge"), ("chromium", "Chromium")];
    kinds.iter().filter_map(|&(id, name)| locate(id).map(|exe| Browser { id, name, exe })).collect()
}

#[cfg(windows)]
fn locate(id: &str) -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let roots = [env("ProgramFiles"), env("ProgramFiles(x86)"), env("LOCALAPPDATA")];
    let rel: &[&str] = match id {
        "chrome" => &["Google/Chrome/Application/chrome.exe"],
        "brave" => &["BraveSoftware/Brave-Browser/Application/brave.exe"],
        "edge" => &["Microsoft/Edge/Application/msedge.exe"],
        _ => &["Chromium/Application/chrome.exe"],
    };
    roots.iter().flatten().flat_map(|r| rel.iter().map(move |p| r.join(p))).find(|p| p.is_file())
}

#[cfg(target_os = "macos")]
fn locate(id: &str) -> Option<PathBuf> {
    let app = match id {
        "chrome" => "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "brave" => "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "edge" => "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        _ => "/Applications/Chromium.app/Contents/MacOS/Chromium",
    };
    Some(PathBuf::from(app)).filter(|p| p.is_file())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn locate(id: &str) -> Option<PathBuf> {
    let names: &[&str] = match id {
        "chrome" => &["google-chrome", "google-chrome-stable"],
        "brave" => &["brave-browser", "brave"],
        "edge" => &["microsoft-edge", "microsoft-edge-stable"],
        _ => &["chromium", "chromium-browser"],
    };
    names.iter().find_map(|n| crate::acp::which(n))
}

/// The browser a setting means: "off", "auto" (the first found), or a browser id.
pub fn chosen(setting: &str) -> Option<&'static Browser> {
    match setting {
        "off" => None,
        "" | "auto" => browsers().first(),
        id => browsers().iter().find(|b| b.id == id).or_else(|| browsers().first()),
    }
}

fn tools_dir() -> PathBuf {
    crate::acp::agents_dir().parent().map(|p| p.join("tools")).unwrap_or_else(std::env::temp_dir).join("browser")
}

/// The profile the agents' browser uses (not the user's own).
pub fn profile_dir() -> PathBuf {
    crate::acp::agents_dir().parent().map(|p| p.join("browser-profile")).unwrap_or_else(|| std::env::temp_dir().join("fluxcode-browser"))
}

fn package_dir() -> PathBuf {
    tools_dir().join("node_modules").join(PACKAGE)
}

/// The installed version of the MCP server, if any.
pub fn installed() -> Option<String> {
    let pkg = std::fs::read_to_string(package_dir().join("package.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&pkg).ok()?;
    let version = v.get("version")?.as_str()?.to_string();
    package_dir().join("build/src/bin/chrome-devtools-mcp.js").is_file().then_some(version)
}

/// Node.js, when it is new enough for the MCP server (20.19 or newer).
pub fn node() -> Result<PathBuf, String> {
    static NODE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    NODE.get_or_init(|| {
        let node = crate::acp::which("node").ok_or_else(|| crate::i18n::tr("The browser needs Node.js 20 or newer (nodejs.org).").to_string())?;
        let out = crate::proc::command(&node.to_string_lossy()).arg("--version").output().map_err(|e| e.to_string())?;
        let v = String::from_utf8_lossy(&out.stdout);
        let mut parts = v.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        if major > 20 || (major == 20 && minor >= 19) {
            Ok(node)
        } else {
            Err(crate::trf!("The browser needs Node.js 20.19 or newer; this PC has {v}."; v => v.trim()))
        }
    })
    .clone()
}

static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn installing() -> bool {
    INSTALLING.load(Ordering::Relaxed)
}

/// Install the pinned MCP server (blocking; run it off the UI thread).
pub fn install() -> Result<(), String> {
    if INSTALLING.swap(true, Ordering::Relaxed) {
        return Err(crate::i18n::tr("The browser tools are already being installed.").into());
    }
    let res = (|| {
        node()?;
        let npm = crate::acp::which("npm").ok_or_else(|| crate::i18n::tr("npm isn't on PATH (it comes with Node.js).").to_string())?;
        let dir = tools_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut c = crate::acp::Launch { cmd: npm, ..Default::default() }.command();
        c.args(["install", "--no-audit", "--no-fund", "--omit=dev", "--loglevel=error", "--prefix"]).arg(&dir).arg(format!("{PACKAGE}@{VERSION}"));
        let out = c.output().map_err(|e| format!("npm could not start ({e})"))?;
        if !out.status.success() {
            return Err(format!("npm: {}", String::from_utf8_lossy(&out.stderr).lines().rev().take(3).collect::<Vec<_>>().join(" ")));
        }
        installed().map(|_| ()).ok_or_else(|| crate::i18n::tr("The browser tools were installed but can't be found.").to_string())
    })();
    INSTALLING.store(false, Ordering::Relaxed);
    res
}

/// Where the agents' browser stands, for Settings.
pub enum State {
    Off,
    NoBrowser,
    NoNode(String),
    NotInstalled(&'static Browser),
    Installing(&'static Browser),
    Ready(&'static Browser, String),
}

pub fn state(setting: &str) -> State {
    if setting == "off" {
        return State::Off;
    }
    let Some(b) = chosen(setting) else { return State::NoBrowser };
    if let Err(e) = node() {
        return State::NoNode(e);
    }
    if installing() {
        return State::Installing(b);
    }
    match installed() {
        Some(v) => State::Ready(b, v),
        None => State::NotInstalled(b),
    }
}

/// The MCP server to give an agent's session (ACP `mcpServers` entry), when it is ready.
/// `workspace` is where it may read and write files (screenshots, traces).
pub fn mcp_server(setting: &str, headless: bool, workspace: Option<&Path>) -> Option<serde_json::Value> {
    let State::Ready(b, _) = state(setting) else { return None };
    let node = node().ok()?;
    let script = package_dir().join("build/src/bin/chrome-devtools-mcp.js");
    let mut args = vec![
        script.to_string_lossy().into_owned(),
        "--executablePath".into(),
        b.exe.to_string_lossy().into_owned(),
        "--userDataDir".into(),
        profile_dir().to_string_lossy().into_owned(),
        // FluxCode sends nothing about its user; neither does its browser tool.
        "--usageStatistics=false".into(),
        "--performanceCrux=false".into(),
        // One agent per session: pages need no id in every call.
        "--pageIdRouting=false".into(),
        // Screenshots small enough not to flood the agent's context.
        "--screenshotFormat=jpeg".into(),
        "--screenshotMaxWidth=1280".into(),
    ];
    if headless {
        args.push("--headless".into());
    }
    if let Some(w) = workspace {
        args.push("--workspace".into());
        args.push(w.to_string_lossy().into_owned());
        args.push("--workspace".into());
        args.push(std::env::temp_dir().to_string_lossy().into_owned());
    }
    Some(serde_json::json!({
        "name": "browser",
        "command": node,
        "args": args,
        "env": [{ "name": "CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS", "value": "1" }],
    }))
}

#[cfg(test)]
mod tests {
    use super::{chosen, mcp_server};

    #[test]
    fn off_gives_no_browser() {
        assert!(chosen("off").is_none());
        assert!(mcp_server("off", false, None).is_none());
    }
}
