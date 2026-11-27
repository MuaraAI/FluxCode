//! Pure-Rust Google Flow (`gflow`) multi-account router, Chrome launcher, video inspector,
//! credit tracker, and stdio MCP server without Python/Node runtime bloat.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const PROFILE_PREFIX: &str = "profile_";
const STATE_FILE: &str = "accounts_router.json";
const MAX_VIDEO_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB Flow edit cap
const VIDEO_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm", "3gp", "avi"];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    #[default]
    Sticky,
    AutoRotate,
    HighestCredits,
}

impl RoutingMode {
    pub const ALL: [RoutingMode; 3] = [
        RoutingMode::Sticky,
        RoutingMode::AutoRotate,
        RoutingMode::HighestCredits,
    ];

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "sticky" => Some(Self::Sticky),
            "auto_rotate" => Some(Self::AutoRotate),
            "highest_credits" => Some(Self::HighestCredits),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sticky => "sticky",
            Self::AutoRotate => "auto_rotate",
            Self::HighestCredits => "highest_credits",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sticky => "Sticky",
            Self::AutoRotate => "Auto Rotate",
            Self::HighestCredits => "Highest Credits",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Sticky => {
                "Keeps every generation on one chosen profile for project continuity and steady pacing."
            }
            Self::AutoRotate => {
                "Cycles ready profiles in order (akun1 -> akun2 -> ...) and skips accounts in cooldown."
            }
            Self::HighestCredits => {
                "Picks the ready profile with the most cached Veo credits from the latest check."
            }
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CooldownEntry {
    pub until: String,
    pub reason: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CreditCacheEntry {
    pub credits: i64,
    pub email: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RouterState {
    pub mode: RoutingMode,
    pub sticky_profile: Option<String>,
    pub order: Vec<String>,
    pub cursor: usize,
    pub cooldown_minutes: u32,
    pub cooldowns: HashMap<String, CooldownEntry>,
    pub auth_expired: HashMap<String, bool>,
    pub credits_cache: HashMap<String, CreditCacheEntry>,
    pub last_credits_check: Option<String>,
}

impl Default for RouterState {
    fn default() -> Self {
        Self {
            mode: RoutingMode::Sticky,
            sticky_profile: None,
            order: Vec::new(),
            cursor: 0,
            cooldown_minutes: 45,
            cooldowns: HashMap::new(),
            auth_expired: HashMap::new(),
            credits_cache: HashMap::new(),
            last_credits_check: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FlowProfile {
    pub name: String,
    pub email: Option<String>,
    pub strategy: String,
    pub chrome_strategy: bool,
    pub acl_hardened: bool,
    pub cookies_present: bool,
    pub is_default: bool,
    pub in_cooldown: bool,
    pub cooldown_until: Option<String>,
    pub cooldown_reason: Option<String>,
    pub auth_expired: bool,
    pub ready: bool,
    pub cached_credits: Option<i64>,
    pub dir: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct Selection {
    pub profile: String,
    pub email: Option<String>,
    pub strategy: String,
    pub mode: RoutingMode,
    pub cached_credits: Option<i64>,
    pub credits_stale: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreditItem {
    pub profile: String,
    pub email: Option<String>,
    pub authenticated: bool,
    pub credits: Option<i64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreditsReport {
    pub status: String,
    pub source: &'static str,
    pub total_credits: i64,
    pub profiles: Vec<CreditItem>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VideoInfo {
    pub path: PathBuf,
    pub extension: String,
    pub extension_supported: bool,
    pub size_bytes: u64,
    pub within_1gb_limit: bool,
    pub inside_output_dir: bool,
    pub duration_seconds: Option<f64>,
    pub dimensions: Option<(u32, u32)>,
}

pub fn gflow_home() -> PathBuf {
    if let Some(v) = std::env::var_os("GFLOW_CLI_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    if cfg!(windows) {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join("AppData").join("Local"));
        base.join("ffroliva").join("gflow-cli")
    } else if cfg!(target_os = "macos") {
        home_dir()
            .join("Library")
            .join("Application Support")
            .join("gflow-cli")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".local").join("share"))
            .join("gflow-cli")
    }
}

pub fn output_dir() -> PathBuf {
    if let Some(v) = std::env::var_os("GFLOW_CLI_OUTPUT_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    home_dir().join("Downloads").join("gflow-cli")
}

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn load_state_in(home: &Path) -> RouterState {
    let path = home.join(STATE_FILE);
    let Ok(raw) = fs::read_to_string(path) else {
        return RouterState::default();
    };
    let mut state: RouterState = serde_json::from_str(&raw).unwrap_or_default();
    state.cooldown_minutes = state.cooldown_minutes.clamp(5, 1440);
    state
}

pub fn load_state() -> RouterState {
    load_state_in(&gflow_home())
}

pub fn save_state_in(home: &Path, state: &RouterState) -> Result<(), String> {
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let target = home.join(STATE_FILE);
    let tmp = home.join(".accounts_router.json.tmp");
    let body = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&tmp, format!("{body}\n")).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &target).map_err(|e| e.to_string())
}

fn read_trimmed(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn read_default_profile_toml(home: &Path) -> Option<String> {
    let raw = fs::read_to_string(home.join("config.toml")).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("default_profile") else {
            continue;
        };
        let Some((_, rhs)) = rest.split_once('=') else {
            continue;
        };
        let val = rhs
            .split('#')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(['"', '\'']);
        if is_safe_name(val) {
            return Some(val.to_string());
        }
    }
    None
}

/// Reads account email from Chromium's plain-text `Default/Preferences` JSON without touching cookie stores.
fn read_chrome_preferences_email(profile_dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(profile_dir.join("Default").join("Preferences")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    v.get("account_info")?
        .as_array()?
        .iter()
        .find_map(|item| item.get("email").and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

pub fn discover_profiles_in(home: &Path, state: &RouterState) -> Vec<FlowProfile> {
    let Ok(rd) = fs::read_dir(home) else {
        return Vec::new();
    };
    let default_name = read_default_profile_toml(home);
    let now = Utc::now();
    let mut dirs: Vec<(String, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if !p.is_dir() {
                return None;
            }
            let fname = e.file_name().to_string_lossy().into_owned();
            let name = fname.strip_prefix(PROFILE_PREFIX)?;
            is_safe_name(name).then(|| (name.to_string(), p))
        })
        .collect();
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    dirs.into_iter()
        .map(|(name, dir)| {
            let email = read_trimmed(&dir.join(".gflow_account"))
                .or_else(|| read_chrome_preferences_email(&dir));
            let cookies_present = dir
                .join("Default")
                .join("Network")
                .join("Cookies")
                .is_file()
                || dir.join("Default").join("Cookies").is_file();
            let strategy = read_trimmed(&dir.join(".gflow_browser_strategy"))
                .unwrap_or_else(|| "internal".into());
            let acl_hardened = dir.join(".gflow_acl_v1").is_file();
            let cd = state.cooldowns.get(&name);
            let until_dt = cd.and_then(|c| DateTime::parse_from_rfc3339(&c.until).ok());
            let in_cooldown = until_dt.is_some_and(|dt| dt.with_timezone(&Utc) > now);
            let auth_expired = state.auth_expired.get(&name).copied().unwrap_or(false);
            let cached_credits = state.credits_cache.get(&name).map(|c| c.credits);

            FlowProfile {
                is_default: default_name.as_deref() == Some(name.as_str()),
                ready: cookies_present && !in_cooldown && !auth_expired,
                name,
                email,
                chrome_strategy: strategy == "chrome",
                strategy,
                acl_hardened,
                cookies_present,
                in_cooldown,
                cooldown_until: in_cooldown
                    .then(|| cd.map(|c| c.until.clone()).unwrap_or_default()),
                cooldown_reason: in_cooldown
                    .then(|| cd.map(|c| c.reason.clone()).unwrap_or_default()),
                auth_expired,
                cached_credits,
                dir,
            }
        })
        .collect()
}

pub fn discover_profiles() -> Vec<FlowProfile> {
    let home = gflow_home();
    let state = load_state_in(&home);
    discover_profiles_in(&home, &state)
}

pub fn next_profile_name(existing: &[FlowProfile]) -> String {
    let mut n = 1usize;
    loop {
        let candidate = format!("akun{n}");
        if !existing.iter().any(|p| p.name == candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Checks Playwright's strict Google Chrome paths first, then falls back to other Chromium browsers.
pub fn chrome_binary() -> Option<(&'static str, PathBuf)> {
    for b in crate::browser::browsers() {
        if b.id == "chrome" {
            return Some(("chrome", b.exe.clone()));
        }
    }
    crate::browser::browsers()
        .first()
        .map(|b| (b.id, b.exe.clone()))
}

#[cfg(windows)]
fn system32(bin: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(bin)
}

/// Restricts directory DACL to the current Windows user so profile cookies stay private.
pub fn harden_profile_acl(dir: &Path) -> bool {
    #[cfg(not(windows))]
    {
        let _ = dir;
        false
    }
    #[cfg(windows)]
    {
        let marker = dir.join(".gflow_acl_v1");
        if marker.is_file() {
            return true;
        }
        let Ok(who) = crate::proc::command(&system32("whoami.exe").to_string_lossy())
            .arg("/user")
            .output()
        else {
            return false;
        };
        let stdout = String::from_utf8_lossy(&who.stdout);
        let Some(sid) = stdout.split_whitespace().find(|tok| {
            tok.starts_with("S-1-")
                && tok
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
        }) else {
            return false;
        };
        let icacls = system32("icacls.exe");
        let grant = format!("*{sid}:(OI)(CI)F");
        let ok = crate::proc::command(&icacls.to_string_lossy())
            .arg(dir)
            .args(["/inheritance:r", "/grant:r", &grant, "/q"])
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            let _ = fs::write(marker, b"");
        }
        ok
    }
}

/// Registers a Flow profile directory + router slot for default-browser login without spawning an isolated browser.
pub fn register_profile_slot_in(home: &Path, profile_name: &str) -> Result<PathBuf, String> {
    if !is_safe_name(profile_name) {
        return Err("Profile name may only contain letters, digits, '-' and '_'.".into());
    }
    let dir = home.join(format!("{PROFILE_PREFIX}{profile_name}"));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    harden_profile_acl(&dir);
    let _ = fs::write(dir.join(".gflow_browser_strategy"), b"default-browser");
    let mut state = load_state_in(home);
    if !state.order.iter().any(|p| p == profile_name) {
        state.order.push(profile_name.to_string());
    }
    state.auth_expired.remove(profile_name);
    let _ = save_state_in(home, &state);
    Ok(dir)
}

pub fn register_profile_slot(profile_name: &str) -> Result<PathBuf, String> {
    register_profile_slot_in(&gflow_home(), profile_name)
}

/// Launches an isolated native browser window directly into Google Flow so the user can sign in once.
pub fn launch_browser_login(profile_name: &str) -> Result<String, String> {
    if !is_safe_name(profile_name) {
        return Err("Profile name may only contain letters, digits, '-' and '_'.".into());
    }
    let Some((kind, exe)) = chrome_binary() else {
        return Err("No Chrome, Edge, or Brave browser found on this PC.".into());
    };
    let home = gflow_home();
    let dir = home.join(format!("{PROFILE_PREFIX}{profile_name}"));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    harden_profile_acl(&dir);
    let _ = fs::write(dir.join(".gflow_browser_strategy"), kind.as_bytes());

    let mut cmd = std::process::Command::new(&exe);
    cmd.arg(format!("--user-data-dir={}", dir.display())).args([
        "--no-first-run",
        "--no-default-browser-check",
        "--window-size=1280,900",
        "--disable-features=CloudSyncedFolderChecker",
        "https://labs.google/fx/tools/flow?hl=en",
    ]);
    cmd.spawn()
        .map_err(|e| format!("Could not launch browser {}: {e}", exe.display()))?;

    let mut state = load_state_in(&home);
    if !state.order.iter().any(|p| p == profile_name) {
        state.order.push(profile_name.to_string());
    }
    state.auth_expired.remove(profile_name);
    let _ = save_state_in(&home, &state);
    Ok(kind.to_string())
}

pub fn remove_profile(profile_name: &str) -> Result<(), String> {
    if !is_safe_name(profile_name) {
        return Err("Invalid profile name.".into());
    }
    let home = gflow_home();
    let dir = home.join(format!("{PROFILE_PREFIX}{profile_name}"));
    if dir.parent() != Some(home.as_path()) {
        return Err("Refusing to remove directory outside Flow home.".into());
    }
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let mut state = load_state_in(&home);
    if state.sticky_profile.as_deref() == Some(profile_name) {
        state.sticky_profile = None;
    }
    state.order.retain(|p| p != profile_name);
    state.cooldowns.remove(profile_name);
    state.auth_expired.remove(profile_name);
    state.credits_cache.remove(profile_name);
    save_state_in(&home, &state)
}

pub fn set_mode_in(
    home: &Path,
    mode: RoutingMode,
    sticky_profile: Option<String>,
    order: Option<Vec<String>>,
    cooldown_minutes: Option<u32>,
) -> Result<RouterState, String> {
    let mut state = load_state_in(home);
    state.mode = mode;
    if let Some(sp) = sticky_profile {
        if sp.is_empty() {
            state.sticky_profile = None;
        } else if is_safe_name(&sp) {
            state.sticky_profile = Some(sp);
        } else {
            return Err("Invalid sticky profile name.".into());
        }
    }
    if let Some(ord) = order {
        state.order = ord.into_iter().filter(|p| is_safe_name(p)).collect();
        state.cursor = 0;
    }
    if let Some(mins) = cooldown_minutes {
        state.cooldown_minutes = mins.clamp(5, 1440);
    }
    save_state_in(home, &state)?;
    Ok(state)
}

pub fn set_mode(
    mode: RoutingMode,
    sticky_profile: Option<String>,
    order: Option<Vec<String>>,
    cooldown_minutes: Option<u32>,
) -> Result<RouterState, String> {
    set_mode_in(&gflow_home(), mode, sticky_profile, order, cooldown_minutes)
}

pub fn select_profile_in(
    home: &Path,
    task_type: &str,
    min_credits: Option<i64>,
    advance_cursor: bool,
) -> Result<Selection, String> {
    let mut state = load_state_in(home);
    let profiles = discover_profiles_in(home, &state);
    if profiles.is_empty() {
        return Err("No saved Flow profiles yet. Add a Flow account first.".into());
    }
    let ready: Vec<&FlowProfile> = profiles.iter().filter(|p| p.ready).collect();
    if ready.is_empty() {
        return Err(
            "No ready Flow profiles (all missing cookies, expired, or in cooldown).".into(),
        );
    }
    let mut warnings = Vec::new();
    let chrome_ready: Vec<&FlowProfile> = ready
        .iter()
        .copied()
        .filter(|p| p.chrome_strategy)
        .collect();
    let pool = if chrome_ready.is_empty() {
        warnings
            .push("No native Google Chrome profile ready; using fallback browser profile.".into());
        ready
    } else {
        chrome_ready
    };

    let selected = match state.mode {
        RoutingMode::Sticky => {
            let pick = state
                .sticky_profile
                .as_deref()
                .and_then(|name| pool.iter().copied().find(|p| p.name == name))
                .or_else(|| pool.iter().copied().find(|p| p.is_default))
                .unwrap_or(pool[0]);
            if let Some(target) = &state.sticky_profile {
                if &pick.name != target {
                    warnings.push(format!(
                        "Sticky profile '{target}' is unavailable; using '{}'.",
                        pick.name
                    ));
                }
            }
            pick
        }
        RoutingMode::AutoRotate => {
            let mut ordered: Vec<&FlowProfile> = Vec::with_capacity(pool.len());
            for name in &state.order {
                if let Some(p) = pool.iter().copied().find(|p| &p.name == name) {
                    if !ordered.iter().any(|o| o.name == p.name) {
                        ordered.push(p);
                    }
                }
            }
            for p in &pool {
                if !ordered.iter().any(|o| o.name == p.name) {
                    ordered.push(*p);
                }
            }
            let idx = state.cursor % ordered.len();
            let pick = ordered[idx];
            if advance_cursor {
                state.cursor = (idx + 1) % ordered.len();
                let _ = save_state_in(home, &state);
            }
            pick
        }
        RoutingMode::HighestCredits => {
            let eligible: Vec<&FlowProfile> = if let Some(min_c) = min_credits {
                let f: Vec<&FlowProfile> = pool
                    .iter()
                    .copied()
                    .filter(|p| p.cached_credits.is_some_and(|c| c >= min_c))
                    .collect();
                if f.is_empty() {
                    warnings.push(format!(
                        "No profile meets min_credits={min_c}; run credit check first."
                    ));
                    pool.clone()
                } else {
                    f
                }
            } else {
                pool.clone()
            };
            if eligible.iter().all(|p| p.cached_credits.is_none()) {
                warnings.push("Cached credits unknown; run a credit check first.".into());
            }
            eligible
                .into_iter()
                .max_by(|a, b| {
                    a.cached_credits
                        .unwrap_or(-1)
                        .cmp(&b.cached_credits.unwrap_or(-1))
                        .then_with(|| a.name.cmp(&b.name))
                })
                .unwrap_or(pool[0])
        }
    };

    let _ = task_type;
    Ok(Selection {
        profile: selected.name.clone(),
        email: selected.email.clone(),
        strategy: selected.strategy.clone(),
        mode: state.mode,
        cached_credits: selected.cached_credits,
        credits_stale: selected.cached_credits.is_none(),
        warnings,
    })
}

pub fn select_profile(
    task_type: &str,
    min_credits: Option<i64>,
    advance_cursor: bool,
) -> Result<Selection, String> {
    select_profile_in(&gflow_home(), task_type, min_credits, advance_cursor)
}

pub fn report_outcome_in(
    home: &Path,
    profile: &str,
    outcome: &str,
    credits_remaining: Option<i64>,
    note: Option<String>,
) -> Result<Value, String> {
    if !is_safe_name(profile) {
        return Err("Invalid profile name.".into());
    }
    let mut state = load_state_in(home);
    let now = Utc::now();
    match outcome {
        "waf_403" | "rate_limited" => {
            let until = (now + ChronoDuration::minutes(state.cooldown_minutes as i64)).to_rfc3339();
            state.cooldowns.insert(
                profile.to_string(),
                CooldownEntry {
                    until,
                    reason: outcome.to_string(),
                    note,
                },
            );
        }
        "auth_expired" => {
            state.auth_expired.insert(profile.to_string(), true);
        }
        "ok" | "clear_cooldown" => {
            state.cooldowns.remove(profile);
            state.auth_expired.remove(profile);
        }
        other => return Err(format!("Unknown outcome '{other}'.")),
    }
    if let Some(credits) = credits_remaining.filter(|&c| c >= 0) {
        state.credits_cache.insert(
            profile.to_string(),
            CreditCacheEntry {
                credits,
                email: None,
                updated_at: now.to_rfc3339(),
            },
        );
    }
    save_state_in(home, &state)?;
    Ok(json!({
        "status": "ok",
        "profile": profile,
        "outcome": outcome,
        "cooldown": state.cooldowns.get(profile),
        "auth_expired": state.auth_expired.get(profile).copied().unwrap_or(false),
    }))
}

pub fn gflow_cli_bin() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GFLOW_CLI_BIN")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    if let Some(p) = crate::acp::which("gflow") {
        return Some(p);
    }
    let local = home_dir()
        .join(".local")
        .join("bin")
        .join(if cfg!(windows) { "gflow.exe" } else { "gflow" });
    local.is_file().then_some(local)
}

/// Checks Flow credit balances across all profiles without spending credits.
/// Uses the optional `gflow` driver for live `flow.google.com` queries when installed,
/// otherwise reports cached balances from pure-Rust `accounts_router.json`.
pub fn check_credits(refresh: bool) -> Result<CreditsReport, String> {
    let home = gflow_home();
    let mut state = load_state_in(&home);
    let profiles = discover_profiles_in(&home, &state);
    if profiles.is_empty() {
        return Ok(CreditsReport {
            status: "ok".into(),
            source: "pure-rust",
            total_credits: 0,
            profiles: Vec::new(),
            note: Some("No saved Flow profiles yet. Add one first.".into()),
        });
    }

    let Some(bin) = gflow_cli_bin() else {
        let items: Vec<CreditItem> = profiles
            .iter()
            .map(|p| CreditItem {
                profile: p.name.clone(),
                email: p.email.clone(),
                authenticated: p.cookies_present && !p.auth_expired,
                credits: p.cached_credits,
                error: (!p.cookies_present).then(|| "Not signed in".into()),
            })
            .collect();
        let total = items.iter().filter_map(|i| i.credits).sum();
        return Ok(CreditsReport {
            status: "cached".into(),
            source: "pure-rust-cache",
            total_credits: total,
            profiles: items,
            note: Some(
                "Showing cached balances (pure Rust). Sign in via browser or install optional gflow CLI for live flow.google.com queries."
                    .into(),
            ),
        });
    };

    let mut cmd = crate::proc::command(&bin.to_string_lossy());
    cmd.args(["credits", "list", "--json"]);
    if refresh {
        cmd.arg("--refresh");
    }
    cmd.env("GFLOW_CLI_HOME", &home);
    let out = cmd
        .output()
        .map_err(|e| format!("Failed to run gflow: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("gflow credits list exited with {:?}", out.status.code())
        } else {
            err
        });
    }
    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("Invalid JSON from gflow credits list: {e}"))?;
    let now_iso = Utc::now().to_rfc3339();
    let mut items = Vec::new();
    for entry in parsed
        .get("profiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = entry.get("profile").and_then(Value::as_str) else {
            continue;
        };
        let auth = entry
            .get("authenticated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let credits = entry.get("credits").and_then(Value::as_i64);
        let email = entry.get("email").and_then(Value::as_str).map(String::from);
        if auth {
            if let Some(c) = credits {
                state.credits_cache.insert(
                    name.to_string(),
                    CreditCacheEntry {
                        credits: c,
                        email: email.clone(),
                        updated_at: now_iso.clone(),
                    },
                );
            }
            state.auth_expired.remove(name);
        } else if matches!(
            entry.get("error_type").and_then(Value::as_str),
            Some("AuthExpiredError" | "AuthMissingError")
        ) {
            state.auth_expired.insert(name.to_string(), true);
        }
        items.push(CreditItem {
            profile: name.to_string(),
            email,
            authenticated: auth,
            credits,
            error: entry.get("error").and_then(Value::as_str).map(String::from),
        });
    }
    state.last_credits_check = Some(now_iso);
    let _ = save_state_in(&home, &state);

    Ok(CreditsReport {
        status: parsed
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("ok")
            .into(),
        source: "gflow-cli",
        total_credits: parsed
            .get("total_credits")
            .and_then(Value::as_i64)
            .unwrap_or_else(|| items.iter().filter_map(|i| i.credits).sum()),
        profiles: items,
        note: None,
    })
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| VIDEO_EXTS.iter().any(|v| v.eq_ignore_ascii_case(ext)))
}

pub type Mp4Meta = (Option<f64>, Option<(u32, u32)>);

/// Pure-Rust ISO BMFF box parser (`moov` -> `mvhd` duration + `trak` -> `tkhd` dimensions).
pub fn parse_mp4_metadata(bytes: &[u8]) -> Option<Mp4Meta> {
    fn walk_boxes<'a>(buf: &'a [u8], target: &[u8; 4]) -> Option<&'a [u8]> {
        let mut off = 0usize;
        while off + 8 <= buf.len() {
            let size32 = u32::from_be_bytes(buf[off..off + 4].try_into().ok()?) as u64;
            let kind = &buf[off + 4..off + 8];
            let (hdr, total) = if size32 == 1 {
                if off + 16 > buf.len() {
                    break;
                }
                (
                    16usize,
                    u64::from_be_bytes(buf[off + 8..off + 16].try_into().ok()?),
                )
            } else if size32 == 0 {
                (8usize, (buf.len() - off) as u64)
            } else {
                (8usize, size32)
            };
            if total < hdr as u64 {
                break;
            }
            let end = off.saturating_add(total as usize).min(buf.len());
            let payload = &buf[off + hdr..end];
            if kind == target {
                return Some(payload);
            }
            if end == off {
                break;
            }
            off = end;
        }
        None
    }

    let moov = walk_boxes(bytes, b"moov")?;
    let mut duration_secs = None;
    if let Some(mvhd) = walk_boxes(moov, b"mvhd") {
        let ver = *mvhd.first()?;
        if ver == 0 && mvhd.len() >= 20 {
            let scale = u32::from_be_bytes(mvhd[12..16].try_into().ok()?);
            let dur = u32::from_be_bytes(mvhd[16..20].try_into().ok()?);
            if scale > 0 {
                duration_secs = Some(((dur as f64 / scale as f64) * 10.0).round() / 10.0);
            }
        } else if ver == 1 && mvhd.len() >= 32 {
            let scale = u32::from_be_bytes(mvhd[20..24].try_into().ok()?);
            let dur = u64::from_be_bytes(mvhd[24..32].try_into().ok()?);
            if scale > 0 {
                duration_secs = Some(((dur as f64 / scale as f64) * 10.0).round() / 10.0);
            }
        }
    }

    let mut dimensions = None;
    let mut off = 0usize;
    while off + 8 <= moov.len() {
        let sz = u32::from_be_bytes(moov[off..off + 4].try_into().ok()?) as usize;
        if sz < 8 {
            break;
        }
        let end = off.saturating_add(sz).min(moov.len());
        if &moov[off + 4..off + 8] == b"trak" {
            if let Some(tkhd) = walk_boxes(&moov[off + 8..end], b"tkhd") {
                if let Some(&ver) = tkhd.first() {
                    let (wo, ho) = if ver == 1 { (84, 88) } else { (76, 80) };
                    if tkhd.len() >= ho + 4 {
                        let w = u32::from_be_bytes(tkhd[wo..wo + 4].try_into().ok()?) >> 16;
                        let h = u32::from_be_bytes(tkhd[ho..ho + 4].try_into().ok()?) >> 16;
                        if w > 0 && h > 0 {
                            dimensions = Some((w, h));
                            break;
                        }
                    }
                }
            }
        }
        off = end;
    }

    Some((duration_secs, dimensions))
}

/// Scans top-level ISO BMFF atoms using seek so multi-hundred-MB MP4 files read only the `moov` box.
fn read_mp4_moov_bounded(path: &Path) -> Option<Mp4Meta> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len <= 2 * 1024 * 1024 {
        let mut buf = Vec::with_capacity(len as usize);
        file.read_to_end(&mut buf).ok()?;
        return parse_mp4_metadata(&buf);
    }
    let mut off = 0u64;
    let mut hdr = [0u8; 16];
    for _ in 0..64 {
        if off + 8 > len {
            break;
        }
        file.seek(SeekFrom::Start(off)).ok()?;
        file.read_exact(&mut hdr[..8]).ok()?;
        let sz32 = u32::from_be_bytes(hdr[0..4].try_into().ok()?) as u64;
        let kind = [hdr[4], hdr[5], hdr[6], hdr[7]];
        let atom_len = if sz32 == 1 && off + 16 <= len {
            file.read_exact(&mut hdr[8..16]).ok()?;
            u64::from_be_bytes(hdr[8..16].try_into().ok()?)
        } else if sz32 == 0 {
            len - off
        } else {
            sz32
        };
        if atom_len < 8 || off.saturating_add(atom_len) > len {
            break;
        }
        if &kind == b"moov" && atom_len <= 8 * 1024 * 1024 {
            let mut box_buf = vec![0u8; atom_len as usize];
            file.seek(SeekFrom::Start(off)).ok()?;
            file.read_exact(&mut box_buf).ok()?;
            return parse_mp4_metadata(&box_buf);
        }
        off = off.saturating_add(atom_len);
    }
    None
}

pub fn inspect_video(path: &Path) -> Result<VideoInfo, String> {
    if !path.is_file() {
        return Err(format!("Video file not found: {}", path.display()));
    }
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let ext_ok = VIDEO_EXTS.contains(&ext.as_str());
    let size_bytes = meta.len();
    let out = output_dir();
    let inside_output_dir = match (path.canonicalize(), out.canonicalize()) {
        (Ok(p), Ok(o)) => p.starts_with(o),
        _ => path.starts_with(&out),
    };
    let (duration_seconds, dimensions) = if matches!(ext.as_str(), "mp4" | "m4v" | "mov") {
        read_mp4_moov_bounded(path).unwrap_or((None, None))
    } else {
        (None, None)
    };
    Ok(VideoInfo {
        path: path.to_path_buf(),
        extension: ext,
        extension_supported: ext_ok,
        size_bytes,
        within_1gb_limit: size_bytes <= MAX_VIDEO_BYTES,
        inside_output_dir,
        duration_seconds,
        dimensions,
    })
}

pub fn format_video_meta(info: &VideoInfo) -> String {
    let mut parts = Vec::new();
    if let Some((w, h)) = info.dimensions {
        parts.push(format!("{w}x{h}"));
    }
    if let Some(d) = info.duration_seconds {
        parts.push(format!("{d:.1}s"));
    }
    let mb = info.size_bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1.0 {
        parts.push(format!("{mb:.1} MB"));
    } else {
        parts.push(format!("{} KB", (info.size_bytes / 1024).max(1)));
    }
    parts.join(" · ")
}

pub fn list_recent_videos(limit: usize) -> Vec<VideoInfo> {
    let out = output_dir();
    let Ok(rd) = fs::read_dir(&out) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.is_file() && is_video_path(&p) {
                let m = e.metadata().ok()?.modified().ok()?;
                Some((m, p))
            } else {
                None
            }
        })
        .collect();
    files.sort_by_key(|a| std::cmp::Reverse(a.0));
    files
        .into_iter()
        .take(limit.min(30))
        .filter_map(|(_, p)| inspect_video(&p).ok())
        .collect()
}

pub fn open_file_in_os(path: &Path) {
    #[cfg(windows)]
    {
        let _ = crate::proc::command("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

/// Returns the ACP `mcpServers` entry launching this `fluxcode.exe --gflow-mcp` binary directly.
pub fn mcp_server_entry(enabled: bool) -> Option<Value> {
    if !enabled {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    Some(json!({
        "name": "gflow",
        "command": exe,
        "args": ["--gflow-mcp"],
        "env": [
            { "name": "GFLOW_CLI_OUTPUT_DIR", "value": output_dir().to_string_lossy().into_owned() },
            { "name": "GFLOW_CLI_HOME", "value": gflow_home().to_string_lossy().into_owned() }
        ]
    }))
}

fn mcp_tools_schema() -> Value {
    json!([
        {
            "name": "gflow_accounts_status",
            "description": "List saved isolated Chrome Google Flow profiles, active routing mode (sticky/auto_rotate/highest_credits), cooldowns, and cached Veo credits.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "gflow_accounts_set_mode",
            "description": "Set Google Flow multi-account routing mode (sticky, auto_rotate, highest_credits), sticky profile, custom order, or cooldown minutes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["sticky", "auto_rotate", "highest_credits"] },
                    "sticky_profile": { "type": "string" },
                    "order": { "type": "array", "items": { "type": "string" } },
                    "cooldown_minutes": { "type": "integer", "minimum": 5, "maximum": 1440 }
                },
                "required": ["mode"],
                "additionalProperties": false
            }
        },
        {
            "name": "gflow_accounts_select",
            "description": "Pick the next ready Google Flow profile using the active routing mode (sticky, auto_rotate, highest_credits).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_type": { "type": "string", "enum": ["image", "video", "edit", "extend"] },
                    "min_credits": { "type": "integer", "minimum": 0 },
                    "advance_cursor": { "type": "boolean" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "gflow_accounts_check_credits",
            "description": "Check Google Flow Veo credit balances across saved profiles (spends 0 credits).",
            "inputSchema": {
                "type": "object",
                "properties": { "refresh": { "type": "boolean" } },
                "additionalProperties": false
            }
        },
        {
            "name": "gflow_accounts_report_outcome",
            "description": "Record generation outcome (ok, rate_limited, waf_403, auth_expired, clear_cooldown) to enforce cooldowns cleanly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile": { "type": "string" },
                    "outcome": { "type": "string", "enum": ["ok", "rate_limited", "waf_403", "auth_expired", "clear_cooldown"] },
                    "credits_remaining": { "type": "integer", "minimum": 0 },
                    "note": { "type": "string" }
                },
                "required": ["profile", "outcome"],
                "additionalProperties": false
            }
        },
        {
            "name": "gflow_accounts_login",
            "description": "Open a native Windows Chrome window for an isolated Flow profile (e.g. akun1, akun2) so the user can sign in.",
            "inputSchema": {
                "type": "object",
                "properties": { "profile": { "type": "string" } },
                "additionalProperties": false
            }
        },
        {
            "name": "gflow_accounts_video_inspect",
            "description": "Inspect a local video file (.mp4/.mov/.m4v/.webm/.3gp/.avi <= 1 GB, dimensions, duration) or list recent Flow output videos.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "video_path": { "type": "string" },
                    "list_recent": { "type": "boolean" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 30 }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "gflow_run",
            "description": "Generate or edit Flow media (image t2i, video t2v/i2v/r2v/edit/extend) with automatic multi-account profile selection via optional installed gflow CLI.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["image_t2i", "video_t2v", "video_i2v", "video_r2v", "video_edit", "video_extend"] },
                    "prompt": { "type": "string" },
                    "source": { "type": "string" },
                    "aspect": { "type": "string" },
                    "profile": { "type": "string" },
                    "max_credits": { "type": "integer", "minimum": 0 }
                },
                "required": ["action", "prompt"],
                "additionalProperties": false
            }
        }
    ])
}

