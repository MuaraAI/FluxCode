use std::path::{Path, PathBuf};
use std::process::{Child, Command};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

pub fn kill_tree(child: &mut Child) -> Result<(), String> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let status = command("taskkill").args(["/T", "/F", "/PID", &pid]).status();
        if status.map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }
    }
    child.kill().map_err(|e| e.to_string())
}

pub fn resolve_venv_python(workspace: &Path) -> Option<PathBuf> {
    for venv in [".venv", "venv", "env"] {
        let p = workspace.join(venv).join("Scripts").join("python.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn is_executable(path: &Path) -> bool {
    let exts: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
        .split(';')
        .filter(|e| !e.is_empty())
        .map(str::to_lowercase)
        .collect();
    let name = path.to_string_lossy().to_lowercase();
    exts.iter().any(|ext| name.ends_with(ext)) && path.is_file()
}

pub fn get_available_shells() -> Vec<String> {
    let mut shells = vec!["cmd".to_string(), "powershell".to_string()];
    let on_path = |exe: &str| {
        std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(exe).is_file()))
    };
    if on_path("pwsh.exe") {
        shells.push("pwsh".into());
    }
    shells
}

pub fn get_system_roots() -> Vec<String> {
    (b'C'..=b'Z')
        .map(|c| format!("{}:\\", c as char))
        .filter(|d| Path::new(d).exists())
        .collect()
}

pub fn get_env_user() -> (String, String) {
    let user = std::env::var("USERNAME").unwrap_or_default();
    let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
    (user, computer)
}

pub fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

pub fn is_path_under(path: &str, root: &str) -> bool {
    let p = normalize_path(path);
    let r = normalize_path(root);
    p == r || (p.starts_with(&r) && p[r.len()..].starts_with('\\'))
}

pub fn get_platform() -> &'static str {
    "windows"
}
