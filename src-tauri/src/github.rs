//! GitHub account: a personal access token (verified against api.github.com
//! with the system `curl.exe`, so no HTTP client is compiled in) or the token
//! of an existing GitHub CLI login. Stored in the app config folder and
//! passed to Flux Terminal commands and OpenCode as `GH_TOKEN` / `GITHUB_TOKEN`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Manager};

use crate::command;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    pub name: String,
    /// "token" (pasted) or "gh" (GitHub CLI)
    pub source: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    token: String,
}

#[derive(Serialize)]
pub struct Status {
    login: Option<String>,
    name: Option<String>,
    source: Option<String>,
    /// Is the GitHub CLI installed?
    gh: bool,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("github.json"))
}

fn load(app: &AppHandle) -> Option<Account> {
    let text = fs::read_to_string(file(app)?).ok()?;
    serde_json::from_str::<Account>(&text).ok().filter(|a| !a.token.is_empty())
}

fn save(app: &AppHandle, account: &Account) -> Result<(), String> {
    let path = file(app).ok_or("No config folder")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Serialize with the token (the field is skipped only when empty).
    let json = serde_json::json!({ "login": account.login, "name": account.name, "source": account.source, "token": account.token });
    fs::write(path, json.to_string()).map_err(|e| e.to_string())
}

/// The stored token, if any (for child-process environments).
pub fn token(app: &AppHandle) -> Option<String> {
    load(app).map(|a| a.token)
}

fn gh_on_path() -> bool {
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
    let mut child = command(if cfg!(windows) { "curl.exe" } else { "curl" })
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

#[tauri::command]
pub fn github_status(app: AppHandle) -> Status {
    let a = load(&app);
    Status {
        login: a.as_ref().map(|a| a.login.clone()),
        name: a.as_ref().map(|a| a.name.clone()),
        source: a.as_ref().map(|a| a.source.clone()),
        gh: gh_on_path(),
    }
}

#[tauri::command]
pub async fn github_login(app: AppHandle, token: String) -> Result<Account, String> {
    let t = token.clone();
    let (login, name) = tauri::async_runtime::spawn_blocking(move || verify(&t)).await.map_err(|e| e.to_string())??;
    let account = Account { login, name, source: "token".into(), token: token.trim().to_string() };
    save(&app, &account)?;
    Ok(account)
}

/// Reuse the GitHub CLI's login (`gh auth token`).
#[tauri::command]
pub async fn github_login_cli(app: AppHandle) -> Result<Account, String> {
    if !gh_on_path() {
        return Err("The GitHub CLI (gh) was not found on PATH. Install it from cli.github.com, run `gh auth login`, then try again.".into());
    }
    let result = tauri::async_runtime::spawn_blocking(|| {
        let out = command("gh").args(["auth", "token"]).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("gh is not signed in: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
        verify(&token).map(|(login, name)| (login, name, token))
    })
    .await
    .map_err(|e| e.to_string())??;
    let (login, name, token) = result;
    let account = Account { login, name, source: "gh".into(), token };
    save(&app, &account)?;
    Ok(account)
}

#[tauri::command]
pub fn github_logout(app: AppHandle) -> Result<(), String> {
    if let Some(f) = file(&app) {
        if f.exists() {
            fs::remove_file(f).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
