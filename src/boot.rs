use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static ON: AtomicBool = AtomicBool::new(false);
static MARKS: Mutex<Vec<(&'static str, f64, (f64, f64))>> = Mutex::new(Vec::new());

pub fn start() {
    START.get_or_init(Instant::now);
    ON.store(
        std::env::var_os("FLUXCODE_STARTUP_LOG").is_some(),
        Ordering::Relaxed,
    );
}

#[cfg(windows)]
fn process_memory_counters(handle: isize) -> Option<(usize, usize)> {
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
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        ..Default::default()
    };
    (unsafe { K32GetProcessMemoryInfo(handle, &mut c, c.cb) } != 0)
        .then_some((c.working_set, c.private))
}

#[cfg(windows)]
pub(crate) fn handle_working_set_bytes(handle: isize) -> Option<usize> {
    process_memory_counters(handle).map(|(ws, _)| ws)
}

pub fn total_ram_mb() -> f64 {
    static TOTAL: OnceLock<f64> = OnceLock::new();
    *TOTAL.get_or_init(|| {
        #[cfg(windows)]
        {
            #[repr(C)]
            struct MemoryStatusEx {
                dw_length: u32,
                dw_memory_load: u32,
                ull_total_phys: u64,
                ull_avail_phys: u64,
                ull_total_page_file: u64,
                ull_avail_page_file: u64,
                ull_total_virtual: u64,
                ull_avail_virtual: u64,
                ull_avail_extended_virtual: u64,
            }
            #[link(name = "kernel32")]
            extern "system" {
                fn GlobalMemoryStatusEx(lp_buffer: *mut MemoryStatusEx) -> i32;
            }
            let mut st: MemoryStatusEx = unsafe { std::mem::zeroed() };
            st.dw_length = std::mem::size_of::<MemoryStatusEx>() as u32;
            if unsafe { GlobalMemoryStatusEx(&mut st) } != 0 && st.ull_total_phys > 0 {
                return st.ull_total_phys as f64 / 1_048_576.0;
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
                if let Some(kb) = info
                    .lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<f64>().ok())
                {
                    if kb > 0.0 {
                        return kb / 1024.0;
                    }
                }
            }
        }
        8192.0
    })
}

#[cfg(windows)]
pub fn memory_mb() -> (f64, f64) {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
    }
    match process_memory_counters(unsafe { GetCurrentProcess() }) {
        Some((ws, priv_b)) => (ws as f64 / 1_048_576., priv_b as f64 / 1_048_576.),
        None => (0., 0.),
    }
}

#[cfg(not(windows))]
pub fn memory_mb() -> (f64, f64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |key: &str| {
        s.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.)
            / 1024.
    };
    (kb("VmRSS:"), kb("RssAnon:"))
}

pub fn mark(label: &'static str) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let ms = START
        .get()
        .map_or(0., |s| s.elapsed().as_secs_f64() * 1000.);
    if let Ok(mut m) = MARKS.lock() {
        m.push((label, ms, memory_mb()));
    }
}

pub fn pending() -> bool {
    ON.load(Ordering::Relaxed)
}

fn write() {
    let Some(path) = std::env::var_os("FLUXCODE_STARTUP_LOG") else {
        return;
    };
    let text: String = MARKS
        .lock()
        .map(|m| {
            m.iter()
                .map(|(l, ms, (ws, pv))| {
                    format!("{ms:8.1} ms  ws {ws:6.1} MB  private {pv:6.1} MB  {l}\n")
                })
                .collect()
        })
        .unwrap_or_default();
    let _ = std::fs::write(path, text);
}

pub fn flush() {
    if !ON.swap(false, Ordering::Relaxed) {
        return;
    }
    write();
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let ms = START
            .get()
            .map_or(0., |s| s.elapsed().as_secs_f64() * 1000.);
        if let Ok(mut m) = MARKS.lock() {
            m.push(("idle, 5 s after the first frame", ms, memory_mb()));
        }
        write();
    });
}
