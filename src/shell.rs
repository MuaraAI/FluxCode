//! "Open with FluxCode" in Windows Explorer (issue #87).

#![cfg_attr(not(windows), allow(dead_code))]

/// The file types FluxCode offers to open.
pub(crate) const EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "flux", "log", "csv", "tsv", "json", "jsonc", "json5", "js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "html", "htm",
    "css", "scss", "sass", "less", "vue", "svelte", "astro", "py", "pyw", "pyi", "rs", "go", "c", "h", "cpp", "cc", "cxx", "hpp", "hh", "java", "kt",
    "kts", "cs", "fs", "swift", "dart", "rb", "php", "lua", "pl", "r", "scala", "ex", "exs", "erl", "hs", "ml", "nim", "zig", "sh", "bash", "zsh",
    "fish", "ps1", "psm1", "bat", "cmd", "sql", "graphql", "proto", "yaml", "yml", "toml", "ini", "cfg", "conf", "env", "properties", "xml", "svg",
    "gradle", "cmake", "mk", "dockerfile", "gitignore", "gitattributes", "editorconfig", "lock",
];

/// Types in `EXTENSIONS` FluxCode never makes their default, only offers in "Open with".
const SCRIPTS: &[&str] = &["bat", "cmd", "ps1", "psm1"];

pub(crate) const PROG_ID: &str = "FluxCode.File";
const MENU_KEY: &str = "FluxCode";
const MENU_LABEL: &str = "Open with FluxCode";

/// The registry keys and values this module writes, as (key, value name, data) under `HKCU\Software\Classes`.
fn entries(exe: &str) -> Vec<(String, Option<&'static str>, String)> {
    let open_file = format!("\"{exe}\" \"%1\"");
    let open_dir = format!("\"{exe}\" \"%V\"");
    let icon = format!("\"{exe}\",0");
    let mut v: Vec<(String, Option<&'static str>, String)> = vec![
        (PROG_ID.into(), None, "FluxCode file".into()),
        (format!("{PROG_ID}\\DefaultIcon"), None, icon.clone()),
        (format!("{PROG_ID}\\shell\\open\\command"), None, open_file.clone()),
        // The name "Open with" shows for FluxCode.exe.
        ("Applications\\FluxCode.exe".into(), Some("FriendlyAppName"), "FluxCode".into()),
        ("Applications\\FluxCode.exe\\shell\\open\\command".into(), None, open_file.clone()),
    ];
    for (key, cmd) in [("*", &open_file), ("Directory", &open_dir), ("Directory\\Background", &open_dir)] {
        v.push((format!("{key}\\shell\\{MENU_KEY}"), None, MENU_LABEL.into()));
        v.push((format!("{key}\\shell\\{MENU_KEY}"), Some("Icon"), icon.clone()));
        v.push((format!("{key}\\shell\\{MENU_KEY}\\command"), None, cmd.clone()));
    }
    v
}

