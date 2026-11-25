
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    StopAndQuit,
}

#[cfg(windows)]
mod bg {
    use super::TrayAction;
    use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::Mutex;

    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
    static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
    static TRAY_TX: Mutex<Option<UnboundedSender<TrayAction>>> = Mutex::new(None);
    static TRAY_LABELS: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

    const GW_OWNER: u32 = 4;
    const SW_HIDE: i32 = 0;
    const SW_SHOW: i32 = 5;
    const SW_RESTORE: i32 = 9;

    const WM_APP: u32 = 0x8000;
    const WM_TRAY: u32 = WM_APP + 1;
    const WM_CLOSE: u32 = 0x0010;
    const WM_DESTROY: u32 = 0x0002;
    const WM_COMMAND: u32 = 0x0111;
    const WM_LBUTTONUP: usize = 0x0202;
    const WM_LBUTTONDBLCLK: usize = 0x0203;
    const WM_RBUTTONUP: usize = 0x0205;
    const WM_CONTEXTMENU: usize = 0x007B;
    const NIN_BALLOONUSERCLICK: usize = 0x0405;

    const NIM_ADD: u32 = 0x0000_0000;
    const NIM_MODIFY: u32 = 0x0000_0001;
    const NIM_DELETE: u32 = 0x0000_0002;
    const NIF_MESSAGE: u32 = 0x0000_0001;
    const NIF_ICON: u32 = 0x0000_0002;
    const NIF_TIP: u32 = 0x0000_0004;
    const NIF_INFO: u32 = 0x0000_0010;
    const NIIF_INFO: u32 = 0x0000_0001;
    const MF_STRING: u32 = 0x0000_0000;
    const MF_SEPARATOR: u32 = 0x0000_0800;
    const TPM_RIGHTBUTTON: u32 = 0x0002;
    const TPM_BOTTOMALIGN: u32 = 0x0020;

