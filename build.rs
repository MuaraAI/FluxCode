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

fn pack_material_icons() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mat_dir = manifest.join("assets").join("material-icons");
    let json_path = mat_dir.join("material-icons.json");
    let icons_dir = mat_dir.join("icons");
    println!("cargo:rerun-if-changed={}", json_path.display());
    println!("cargo:rerun-if-changed={}", icons_dir.display());

    let json_raw =
        std::fs::read(&json_path).expect("read assets/material-icons/material-icons.json");
    let json_packed = miniz_oxide::deflate::compress_to_vec(&json_raw, 10);
    std::fs::write(out.join("material-icons.json.z"), json_packed)
        .expect("write material-icons.json.z");

    let mut svgs: Vec<(String, Vec<u8>)> = std::fs::read_dir(&icons_dir)
        .expect("read assets/material-icons/icons")
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("svg")) {
                let name = p.file_name()?.to_string_lossy().into_owned();
                let bytes = std::fs::read(&p).ok()?;
                Some((name, bytes))
            } else {
                None
            }
        })
        .collect();
    svgs.sort_by(|a, b| a.0.cmp(&b.0));

    const CHUNK_SIZE: usize = 32;
    let mut chunks_meta: Vec<(u32, u32, u32)> = Vec::new();
    let mut entries_meta: Vec<(String, u16, u32, u32)> = Vec::with_capacity(svgs.len());
    let mut payload: Vec<u8> = Vec::new();

    for (chunk_idx, group) in svgs.chunks(CHUNK_SIZE).enumerate() {
        let mut raw_chunk = Vec::new();
        for (name, bytes) in group {
            let off = raw_chunk.len() as u32;
            let len = bytes.len() as u32;
            raw_chunk.extend_from_slice(bytes);
            entries_meta.push((name.clone(), chunk_idx as u16, off, len));
        }
        let compressed = miniz_oxide::deflate::compress_to_vec(&raw_chunk, 10);
        let data_offset = payload.len() as u32;
        let comp_len = compressed.len() as u32;
        let raw_len = raw_chunk.len() as u32;
        payload.extend_from_slice(&compressed);
        chunks_meta.push((data_offset, comp_len, raw_len));
    }

    let mut pack =
        Vec::with_capacity(64 + chunks_meta.len() * 12 + entries_meta.len() * 32 + payload.len());
    pack.extend_from_slice(&(chunks_meta.len() as u32).to_le_bytes());
    for (off, comp, raw) in &chunks_meta {
        pack.extend_from_slice(&off.to_le_bytes());
        pack.extend_from_slice(&comp.to_le_bytes());
        pack.extend_from_slice(&raw.to_le_bytes());
    }
    pack.extend_from_slice(&(entries_meta.len() as u32).to_le_bytes());
    for (name, cidx, off, len) in &entries_meta {
        let nb = name.as_bytes();
        pack.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        pack.extend_from_slice(nb);
        pack.extend_from_slice(&cidx.to_le_bytes());
        pack.extend_from_slice(&off.to_le_bytes());
        pack.extend_from_slice(&len.to_le_bytes());
    }
    pack.extend_from_slice(&payload);
    std::fs::write(out.join("material-icons.pack"), pack).expect("write material-icons.pack");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-env-changed=FLUXCODE_BUILD");

    let build = match std::env::var("FLUXCODE_BUILD") {
        Ok(b) if valid(&b) => b,
        Ok(b) if !b.is_empty() => {
            panic!("FLUXCODE_BUILD must look like BuildDev-2026-11-09-17-30-12, got {b:?}")
        }
        _ => utc_now(),
    };
    println!("cargo:rustc-env=FLUXCODE_BUILD={build}");
    compile_zig_accel();
    release_builds(&build);
    pack_languages();
    pack_material_icons();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    std::fs::copy(manifest.join("assets/icon.ico"), out.join("fluxcode.ico")).expect("copy icon");

    let n: Vec<u16> = build
        .split('-')
        .skip(1)
        .map(|p| p.parse().unwrap_or(0))
        .collect();
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
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_optional()
        .expect("embed resources");
}

fn zig_target(rust_target: &str) -> Option<&'static str> {
    Some(match rust_target {
        "x86_64-pc-windows-gnu" => "x86_64-windows-gnu",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "x86_64-apple-darwin" => "x86_64-macos",
        "aarch64-apple-darwin" => "aarch64-macos",
        _ => return None,
    })
}