/// The `"exe" "%1"` command FluxCode would register for itself.
#[cfg(windows)]
fn exe_path() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let s = exe.to_string_lossy();
    Ok(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

#[cfg(windows)]
pub(crate) mod reg {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    pub type Hkey = isize;
    // (HKEY)(ULONG_PTR)(LONG)0x8000000x: sign-extended on 64-bit Windows.
    pub const HKCR: Hkey = 0x8000_0000u32 as i32 as isize;
    pub const HKCU: Hkey = 0x8000_0001u32 as i32 as isize;
    pub const HKLM: Hkey = 0x8000_0002u32 as i32 as isize;
    const KEY_ALL_ACCESS: u32 = 0xF003F;
    const REG_NONE: u32 = 0;
    const REG_SZ: u32 = 1;
    const RRF_RT_REG_SZ: u32 = 0x2;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_MORE_DATA: i32 = 234;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(key: Hkey, sub: *const u16, reserved: u32, class: *const u16, options: u32, sam: u32, sa: *const c_void, out: *mut Hkey, disposition: *mut u32) -> i32;
        fn RegSetValueExW(key: Hkey, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
        fn RegDeleteTreeW(key: Hkey, sub: *const u16) -> i32;
        fn RegDeleteKeyValueW(key: Hkey, sub: *const u16, name: *const u16) -> i32;
        fn RegGetValueW(key: Hkey, sub: *const u16, name: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
        fn RegOpenKeyExW(key: Hkey, sub: *const u16, options: u32, sam: u32, out: *mut Hkey) -> i32;
        fn RegQueryInfoKeyW(
            key: Hkey, class: *mut u16, class_len: *mut u32, reserved: *mut u32, subkeys: *mut u32, max_subkey: *mut u32, max_class: *mut u32,
            values: *mut u32, max_value_name: *mut u32, max_value: *mut u32, security: *mut u32, written: *mut c_void,
        ) -> i32;
        fn RegDeleteKeyW(key: Hkey, sub: *const u16) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn opt_wide(s: Option<&str>) -> Option<Vec<u16>> {
        s.map(wide)
    }

    fn ptr(w: &Option<Vec<u16>>) -> *const u16 {
        w.as_ref().map_or(null(), |w| w.as_ptr())
    }

    fn check(rc: i32, what: &str) -> Result<(), String> {
        if rc == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("{what}: Windows error {rc}"))
        }
    }

    /// Set a string value (or, with `data` None, an empty REG_NONE marker) on `root\sub`, creating the key.
    pub fn set(root: Hkey, sub: &str, name: Option<&str>, data: Option<&str>) -> Result<(), String> {
        let sub_w = wide(sub);
        let mut key: Hkey = 0;
        check(
            unsafe { RegCreateKeyExW(root, sub_w.as_ptr(), 0, null(), 0, KEY_ALL_ACCESS, null(), &mut key, null_mut()) },
            sub,
        )?;
        let name_w = opt_wide(name);
        let rc = match data {
            Some(d) => {
                let d = wide(d);
                unsafe { RegSetValueExW(key, ptr(&name_w), 0, REG_SZ, d.as_ptr().cast(), (d.len() * 2) as u32) }
            }
            None => unsafe { RegSetValueExW(key, ptr(&name_w), 0, REG_NONE, null(), 0) },
        };
        unsafe { RegCloseKey(key) };
        check(rc, sub)
    }

    /// A string value, or None when the key or value is missing.
    pub fn get(root: Hkey, sub: &str, name: Option<&str>) -> Option<String> {
        let sub_w = wide(sub);
        let name_w = opt_wide(name);
        let mut buf = vec![0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        // REG_EXPAND_SZ values (PATH) come back expanded; a long one asks for a bigger buffer.
        let mut rc = unsafe { RegGetValueW(root, sub_w.as_ptr(), ptr(&name_w), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut len) };
        if rc == ERROR_MORE_DATA {
            buf = vec![0u16; len as usize / 2 + 64];
            len = (buf.len() * 2) as u32;
            rc = unsafe { RegGetValueW(root, sub_w.as_ptr(), ptr(&name_w), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut len) };
        }
        if rc != ERROR_SUCCESS {
            return None;
        }
        let n = (len as usize / 2).saturating_sub(1).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]))
    }

    /// Delete `root\sub` with everything under it; a missing key is fine.
    pub fn delete_tree(root: Hkey, sub: &str) -> Result<(), String> {
        let rc = unsafe { RegDeleteTreeW(root, wide(sub).as_ptr()) };
        if rc == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(rc, sub)
    }

    /// Delete `root\sub` when it holds no values and no keys (one `register` made for a type Windows didn't know).
    pub fn delete_if_empty(root: Hkey, sub: &str) {
        let sub_w = wide(sub);
        let mut key: Hkey = 0;
        if unsafe { RegOpenKeyExW(root, sub_w.as_ptr(), 0, KEY_ALL_ACCESS, &mut key) } != ERROR_SUCCESS {
            return;
        }
        let (mut subkeys, mut values) = (0u32, 0u32);
        let n: *mut u32 = null_mut();
        let rc = unsafe { RegQueryInfoKeyW(key, null_mut(), n, n, &mut subkeys, n, n, &mut values, n, n, n, null_mut()) };
        unsafe { RegCloseKey(key) };
        if rc == ERROR_SUCCESS && subkeys == 0 && values == 0 {
            unsafe { RegDeleteKeyW(root, sub_w.as_ptr()) };
        }
    }

    /// Delete one value; a missing key or value is fine.
    pub fn delete_value(root: Hkey, sub: &str, name: Option<&str>) -> Result<(), String> {
        let name_w = opt_wide(name);
        let rc = unsafe { RegDeleteKeyValueW(root, wide(sub).as_ptr(), ptr(&name_w)) };
        if rc == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(rc, sub)
    }
}

#[cfg(windows)]
const CLASSES: &str = "Software\\Classes";

/// Tell Explorer the file types changed, so menus and icons update without a sign-out.
#[cfg(windows)]
fn notify_shell() {
    #[link(name = "shell32")]
    extern "system" {
        fn SHChangeNotify(event: i32, flags: u32, item1: *const std::ffi::c_void, item2: *const std::ffi::c_void);
    }
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, 0, std::ptr::null(), std::ptr::null()) };
}

