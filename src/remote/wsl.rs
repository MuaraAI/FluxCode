
use std::process::Command;

use super::WslTarget;

pub fn available() -> bool {
    cfg!(windows)
}

pub fn distros() -> Vec<String> {
    if !available() {
        return Vec::new();
    }
    let Ok(out) = crate::proc::command("wsl.exe").args(["-l", "-q"]).output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    decode_utf16(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|p| u16::from_le_bytes(*p)).collect();
    String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string()
}

pub fn home_dir(distro: &str) -> Option<String> {
    let out = crate::proc::command("wsl.exe")
        .args(["-d", distro, "--exec", "sh", "-c", "echo $HOME"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let home = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!home.is_empty()).then_some(home)
}

pub fn list_dirs(distro: &str, path: &str) -> Vec<String> {
    let out = crate::proc::command("wsl.exe")
        .args(["-d", distro, "--exec", "sh", "-c", r#"cd "$1" || exit 1; shift; ls -1pa"#, "--", path])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.ends_with('/'))
        .map(|l| l.trim_end_matches('/').to_string())
        .filter(|l| l != "." && l != "..")
        .collect()
}

pub fn shell_command(target: &WslTarget, cwd: &str, line: &str) -> Command {
    let mut cmd = crate::proc::command("wsl.exe");
    cmd.args([
        "-d",
        &target.distro,
        "--exec",
        "sh",
        "-lc",
        r#"cd "$1" || exit 1; shift; exec sh -lc "$1""#,
        "--",
        cwd,
        line,
    ]);
    cmd
}

pub fn program_command(target: &WslTarget, cwd: &str, program: &str, args: &[&str]) -> Command {
    let mut cmd = crate::proc::command("wsl.exe");
    cmd.args(["-d", &target.distro, "--exec", "sh", "-c", r#"cd "$1" || exit 1; shift; exec "$@""#, "--", cwd, program]);
    cmd.args(args);
    cmd
}

pub fn program_command_env(target: &WslTarget, cwd: &str, env: &[(&str, &str)], program: &str, args: &[&str]) -> Command {
    let mut exports = String::new();
    for (k, v) in env {
        exports.push_str(&format!("export {k}='{v}'; "));
    }
    let script = format!(r#"cd "$1" || exit 1; shift; {exports}exec "$@""#);
    let mut cmd = crate::proc::command("wsl.exe");
    cmd.args(["-d", &target.distro, "--exec", "sh", "-c", &script, "--", cwd, program]);
    cmd.args(args);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distro_list_is_utf16() {
        let bytes = [0x55, 0x00, 0x62, 0x00, 0x75, 0x00, 0x6e, 0x00, 0x74, 0x00, 0x75, 0x00, 0x0d, 0x00, 0x0a, 0x00];
        assert_eq!(decode_utf16(&bytes), "Ubuntu\r\n");
    }

    #[test]
    fn a_line_runs_through_exec() {
        let t = WslTarget { distro: "Ubuntu".into(), linux_root: "/home/me".into(), unc_root: Default::default() };
        let cmd = shell_command(&t, "/home/me", "ls -la");
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args[0..4], ["-d", "Ubuntu", "--exec", "sh"]);
        assert_eq!(args[5], r#"cd "$1" || exit 1; shift; exec sh -lc "$1""#);
        assert_eq!(args[6..], ["--", "/home/me", "ls -la"]);
    }

    #[test]
    fn a_program_keeps_its_arguments() {
        let t = WslTarget { distro: "Ubuntu".into(), linux_root: "/home/me".into(), unc_root: Default::default() };
        let cmd = program_command(&t, "/home/me", "git", &["status", "--porcelain"]);
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args[6..], ["--", "/home/me", "git", "status", "--porcelain"]);
    }

    #[test]
    fn environment_is_exported_inside_the_distro() {
        let t = WslTarget { distro: "Ubuntu".into(), linux_root: "/home/me".into(), unc_root: Default::default() };
        let cmd = program_command_env(&t, "/home/me", &[("GIT_TERMINAL_PROMPT", "0")], "git", &["status"]);
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args[5].contains("export GIT_TERMINAL_PROMPT='0';"), "{}", args[5]);
        assert_eq!(args[6..], ["--", "/home/me", "git", "status"]);
    }
}
