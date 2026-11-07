use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.process_group(0);
    cmd
}

pub fn kill_tree(child: &mut Child) -> Result<(), String> {
    let pid = child.id() as i32;
    unsafe {
        let _ = libc::killpg(pid, libc::SIGTERM);
        if libc::killpg(pid, libc::SIGKILL) != 0 {
            let _ = child.kill();
        }
    }
    Ok(())
}


pub fn resolve_venv_python(workspace: &Path) -> Option<PathBuf> {
    for venv in [".venv", "venv", "env"] {
        let p = workspace.join(venv).join("bin").join("python");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn is_executable(path: &Path) -> bool {
    path.is_file()
        && fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

pub fn get_available_shells() -> Vec<String> {
    let mut shells = Vec::new();
    if let Ok(content) = fs::read_to_string("/etc/shells") {
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                if let Some(name) = Path::new(line).file_name().and_then(|n| n.to_str()) {
                    if !shells.iter().any(|s: &String| s == name) && Path::new(line).is_file() {
                        shells.push(name.to_string());
                    }
                }
            }
        }
    }
    if shells.is_empty() {
        shells.push("bash".to_string());
        shells.push("sh".to_string());
    }
    shells
}

pub fn get_system_roots() -> Vec<String> {
    vec!["/".to_string()]
}

pub fn get_env_user() -> (String, String) {
    let user = std::env::var("USER").unwrap_or_default();
    let computer = std::env::var("HOSTNAME").unwrap_or_else(|_| {
        let mut buf = vec![0u8; 256];
        unsafe {
            if libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) == 0 {
                let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                String::from_utf8_lossy(&buf[..len]).to_string()
            } else {
                String::new()
            }
        }
    });
    (user, computer)
}

pub fn normalize_path(path: &str) -> String {
    let p = path.trim();
    if p == "/" {
        return "/".to_string();
    }
    p.trim_end_matches('/').to_string()
}

pub fn is_path_under(path: &str, root: &str) -> bool {
    let p = normalize_path(path);
    let r = normalize_path(root);
    if r == "/" {
        return p.starts_with('/');
    }
    p == r || (p.starts_with(&r) && p[r.len()..].starts_with('/'))
}

pub fn get_platform() -> &'static str {
    "linux"
}