/// True when the right-click entry points at this FluxCode.exe.
#[cfg(windows)]
pub(crate) fn is_registered() -> bool {
    let Ok(exe) = exe_path() else { return false };
    reg::get(reg::HKCU, &format!("{CLASSES}\\*\\shell\\{MENU_KEY}\\command"), None)
        .is_some_and(|cmd| cmd.to_lowercase().contains(&exe.to_lowercase()))
}

/// Add the file types and the right-click entries for this FluxCode.exe.
#[cfg(windows)]
pub(crate) fn register() -> Result<(), String> {
    use reg::{get, set, HKCR, HKCU};
    let exe = exe_path()?;
    for (key, name, data) in entries(&exe) {
        set(HKCU, &format!("{CLASSES}\\{key}"), name, Some(&data))?;
    }
    for ext in EXTENSIONS {
        let key = format!("{CLASSES}\\.{ext}");
        set(HKCU, &format!("{key}\\OpenWithProgids"), Some(PROG_ID), None)?;
        set(HKCU, &format!("{CLASSES}\\Applications\\FluxCode.exe\\SupportedTypes"), Some(&format!(".{ext}")), Some(""))?;
        // The default only where nothing opens this type yet: an app the user or Windows chose stays, and a script type never opens in the editor on a double-click unless the user asks for it.
        if !SCRIPTS.contains(ext) && get(HKCR, &format!(".{ext}"), None).filter(|v| !v.is_empty()).is_none() {
            set(HKCU, &key, None, Some(PROG_ID))?;
        }
    }
    notify_shell();
    Ok(())
}

/// Remove what `register` added.
#[cfg(windows)]
pub(crate) fn unregister() -> Result<(), String> {
    use reg::{delete_tree, delete_value, get, HKCU};
    let mut first_err = None;
    let mut keep = |r: Result<(), String>| {
        if let Err(e) = r {
            first_err.get_or_insert(e);
        }
    };
    for ext in EXTENSIONS {
        let key = format!("{CLASSES}\\.{ext}");
        keep(delete_value(HKCU, &format!("{key}\\OpenWithProgids"), Some(PROG_ID)));
        if get(HKCU, &key, None).as_deref() == Some(PROG_ID) {
            keep(delete_value(HKCU, &key, None));
        }
        reg::delete_if_empty(HKCU, &format!("{key}\\OpenWithProgids"));
        reg::delete_if_empty(HKCU, &key);
    }
    for key in [PROG_ID.to_string(), "Applications\\FluxCode.exe".into()]
        .into_iter()
        .chain(["*", "Directory", "Directory\\Background"].map(|k| format!("{k}\\shell\\{MENU_KEY}")))
    {
        keep(delete_tree(HKCU, &format!("{CLASSES}\\{key}")));
    }
    notify_shell();
    first_err.map_or(Ok(()), Err)
}

#[cfg(not(windows))]
pub(crate) fn is_registered() -> bool {
    false
}

#[cfg(not(windows))]
pub(crate) fn register() -> Result<(), String> {
    Err("Only on Windows".into())
}

#[cfg(not(windows))]
pub(crate) fn unregister() -> Result<(), String> {
    Ok(())
}

/// `fluxcode --register-shell` / `--unregister-shell`, run by the installer.
pub(crate) fn cli(register_it: bool) -> i32 {
    let r = if register_it { register() } else { unregister() };
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_quote_the_exe_and_the_path() {
        let e = entries(r"C:\Users\A B\.fluxcode\app\FluxCode.exe");
        let open = e.iter().find(|(k, n, _)| k == "FluxCode.File\\shell\\open\\command" && n.is_none()).unwrap();
        assert_eq!(open.2, r#""C:\Users\A B\.fluxcode\app\FluxCode.exe" "%1""#);
        let bg = e.iter().find(|(k, _, _)| k == "Directory\\Background\\shell\\FluxCode\\command").unwrap();
        assert!(bg.2.ends_with(r#" "%V""#));
        assert!(e.iter().any(|(k, n, d)| k == "*\\shell\\FluxCode" && n.is_none() && d == "Open with FluxCode"));
    }

    #[test]
    fn extensions_are_bare_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for e in EXTENSIONS {
            assert!(!e.starts_with('.') && *e == e.to_lowercase(), "{e}");
            assert!(seen.insert(*e), "{e} twice");
        }
    }
}
