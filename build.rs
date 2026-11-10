//! Build ID and Windows resources.
//!
//! FluxCode has no version number while it is in development. Each build is named
//! after the moment it was made: `BuildDev-YYYY-MM-DD-HH-MM-SS`. CI and
//! `packaging/windows/build.ps1` pass it in `FLUXCODE_BUILD` (Asia/Jakarta time);
//! a plain `cargo build` falls back to the current UTC time. The app reads it as
//! `env!("FLUXCODE_BUILD")` and the updater compares it with release tags.
//!
//! On Windows the FluxCode icon becomes resource ID 1 (GPUI uses it for the window
//! and taskbar) and the build ID goes into the version info Explorer shows. The .rc
//! file is written into OUT_DIR because MinGW windres breaks on spaces in paths.
//!
//! The What's new page names the build each release came in. Git knows: the commit that
//! added a release's `@title en:` line to changelog.flux, then the first `BuildDev-*`
//! tag that contains it. A release no tag contains yet came in the build being made.
//! The answers go into OUT_DIR/release_builds.rs; without git or tags they're empty
//! and the page shows dates only.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-env-changed=FLUXCODE_BUILD");

    let build = match std::env::var("FLUXCODE_BUILD") {
        Ok(b) if valid(&b) => b,
        Ok(b) if !b.is_empty() => panic!("FLUXCODE_BUILD must look like BuildDev-2026-11-09-17-30-12, got {b:?}"),
        _ => utc_now(),
    };
    println!("cargo:rustc-env=FLUXCODE_BUILD={build}");
    release_builds(&build);

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    std::fs::copy(manifest.join("assets/icon.ico"), out.join("fluxcode.ico")).expect("copy icon");

    // Numeric file version from the build time: year, MMDD, HHMM, seconds.
    let n: Vec<u16> = build.split('-').skip(1).map(|p| p.parse().unwrap_or(0)).collect();
    let (y, md, hm, s) = (n[0], n[1] * 100 + n[2], n[3] * 100 + n[4], n[5]);
    let rc = format!(
        r#"1 ICON "fluxcode.ico"
1 VERSIONINFO
FILEVERSION {y},{md},{hm},{s}
PRODUCTVERSION {y},{md},{hm},{s}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "MuaraAI"
      VALUE "FileDescription", "FluxCode Development"
      VALUE "ProductName", "FluxCode Development"
      VALUE "FileVersion", "{build}"
      VALUE "ProductVersion", "{build}"
      VALUE "OriginalFilename", "FluxCode.exe"
      VALUE "LegalCopyright", "Copyright (C) 2026 MuaraAI and contributors. GPL-3.0-or-later"
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

/// Write `RELEASE_BUILDS: &[(title, build)]` for the releases in changelog.flux.
fn release_builds(this_build: &str) {
    println!("cargo:rerun-if-changed=changelog.flux");
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let git = |args: &[&str]| -> Option<String> {
        let o = std::process::Command::new("git").args(args).current_dir(&manifest).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let mut rows = String::new();
    // Only when this is a checkout with build tags (CI fetches them; so does a clone).
    let tagged = git(&["tag", "-l", "BuildDev-*"]).is_some_and(|t| !t.trim().is_empty());
    let text = std::fs::read_to_string(manifest.join("changelog.flux")).unwrap_or_default();
    for line in text.lines() {
        let Some(title) = line.trim_end().strip_prefix("@title en:").map(str::trim) else { continue };
        if title.is_empty() || title == "..." || !tagged {
            continue;
        }
        let pick = format!("-S@title en: {title}");
        let Some(first) = git(&["log", "--format=%H", "--reverse", &pick, "--", "changelog.flux"]).and_then(|s| s.lines().next().map(str::to_string)) else {
            // Not committed yet: it comes with the build being made.
            rows.push_str(&format!("    ({title:?}, {this_build:?}),\n"));
            continue;
        };
        let mut tags: Vec<String> = git(&["tag", "--contains", &first, "--list", "BuildDev-*"]).unwrap_or_default().lines().map(str::to_string).collect();
        tags.sort();
        let build = tags.into_iter().next().unwrap_or_else(|| this_build.to_string());
        rows.push_str(&format!("    ({title:?}, {build:?}),\n"));
    }
    let src = format!("/// (release title, the build it came in), from git; see build.rs.\npub const RELEASE_BUILDS: &[(&str, &str)] = &[\n{rows}];\n");
    std::fs::write(out.join("release_builds.rs"), src).expect("write release_builds.rs");
}

/// `BuildDev-` followed by six dash-separated numbers of the right widths.
fn valid(b: &str) -> bool {
    let Some(rest) = b.strip_prefix("BuildDev-") else { return false };
    let parts: Vec<&str> = rest.split('-').collect();
    let widths = [4, 2, 2, 2, 2, 2];
    parts.len() == 6 && parts.iter().zip(widths).all(|(p, w)| p.len() == w && p.bytes().all(|c| c.is_ascii_digit()))
}

/// The current UTC time as a build ID (no date crate in the build script).
fn utc_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's days_from_civil, inverted.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("BuildDev-{y:04}-{m:02}-{d:02}-{:02}-{:02}-{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}
