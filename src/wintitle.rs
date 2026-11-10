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

/// Ask for the user's attention while FluxCode is in the background: the taskbar button
/// flashes until the window comes to the front (an agent waiting for a permission).
/// Nothing happens when FluxCode is already the window in front.
#[cfg(windows)]
pub fn flash_attention() {
    #[repr(C)]
    struct FlashInfo {
        size: u32,
        hwnd: isize,
        flags: u32,
        count: u32,
        timeout: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn EnumThreadWindows(thread: u32, f: extern "system" fn(isize, isize) -> i32, data: isize) -> i32;
        fn IsWindowVisible(hwnd: isize) -> i32;
        fn GetWindow(hwnd: isize, cmd: u32) -> isize;
        fn GetForegroundWindow() -> isize;
        fn FlashWindowEx(info: *const FlashInfo) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    const GW_OWNER: u32 = 4;
    const FLASHW_ALL: u32 = 3;
    const FLASHW_TIMERNOFG: u32 = 12;
    // The UI thread's visible top-level window: the workbench.
    extern "system" fn pick(hwnd: isize, out: isize) -> i32 {
        unsafe {
            if IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER) == 0 {
                *(out as *mut isize) = hwnd;
                return 0;
            }
        }
        1
    }
    unsafe {
        let mut hwnd: isize = 0;
        EnumThreadWindows(GetCurrentThreadId(), pick, &mut hwnd as *mut isize as isize);
        if hwnd == 0 || GetForegroundWindow() == hwnd {
            return;
        }
        let info = FlashInfo { size: std::mem::size_of::<FlashInfo>() as u32, hwnd, flags: FLASHW_ALL | FLASHW_TIMERNOFG, count: 0, timeout: 0 };
        FlashWindowEx(&info);
    }
}

#[cfg(not(windows))]
pub fn flash_attention() {}
