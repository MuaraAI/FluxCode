
use gpui_kit::{AsyncWindowContext, Context, Window};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static GEN: AtomicU64 = AtomicU64::new(0);

const AFTER_START: Duration = Duration::from_secs(6);
const IN_BACKGROUND: Duration = Duration::from_secs(15);

pub fn trim() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn K32EmptyWorkingSet(process: isize) -> i32;
        }
        unsafe {
            K32EmptyWorkingSet(GetCurrentProcess());
        }
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        unsafe {
            malloc_trim(0);
        }
    }
}

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
