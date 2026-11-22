
fn pack_languages() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    for lang in ["fr", "ms"] {
        let src = manifest.join("i18n").join(format!("{lang}.tsv"));
        println!("cargo:rerun-if-changed={}", src.display());
        let text = std::fs::read(&src).unwrap_or_default();
        let packed = miniz_oxide::deflate::compress_to_vec(&text, 10);
        std::fs::write(out.join(format!("{lang}.tsv.z")), packed).expect("write language pack");
    }
}

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
    pack_languages();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    std::fs::copy(manifest.join("assets/icon.ico"), out.join("fluxcode.ico")).expect("copy icon");

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

fn release_builds(this_build: &str) {
    println!("cargo:rerun-if-changed=changelog.flux");
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let git = |args: &[&str]| -> Option<String> {
        let o = std::process::Command::new("git").args(args).current_dir(&manifest).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let mut rows = String::new();
    let tagged = git(&["tag", "-l", "BuildDev-*"]).is_some_and(|t| !t.trim().is_empty());
    let text = std::fs::read_to_string(manifest.join("changelog.flux")).unwrap_or_default();
    for line in text.lines() {
        let Some(title) = line.trim_end().strip_prefix("@title en:").map(str::trim) else { continue };
        if title.is_empty() || title == "..." || !tagged {
            continue;
        }
        let pick = format!("-S@title en: {title}");
        let Some(first) = git(&["log", "--format=%H", "--reverse", &pick, "--", "changelog.flux"]).and_then(|s| s.lines().next().map(str::to_string)) else {
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

fn valid(b: &str) -> bool {
    let Some(rest) = b.strip_prefix("BuildDev-") else { return false };
    let parts: Vec<&str> = rest.split('-').collect();
    let widths = [4, 2, 2, 2, 2, 2];
    parts.len() == 6 && parts.iter().zip(widths).all(|(p, w)| p.len() == w && p.bytes().all(|c| c.is_ascii_digit()))
}

fn utc_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
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
