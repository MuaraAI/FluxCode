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
pub(crate) struct ProcessJob(isize);

#[cfg(windows)]
impl ProcessJob {
    pub(crate) fn for_child(child: &std::process::Child) -> Option<Self> {
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
            fn SetInformationJobObject(
                job: isize,
                class: u32,
                info: *const std::ffi::c_void,
                len: u32,
            ) -> i32;
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

    pub(crate) fn terminate_and_close(mut self) {
        self.terminate_inner();
    }

    fn pids(&self) -> Vec<u32> {
        if self.0 == 0 {
            return Vec::new();
        }
        #[repr(C)]
        struct JobPidList {
            assigned: u32,
            in_list: u32,
            pids: [usize; 64],
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn QueryInformationJobObject(
                job: isize,
                class: u32,
                info: *mut std::ffi::c_void,
                len: u32,
                ret_len: *mut u32,
            ) -> i32;
        }
        const JOB_OBJECT_BASIC_PROCESS_ID_LIST: u32 = 3;
        unsafe {
            let mut list: JobPidList = std::mem::zeroed();
            if QueryInformationJobObject(
                self.0,
                JOB_OBJECT_BASIC_PROCESS_ID_LIST,
                &mut list as *mut JobPidList as *mut std::ffi::c_void,
                std::mem::size_of::<JobPidList>() as u32,
                std::ptr::null_mut(),
            ) == 0
            {
                return Vec::new();
            }
            let count = (list.in_list as usize).min(list.pids.len());
            list.pids[..count].iter().map(|&p| p as u32).collect()
        }
    }

    pub(crate) fn working_set_mb(&self) -> f64 {
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn CloseHandle(handle: isize) -> i32;
        }
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const PROCESS_VM_READ: u32 = 0x0010;
        let mut total_bytes: u64 = 0;
        for pid in self.pids() {
            unsafe {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
                if h != 0 {
                    if let Some(ws) = crate::boot::handle_working_set_bytes(h) {
                        total_bytes = total_bytes.saturating_add(ws as u64);
                    }
                    CloseHandle(h);
                }
            }
        }
        total_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Non-destructively trims cold resident RAM pages across the entire Job Object and
    /// optionally engages Windows EcoQoS (`PROCESS_POWER_THROTTLING_EXECUTION_SPEED`).
    /// The agent remains 100% live and connected.
    pub(crate) fn set_thermal_state(&self, trim_ws: bool, eco_qos: Option<bool>) -> usize {
        #[repr(C)]
        struct PowerThrottlingState {
            version: u32,
            control_mask: u32,
            state_mask: u32,
        }
        #[repr(C)]
        struct MemoryPriorityInformation {
            memory_priority: u32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn SetProcessWorkingSetSize(proc: isize, min_sz: usize, max_sz: usize) -> i32;
            fn SetProcessInformation(
                proc: isize,
                class: u32,
                info: *const std::ffi::c_void,
                len: u32,
            ) -> i32;
            fn SetPriorityClass(proc: isize, class: u32) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const PROCESS_SET_QUOTA: u32 = 0x0100;
        const PROCESS_SET_INFORMATION: u32 = 0x0200;
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const PROCESS_MEMORY_PRIORITY: u32 = 0;
        const PROCESS_POWER_THROTTLING: u32 = 4;
        const EXECUTION_SPEED: u32 = 0x1;
        const MEMORY_PRIORITY_LOW: u32 = 2;
        const MEMORY_PRIORITY_NORMAL: u32 = 5;
        const NORMAL_PRIORITY_CLASS: u32 = 0x0020;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x4000;

        let access = PROCESS_QUERY_LIMITED_INFORMATION
            | if trim_ws { PROCESS_SET_QUOTA } else { 0 }
            | if eco_qos.is_some() {
                PROCESS_SET_INFORMATION
            } else {
                0
            };
        let mut touched = 0usize;
        for pid in self.pids() {
            unsafe {
                let h = OpenProcess(access, 0, pid);
                if h != 0 {
                    if let Some(eco) = eco_qos {
                        let pt = PowerThrottlingState {
                            version: 1,
                            control_mask: EXECUTION_SPEED,
                            state_mask: if eco { EXECUTION_SPEED } else { 0 },
                        };
                        let _ = SetProcessInformation(
                            h,
                            PROCESS_POWER_THROTTLING,
                            &pt as *const PowerThrottlingState as *const std::ffi::c_void,
                            std::mem::size_of::<PowerThrottlingState>() as u32,
                        );
                        let mp = MemoryPriorityInformation {
                            memory_priority: if eco {
                                MEMORY_PRIORITY_LOW
                            } else {
                                MEMORY_PRIORITY_NORMAL
                            },
                        };
                        let _ = SetProcessInformation(
                            h,
                            PROCESS_MEMORY_PRIORITY,
                            &mp as *const MemoryPriorityInformation as *const std::ffi::c_void,
                            std::mem::size_of::<MemoryPriorityInformation>() as u32,
                        );
                        let _ = SetPriorityClass(
                            h,
                            if eco {
                                BELOW_NORMAL_PRIORITY_CLASS
                            } else {
                                NORMAL_PRIORITY_CLASS
                            },
                        );
                    }
                    if trim_ws {
                        let _ = SetProcessWorkingSetSize(h, usize::MAX, usize::MAX);
                    }
                    CloseHandle(h);
                    touched += 1;
                }
            }
        }
        touched
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

#[cfg(windows)]
fn resume_process_threads(pid: u32) -> bool {
    #[repr(C)]
    struct ThreadEntry32 {
        dw_size: u32,
        cnt_usage: u32,
        th32_thread_id: u32,
        th32_owner_process_id: u32,
        tp_base_pri: i32,
        tp_delta_pri: i32,
        dw_flags: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        fn Thread32First(snap: isize, entry: *mut ThreadEntry32) -> i32;
        fn Thread32Next(snap: isize, entry: *mut ThreadEntry32) -> i32;
        fn OpenThread(access: u32, inherit: i32, tid: u32) -> isize;
        fn ResumeThread(thread: isize) -> u32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
    const THREAD_SUSPEND_RESUME: u32 = 0x0002;
    const INVALID_HANDLE_VALUE: isize = -1;
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snap == 0 || snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: ThreadEntry32 = std::mem::zeroed();
        entry.dw_size = std::mem::size_of::<ThreadEntry32>() as u32;
        let mut resumed = false;
        if Thread32First(snap, &mut entry) != 0 {
            loop {
                if entry.th32_owner_process_id == pid {
                    let th = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32_thread_id);
                    if th != 0 {
                        if ResumeThread(th) != u32::MAX {
                            resumed = true;
                        }
                        CloseHandle(th);
                    }
                }
                if Thread32Next(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        resumed
    }
}

#[cfg(windows)]
fn terminate_process_tree(root_pid: u32) {
    #[repr(C)]
    struct ProcessEntry32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        fn Process32FirstW(snap: isize, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snap: isize, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn TerminateProcess(proc: isize, code: u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const INVALID_HANDLE_VALUE: isize = -1;
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut root_alive = false;
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap != 0 && snap != INVALID_HANDLE_VALUE {
            let mut entry: ProcessEntry32W = std::mem::zeroed();
            entry.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    if entry.th32_process_id == root_pid {
                        root_alive = true;
                    }
                    children
                        .entry(entry.th32_parent_process_id)
                        .or_default()
                        .push(entry.th32_process_id);
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
    }
    let mut targets = Vec::new();
    let mut stack = vec![root_pid];
    let mut seen = std::collections::HashSet::from([root_pid]);
    while let Some(pid) = stack.pop() {
        if let Some(kids) = children.get(&pid) {
            for &kid in kids {
                if seen.insert(kid) {
                    targets.push(kid);
                    stack.push(kid);
                }
            }
        }
    }
    if root_alive {
        targets.push(root_pid);
    }
    for pid in targets {
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if h != 0 {
                TerminateProcess(h, 1);
                CloseHandle(h);
            }
        }
    }
}

impl Drop for Procs {
    fn drop(&mut self) {
        self.kill_all();
    }
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
        (
            Self {
                next: 0,
                tx,
                running: HashMap::new(),
            },
            rx,
        )
    }

    pub fn is_running(&self, id: u32) -> bool {
        self.running.contains_key(&id)
    }

    pub fn spawn(&mut self, cmd: Command, cwd: Option<&Path>) -> Result<u32, String> {
        self.spawn_inner(cmd, cwd, false, None)
    }

    /// Spawns an AI agent in an isolated process group & Job Object while keeping full host PATH/workspace access.
    pub fn spawn_isolated(
        &mut self,
        cmd: Command,
        cwd: Option<&Path>,
        agent_tag: &str,
    ) -> Result<u32, String> {
        self.spawn_inner(cmd, cwd, true, Some(agent_tag))
    }

    fn spawn_inner(
        &mut self,
        mut cmd: Command,
        cwd: Option<&Path>,
        isolated: bool,
        agent_tag: Option<&str>,
    ) -> Result<u32, String> {
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        if isolated {
            // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED lets us bind the
            // Job Object before the agent executes a single instruction or spawns any child.
            cmd.creation_flags(0x0800_0000 | 0x0000_0200 | 0x0000_0004);
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
        let job = if isolated {
            let job = ProcessJob::for_child(&child);
            if !resume_process_threads(child.id()) {
                let _ = child.kill();
                drop(job);
                return Err("failed to resume isolated agent process".into());
            }
            job
        } else {
            None
        };
        self.next += 1;
        let id = self.next;
        let readers: Vec<_> = [
            child
                .stdout
                .take()
                .map(|p| pump(self.tx.clone(), id, false, p)),
            child
                .stderr
                .take()
                .map(|p| pump(self.tx.clone(), id, true, p)),
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

    pub fn job_working_set_mb(&self, id: u32) -> f64 {
        #[cfg(windows)]
        {
            self.running
                .get(&id)
                .and_then(|e| e.job.as_ref())
                .map_or(0.0, ProcessJob::working_set_mb)
        }
        #[cfg(not(windows))]
        {
            let _ = id;
            0.0
        }
    }

    /// Non-destructively trims cold working-set RAM and optionally enables Windows EcoQoS on a live agent job.
    pub fn cool_job(&self, id: u32, trim_ws: bool, eco_qos: Option<bool>) -> usize {
        #[cfg(windows)]
        {
            self.running
                .get(&id)
                .and_then(|e| e.job.as_ref())
                .map_or(0, |j| j.set_thermal_state(trim_ws, eco_qos))
        }
        #[cfg(not(windows))]
        {
            let _ = (id, trim_ws, eco_qos);
            0
        }
    }

    /// Restores full CPU clocks and normal priority on a live agent job before dispatching a turn.
    pub fn wake_job(&self, id: u32) {
        let _ = self.cool_job(id, false, Some(false));
    }

    pub fn kill_all(&mut self) {
        let ids: Vec<u32> = self.running.keys().copied().collect();
        for id in ids {
            self.kill(id);
        }
    }

    pub fn kill(&mut self, id: u32) {
        let Some(mut entry) = self.running.remove(&id) else {
            return;
        };
        drop(entry.stdin.take());
        #[cfg(windows)]
        {
            if let Some(job) = entry.job.take() {
                job.terminate_and_close();
            }
            // Sweep any process or child still lingering under this PID (fast pure-Win32 snapshot).
            terminate_process_tree(entry.pid);
        }
        #[cfg(not(windows))]
        let _ = command("kill")
            .args(["-9", &entry.pid.to_string()])
            .status();
    }
}

fn pump(
    tx: UnboundedSender<ProcEvent>,
    id: u32,
    err: bool,
    mut pipe: impl Read + Send + 'static,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 16384];
        let mut carry: Vec<u8> = Vec::new();
        loop {
            let n = match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if carry.is_empty() {
                if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                    if tx
                        .unbounded_send(ProcEvent::Output {
                            id,
                            text: s.to_owned(),
                            err,
                        })
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }
            }
            carry.extend_from_slice(&buf[..n]);
            let valid = match std::str::from_utf8(&carry) {
                Ok(_) => carry.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => carry.len(),
            };
            let text = String::from_utf8_lossy(&carry[..valid]).into_owned();
            if valid == carry.len() {
                carry.clear();
                if carry.capacity() > 32_768 {
                    carry.shrink_to(4096);
                }
            } else {
                carry.drain(..valid);
            }
            if tx
                .unbounded_send(ProcEvent::Output { id, text, err })
                .is_err()
            {
                break;
            }
        }
        if !carry.is_empty() {
            let _ = tx.unbounded_send(ProcEvent::Output {
                id,
                text: String::from_utf8_lossy(&carry).into_owned(),
                err,
            });
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
    fn isolated_job_terminates_spawned_process_and_children() {
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn GetExitCodeProcess(proc: isize, code: *mut u32) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        let is_alive = |pid: u32| -> bool {
            unsafe {
                let h = OpenProcess(0x0400, 0, pid); // PROCESS_QUERY_INFORMATION
                if h == 0 {
                    return false;
                }
                let mut code = 0u32;
                let ok = GetExitCodeProcess(h, &mut code) != 0;
                CloseHandle(h);
                ok && code == 259 // STILL_ACTIVE
            }
        };
        let (mut procs, _rx) = super::Procs::new();
        let mut cmd = super::command("cmd.exe");
        cmd.args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]);
        let id = procs
            .spawn_isolated(cmd, None, "test-agent")
            .expect("spawn isolated");
        let entry = procs.running.get(&id).expect("entry tracked");
        assert!(
            entry.job.is_some(),
            "isolated agent must hold a Windows Job Object"
        );
        let pid = entry.pid;
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert!(is_alive(pid), "resumed process should be running");
        let ws_before = procs.job_working_set_mb(id);
        assert!(ws_before > 0.0, "job working set should report positive MB");
        assert!(
            procs.cool_job(id, true, Some(true)) >= 1,
            "cooling should touch job processes"
        );
        procs.wake_job(id);
        assert!(
            is_alive(pid),
            "cooling and waking must keep process 100% alive"
        );
        procs.kill(id);
        assert!(!procs.is_running(id));
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert!(!is_alive(pid), "terminated job process must not linger");
    }
}
