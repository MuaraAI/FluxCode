//! GitHub account, ported from `src-tauri/src/github.rs`: a personal access
//! token (verified against api.github.com with the system `curl.exe`, so no
//! HTTP client is compiled in) or the token of an existing GitHub CLI login.
//! Passed to terminal commands and OpenCode as `GH_TOKEN` / `GITHUB_TOKEN`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    pub name: String,
    /// "token" (pasted) or "gh" (GitHub CLI)
    pub source: String,
    #[serde(default)]
    token: String,
}

fn file() -> PathBuf {
    crate::settings::config_dir().join("github.json")
}

pub fn load() -> Option<Account> {
    let text = fs::read_to_string(file()).ok()?;
    serde_json::from_str::<Account>(&text).ok().filter(|a| !a.token.is_empty())
}

fn save(account: &Account) -> Result<(), String> {
    let path = file();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_string(account).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// The stored token, if any (for child-process environments).
pub fn token() -> Option<String> {
    load().map(|a| a.token)
}

/// Add the token to a child's environment.
pub fn apply_env(cmd: &mut std::process::Command) {
    if let Some(t) = token() {
        cmd.env("GH_TOKEN", &t).env("GITHUB_TOKEN", &t);
    }
}

pub fn gh_on_path() -> bool {
    let exe = if cfg!(windows) { "gh.exe" } else { "gh" };
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(exe).is_file()))
}

/// Ask api.github.com who the token belongs to. The token goes to curl through
/// stdin (`--config -`), never on the command line.
fn verify(token: &str) -> Result<(String, String), String> {
    let token = token.trim();
    if token.is_empty() || token.contains(|c: char| c.is_whitespace() || c == '"') {
        return Err("That doesn't look like a token.".into());
    }
    let mut child = crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-s", "-S", "--max-time", "15", "--config", "-", "https://api.github.com/user"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl could not start ({e}). It ships with Windows 10 1803+ and most Linux distributions."))?;
    {
        let stdin = child.stdin.as_mut().ok_or("no stdin")?;
        let cfg = format!("header = \"Authorization: Bearer {token}\"\nheader = \"User-Agent: FluxCode\"\nheader = \"Accept: application/vnd.github+json\"\n");
        stdin.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("Could not reach GitHub: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| "GitHub sent an unexpected answer.".to_string())?;
    match v.get("login").and_then(|l| l.as_str()) {
        Some(login) => Ok((login.to_string(), v.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string())),
        None => Err(v.get("message").and_then(|m| m.as_str()).unwrap_or("The token was rejected.").to_string()),
    }
}

pub fn login_token(token: &str) -> Result<Account, String> {
    let (login, name) = verify(token)?;
    let account = Account { login, name, source: "token".into(), token: token.trim().to_string() };
    save(&account)?;
    Ok(account)
}

/// Reuse the GitHub CLI's login (`gh auth token`).
pub fn login_cli() -> Result<Account, String> {
    if !gh_on_path() {
        return Err("The GitHub CLI (gh) was not found on PATH. Install it from cli.github.com, run `gh auth login`, then try again.".into());
    }
    let out = crate::proc::command("gh").args(["auth", "token"]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("gh is not signed in: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (login, name) = verify(&token)?;
    let account = Account { login, name, source: "gh".into(), token };
    save(&account)?;
    Ok(account)
}

pub fn logout() -> Result<(), String> {
    let f = file();
    if f.exists() {
        fs::remove_file(f).map_err(|e| e.to_string())?;
    }
    Ok(())
}
