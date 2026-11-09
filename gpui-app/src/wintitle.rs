//! Windows title bar behavior that does not depend on GPUI's window-control
//! hit areas.
//!
//! Why: with GPUI Fast, a frame that reuses the workbench's cached paint (for
//! example when only the editor's caret blinks) keeps normal hitboxes and click
//! listeners but drops the min/max/close/drag "window control" areas. The X
//! button then needs a first click to repaint before a second one closes. So on
//! Windows the buttons are plain click handlers, and dragging hands the move to
//! Windows the classic way: release the capture and post WM_NCLBUTTONDOWN with
//! HTCAPTION, which also keeps Aero Snap.

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering};

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetActiveWindow() -> isize;
        fn GetCursorPos(p: *mut Point) -> i32;
        fn ReleaseCapture() -> i32;
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    }

    const WM_NCLBUTTONDOWN: u32 = 0x00A1;
    const HTCAPTION: usize = 2;

    /// When we posted the move (ms since start); the mouse-down GPUI reports for our
    /// own posted message must not start another one.
    static POSTED_AT: AtomicU64 = AtomicU64::new(0);

    fn now_ms() -> u64 {
        use std::sync::OnceLock;
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64 + 1
    }

    /// Start moving the window with the mouse (call from a left mouse-down on the bar).
    pub fn begin_move() {
        let posted = POSTED_AT.swap(0, Ordering::Relaxed);
        if posted != 0 && now_ms().saturating_sub(posted) < 500 {
            return; // this is the echo of our own WM_NCLBUTTONDOWN
        }
        unsafe {
            let hwnd = GetActiveWindow();
            if hwnd == 0 {
                return;
            }
            let mut p = Point { x: 0, y: 0 };
            if GetCursorPos(&mut p) == 0 {
                return;
            }
            ReleaseCapture();
            POSTED_AT.store(now_ms(), Ordering::Relaxed);
            let lparam = ((p.y as u32 & 0xffff) << 16 | (p.x as u32 & 0xffff)) as i32 as isize;
            PostMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, lparam);
        }
    }
}

#[cfg(windows)]
pub use imp::begin_move;

#[cfg(not(windows))]
pub fn begin_move() {}
