
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{ChildStdin, Command, Stdio};
use std::thread;

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
struct ProcessJob(isize);

#[cfg(windows)]
impl ProcessJob {
    fn for_child(child: &std::process::Child) -> Option<Self> {
        #[repr(C)]
        struct BasicLimit {
            per_process_user_time_limit: i64,
            per_job_user_time_limit: i64,
            limit_flags: u32,
            minimum_working_set_size: usize,
            maximum_working_set_size: usize,
            active_process_limit: u32,
            affinity: usize,
            priority_class: u32,
            scheduling_class: u32,
        }
        #[repr(C)]
        struct IoCounters {
            read_operation_count: u64,
            write_operation_count: u64,
            other_operation_count: u64,
            read_transfer_count: u64,
            write_transfer_count: u64,
            other_transfer_count: u64,
        }
        #[repr(C)]
        struct ExtendedLimit {
            basic: BasicLimit,
            io: IoCounters,
            process_memory_limit: usize,
            job_memory_limit: usize,
            peak_process_memory_used: usize,
            peak_job_memory_used: usize,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn CreateJobObjectW(attrs: *const std::ffi::c_void, name: *const u16) -> isize;
            fn SetInformationJobObject(job: isize, class: u32, info: *const std::ffi::c_void, len: u32) -> i32;
            fn AssignProcessToJobObject(job: isize, proc: isize) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;
        const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job == 0 {
                return None;
            }
            let mut info: ExtendedLimit = std::mem::zeroed();
            info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok_info = SetInformationJobObject(
                job,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &info as *const ExtendedLimit as *const std::ffi::c_void,
                std::mem::size_of::<ExtendedLimit>() as u32,
            );
            let ok_assign = if ok_info != 0 {
                AssignProcessToJobObject(job, child.as_raw_handle() as isize)
            } else {
                0
            };
            if ok_assign == 0 {
                CloseHandle(job);
                return None;
            }
            Some(Self(job))
        }
    }

    fn terminate_and_close(mut self) {
        self.terminate_inner();
    }

    fn terminate_inner(&mut self) {
        if self.0 == 0 {
            return;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn TerminateJobObject(job: isize, exit_code: u32) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        unsafe {
            TerminateJobObject(self.0, 1);
            CloseHandle(self.0);
        }
        self.0 = 0;
    }
}

#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        self.terminate_inner();
    }
}

struct ProcEntry {
    pid: u32,
    stdin: Option<ChildStdin>,
    #[cfg(windows)]
    job: Option<ProcessJob>,
}

pub enum ProcEvent {
    Output { id: u32, text: String, err: bool },
    Exit { id: u32, code: Option<i32> },
}

pub struct Procs {
    next: u32,
    tx: UnboundedSender<ProcEvent>,
    running: HashMap<u32, ProcEntry>,
}

pub fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

pub fn shell_command(line: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = command("cmd.exe");
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
    if cfg!(windows) {
        static PY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *PY.get_or_init(|| crate::term::on_path("py")) {
            return "py".into();
        }
        "python".into()
    } else {
        "python3".into()
    }
}

impl Procs {
    pub fn new() -> (Self, UnboundedReceiver<ProcEvent>) {
        let (tx, rx) = unbounded();
        (Self { next: 0, tx, running: HashMap::new() }, rx)
    }

    pub fn is_running(&self, id: u32) -> bool {
        self.running.contains_key(&id)
    }

    pub fn spawn(&mut self, cmd: Command, cwd: Option<&Path>) -> Result<u32, String> {
        self.spawn_inner(cmd, cwd, false, None)
    }

    /// Spawns an AI agent in an isolated process group & Job Object while keeping full host PATH/workspace access.
    pub fn spawn_isolated(&mut self, cmd: Command, cwd: Option<&Path>, agent_tag: &str) -> Result<u32, String> {
        self.spawn_inner(cmd, cwd, true, Some(agent_tag))
    }

    fn spawn_inner(&mut self, mut cmd: Command, cwd: Option<&Path>, isolated: bool, agent_tag: Option<&str>) -> Result<u32, String> {
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        if isolated {
            // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP keeps the agent decoupled from host console signals.
            cmd.creation_flags(0x0800_0000 | 0x0000_0200);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1")
            .env("CARGO_TERM_COLOR", "never");
        if isolated {
            cmd.env("FLUXCODE_AGENT_ENV", "1")
                .env("FLUXCODE_HOST_PID", std::process::id().to_string());
            if let Some(tag) = agent_tag {
                cmd.env("FLUXCODE_AGENT_ID", tag);
            }
        }
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        let job = if isolated { ProcessJob::for_child(&child) } else { None };
        self.next += 1;
        let id = self.next;
        let readers: Vec<_> = [
            child.stdout.take().map(|p| pump(self.tx.clone(), id, false, p)),
            child.stderr.take().map(|p| pump(self.tx.clone(), id, true, p)),
        ]
        .into_iter()
        .flatten()
        .collect();
        self.running.insert(
            id,
            ProcEntry {
                pid: child.id(),
                stdin: child.stdin.take(),
                #[cfg(windows)]
                job,
            },
        );
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

    pub fn forget(&mut self, id: u32) {
        self.running.remove(&id);
    }

    pub fn write_stdin(&mut self, id: u32, text: &str) {
        if let Some(entry) = self.running.get_mut(&id) {
            if let Some(stdin) = entry.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
                let _ = stdin.flush();
            }
        }
    }

    pub fn kill_all(&mut self) {
        let ids: Vec<u32> = self.running.keys().copied().collect();
        for id in ids {
            self.kill(id);
        }
    }

    pub fn kill(&mut self, id: u32) {
        let Some(mut entry) = self.running.remove(&id) else { return };
        drop(entry.stdin.take());
        #[cfg(windows)]
        {
            if let Some(job) = entry.job.take() {
                job.terminate_and_close();
                return;
            }
            let _ = command("taskkill").args(["/T", "/F", "/PID", &entry.pid.to_string()]).status();
        }
        #[cfg(not(windows))]
        let _ = command("kill").args(["-9", &entry.pid.to_string()]).status();
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

    #[cfg(windows)]
    #[test]
    fn isolated_job_terminates_spawned_process() {
        let (mut procs, _rx) = super::Procs::new();
        let mut cmd = super::command("cmd.exe");
        cmd.args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]);
        let id = procs.spawn_isolated(cmd, None, "test-agent").expect("spawn isolated");
        assert!(procs.is_running(id));
        procs.kill(id);
        assert!(!procs.is_running(id));
    }
}