fn dispatch_mcp_tool(name: &str, args: &Value, home: &Path) -> Result<Value, String> {
    match name {
        "gflow_accounts_status" => {
            let state = load_state_in(home);
            let profiles = discover_profiles_in(home, &state);
            Ok(json!({
                "status": "ok",
                "mode": state.mode,
                "sticky_profile": state.sticky_profile,
                "order": state.order,
                "cooldown_minutes": state.cooldown_minutes,
                "gflow_home": home,
                "output_dir": output_dir(),
                "chrome": chrome_binary().map(|(k, p)| json!({ "kind": k, "path": p })),
                "gflow_cli_installed": gflow_cli_bin().is_some(),
                "profiles": profiles,
                "last_credits_check": state.last_credits_check,
            }))
        }
        "gflow_accounts_set_mode" => {
            let mode_s = args.get("mode").and_then(Value::as_str).unwrap_or("");
            let mode =
                RoutingMode::from_str(mode_s).ok_or_else(|| format!("Invalid mode '{mode_s}'."))?;
            let sticky = args
                .get("sticky_profile")
                .and_then(Value::as_str)
                .map(String::from);
            let order = args.get("order").and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            });
            let mins = args
                .get("cooldown_minutes")
                .and_then(Value::as_u64)
                .map(|m| m as u32);
            let st = set_mode_in(home, mode, sticky, order, mins)?;
            Ok(json!({ "status": "ok", "state": st, "description": mode.blurb() }))
        }
        "gflow_accounts_select" => {
            let task = args
                .get("task_type")
                .and_then(Value::as_str)
                .unwrap_or("image");
            let min_c = args.get("min_credits").and_then(Value::as_i64);
            let adv = args
                .get("advance_cursor")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let sel = select_profile_in(home, task, min_c, adv)?;
            serde_json::to_value(sel).map_err(|e| e.to_string())
        }
        "gflow_accounts_check_credits" => {
            let refresh = args
                .get("refresh")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let report = check_credits(refresh)?;
            serde_json::to_value(report).map_err(|e| e.to_string())
        }
        "gflow_accounts_report_outcome" => {
            let profile = args.get("profile").and_then(Value::as_str).unwrap_or("");
            let outcome = args.get("outcome").and_then(Value::as_str).unwrap_or("");
            let rem = args.get("credits_remaining").and_then(Value::as_i64);
            let note = args.get("note").and_then(Value::as_str).map(String::from);
            report_outcome_in(home, profile, outcome, rem, note)
        }
        "gflow_accounts_login" => {
            let state = load_state_in(home);
            let existing = discover_profiles_in(home, &state);
            let name = args
                .get("profile")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| next_profile_name(&existing));
            let kind = launch_browser_login(&name)?;
            Ok(json!({
                "status": "ok",
                "profile": name,
                "browser": kind,
                "message": "Opened isolated browser window for Flow login."
            }))
        }
        "gflow_accounts_video_inspect" => {
            let recent = args
                .get("list_recent")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(p) = args
                .get("video_path")
                .and_then(Value::as_str)
                .filter(|_| !recent)
            {
                let info = inspect_video(Path::new(p))?;
                serde_json::to_value(info).map_err(|e| e.to_string())
            } else {
                let lim = args.get("limit").and_then(Value::as_u64).unwrap_or(10) as usize;
                Ok(json!({
                    "status": "ok",
                    "output_dir": output_dir(),
                    "videos": list_recent_videos(lim),
                }))
            }
        }
        "gflow_run" => {
            let Some(bin) = gflow_cli_bin() else {
                return Err(
                    "Optional `gflow` CLI binary not installed on PATH; Flow account routing and login work natively in Rust, while headless Flow UI automation requires installing `gflow-cli`."
                        .into(),
                );
            };
            let action = args.get("action").and_then(Value::as_str).unwrap_or("");
            let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or("");
            let profile = match args
                .get("profile")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                Some(p) => p.to_string(),
                None => {
                    select_profile_in(
                        home,
                        if action.starts_with("image") {
                            "image"
                        } else {
                            "video"
                        },
                        args.get("max_credits").and_then(Value::as_i64),
                        true,
                    )?
                    .profile
                }
            };
            let mut cmd = crate::proc::command(&bin.to_string_lossy());
            match action {
                "image_t2i" => {
                    cmd.args(["image", "t2i", prompt, "--profile", &profile, "--json"]);
                }
                "video_t2v" => {
                    cmd.args(["video", "t2v", prompt, "--profile", &profile, "--json"]);
                }
                "video_i2v" => {
                    let src = args
                        .get("source")
                        .and_then(Value::as_str)
                        .ok_or("source required")?;
                    cmd.args(["video", "i2v", src, prompt, "--profile", &profile, "--json"]);
                }
                "video_r2v" => {
                    let src = args
                        .get("source")
                        .and_then(Value::as_str)
                        .ok_or("source required")?;
                    cmd.args([
                        "video",
                        "r2v",
                        "--ref",
                        src,
                        prompt,
                        "--profile",
                        &profile,
                        "--json",
                    ]);
                }
                "video_edit" => {
                    let src = args
                        .get("source")
                        .and_then(Value::as_str)
                        .ok_or("source required")?;
                    cmd.args([
                        "video",
                        "edit",
                        src,
                        prompt,
                        "--profile",
                        &profile,
                        "--json",
                    ]);
                }
                "video_extend" => {
                    let src = args
                        .get("source")
                        .and_then(Value::as_str)
                        .ok_or("source required")?;
                    cmd.args([
                        "video",
                        "extend",
                        src,
                        prompt,
                        "--profile",
                        &profile,
                        "--json",
                    ]);
                }
                other => return Err(format!("Unsupported action '{other}'.")),
            }
            if let Some(aspect) = args
                .get("aspect")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                cmd.args(["--aspect", aspect]);
            }
            if let Some(cap) = args.get("max_credits").and_then(Value::as_i64) {
                cmd.args(["--max-credits", &cap.to_string()]);
            }
            cmd.env("GFLOW_CLI_HOME", home);
            let out = cmd
                .output()
                .map_err(|e| format!("Failed to run gflow: {e}"))?;
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if out.status.success() {
                let _ = report_outcome_in(home, &profile, "ok", None, None);
                Ok(serde_json::from_str(&text).unwrap_or_else(
                    |_| json!({ "status": "ok", "profile": profile, "output": text }),
                ))
            } else {
                let err_txt = format!("{text}\n{}", String::from_utf8_lossy(&out.stderr));
                if err_txt.contains("403") || err_txt.contains("WafRejection") {
                    let _ = report_outcome_in(home, &profile, "waf_403", None, None);
                } else if err_txt.contains("RateLimit") || err_txt.contains("429") {
                    let _ = report_outcome_in(home, &profile, "rate_limited", None, None);
                }
                Err(err_txt.trim().to_string())
            }
        }
        other => Err(format!("Unknown tool '{other}'.")),
    }
}

