//! Child processes with streamed output (ported from `start_process` in
//! `src-tauri/src/lib.rs`). Output arrives on a channel the workbench drains.

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{ChildStdin, Command, Stdio};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub enum ProcEvent {
    Output { id: u32, text: String, err: bool },
    Exit { id: u32, code: Option<i32> },
}

pub struct Procs {
    next: u32,
    tx: UnboundedSender<ProcEvent>,
    running: HashMap<u32, (u32, Option<ChildStdin>)>,
}

/// `Command` that never flashes a console window on Windows.
pub fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// One terminal line through `cmd.exe` (Windows) or `sh` (elsewhere).
pub fn shell_command(line: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = command("cmd.exe");
        // /s keeps the line verbatim; chcp 65001 makes cmd's output UTF-8.
        c.raw_arg(format!("/d /s /c \"chcp 65001>nul & {line}\""));
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = command("sh");
        c.args(["-c", line]);
        c
    }
}

/// Python of the workspace venv if there is one, else `python` on PATH.
pub fn python(workspace: Option<&Path>) -> String {
    if let Some(ws) = workspace {
        for venv in [".venv", "venv", "env"] {
            let p = if cfg!(windows) {
                ws.join(venv).join("Scripts").join("python.exe")
            } else {
                ws.join(venv).join("bin").join("python")
            };
            if p.is_file() {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    if cfg!(windows) { "python".into() } else { "python3".into() }
}

impl Procs {
    pub fn new() -> (Self, UnboundedReceiver<ProcEvent>) {
        let (tx, rx) = unbounded();
        (Self { next: 0, tx, running: HashMap::new() }, rx)
    }

    pub fn is_running(&self, id: u32) -> bool {
        self.running.contains_key(&id)
    }

    pub fn spawn(&mut self, mut cmd: Command, cwd: Option<&Path>) -> Result<u32, String> {
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Python: unbuffered + UTF-8 so output streams live and unicode prints work.
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1")
            .env("CARGO_TERM_COLOR", "never");
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        self.next += 1;
        let id = self.next;
        let readers: Vec<_> = [
            child.stdout.take().map(|p| pump(self.tx.clone(), id, false, p)),
            child.stderr.take().map(|p| pump(self.tx.clone(), id, true, p)),
        ]
        .into_iter()
        .flatten()
        .collect();
        self.running.insert(id, (child.id(), child.stdin.take()));
        let tx = self.tx.clone();
        thread::spawn(move || {
            for r in readers {
                let _ = r.join();
            }
            let code = child.wait().ok().and_then(|s| s.code());
            let _ = tx.unbounded_send(ProcEvent::Exit { id, code });
        });
        Ok(id)
    }

    /// Called when the Exit event is handled.
    pub fn forget(&mut self, id: u32) {
        self.running.remove(&id);
    }

    pub fn write_stdin(&mut self, id: u32, text: &str) {
        if let Some((_, Some(stdin))) = self.running.get_mut(&id) {
            let _ = stdin.write_all(text.as_bytes());
            let _ = stdin.flush();
        }
    }

    pub fn kill(&mut self, id: u32) {
        if let Some((pid, _)) = self.running.get(&id) {
            #[cfg(windows)]
            let _ = command("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).status();
            #[cfg(not(windows))]
            let _ = command("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

fn pump(tx: UnboundedSender<ProcEvent>, id: u32, err: bool, mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut carry: Vec<u8> = Vec::new();
        loop {
            let n = match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            carry.extend_from_slice(&buf[..n]);
            // Only send complete UTF-8 sequences; keep a split character for the next read.
            let valid = match std::str::from_utf8(&carry) {
                Ok(_) => carry.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => carry.len(),
            };
            let text = String::from_utf8_lossy(&carry[..valid]).into_owned();
            carry.drain(..valid);
            if tx.unbounded_send(ProcEvent::Output { id, text, err }).is_err() {
                break;
            }
        }
        if !carry.is_empty() {
            let _ = tx.unbounded_send(ProcEvent::Output { id, text: String::from_utf8_lossy(&carry).into_owned(), err });
        }
    })
}

/// Remove ANSI escape sequences (colours, cursor moves) from terminal output.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            match it.peek() {
                Some('[') => {
                    it.next();
                    while let Some(&n) = it.peek() {
                        it.next();
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC ... BEL or ST
                    while let Some(n) = it.next() {
                        if n == '\u{7}' || n == '\u{1b}' {
                            break;
                        }
                    }
                }
                _ => {
                    it.next();
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn strips_colours() {
        assert_eq!(super::strip_ansi("\u{1b}[31mred\u{1b}[0m ok\r"), "red ok");
    }
}
