//! Windows resources: the FluxCode icon (resource ID 1, which GPUI also uses
//! for the window and taskbar icon) and version info for Explorer / Start.
//! The .rc file is written into OUT_DIR because MinGW windres breaks on the
//! spaces in this repository's path.

fn main() {
    println!("cargo:rerun-if-changed=../src-tauri/icons/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    std::fs::copy(manifest.join("../src-tauri/icons/icon.ico"), out.join("fluxcode.ico")).expect("copy icon");

    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let nums: Vec<u16> = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .take(3)
        .map(|s| s.parse().unwrap_or(0))
        .collect();
    let (a, b, c) = (nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(0), nums.get(2).copied().unwrap_or(0));
    let rc = format!(
        r#"1 ICON "fluxcode.ico"
1 VERSIONINFO
FILEVERSION {a},{b},{c},0
PRODUCTVERSION {a},{b},{c},0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "MuaraAI"
      VALUE "FileDescription", "FluxCode"
      VALUE "ProductName", "FluxCode"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "FluxCode.exe"
      VALUE "LegalCopyright", "Copyright (c) 2026 MuaraAI"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let rc_path = out.join("fluxcode.rc");
    std::fs::write(&rc_path, rc).expect("write rc");
    embed_resource::compile(&rc_path, embed_resource::NONE).manifest_optional().expect("embed resources");
}
