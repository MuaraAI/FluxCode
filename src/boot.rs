//! Start-up timing. With `FLUXCODE_STARTUP_LOG=<file>` set, FluxCode writes how many
//! milliseconds after `main` each start-up step finished, once the first frame is
//! drawn. Without it, `mark` only checks one atomic flag.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static ON: AtomicBool = AtomicBool::new(false);
static MARKS: Mutex<Vec<(&'static str, f64)>> = Mutex::new(Vec::new());

/// Call first thing in `main`.
pub fn start() {
    START.get_or_init(Instant::now);
    ON.store(std::env::var_os("FLUXCODE_STARTUP_LOG").is_some(), Ordering::Relaxed);
}

pub fn mark(label: &'static str) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let ms = START.get().map_or(0., |s| s.elapsed().as_secs_f64() * 1000.);
    if let Ok(mut m) = MARKS.lock() {
        m.push((label, ms));
    }
}

/// Logging is on and the first frame has not been written yet.
pub fn pending() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Write the marks (once), after the first frame.
pub fn flush() {
    if !ON.swap(false, Ordering::Relaxed) {
        return;
    }
    let Some(path) = std::env::var_os("FLUXCODE_STARTUP_LOG") else { return };
    let text: String = MARKS.lock().map(|m| m.iter().map(|(l, ms)| format!("{ms:8.1} ms  {l}\n")).collect()).unwrap_or_default();
    let _ = std::fs::write(path, text);
}