pub fn handle_mcp_request(msg: &Value, home: &Path) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").unwrap_or(&Value::Null);

    let reply = match method {
        "initialize" => {
            let ver = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05");
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": ver,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "fluxcode-gflow", "version": crate::update::BUILD },
                    "instructions": "Use gflow_accounts_select to choose a ready isolated Flow profile according to sticky, auto_rotate, or highest_credits mode."
                }
            })
        }
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": mcp_tools_schema() }
        }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").unwrap_or(&Value::Null);
            let (payload, is_error) = match dispatch_mcp_tool(name, args, home) {
                Ok(v) => (v, false),
                Err(e) => (json!({ "status": "error", "error": e }), true),
            };
            let text = serde_json::to_string_pretty(&payload).unwrap_or_default();
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": payload,
                    "isError": is_error,
                }
            })
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    };
    Some(reply)
}

/// Entrypoint for `fluxcode --gflow-mcp` stdio JSON-RPC server.
pub fn mcp_cli() -> i32 {
    let home = gflow_home();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in BufReader::new(stdin.lock()).lines() {
        let Ok(raw) = line else {
            break;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(trimmed) else {
            let err = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } });
            let _ = writeln!(out, "{err}");
            let _ = out.flush();
            continue;
        };
        if let Some(reply) = handle_mcp_request(&msg, &home) {
            if writeln!(out, "{reply}").is_err() || out.flush().is_err() {
                break;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_profile(home: &Path, name: &str, pref_email: Option<&str>, strategy: &str) {
        let dir = home.join(format!("profile_{name}"));
        fs::create_dir_all(dir.join("Default").join("Network")).unwrap();
        fs::write(dir.join("Default").join("Network").join("Cookies"), b"ok").unwrap();
        fs::write(dir.join(".gflow_browser_strategy"), strategy.as_bytes()).unwrap();
        if let Some(email) = pref_email {
            fs::write(
                dir.join("Default").join("Preferences"),
                serde_json::json!({ "account_info": [{ "email": email }] }).to_string(),
            )
            .unwrap();
        }
    }

    #[test]
    fn discovers_profiles_and_reads_chrome_preferences_email() {
        let root = std::env::temp_dir().join(format!("fluxcode-gflow-disc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        mk_profile(&root, "akun1", Some("user1@gmail.com"), "chrome");
        mk_profile(&root, "akun2", Some("user2@gmail.com"), "chrome");
        fs::write(root.join("config.toml"), "default_profile = \"akun2\"\n").unwrap();

        let state = load_state_in(&root);
        let list = discover_profiles_in(&root, &state);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].email.as_deref(), Some("user1@gmail.com"));
        assert!(!list[0].is_default);
        assert!(list[1].is_default);
        assert_eq!(next_profile_name(&list), "akun3");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn routing_modes_sticky_auto_rotate_highest_credits_and_cooldowns() {
        let root = std::env::temp_dir().join(format!("fluxcode-gflow-rot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        mk_profile(&root, "akun1", Some("a@gmail.com"), "chrome");
        mk_profile(&root, "akun2", Some("b@gmail.com"), "chrome");
        mk_profile(&root, "akun3", Some("c@gmail.com"), "chrome");

        set_mode_in(&root, RoutingMode::Sticky, Some("akun2".into()), None, None).unwrap();
        assert_eq!(
            select_profile_in(&root, "image", None, true)
                .unwrap()
                .profile,
            "akun2"
        );
        assert_eq!(
            select_profile_in(&root, "video", None, true)
                .unwrap()
                .profile,
            "akun2"
        );

        set_mode_in(
            &root,
            RoutingMode::AutoRotate,
            None,
            Some(vec!["akun2".into(), "akun1".into(), "akun3".into()]),
            Some(45),
        )
        .unwrap();
        let seq: Vec<String> = (0..4)
            .map(|_| {
                select_profile_in(&root, "image", None, true)
                    .unwrap()
                    .profile
            })
            .collect();
        assert_eq!(seq, ["akun2", "akun1", "akun3", "akun2"]);

        set_mode_in(&root, RoutingMode::HighestCredits, None, None, None).unwrap();
        report_outcome_in(&root, "akun1", "ok", Some(150), None).unwrap();
        report_outcome_in(&root, "akun3", "ok", Some(900), None).unwrap();
        assert_eq!(
            select_profile_in(&root, "video", Some(500), true)
                .unwrap()
                .profile,
            "akun3"
        );

        report_outcome_in(&root, "akun3", "waf_403", None, None).unwrap();
        assert_eq!(
            select_profile_in(&root, "video", None, true)
                .unwrap()
                .profile,
            "akun1"
        );
        report_outcome_in(&root, "akun3", "clear_cooldown", None, None).unwrap();
        assert_eq!(
            select_profile_in(&root, "video", None, true)
                .unwrap()
                .profile,
            "akun3"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pure_rust_mp4_parser_reads_mvhd_duration_and_tkhd_dimensions() {
        let mut mvhd = vec![0u8; 20];
        mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[16..20].copy_from_slice(&8000u32.to_be_bytes());
        let mut mvhd_box = Vec::new();
        mvhd_box.extend_from_slice(&((8 + mvhd.len()) as u32).to_be_bytes());
        mvhd_box.extend_from_slice(b"mvhd");
        mvhd_box.extend_from_slice(&mvhd);

        let mut tkhd = vec![0u8; 84];
        tkhd[76..80].copy_from_slice(&(1280u32 << 16).to_be_bytes());
        tkhd[80..84].copy_from_slice(&(720u32 << 16).to_be_bytes());
        let mut tkhd_box = Vec::new();
        tkhd_box.extend_from_slice(&((8 + tkhd.len()) as u32).to_be_bytes());
        tkhd_box.extend_from_slice(b"tkhd");
        tkhd_box.extend_from_slice(&tkhd);

        let mut trak_box = Vec::new();
        trak_box.extend_from_slice(&((8 + tkhd_box.len()) as u32).to_be_bytes());
        trak_box.extend_from_slice(b"trak");
        trak_box.extend_from_slice(&tkhd_box);

        let mut moov_box = Vec::new();
        let moov_len = 8 + mvhd_box.len() + trak_box.len();
        moov_box.extend_from_slice(&(moov_len as u32).to_be_bytes());
        moov_box.extend_from_slice(b"moov");
        moov_box.extend_from_slice(&mvhd_box);
        moov_box.extend_from_slice(&trak_box);

        let (dur, dims) = parse_mp4_metadata(&moov_box).expect("valid moov");
        assert_eq!(dur, Some(8.0));
        assert_eq!(dims, Some((1280, 720)));
        assert!(parse_mp4_metadata(b"not-an-mp4").is_none());
    }

    #[test]
    fn mcp_jsonrpc_handles_initialize_list_and_status() {
        let root = std::env::temp_dir().join(format!("fluxcode-gflow-mcp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        mk_profile(&root, "akun1", Some("x@gmail.com"), "chrome");

        let init = handle_mcp_request(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2024-11-05" } }),
            &root,
        )
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "fluxcode-gflow");
        assert!(handle_mcp_request(
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            &root
        )
        .is_none());

        let list = handle_mcp_request(
            &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            &root,
        )
        .unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 8);

        let call = handle_mcp_request(
            &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "gflow_accounts_status", "arguments": {} } }),
            &root,
        )
        .unwrap();
        assert_eq!(call["result"]["isError"], false);
        assert_eq!(
            call["result"]["structuredContent"]["profiles"][0]["name"],
            "akun1"
        );
        let dir = register_profile_slot_in(&root, "akun9").expect("slot");
        assert!(dir.is_dir());
        assert_eq!(
            read_trimmed(&dir.join(".gflow_browser_strategy")).as_deref(),
            Some("default-browser")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
