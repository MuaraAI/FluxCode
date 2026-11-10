//! Give memory FluxCode is not using back to the system.
//!
//! - 6 s after start: the pages only start-up touched (font enumeration, shader and
//!   driver set-up, the first layout) leave the working set.
//! - After the window has been in the background for 15 s: the rest of the working
//!   set goes too, so a FluxCode left open behind other windows holds almost no RAM.
//!
//! Nothing is freed that FluxCode still holds: Windows moves the pages to its standby
//! list and faults them back in, a few milliseconds' worth, when they are used again.
//! On Linux the allocator returns its free heap pages (glibc `malloc_trim`).

use gpui_kit::{AsyncWindowContext, Context, Window};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Bumped on every activation change, so a pending background trim knows the window
/// came back in the meantime.
static GEN: AtomicU64 = AtomicU64::new(0);

const AFTER_START: Duration = Duration::from_secs(6);
const IN_BACKGROUND: Duration = Duration::from_secs(15);

/// Release what the process is not using right now.
pub fn trim() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn K32EmptyWorkingSet(process: isize) -> i32;
        }
        // SAFETY: both take and return plain handles / flags.
        unsafe {
            K32EmptyWorkingSet(GetCurrentProcess());
        }
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: glibc's documented call; it only returns free heap memory.
        unsafe {
            malloc_trim(0);
        }
    }
}

/// Start both trims for the workbench window.
pub fn watch<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    cx.spawn_in(window, async move |_, cx: &mut AsyncWindowContext| {
        cx.background_executor().timer(AFTER_START).await;
        trim();
    })
    .detach();
    cx.observe_window_activation(window, |_, window, cx| {
        let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
        if window.is_window_active() {
            return;
        }
        cx.spawn_in(window, async move |_, cx: &mut AsyncWindowContext| {
            cx.background_executor().timer(IN_BACKGROUND).await;
            if GEN.load(Ordering::Relaxed) == gen {
                trim();
            }
        })
        .detach();
    })
    .detach();
}