    const ID_SHOW: usize = 1001;
    const ID_QUIT: usize = 1002;
    const TRAY_UID: u32 = 1;
    const HWND_MESSAGE: isize = -3;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        lprivate: u32,
    }

    #[repr(C)]
    struct WndClassW {
        style: u32,
        wnd_proc: extern "system" fn(isize, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    struct NotifyIconDataW {
        cb_size: u32,
        hwnd: isize,
        u_id: u32,
        u_flags: u32,
        u_callback_message: u32,
        h_icon: isize,
        sz_tip: [u16; 128],
        dw_state: u32,
        dw_state_mask: u32,
        sz_info: [u16; 256],
        u_timeout_or_version: u32,
        sz_info_title: [u16; 64],
        dw_info_flags: u32,
        guid_item: [u32; 4],
        h_balloon_icon: isize,
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumThreadWindows(thread: u32, f: extern "system" fn(isize, isize) -> i32, data: isize) -> i32;
        fn IsWindow(hwnd: isize) -> i32;
        fn IsWindowVisible(hwnd: isize) -> i32;
        fn IsIconic(hwnd: isize) -> i32;
        fn GetWindow(hwnd: isize, cmd: u32) -> isize;
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn GetCursorPos(p: *mut Point) -> i32;
        fn RegisterClassW(wc: *const WndClassW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class_name: *const u16,
            window_name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *const std::ffi::c_void,
        ) -> isize;
        fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn PostQuitMessage(code: i32);
        fn DestroyWindow(hwnd: isize) -> i32;
        fn LoadIconW(instance: isize, name: *const u16) -> isize;
        fn CreatePopupMenu() -> isize;
        fn AppendMenuW(menu: isize, flags: u32, id: usize, item: *const u16) -> i32;
        fn TrackPopupMenu(menu: isize, flags: u32, x: i32, y: i32, reserved: i32, hwnd: isize, rect: *const std::ffi::c_void) -> i32;
        fn DestroyMenu(menu: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;
    }

    fn wide_buf<const N: usize>(s: &str) -> [u16; N] {
        let mut out = [0u16; N];
        for (dst, ch) in out.iter_mut().take(N.saturating_sub(1)).zip(s.encode_utf16()) {
            *dst = ch;
        }
        out
    }

    fn wide_z(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn app_icon() -> isize {
        unsafe {
            let hmod = GetModuleHandleW(std::ptr::null());
            let icon = LoadIconW(hmod, std::ptr::without_provenance(1));
            if icon != 0 {
                icon
            } else {
                LoadIconW(0, std::ptr::without_provenance(32512)) // IDI_APPLICATION
            }
        }
    }

    extern "system" fn pick_main(hwnd: isize, out: isize) -> i32 {
        unsafe {
            if IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER) == 0 {
                *(out as *mut isize) = hwnd;
                return 0;
            }
        }
        1
    }

    pub fn capture_main_hwnd() -> isize {
        unsafe {
            let mut hwnd: isize = 0;
            EnumThreadWindows(GetCurrentThreadId(), pick_main, &mut hwnd as *mut isize as isize);
            if hwnd != 0 {
                MAIN_HWND.store(hwnd, Ordering::Relaxed);
                return hwnd;
            }
            let saved = MAIN_HWND.load(Ordering::Relaxed);
            if saved != 0 && IsWindow(saved) != 0 { saved } else { 0 }
        }
    }

    pub fn hide_main_window() -> bool {
        let hwnd = capture_main_hwnd();
        if hwnd == 0 {
            return false;
        }
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        true
    }

    pub fn restore_main_window() -> bool {
        let hwnd = MAIN_HWND.load(Ordering::Relaxed);
        if hwnd == 0 {
            return false;
        }
        unsafe {
            if IsWindow(hwnd) == 0 {
                return false;
            }
            ShowWindow(hwnd, SW_SHOW);
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
        true
    }

    fn build_nid(hwnd: isize, tooltip: &str, balloon_title: Option<&str>, balloon_body: Option<&str>) -> NotifyIconDataW {
        let mut nid: NotifyIconDataW = unsafe { std::mem::zeroed() };
        nid.cb_size = std::mem::size_of::<NotifyIconDataW>() as u32;
        nid.hwnd = hwnd;
        nid.u_id = TRAY_UID;
        nid.u_flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.u_callback_message = WM_TRAY;
        nid.h_icon = app_icon();
        nid.sz_tip = wide_buf::<128>(tooltip);
        if let (Some(title), Some(body)) = (balloon_title, balloon_body) {
            nid.u_flags |= NIF_INFO;
            nid.sz_info_title = wide_buf::<64>(title);
            nid.sz_info = wide_buf::<256>(body);
            nid.dw_info_flags = NIIF_INFO;
        }
        nid
    }

    fn emit(action: TrayAction) {
        if let Ok(g) = TRAY_TX.lock() {
            if let Some(tx) = g.as_ref() {
                let _ = tx.unbounded_send(action);
            }
        }
    }

    extern "system" fn tray_wnd_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            WM_TRAY => {
                let ev = (lparam as usize) & 0xffff;
                match ev {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_BALLOONUSERCLICK => emit(TrayAction::Show),
                    WM_RBUTTONUP | WM_CONTEXTMENU => unsafe {
                        let menu = CreatePopupMenu();
                        if menu != 0 {
                            let (open_lbl, quit_lbl) = TRAY_LABELS
                                .lock()
                                .map(|g| (g.0.clone(), g.1.clone()))
                                .unwrap_or_else(|_| ("Open FluxCode".into(), "Stop AI Agent & Exit".into()));
                            let open_w = wide_z(&open_lbl);
                            let quit_w = wide_z(&quit_lbl);
                            AppendMenuW(menu, MF_STRING, ID_SHOW, open_w.as_ptr());
                            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                            AppendMenuW(menu, MF_STRING, ID_QUIT, quit_w.as_ptr());
                            let mut pt = Point { x: 0, y: 0 };
                            GetCursorPos(&mut pt);
                            SetForegroundWindow(hwnd);
                            TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
                            DestroyMenu(menu);
                        }
                    },
                    _ => {}
                }
                0
            }
            WM_COMMAND => {
                match wparam & 0xffff {
                    ID_SHOW => emit(TrayAction::Show),
                    ID_QUIT => emit(TrayAction::StopAndQuit),
                    _ => {}
                }
                0
            }
            WM_CLOSE => unsafe {
                let nid = build_nid(hwnd, "", None, None);
                Shell_NotifyIconW(NIM_DELETE, &nid);
                DestroyWindow(hwnd);
                0
            },
            WM_DESTROY => unsafe {
                PostQuitMessage(0);
                0
            },
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    pub fn tray_show(
        tooltip: &str,
        balloon_title: Option<&str>,
        balloon_body: Option<&str>,
        open_label: &str,
        quit_label: &str,
    ) -> Option<UnboundedReceiver<TrayAction>> {
        tray_hide();
        let (tx, rx) = unbounded();
        if let Ok(mut g) = TRAY_TX.lock() {
            *g = Some(tx);
        }
        if let Ok(mut g) = TRAY_LABELS.lock() {
            *g = (open_label.to_string(), quit_label.to_string());
        }
        let tip = tooltip.to_string();
        let b_title = balloon_title.map(str::to_string);
        let b_body = balloon_body.map(str::to_string);
        std::thread::Builder::new()
            .name("fluxcode-tray".into())
            .spawn(move || unsafe {
                let hinst = GetModuleHandleW(std::ptr::null());
                let class_w = wide_z("FluxCodeTrayHost");
                let wc = WndClassW {
                    style: 0,
                    wnd_proc: tray_wnd_proc,
                    cls_extra: 0,
                    wnd_extra: 0,
                    instance: hinst,
                    icon: 0,
                    cursor: 0,
                    background: 0,
                    menu_name: std::ptr::null(),
                    class_name: class_w.as_ptr(),
                };
                RegisterClassW(&wc);
                let title_w = wide_z("FluxCode");
                let hwnd = CreateWindowExW(
                    0,
                    class_w.as_ptr(),
                    title_w.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    0,
                    hinst,
                    std::ptr::null(),
                );
                if hwnd == 0 {
                    return;
                }
                TRAY_HWND.store(hwnd, Ordering::Relaxed);
                let nid = build_nid(hwnd, &tip, b_title.as_deref(), b_body.as_deref());
                Shell_NotifyIconW(NIM_ADD, &nid);
                let mut msg: Msg = std::mem::zeroed();
                while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                TRAY_HWND.compare_exchange(hwnd, 0, Ordering::Relaxed, Ordering::Relaxed).ok();
            })
            .ok()?;
        Some(rx)
    }

    pub fn tray_update(tooltip: &str, balloon_title: Option<&str>, balloon_body: Option<&str>) {
        let hwnd = TRAY_HWND.load(Ordering::Relaxed);
        if hwnd == 0 {
            return;
        }
        let nid = build_nid(hwnd, tooltip, balloon_title, balloon_body);
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn tray_hide() {
        let hwnd = TRAY_HWND.swap(0, Ordering::Relaxed);
        if hwnd != 0 {
            unsafe {
                let nid = build_nid(hwnd, "", None, None);
                Shell_NotifyIconW(NIM_DELETE, &nid);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        if let Ok(mut g) = TRAY_TX.lock() {
            *g = None;
        }
    }
}

#[cfg(windows)]
pub use bg::{capture_main_hwnd, hide_main_window, restore_main_window, tray_hide, tray_show, tray_update};

#[cfg(not(windows))]
pub fn capture_main_hwnd() -> isize {
    0
}
#[cfg(not(windows))]
pub fn hide_main_window() -> bool {
    false
}
#[cfg(not(windows))]
pub fn restore_main_window() -> bool {
    false
}
#[cfg(not(windows))]
pub fn tray_show(
    _tooltip: &str,
    _balloon_title: Option<&str>,
    _balloon_body: Option<&str>,
    _open_label: &str,
    _quit_label: &str,
) -> Option<futures::channel::mpsc::UnboundedReceiver<TrayAction>> {
    None
}
#[cfg(not(windows))]
pub fn tray_update(_tooltip: &str, _balloon_title: Option<&str>, _balloon_body: Option<&str>) {}
#[cfg(not(windows))]
pub fn tray_hide() {}