fn find_zig() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("ZIG").filter(|v| !v.is_empty()) {
        return Some(std::path::PathBuf::from(p));
    }
    if std::process::Command::new("zig")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Some(std::path::PathBuf::from("zig"));
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let winget_zig = std::path::PathBuf::from(local).join("Microsoft/WinGet/Links/zig.exe");
        if winget_zig.is_file() {
            return Some(winget_zig);
        }
    }
    None
}

fn compile_zig_accel() {
    println!("cargo::rustc-check-cfg=cfg(flux_zig)");
    println!("cargo:rerun-if-changed=zig/src/flux_accel.zig");
    println!("cargo:rerun-if-env-changed=FLUXCODE_NO_ZIG");
    println!("cargo:rerun-if-env-changed=ZIG");

    if std::env::var("FLUXCODE_NO_ZIG").as_deref() == Ok("1") {
        return;
    }
    let Some(zig) = find_zig() else { return };
    let rust_target = std::env::var("TARGET").unwrap_or_default();
    let Some(ztarget) = zig_target(&rust_target) else {
        return;
    };

    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("zig/src/flux_accel.zig");
    if !src.is_file() {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let lib_name = if rust_target.contains("windows-msvc") {
        "flux_accel.lib"
    } else {
        "libflux_accel.a"
    };
    let out_lib = out.join(lib_name);

    let run = |opt_args: &[&str]| -> bool {
        let mut cmd = std::process::Command::new(&zig);
        cmd.current_dir(&out)
            .arg("build-lib")
            .arg(&src)
            .arg("-static")
            .args(opt_args)
            .args(["-fsingle-threaded", "-fno-stack-check", "-target", ztarget])
            .arg(format!("-femit-bin={}", out_lib.display()));
        if !rust_target.contains("windows") {
            cmd.arg("-fPIC");
        }
        cmd.output().is_ok_and(|o| o.status.success()) && out_lib.is_file()
    };

    if run(&["-O", "fast"]) || run(&["-OReleaseFast"]) {
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=flux_accel");
        println!("cargo:rustc-cfg=flux_zig");
    }
}

fn release_builds(this_build: &str) {
    println!("cargo:rerun-if-changed=changelog.flux");
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let git = |args: &[&str]| -> Option<String> {
        let o = std::process::Command::new("git")
            .args(args)
            .current_dir(&manifest)
            .output()
            .ok()?;
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let mut rows = String::new();
    let tagged = git(&["tag", "-l", "BuildDev-*"]).is_some_and(|t| !t.trim().is_empty());
    let text = std::fs::read_to_string(manifest.join("changelog.flux")).unwrap_or_default();
    for line in text.lines() {
        let Some(title) = line.trim_end().strip_prefix("@title en:").map(str::trim) else {
            continue;
        };
        if title.is_empty() || title == "..." || !tagged {
            continue;
        }
        let pick = format!("-S@title en: {title}");
        let Some(first) = git(&[
            "log",
            "--format=%H",
            "--reverse",
            &pick,
            "--",
            "changelog.flux",
        ])
        .and_then(|s| s.lines().next().map(str::to_string)) else {
            rows.push_str(&format!("    ({title:?}, {this_build:?}),\n"));
            continue;
        };
        let mut tags: Vec<String> = git(&["tag", "--contains", &first, "--list", "BuildDev-*"])
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        tags.sort();
        let build = tags
            .into_iter()
            .next()
            .unwrap_or_else(|| this_build.to_string());
        rows.push_str(&format!("    ({title:?}, {build:?}),\n"));
    }
    let src = format!("/// (release title, the build it came in), from git; see build.rs.\npub const RELEASE_BUILDS: &[(&str, &str)] = &[\n{rows}];\n");
    std::fs::write(out.join("release_builds.rs"), src).expect("write release_builds.rs");
}

fn valid(b: &str) -> bool {
    let Some(rest) = b.strip_prefix("BuildDev-") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('-').collect();
    let widths = [4, 2, 2, 2, 2, 2];
    parts.len() == 6
        && parts
            .iter()
            .zip(widths)
            .all(|(p, w)| p.len() == w && p.bytes().all(|c| c.is_ascii_digit()))
}

fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
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
    format!(
        "BuildDev-{y:04}-{m:02}-{d:02}-{:02}-{:02}-{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}
