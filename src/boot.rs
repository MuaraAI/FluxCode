//! Start-up timing and memory. With `FLUXCODE_STARTUP_LOG=<file>` set, FluxCode writes
//! how many milliseconds after `main` each start-up step finished, with the process's
//! working set and private memory at that moment, once the first frame is drawn (and
//! again 5 s later, idle). Without it, `mark` only checks one atomic flag.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static ON: AtomicBool = AtomicBool::new(false);
static MARKS: Mutex<Vec<(&'static str, f64, (f64, f64))>> = Mutex::new(Vec::new());

/// Call first thing in `main`.
pub fn start() {
    START.get_or_init(Instant::now);
    ON.store(std::env::var_os("FLUXCODE_STARTUP_LOG").is_some(), Ordering::Relaxed);
}

/// (working set, private bytes) of this process, in MB.
#[cfg(windows)]
pub fn memory_mb() -> (f64, f64) {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_nonpaged: usize,
        quota_nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    // SAFETY: a plain out-struct of the documented PROCESS_MEMORY_COUNTERS_EX layout.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } != 0;
    if !ok {
        return (0., 0.);
    }
    (c.working_set as f64 / 1_048_576., c.private as f64 / 1_048_576.)
}

#[cfg(not(windows))]
pub fn memory_mb() -> (f64, f64) {
    // VmRSS and RssAnon from /proc (Linux).
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |key: &str| s.lines().find(|l| l.starts_with(key)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.) / 1024.;
    (kb("VmRSS:"), kb("RssAnon:"))
}

pub fn mark(label: &'static str) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let ms = START.get().map_or(0., |s| s.elapsed().as_secs_f64() * 1000.);
    if let Ok(mut m) = MARKS.lock() {
        m.push((label, ms, memory_mb()));
    }
}

/// Logging is on and the first frame has not been written yet.
pub fn pending() -> bool {
    ON.load(Ordering::Relaxed)
}

fn write() {
    let Some(path) = std::env::var_os("FLUXCODE_STARTUP_LOG") else { return };
    let text: String = MARKS
        .lock()
        .map(|m| m.iter().map(|(l, ms, (ws, pv))| format!("{ms:8.1} ms  ws {ws:6.1} MB  private {pv:6.1} MB  {l}\n")).collect())
        .unwrap_or_default();
    let _ = std::fs::write(path, text);
}

/// Write the marks after the first frame, then once more 5 s later (idle memory).
pub fn flush() {
    if !ON.swap(false, Ordering::Relaxed) {
        return;
    }
    write();
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let ms = START.get().map_or(0., |s| s.elapsed().as_secs_f64() * 1000.);
        if let Ok(mut m) = MARKS.lock() {
            m.push(("idle, 5 s after the first frame", ms, memory_mb()));
        }
        write();
    });
}
