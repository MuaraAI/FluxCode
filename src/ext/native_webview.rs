use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Bounded pool capacity sized via Little's Law (L = lambda * W) for 1-2 active UIs + standby.
pub const MAX_LIVE_CONTROLLERS: usize = 3;
/// Denning Working Set cooling window (seconds hidden before calling TrySuspend).
pub const SUSPEND_AFTER_SECS: u64 = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PhysicalRect {
    /// Edge-quantized physical pixel bounds (Dirty Rectangle, Foley et al. 1990).
    pub fn from_logical(x: f32, y: f32, w: f32, h: f32, scale: f32) -> Self {
        let s = if scale.is_finite() && scale > 0. {
            scale
        } else {
            1.
        };
        let x0 = (x * s).round() as i32;
        let y0 = (y * s).round() as i32;
        let x1 = ((x + w.max(0.)) * s).round() as i32;
        let y1 = ((y + h.max(0.)) * s).round() as i32;
        Self {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0),
            h: (y1 - y0).max(0),
        }
    }

    pub fn is_empty(self) -> bool {
        self.w <= 1 || self.h <= 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWebviewState {
    Loading,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub struct FrameSyncOutcome {
    pub evicted_handles: Vec<String>,
    pub visibility_changes: Vec<(String, bool)>,
}

/// Corbato 16-bit page-aging LRU + Denning Working Set policy for live WebView2 controllers.
#[derive(Clone, Debug)]
struct PoolEntry {
    key: String,
    age: u16,
    referenced: bool,
    visible: bool,
    suspended: bool,
    hidden_since: Option<Instant>,
}

#[derive(Default, Debug)]
pub struct WebviewPoolPolicy {
    entries: Vec<PoolEntry>,
}

impl WebviewPoolPolicy {
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn touch(&mut self, key: &str, now: Instant) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key) {
            e.referenced = true;
            e.visible = true;
            e.suspended = false;
            e.hidden_since = None;
        } else {
            let _ = now;
            self.entries.push(PoolEntry {
                key: key.to_string(),
                age: 1u16 << 15,
                referenced: true,
                visible: true,
                suspended: false,
                hidden_since: None,
            });
        }
    }

    /// 16-bit aging shift register step: A_i(t) = (A_i(t-1) >> 1) | (R_i << 15).
    pub fn age_tick(&mut self) {
        for e in &mut self.entries {
            e.age = (e.age >> 1) | if e.referenced { 1u16 << 15 } else { 0 };
            e.referenced = false;
        }
    }

    pub fn mark_hidden(&mut self, key: &str, now: Instant) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key) {
            e.visible = false;
            if e.hidden_since.is_none() {
                e.hidden_since = Some(now);
            }
        }
    }

    pub fn mark_suspended(&mut self, key: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key) {
            e.suspended = true;
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|e| e.key != key);
    }

    pub fn pick_eviction_victim(&self, keep: &[&str]) -> Option<String> {
        if self.entries.len() < MAX_LIVE_CONTROLLERS {
            return None;
        }
        self.entries
            .iter()
            .filter(|e| !e.visible && !keep.contains(&e.key.as_str()))
            .min_by_key(|e| (e.age, e.hidden_since))
            .map(|e| e.key.clone())
    }

    pub fn collect_suspend_candidates(&self, now: Instant, window_active: bool) -> Vec<String> {
        let tau = Duration::from_secs(SUSPEND_AFTER_SECS);
        self.entries
            .iter()
            .filter(|e| !e.suspended)
            .filter(|e| {
                if !window_active {
                    true
                } else {
                    !e.visible
                        && e.hidden_since
                            .is_some_and(|t| now.saturating_duration_since(t) >= tau)
                }
            })
            .map(|e| e.key.clone())
            .collect()
    }
}

fn webview2_arch_dir() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    }
}

fn parse_version_tuple(name: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = name.split('.');
    let a = parts.next()?.parse::<u32>().ok()?;
    let b = parts.next()?.parse::<u32>().ok()?;
    let c = parts.next()?.parse::<u32>().ok()?;
    let d = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((a, b, c, d))
}

/// Scans an EdgeWebView Application root and picks the highest valid EmbeddedBrowserWebView.dll.
pub fn find_webview2_dll_in_dir(app_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(app_dir).ok()?;
    let arch = webview2_arch_dir();
    let mut best: Option<((u32, u32, u32, u32), PathBuf)> = None;
    for ent in entries.flatten() {
        let name = ent.file_name();
        let Some(ver) = name.to_str().and_then(parse_version_tuple) else {
            continue;
        };
        let dll = ent
            .path()
            .join("EBWebView")
            .join(arch)
            .join("EmbeddedBrowserWebView.dll");
        if dll.is_file() && best.as_ref().is_none_or(|(b_ver, _)| ver > *b_ver) {
            best = Some((ver, dll));
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(windows)]
mod win {
    use super::{
        find_webview2_dll_in_dir, webview2_arch_dir, FrameSyncOutcome, NativeWebviewState,
        PhysicalRect, WebviewPoolPolicy,
    };
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;
    use std::rc::{Rc, Weak};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::OnceLock;
    use std::time::Instant;

    const S_OK: i32 = 0;
    const E_NOINTERFACE: i32 = 0x8000_4002_u32 as i32;

    const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as isize;
    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as isize;
    const KEY_READ: u32 = 0x0002_0019;
    const KEY_WOW64_32KEY: u32 = 0x0200;
    const REG_SZ: u32 = 1;

    const WS_CHILD: u32 = 0x4000_0000;
    const WS_CLIPCHILDREN: u32 = 0x0200_0000;
    const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const SW_HIDE: i32 = 0;
    const SW_SHOWNORMAL: i32 = 1;
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    const fn guid_u128(v: u128) -> Guid {
        Guid {
            data1: (v >> 96) as u32,
            data2: ((v >> 80) & 0xFFFF) as u16,
            data3: ((v >> 64) & 0xFFFF) as u16,
            data4: (v as u64).to_be_bytes(),
        }
    }

    const IID_IUNKNOWN: Guid = guid_u128(0x00000000_0000_0000_c000_000000000046);
    const IID_ICOREWEBVIEW2_CONTROLLER2: Guid = guid_u128(0xc979903e_d4ca_4228_92eb_47ee3fa96eab);
    const IID_ICOREWEBVIEW2_3: Guid = guid_u128(0xa0d6df20_3b92_416d_aa0c_437a9c727857);
    const IID_ENV_COMPLETED: Guid = guid_u128(0x4e8a3389_c9d8_4bd2_b6b5_124fee6cc14d);
    const IID_CTRL_COMPLETED: Guid = guid_u128(0x6c4819f3_c9b7_4260_8127_c9f5bde7f68c);
    const IID_SUSPEND_COMPLETED: Guid = guid_u128(0x00f206a7_9d17_4605_91f6_4e8e4de192e3);
    const IID_NEW_WINDOW_HANDLER: Guid = guid_u128(0xd4c185fe_c81c_4989_97af_2d3fa7ab5651);

    #[repr(C)]
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CoreWebView2Color {
        a: u8,
        r: u8,
        g: u8,
        b: u8,
    }

    impl CoreWebView2Color {
        fn opaque_rgb(rgb: u32) -> Self {
            Self {
                a: 255,
                r: ((rgb >> 16) & 0xFF) as u8,
                g: ((rgb >> 8) & 0xFF) as u8,
                b: (rgb & 0xFF) as u8,
            }
        }
    }

    type CreateEnvInternalFn = unsafe extern "system" fn(
        check_running: u8,
        runtime_type: u32,
        user_data_dir: *const u16,
        env_options: *mut c_void,
        handler: *mut c_void,
    ) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(path: *const u16) -> isize;
        fn GetProcAddress(module: isize, proc_name: *const u8) -> *const c_void;
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            key: isize,
            sub_key: *const u16,
            options: u32,
            sam: u32,
            out: *mut isize,
        ) -> i32;
        fn RegQueryValueExW(
            key: isize,
            value_name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            cb_data: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
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
            param: *const c_void,
        ) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn IsWindow(hwnd: isize) -> i32;
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoTaskMemFree(pv: *mut c_void);
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: isize,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn read_wide_z(ptr: *const u16) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let mut len = 0usize;
        unsafe {
            while *ptr.add(len) != 0 && len < 8192 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            std::ffi::OsString::from_wide(slice)
                .to_string_lossy()
                .into_owned()
        }
    }

    fn read_reg_ebwebview(root: isize, subkey: &str, sam_extra: u32) -> Option<PathBuf> {
        let sub_w = wide(subkey);
        let val_w = wide("EBWebView");
        let mut hkey: isize = 0;
        let rc = unsafe { RegOpenKeyExW(root, sub_w.as_ptr(), 0, KEY_READ | sam_extra, &mut hkey) };
        if rc != 0 || hkey == 0 {
            return None;
        }
        let mut kind: u32 = 0;
        let mut bytes: u32 = 1024;
        let mut buf = vec![0u16; (bytes as usize) / 2];
        let qrc = unsafe {
            RegQueryValueExW(
                hkey,
                val_w.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                buf.as_mut_ptr() as *mut u8,
                &mut bytes,
            )
        };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        if qrc != 0 || kind != REG_SZ || bytes < 2 {
            return None;
        }
        let chars = (bytes as usize / 2).min(buf.len());
        let trimmed = buf[..chars].split(|&c| c == 0).next().unwrap_or(&[]);
        if trimmed.is_empty() {
            return None;
        }
        let folder = PathBuf::from(std::ffi::OsString::from_wide(trimmed));
        let dll = folder
            .join(webview2_arch_dir())
            .join("EmbeddedBrowserWebView.dll");
        dll.is_file().then_some(dll)
    }

    fn locate_webview2_dll() -> Option<PathBuf> {
        static CACHED: OnceLock<Option<PathBuf>> = OnceLock::new();
        CACHED
            .get_or_init(|| {
                let sub =
                    r"SOFTWARE\Microsoft\EdgeUpdate\ClientState\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
                read_reg_ebwebview(HKEY_LOCAL_MACHINE, sub, KEY_WOW64_32KEY)
                    .or_else(|| read_reg_ebwebview(HKEY_CURRENT_USER, sub, 0))
                    .or_else(|| {
                        let pf86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from)?;
                        find_webview2_dll_in_dir(
                            &pf86.join("Microsoft").join("EdgeWebView").join("Application"),
                        )
                    })
                    .or_else(|| {
                        let pf = std::env::var_os("ProgramFiles").map(PathBuf::from)?;
                        find_webview2_dll_in_dir(
                            &pf.join("Microsoft").join("EdgeWebView").join("Application"),
                        )
                    })
            })
            .clone()
    }

    fn ensure_host_window_class() -> Vec<u16> {
        static REGISTERED: OnceLock<bool> = OnceLock::new();
        let cls = wide("FluxCodeExtWebviewHost");
        REGISTERED.get_or_init(|| crate::wintitle::register_child_host_class(cls.as_ptr()));
        cls
    }

    /// Reference-counted raw COM interface wrapper with deterministic Release on Drop.
    struct ComPtr {
        ptr: *mut c_void,
    }

    impl ComPtr {
        unsafe fn from_raw_addref(raw: *mut c_void) -> Option<Self> {
            if raw.is_null() {
                return None;
            }
            let p = Self { ptr: raw };
            let add_ref: unsafe extern "system" fn(*mut c_void) -> u32 =
                unsafe { std::mem::transmute(p.slot(1)) };
            unsafe { add_ref(p.ptr) };
            Some(p)
        }

        unsafe fn from_raw_owned(raw: *mut c_void) -> Option<Self> {
            (!raw.is_null()).then_some(Self { ptr: raw })
        }

        fn as_raw(&self) -> *mut c_void {
            self.ptr
        }

        unsafe fn slot(&self, idx: usize) -> *const c_void {
            unsafe { *(*(self.ptr as *const *const *const c_void)).add(idx) }
        }

        fn query(&self, iid: &Guid) -> Option<ComPtr> {
            let mut out: *mut c_void = std::ptr::null_mut();
            let qi: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32 =
                unsafe { std::mem::transmute(self.slot(0)) };
            let hr = unsafe { qi(self.ptr, iid as *const Guid, &mut out) };
            if hr >= 0 {
                unsafe { Self::from_raw_owned(out) }
            } else {
                None
            }
        }
    }

    impl Drop for ComPtr {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe {
                    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
                        std::mem::transmute(self.slot(2));
                    release(self.ptr);
                }
            }
        }
    }

    #[repr(C)]
    struct CallbackVtbl {
        query_interface:
            unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        invoke: *const c_void,
    }

    // CallbackVtbl holds only immutable function pointers valid for the process lifetime.
    unsafe impl Sync for CallbackVtbl {}

    #[repr(C)]
    struct ComCallback<T> {
        vtbl: *const CallbackVtbl,
        iid: Guid,
        refs: AtomicU32,
        payload: T,
    }

    unsafe extern "system" fn cb_qi<T>(
        this: *mut c_void,
        riid: *const Guid,
        ppv: *mut *mut c_void,
    ) -> i32 {
        if ppv.is_null() || riid.is_null() {
            return E_NOINTERFACE;
        }
        let me = unsafe { &*(this as *const ComCallback<T>) };
        let wanted = unsafe { *riid };
        if wanted == IID_IUNKNOWN || wanted == me.iid {
            unsafe {
                *ppv = this;
            }
            cb_add_ref::<T>(this);
            S_OK
        } else {
            unsafe {
                *ppv = std::ptr::null_mut();
            }
            E_NOINTERFACE
        }
    }

    unsafe extern "system" fn cb_add_ref<T>(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const ComCallback<T>) };
        me.refs.fetch_add(1, Ordering::Relaxed) + 1
    }

    unsafe extern "system" fn cb_release<T>(this: *mut c_void) -> u32 {
        let me = unsafe { &*(this as *const ComCallback<T>) };
        let prev = me.refs.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            unsafe {
                drop(Box::from_raw(this as *mut ComCallback<T>));
            }
        }
        prev - 1
    }

    static ENV_VTBL: CallbackVtbl = CallbackVtbl {
        query_interface: cb_qi::<Weak<RefCell<Inner>>>,
        add_ref: cb_add_ref::<Weak<RefCell<Inner>>>,
        release: cb_release::<Weak<RefCell<Inner>>>,
        invoke: on_env_created as *const c_void,
    };
    static CTRL_VTBL: CallbackVtbl = CallbackVtbl {
        query_interface: cb_qi::<CtrlPayload>,
        add_ref: cb_add_ref::<CtrlPayload>,
        release: cb_release::<CtrlPayload>,
        invoke: on_ctrl_created as *const c_void,
    };
    static SUSPEND_VTBL: CallbackVtbl = CallbackVtbl {
        query_interface: cb_qi::<()>,
        add_ref: cb_add_ref::<()>,
        release: cb_release::<()>,
        invoke: on_try_suspend_done as *const c_void,
    };
    static NEW_WIN_VTBL: CallbackVtbl = CallbackVtbl {
        query_interface: cb_qi::<()>,
        add_ref: cb_add_ref::<()>,
        release: cb_release::<()>,
        invoke: on_new_window_requested as *const c_void,
    };

    fn alloc_callback<T>(vtbl: &'static CallbackVtbl, iid: Guid, payload: T) -> *mut c_void {
        Box::into_raw(Box::new(ComCallback {
            vtbl,
            iid,
            refs: AtomicU32::new(1),
            payload,
        })) as *mut c_void
    }

    unsafe extern "system" fn on_env_created(
        this: *mut c_void,
        hr: i32,
        env_raw: *mut c_void,
    ) -> i32 {
        let me = unsafe { &*(this as *const ComCallback<Weak<RefCell<Inner>>>) };
        if let Some(rc) = me.payload.upgrade() {
            let env = if hr >= 0 {
                unsafe { ComPtr::from_raw_addref(env_raw) }
            } else {
                None
            };
            let mut inner = rc.borrow_mut();
            inner.env_creating = false;
            if let Some(env_ptr) = env {
                inner.env = Some(env_ptr);
                let pending: Vec<String> = inner.pending_ctrl.drain(..).collect();
                drop(inner);
                for key in pending {
                    start_controller_for_slot(&rc, &key);
                }
            } else {
                inner.env_failed = true;
                let failed: Vec<String> = inner.pending_ctrl.drain(..).collect();
                for key in failed {
                    if let Some(s) = inner.slots.get_mut(&key) {
                        s.state = NativeWebviewState::Failed;
                    }
                }
            }
        }
        S_OK
    }

    struct CtrlPayload {
        inner: Weak<RefCell<Inner>>,
        key: String,
    }

    unsafe extern "system" fn on_ctrl_created(
        this: *mut c_void,
        hr: i32,
        ctrl_raw: *mut c_void,
    ) -> i32 {
        let me = unsafe { &*(this as *const ComCallback<CtrlPayload>) };
        if let Some(rc) = me.payload.inner.upgrade() {
            let ctrl = if hr >= 0 {
                unsafe { ComPtr::from_raw_addref(ctrl_raw) }
            } else {
                None
            };
            finish_controller_init(&rc, &me.payload.key, ctrl);
        }
        S_OK
    }

    unsafe extern "system" fn on_try_suspend_done(_this: *mut c_void, _hr: i32, _ok: i32) -> i32 {
        S_OK
    }

    unsafe extern "system" fn on_new_window_requested(
        _this: *mut c_void,
        _sender: *mut c_void,
        args_raw: *mut c_void,
    ) -> i32 {
        if let Some(args) = unsafe { ComPtr::from_raw_addref(args_raw) } {
            let mut uri_ptr: *mut u16 = std::ptr::null_mut();
            let get_uri: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> i32 =
                unsafe { std::mem::transmute(args.slot(3)) };
            if unsafe { get_uri(args.as_raw(), &mut uri_ptr) } >= 0 && !uri_ptr.is_null() {
                let uri = read_wide_z(uri_ptr);
                unsafe { CoTaskMemFree(uri_ptr as *mut c_void) };
                if uri.starts_with("http://") || uri.starts_with("https://") {
                    let set_handled: unsafe extern "system" fn(*mut c_void, i32) -> i32 =
                        unsafe { std::mem::transmute(args.slot(6)) };
                    unsafe {
                        let _ = set_handled(args.as_raw(), 1);
                    }
                    let op_w = wide("open");
                    let uri_w = wide(&uri);
                    unsafe {
                        let _ = ShellExecuteW(
                            0,
                            op_w.as_ptr(),
                            uri_w.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                            SW_SHOWNORMAL,
                        );
                    }
                }
            }
        }
        S_OK
    }

    fn ctrl_set_visible(ctrl: &ComPtr, visible: bool) {
        let f: unsafe extern "system" fn(*mut c_void, i32) -> i32 =
            unsafe { std::mem::transmute(ctrl.slot(4)) };
        unsafe {
            let _ = f(ctrl.as_raw(), if visible { 1 } else { 0 });
        }
    }

    fn ctrl_set_bounds(ctrl: &ComPtr, rect: Rect) {
        let f: unsafe extern "system" fn(*mut c_void, Rect) -> i32 =
            unsafe { std::mem::transmute(ctrl.slot(6)) };
        unsafe {
            let _ = f(ctrl.as_raw(), rect);
        }
    }

    fn ctrl_notify_parent_moved(ctrl: &ComPtr) {
        let f: unsafe extern "system" fn(*mut c_void) -> i32 =
            unsafe { std::mem::transmute(ctrl.slot(23)) };
        unsafe {
            let _ = f(ctrl.as_raw());
        }
    }

    fn ctrl_close(ctrl: &ComPtr) {
        let f: unsafe extern "system" fn(*mut c_void) -> i32 =
            unsafe { std::mem::transmute(ctrl.slot(24)) };
        unsafe {
            let _ = f(ctrl.as_raw());
        }
    }

    fn ctrl_get_webview2(ctrl: &ComPtr) -> Option<ComPtr> {
        let mut out: *mut c_void = std::ptr::null_mut();
        let f: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
            unsafe { std::mem::transmute(ctrl.slot(25)) };
        if unsafe { f(ctrl.as_raw(), &mut out) } >= 0 {
            unsafe { ComPtr::from_raw_owned(out) }
        } else {
            None
        }
    }

    fn ctrl2_set_bg_color(ctrl2: &ComPtr, rgb: u32) {
        let f: unsafe extern "system" fn(*mut c_void, CoreWebView2Color) -> i32 =
            unsafe { std::mem::transmute(ctrl2.slot(27)) };
        unsafe {
            let _ = f(ctrl2.as_raw(), CoreWebView2Color::opaque_rgb(rgb));
        }
    }

    fn webview_configure_settings(wv: &ComPtr) {
        let mut settings_raw: *mut c_void = std::ptr::null_mut();
        let get_settings: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
            unsafe { std::mem::transmute(wv.slot(3)) };
        if unsafe { get_settings(wv.as_raw(), &mut settings_raw) } < 0 {
            return;
        }
        let Some(settings) = (unsafe { ComPtr::from_raw_owned(settings_raw) }) else {
            return;
        };
        let put_bool = |slot_ix: usize, val: i32| {
            let f: unsafe extern "system" fn(*mut c_void, i32) -> i32 =
                unsafe { std::mem::transmute(settings.slot(slot_ix)) };
            unsafe {
                let _ = f(settings.as_raw(), val);
            }
        };
        put_bool(4, 1); // IsScriptEnabled
        put_bool(6, 1); // IsWebMessageEnabled
        put_bool(10, 0); // IsStatusBarEnabled
        put_bool(12, 1); // AreDevToolsEnabled
        put_bool(14, 1); // AreDefaultContextMenusEnabled
        put_bool(18, 0); // IsZoomControlEnabled
    }

    fn webview_navigate(wv: &ComPtr, url: &str) {
        let w = wide(url);
        let f: unsafe extern "system" fn(*mut c_void, *const u16) -> i32 =
            unsafe { std::mem::transmute(wv.slot(5)) };
        unsafe {
            let _ = f(wv.as_raw(), w.as_ptr());
        }
    }

    fn webview_reload(wv: &ComPtr) {
        let f: unsafe extern "system" fn(*mut c_void) -> i32 =
            unsafe { std::mem::transmute(wv.slot(31)) };
        unsafe {
            let _ = f(wv.as_raw());
        }
    }

    fn webview3_try_suspend(wv3: &ComPtr) {
        let cb = alloc_callback(&SUSPEND_VTBL, IID_SUSPEND_COMPLETED, ());
        let f: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32 =
            unsafe { std::mem::transmute(wv3.slot(68)) };
        unsafe {
            let _ = f(wv3.as_raw(), cb);
            cb_release::<()>(cb);
        }
    }

    fn webview3_resume(wv3: &ComPtr) {
        let f: unsafe extern "system" fn(*mut c_void) -> i32 =
            unsafe { std::mem::transmute(wv3.slot(69)) };
        unsafe {
            let _ = f(wv3.as_raw());
        }
    }

    struct WebviewSlot {
        hwnd: isize,
        url: String,
        navigated_url: String,
        controller: Option<ComPtr>,
        controller2: Option<ComPtr>,
        webview: Option<ComPtr>,
        webview3: Option<ComPtr>,
        new_win_token: i64,
        creating: bool,
        state: NativeWebviewState,
        want_rect: PhysicalRect,
        want_visible: bool,
        applied_rect: PhysicalRect,
        applied_visible: bool,
        applied_bg: Option<u32>,
    }

    impl WebviewSlot {
        fn destroy(&mut self) {
            if self.new_win_token != 0 {
                if let Some(wv) = &self.webview {
                    let remove_nw: unsafe extern "system" fn(*mut c_void, i64) -> i32 =
                        unsafe { std::mem::transmute(wv.slot(45)) };
                    unsafe {
                        let _ = remove_nw(wv.as_raw(), self.new_win_token);
                    }
                }
                self.new_win_token = 0;
            }
            if let Some(ctrl) = &self.controller {
                ctrl_set_visible(ctrl, false);
                ctrl_close(ctrl);
            }
            self.webview3 = None;
            self.webview = None;
            self.controller2 = None;
            self.controller = None;
            if self.hwnd != 0 {
                unsafe {
                    if IsWindow(self.hwnd) != 0 {
                        let _ = DestroyWindow(self.hwnd);
                    }
                }
                self.hwnd = 0;
            }
        }
    }

    struct Inner {
        env_creating: bool,
        env_failed: bool,
        env: Option<ComPtr>,
        pending_ctrl: VecDeque<String>,
        slots: HashMap<String, WebviewSlot>,
        pool: WebviewPoolPolicy,
        bg_rgb: u32,
        frame_id: u64,
        blocked: bool,
        parent_hwnd: isize,
        last_parent_pos: (i32, i32),
        parent_moved: bool,
        painted: Vec<(String, String, PhysicalRect)>,
        last_visible: HashMap<String, bool>,
    }

    impl Inner {
        fn shutdown(&mut self) {
            self.frame_id = 0;
            self.pending_ctrl.clear();
            self.painted.clear();
            for (_, mut slot) in self.slots.drain() {
                slot.destroy();
            }
            self.pool = WebviewPoolPolicy::default();
            self.last_visible.clear();
            self.env = None;
        }
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            self.shutdown();
        }
    }

    fn ensure_environment(rc: &Rc<RefCell<Inner>>) {
        let mut inner = rc.borrow_mut();
        if inner.env.is_some() || inner.env_creating || inner.env_failed {
            return;
        }
        let Some(dll_path) = locate_webview2_dll() else {
            inner.env_failed = true;
            return;
        };
        inner.env_creating = true;
        drop(inner);

        let dll_w = wide(&dll_path.to_string_lossy());
        let hmod = unsafe { LoadLibraryW(dll_w.as_ptr()) };
        if hmod == 0 {
            rc.borrow_mut().env_creating = false;
            rc.borrow_mut().env_failed = true;
            return;
        }
        let proc = unsafe {
            GetProcAddress(
                hmod,
                c"CreateWebViewEnvironmentWithOptionsInternal".as_ptr() as *const u8,
            )
        };
        if proc.is_null() {
            rc.borrow_mut().env_creating = false;
            rc.borrow_mut().env_failed = true;
            return;
        }
        let create_env: CreateEnvInternalFn = unsafe { std::mem::transmute(proc) };
        let data_dir = crate::settings::config_dir().join("webview2-ext");
        let _ = std::fs::create_dir_all(&data_dir);
        let dir_w = wide(&data_dir.to_string_lossy());
        let handler = alloc_callback(&ENV_VTBL, IID_ENV_COMPLETED, Rc::downgrade(rc));
        let hr = unsafe { create_env(1, 0, dir_w.as_ptr(), std::ptr::null_mut(), handler) };
        unsafe {
            cb_release::<Weak<RefCell<Inner>>>(handler);
        }
        if hr < 0 {
            let mut inn = rc.borrow_mut();
            inn.env_creating = false;
            inn.env_failed = true;
        }
    }

    fn start_controller_for_slot(rc: &Rc<RefCell<Inner>>, key: &str) {
        let (env_raw, hwnd) = {
            let mut inner = rc.borrow_mut();
            let Some(env_ptr) = inner.env.as_ref().map(ComPtr::as_raw) else {
                if !inner.pending_ctrl.iter().any(|k| k == key) {
                    inner.pending_ctrl.push_back(key.to_string());
                }
                return;
            };
            let Some(slot) = inner.slots.get_mut(key) else {
                return;
            };
            if slot.controller.is_some() || slot.creating || slot.hwnd == 0 {
                return;
            }
            slot.creating = true;
            (env_ptr, slot.hwnd)
        };
        let handler = alloc_callback(
            &CTRL_VTBL,
            IID_CTRL_COMPLETED,
            CtrlPayload {
                inner: Rc::downgrade(rc),
                key: key.to_string(),
            },
        );
        let create_ctrl: unsafe extern "system" fn(*mut c_void, isize, *mut c_void) -> i32 = unsafe {
            let vtbl = *(env_raw as *const *const *const c_void);
            std::mem::transmute(*vtbl.add(3))
        };
        let hr = unsafe { create_ctrl(env_raw, hwnd, handler) };
        unsafe {
            cb_release::<CtrlPayload>(handler);
        }
        if hr < 0 {
            if let Some(slot) = rc.borrow_mut().slots.get_mut(key) {
                slot.creating = false;
                slot.state = NativeWebviewState::Failed;
            }
        }
    }

    fn finish_controller_init(rc: &Rc<RefCell<Inner>>, key: &str, ctrl_opt: Option<ComPtr>) {
        let mut inner = rc.borrow_mut();
        let bg_rgb = inner.bg_rgb;
        let Some(slot) = inner.slots.get_mut(key) else {
            if let Some(ctrl) = &ctrl_opt {
                ctrl_close(ctrl);
            }
            return;
        };
        slot.creating = false;
        let Some(ctrl) = ctrl_opt else {
            slot.state = NativeWebviewState::Failed;
            return;
        };
        let ctrl2 = ctrl.query(&IID_ICOREWEBVIEW2_CONTROLLER2);
        if let Some(c2) = &ctrl2 {
            ctrl2_set_bg_color(c2, bg_rgb);
            slot.applied_bg = Some(bg_rgb);
        }
        let webview = ctrl_get_webview2(&ctrl);
        let webview3 = webview
            .as_ref()
            .and_then(|wv| wv.query(&IID_ICOREWEBVIEW2_3));
        if let Some(wv) = &webview {
            webview_configure_settings(wv);
            let nw_cb = alloc_callback(&NEW_WIN_VTBL, IID_NEW_WINDOW_HANDLER, ());
            let mut token: i64 = 0;
            let add_nw: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut i64) -> i32 =
                unsafe { std::mem::transmute(wv.slot(44)) };
            if unsafe { add_nw(wv.as_raw(), nw_cb, &mut token) } >= 0 {
                slot.new_win_token = token;
            }
            unsafe {
                cb_release::<()>(nw_cb);
            }
            if !slot.url.is_empty() {
                webview_navigate(wv, &slot.url);
                slot.navigated_url = slot.url.clone();
            }
        }
        slot.controller = Some(ctrl);
        slot.controller2 = ctrl2;
        slot.webview = webview;
        slot.webview3 = webview3;
        slot.state = NativeWebviewState::Ready;
        apply_slot_geometry(slot, bg_rgb, false);
    }

    fn apply_slot_geometry(slot: &mut WebviewSlot, bg_rgb: u32, force_pos_notify: bool) {
        if slot.hwnd == 0 {
            return;
        }
        let want_vis = slot.want_visible && !slot.want_rect.is_empty();
        if !want_vis {
            if slot.applied_visible {
                if let Some(ctrl) = &slot.controller {
                    ctrl_set_visible(ctrl, false);
                }
                unsafe {
                    let _ = ShowWindow(slot.hwnd, SW_HIDE);
                }
                slot.applied_visible = false;
            }
            return;
        }
        let r = slot.want_rect;
        if slot.applied_bg != Some(bg_rgb) {
            if let Some(c2) = &slot.controller2 {
                ctrl2_set_bg_color(c2, bg_rgb);
                slot.applied_bg = Some(bg_rgb);
            }
        }
        if slot.applied_rect != r || !slot.applied_visible {
            unsafe {
                let _ = SetWindowPos(
                    slot.hwnd,
                    0,
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            if let Some(ctrl) = &slot.controller {
                ctrl_set_bounds(
                    ctrl,
                    Rect {
                        left: 0,
                        top: 0,
                        right: r.w,
                        bottom: r.h,
                    },
                );
            }
            slot.applied_rect = r;
        }
        if !slot.applied_visible {
            if let Some(ctrl) = &slot.controller {
                if let Some(wv3) = &slot.webview3 {
                    webview3_resume(wv3);
                }
                ctrl_set_visible(ctrl, true);
            }
            slot.applied_visible = true;
        } else if force_pos_notify {
            if let Some(ctrl) = &slot.controller {
                ctrl_notify_parent_moved(ctrl);
            }
        }
    }

    fn create_child_hwnd(parent_hwnd: isize) -> isize {
        if parent_hwnd == 0 {
            return 0;
        }
        let cls = ensure_host_window_class();
        let hinst = unsafe { GetModuleHandleW(std::ptr::null()) };
        unsafe {
            CreateWindowExW(
                0,
                cls.as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                parent_hwnd,
                0,
                hinst,
                std::ptr::null(),
            )
        }
    }

    #[derive(Clone)]
    pub struct NativeWebviewManager {
        inner: Rc<RefCell<Inner>>,
    }

    impl NativeWebviewManager {
        pub fn new() -> Self {
            Self {
                inner: Rc::new(RefCell::new(Inner {
                    env_creating: false,
                    env_failed: false,
                    env: None,
                    pending_ctrl: VecDeque::new(),
                    slots: HashMap::new(),
                    pool: WebviewPoolPolicy::default(),
                    bg_rgb: 0x141820,
                    frame_id: 0,
                    blocked: false,
                    parent_hwnd: 0,
                    last_parent_pos: (i32::MIN, i32::MIN),
                    parent_moved: false,
                    painted: Vec::new(),
                    last_visible: HashMap::new(),
                })),
            }
        }

        pub fn is_available() -> bool {
            locate_webview2_dll().is_some()
        }

        pub fn state(&self, handle: &str) -> NativeWebviewState {
            let inner = self.inner.borrow();
            if inner.env_failed {
                return NativeWebviewState::Failed;
            }
            inner
                .slots
                .get(handle)
                .map(|s| s.state)
                .unwrap_or(NativeWebviewState::Loading)
        }

        pub fn begin_frame(&self, frame_id: u64, blocked: bool, bg_rgb: u32, parent_hwnd: isize) {
            let mut inner = self.inner.borrow_mut();
            inner.frame_id = frame_id;
            inner.blocked = blocked;
            inner.bg_rgb = bg_rgb;
            inner.parent_hwnd = parent_hwnd;
            inner.painted.clear();
            inner.parent_moved = false;
            if parent_hwnd != 0 {
                let mut rc = Rect::default();
                if unsafe { GetWindowRect(parent_hwnd, &mut rc) } != 0 {
                    let pos = (rc.left, rc.top);
                    if inner.last_parent_pos != pos {
                        inner.last_parent_pos = pos;
                        inner.parent_moved = true;
                    }
                }
            }
        }

        pub fn present_slot(&self, frame_id: u64, handle: &str, url: &str, rect: PhysicalRect) {
            let mut inner = self.inner.borrow_mut();
            if inner.frame_id != frame_id || inner.blocked || rect.is_empty() || handle.is_empty() {
                return;
            }
            if let Some(prev) = inner.painted.iter_mut().find(|(h, _, _)| h == handle) {
                prev.1 = url.to_string();
                prev.2 = rect;
            } else {
                inner
                    .painted
                    .push((handle.to_string(), url.to_string(), rect));
            }
        }

        pub fn end_frame(&self, frame_id: u64) -> FrameSyncOutcome {
            let mut out = FrameSyncOutcome::default();
            let now = Instant::now();
            let mut need_env = false;
            let mut to_start: Vec<String> = Vec::new();
            {
                let mut inner = self.inner.borrow_mut();
                if inner.frame_id != frame_id {
                    return out;
                }
                let bg_rgb = inner.bg_rgb;
                let parent_hwnd = inner.parent_hwnd;
                let blocked = inner.blocked;
                let parent_moved = inner.parent_moved;
                let painted = std::mem::take(&mut inner.painted);

                let active_keys: Vec<String> = if blocked {
                    Vec::new()
                } else {
                    painted.iter().map(|(h, _, _)| h.clone()).collect()
                };

                // Hide any slot not actively painted this frame.
                let slot_keys: Vec<String> = inner.slots.keys().cloned().collect();
                for k in slot_keys {
                    if !active_keys.contains(&k) {
                        if let Some(slot) = inner.slots.get_mut(&k) {
                            slot.want_visible = false;
                            apply_slot_geometry(slot, bg_rgb, false);
                        }
                        inner.pool.mark_hidden(&k, now);
                        if inner.last_visible.insert(k.clone(), false) == Some(true) {
                            out.visibility_changes.push((k, false));
                        }
                    }
                }

                if blocked {
                    return out;
                }

                let env_ready = inner.env.is_some();
                let env_failed = inner.env_failed;

                for (handle, url, rect) in painted {
                    let keep_refs: Vec<&str> = active_keys.iter().map(String::as_str).collect();
                    if !inner.slots.contains_key(&handle) {
                        while let Some(victim) = inner.pool.pick_eviction_victim(&keep_refs) {
                            if let Some(mut old) = inner.slots.remove(&victim) {
                                old.destroy();
                            }
                            inner.pool.remove(&victim);
                            inner.last_visible.remove(&victim);
                            out.evicted_handles.push(victim);
                        }
                        let hwnd = create_child_hwnd(parent_hwnd);
                        inner.slots.insert(
                            handle.clone(),
                            WebviewSlot {
                                hwnd,
                                url: url.clone(),
                                navigated_url: String::new(),
                                controller: None,
                                controller2: None,
                                webview: None,
                                webview3: None,
                                new_win_token: 0,
                                creating: false,
                                state: if env_failed {
                                    NativeWebviewState::Failed
                                } else {
                                    NativeWebviewState::Loading
                                },
                                want_rect: rect,
                                want_visible: true,
                                applied_rect: PhysicalRect::default(),
                                applied_visible: false,
                                applied_bg: None,
                            },
                        );
                        need_env |= !env_ready && !env_failed;
                        if env_ready {
                            to_start.push(handle.clone());
                        } else if !env_failed && !inner.pending_ctrl.iter().any(|k| k == &handle) {
                            inner.pending_ctrl.push_back(handle.clone());
                        }
                    }

                    inner.pool.touch(&handle, now);
                    if let Some(slot) = inner.slots.get_mut(&handle) {
                        slot.url = url.clone();
                        slot.want_rect = rect;
                        slot.want_visible = true;
                        if slot.controller.is_some() {
                            if slot.navigated_url != url && !url.is_empty() {
                                if let Some(wv) = &slot.webview {
                                    webview_navigate(wv, &url);
                                    slot.navigated_url = url;
                                }
                            }
                            apply_slot_geometry(slot, bg_rgb, parent_moved);
                        } else if env_ready
                            && !slot.creating
                            && slot.state != NativeWebviewState::Failed
                        {
                            to_start.push(handle.clone());
                        } else if !env_ready && !env_failed {
                            need_env = true;
                        }
                    }

                    if inner.last_visible.insert(handle.clone(), true) != Some(true) {
                        out.visibility_changes.push((handle, true));
                    }
                }
                inner.pool.age_tick();
            }

            if need_env {
                ensure_environment(&self.inner);
            }
            for k in to_start {
                start_controller_for_slot(&self.inner, &k);
            }
            out
        }

        pub fn reload(&self, handle: &str) {
            let inner = self.inner.borrow();
            if let Some(slot) = inner.slots.get(handle) {
                if let Some(wv) = &slot.webview {
                    webview_reload(wv);
                }
            }
        }

        pub fn set_background_rgb(&self, bg_rgb: u32) {
            let mut inner = self.inner.borrow_mut();
            inner.bg_rgb = bg_rgb;
            for slot in inner.slots.values_mut() {
                if slot.applied_bg != Some(bg_rgb) {
                    if let Some(c2) = &slot.controller2 {
                        ctrl2_set_bg_color(c2, bg_rgb);
                        slot.applied_bg = Some(bg_rgb);
                    }
                }
            }
        }

        pub fn maintenance_tick(&self, window_active: bool) {
            let now = Instant::now();
            let mut inner = self.inner.borrow_mut();
            let bg_rgb = inner.bg_rgb;
            if !window_active {
                let keys: Vec<String> = inner.slots.keys().cloned().collect();
                for k in keys {
                    if let Some(slot) = inner.slots.get_mut(&k) {
                        slot.want_visible = false;
                        apply_slot_geometry(slot, bg_rgb, false);
                    }
                }
            }
            let suspend_keys = inner.pool.collect_suspend_candidates(now, window_active);
            for k in suspend_keys {
                if let Some(slot) = inner.slots.get_mut(&k) {
                    if !slot.applied_visible {
                        if let Some(wv3) = &slot.webview3 {
                            webview3_try_suspend(wv3);
                            inner.pool.mark_suspended(&k);
                        }
                    }
                }
            }
        }

        pub fn shutdown(&self) {
            self.inner.borrow_mut().shutdown();
        }

        #[cfg(test)]
        pub(super) fn test_frame_id(&self) -> u64 {
            self.inner.borrow().frame_id
        }
    }
}

#[cfg(not(windows))]
mod non_win {
    use super::{FrameSyncOutcome, NativeWebviewState, PhysicalRect};

    #[derive(Clone)]
    pub struct NativeWebviewManager;

    impl NativeWebviewManager {
        pub fn new() -> Self {
            Self
        }
        pub fn is_available() -> bool {
            false
        }
        pub fn state(&self, _handle: &str) -> NativeWebviewState {
            NativeWebviewState::Failed
        }
        pub fn begin_frame(&self, _f: u64, _blocked: bool, _bg: u32, _parent: isize) {}
        pub fn present_slot(&self, _f: u64, _h: &str, _u: &str, _r: PhysicalRect) {}
        pub fn end_frame(&self, _f: u64) -> FrameSyncOutcome {
            FrameSyncOutcome::default()
        }
        pub fn reload(&self, _handle: &str) {}
        pub fn set_background_rgb(&self, _bg_rgb: u32) {}
        pub fn maintenance_tick(&self, _window_active: bool) {}
        pub fn shutdown(&self) {}
        #[cfg(test)]
        pub(super) fn test_frame_id(&self) -> u64 {
            0
        }
    }
}

#[cfg(windows)]
pub use win::NativeWebviewManager;

#[cfg(not(windows))]
pub use non_win::NativeWebviewManager;

#[cfg(test)]
mod tests {
    use super::{
        find_webview2_dll_in_dir, webview2_arch_dir, PhysicalRect, WebviewPoolPolicy,
        MAX_LIVE_CONTROLLERS, SUSPEND_AFTER_SECS,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn physical_rect_quantizes_edges_consistently() {
        let r = PhysicalRect::from_logical(40.4, 35.6, 260.4, 500.2, 1.5);
        assert_eq!(r.x, 61);
        assert_eq!(r.y, 53);
        assert_eq!(r.w, 390);
        assert_eq!(r.h, 751);
        assert!(!r.is_empty());
        assert!(PhysicalRect::from_logical(0., 0., 0.5, 100., 1.0).is_empty());
    }

    #[test]
    fn lru_aging_pool_evicts_lowest_age_hidden_controller() {
        let mut pool = WebviewPoolPolicy::default();
        let t0 = Instant::now();
        pool.touch("v1", t0);
        pool.age_tick();
        pool.touch("v2", t0 + Duration::from_secs(1));
        pool.age_tick();
        pool.touch("v3", t0 + Duration::from_secs(2));
        pool.age_tick();

        pool.mark_hidden("v1", t0 + Duration::from_secs(3));
        pool.mark_hidden("v2", t0 + Duration::from_secs(4));
        assert_eq!(pool.len(), MAX_LIVE_CONTROLLERS);

        // v1 had more decay ticks without re-touch, so v1 has the lowest age.
        let victim = pool.pick_eviction_victim(&["v4"]);
        assert_eq!(victim.as_deref(), Some("v1"));
    }

    #[test]
    fn denning_working_set_suspends_after_tau_or_blur() {
        let mut pool = WebviewPoolPolicy::default();
        let t0 = Instant::now();
        pool.touch("v1", t0);
        pool.touch("v2", t0);
        pool.mark_hidden("v1", t0);

        assert!(pool
            .collect_suspend_candidates(t0 + Duration::from_secs(SUSPEND_AFTER_SECS - 1), true)
            .is_empty());
        assert_eq!(
            pool.collect_suspend_candidates(t0 + Duration::from_secs(SUSPEND_AFTER_SECS), true),
            vec!["v1".to_string()]
        );

        let on_blur = pool.collect_suspend_candidates(t0 + Duration::from_secs(1), false);
        assert_eq!(on_blur.len(), 2);
    }

    #[test]
    fn runtime_dll_discovery_prefers_highest_numeric_version() {
        let root = std::env::temp_dir().join(format!("fx-wv2-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let arch = webview2_arch_dir();
        let p99 = root.join("99.0.1.0").join("EBWebView").join(arch);
        let p158 = root.join("158.0.4370.45").join("EBWebView").join(arch);
        std::fs::create_dir_all(&p99).unwrap();
        std::fs::create_dir_all(&p158).unwrap();
        std::fs::write(p99.join("EmbeddedBrowserWebView.dll"), b"").unwrap();
        std::fs::write(p158.join("EmbeddedBrowserWebView.dll"), b"").unwrap();

        let chosen = find_webview2_dll_in_dir(&root).unwrap();
        assert!(chosen.starts_with(&p158));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cloning_manager_and_dropping_clone_preserves_state() {
        let mgr = super::NativeWebviewManager::new();
        mgr.begin_frame(42, false, 0x112233, 0);
        let cloned = mgr.clone();
        drop(cloned);
        if cfg!(windows) {
            assert_eq!(mgr.test_frame_id(), 42);
        }
        mgr.shutdown();
        assert_eq!(mgr.test_frame_id(), 0);
    }
}
