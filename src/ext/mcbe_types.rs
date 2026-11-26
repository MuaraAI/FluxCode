use super::bedrock::McbeChannel;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, Hover, HoverContents,
    MarkupContent, MarkupKind, Position, Range, TextEdit,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstalledMcbe {
    pub official: Option<String>,
    pub official_engine: Option<[u32; 3]>,
    pub preview: Option<String>,
    pub preview_engine: Option<[u32; 3]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkgVersions {
    pub name: &'static str,
    pub summary: &'static str,
    pub official_npm: String,
    pub official_manifest: String,
    pub preview_npm: String,
    pub preview_manifest: String,
    pub beta_only: bool,
}

impl PkgVersions {
    pub fn npm_for(&self, ch: McbeChannel) -> &str {
        match ch {
            McbeChannel::Official => &self.official_npm,
            McbeChannel::Preview => &self.preview_npm,
        }
    }

    pub fn manifest_for(&self, ch: McbeChannel) -> &str {
        match ch {
            McbeChannel::Official => &self.official_manifest,
            McbeChannel::Preview => &self.preview_manifest,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: CompletionItemKind,
    pub detail: String,
    pub doc: String,
    pub type_name: String,
}

#[derive(Clone, Debug)]
pub struct MemberDef {
    pub name: String,
    pub kind: CompletionItemKind,
    pub detail: String,
    pub doc: String,
    pub return_type: String,
}

#[derive(Clone, Debug, Default)]
pub struct TypeDef {
    pub kind: &'static str,
    pub extends: Option<String>,
    #[allow(dead_code)]
    pub doc: String,
    pub members: Vec<MemberDef>,
}

#[derive(Clone, Debug, Default)]
pub struct ModuleSymbols {
    pub exports: Vec<SymbolEntry>,
    pub types: HashMap<String, TypeDef>,
}

#[derive(Clone, Debug, Default)]
pub struct TypeIndex {
    pub channel: Option<McbeChannel>,
    pub modules: HashMap<String, ModuleSymbols>,
}

struct McbeRegistry {
    active: bool,
    channel: McbeChannel,
    pkg_versions: HashMap<String, String>,
    catalog: Vec<PkgVersions>,
    index: Option<Arc<TypeIndex>>,
}

static REGISTRY: OnceLock<RwLock<McbeRegistry>> = OnceLock::new();

fn registry() -> &'static RwLock<McbeRegistry> {
    REGISTRY.get_or_init(|| {
        RwLock::new(McbeRegistry {
            active: false,
            channel: McbeChannel::Official,
            pkg_versions: HashMap::new(),
            catalog: default_catalog(),
            index: None,
        })
    })
}

pub fn default_catalog() -> Vec<PkgVersions> {
    vec![
        PkgVersions {
            name: "@minecraft/server",
            summary: "Core entities, blocks, worlds, events & system ticks",
            official_npm: "2.11.0".into(),
            official_manifest: "2.11.0".into(),
            preview_npm: "2.12.0-rc.1.26.70-preview.26".into(),
            preview_manifest: "2.12.0-beta".into(),
            beta_only: false,
        },
        PkgVersions {
            name: "@minecraft/server-ui",
            summary: "ActionFormData, ModalFormData & MessageFormData dialogs",
            official_npm: "2.3.0".into(),
            official_manifest: "2.3.0".into(),
            preview_npm: "2.4.0-rc.1.26.70-preview.26".into(),
            preview_manifest: "2.4.0-beta".into(),
            beta_only: false,
        },
        PkgVersions {
            name: "@minecraft/server-gametest",
            summary: "Automated GameTest framework & simulated players",
            official_npm: "1.0.0-beta.1.26.66-stable".into(),
            official_manifest: "1.0.0-beta".into(),
            preview_npm: "1.0.0-beta.1.26.66-stable".into(),
            preview_manifest: "1.0.0-beta".into(),
            beta_only: true,
        },
        PkgVersions {
            name: "@minecraft/server-net",
            summary: "HTTP client requests (BDS / Dedicated Server)",
            official_npm: "1.0.0-beta.1.26.66-stable".into(),
            official_manifest: "1.0.0-beta".into(),
            preview_npm: "1.0.0-beta.1.26.66-stable".into(),
            preview_manifest: "1.0.0-beta".into(),
            beta_only: true,
        },
        PkgVersions {
            name: "@minecraft/server-admin",
            summary: "Server secrets, variables & transfer helpers (BDS)",
            official_npm: "1.0.0-beta.1.26.66-stable".into(),
            official_manifest: "1.0.0-beta".into(),
            preview_npm: "1.0.0-beta.1.26.66-stable".into(),
            preview_manifest: "1.0.0-beta".into(),
            beta_only: true,
        },
        PkgVersions {
            name: "@minecraft/common",
            summary: "Shared MCBE types, errors & vector helpers",
            official_npm: "1.3.0".into(),
            official_manifest: "1.3.0".into(),
            preview_npm: "1.3.0-rc.1.26.30-preview.26".into(),
            preview_manifest: "1.3.0-beta".into(),
            beta_only: false,
        },
        PkgVersions {
            name: "@minecraft/debug-utilities",
            summary: "Watchdog profiling & world debug shape drawing",
            official_npm: "1.0.0-beta.1.26.66-stable".into(),
            official_manifest: "1.0.0-beta".into(),
            preview_npm: "1.0.0-beta.1.26.66-stable".into(),
            preview_manifest: "1.0.0-beta".into(),
            beta_only: true,
        },
    ]
}

pub fn catalog() -> Vec<PkgVersions> {
    registry()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .catalog
        .clone()
}

pub fn is_workspace_active() -> bool {
    registry().read().unwrap_or_else(|e| e.into_inner()).active
}

pub fn manifest_version_for(npm_version: &str) -> String {
    let trimmed = npm_version.trim();
    if let Some((base, _)) = trimmed.split_once("-beta") {
        return format!("{base}-beta");
    }
    if let Some((base, _)) = trimmed.split_once("-rc") {
        return format!("{base}-beta");
    }
    if let Some((base, _)) = trimmed.split_once("-preview") {
        return format!("{base}-beta");
    }
    trimmed.to_string()
}

/// Parses Windows Appx package full name (e.g. `Microsoft.MinecraftUWP_1.26.6501.0_x64__...`)
/// into `(McbeChannel, display_version, min_engine_version)`.
pub fn parse_appx_mcbe_version(pkg_name: &str) -> Option<(McbeChannel, String, [u32; 3])> {
    let channel = if pkg_name.starts_with("Microsoft.MinecraftUWP_") {
        McbeChannel::Official
    } else if pkg_name.starts_with("Microsoft.MinecraftWindowsBeta_") {
        McbeChannel::Preview
    } else {
        return None;
    };
    let rest = pkg_name.split_once('_')?.1;
    let ver_str = rest.split('_').next()?;
    let parts: Vec<u32> = ver_str.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() < 3 {
        return None;
    }
    let (maj, min, raw_patch) = (parts[0], parts[1], parts[2]);
    let (patch, label) = if raw_patch >= 100 {
        let p = raw_patch / 100;
        let sub = raw_patch % 100;
        let lbl = match channel {
            McbeChannel::Official => format!("{maj}.{min}.{p}"),
            McbeChannel::Preview => format!("{maj}.{min}.{p}-preview.{sub}"),
        };
        (p, lbl)
    } else {
        (raw_patch, format!("{maj}.{min}.{raw_patch}"))
    };
    Some((channel, label, [maj, min, patch]))
}

pub fn detect_installed_mcbe() -> InstalledMcbe {
    let mut info = InstalledMcbe::default();
    #[cfg(windows)]
    {
        #[link(name = "advapi32")]
        extern "system" {
            fn RegOpenKeyExW(hkey: isize, subkey: *const u16, opts: u32, sam: u32, out: *mut isize) -> i32;
            fn RegEnumKeyExW(hkey: isize, index: u32, name: *mut u16, len: *mut u32, res: *mut u32, cls: *mut u16, cls_len: *mut u32, ft: *mut u64) -> i32;
            fn RegCloseKey(hkey: isize) -> i32;
        }
        const HKEY_CURRENT_USER: isize = -2147483647_isize;
        const KEY_READ: u32 = 0x20019;
        let subkey: Vec<u16> = "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\AppModel\\Repository\\Packages\0"
            .encode_utf16()
            .collect();
        let mut hkey: isize = 0;
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) == 0 && hkey != 0 {
                let mut buf = [0u16; 260];
                let mut idx = 0u32;
                loop {
                    let mut len = buf.len() as u32;
                    let rc = RegEnumKeyExW(
                        hkey,
                        idx,
                        buf.as_mut_ptr(),
                        &mut len,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );
                    if rc != 0 {
                        break;
                    }
                    idx += 1;
                    let name = String::from_utf16_lossy(&buf[..len as usize]);
                    if let Some((ch, lbl, eng)) = parse_appx_mcbe_version(&name) {
                        match ch {
                            McbeChannel::Official => {
                                info.official = Some(lbl);
                                info.official_engine = Some(eng);
                            }
                            McbeChannel::Preview => {
                                info.preview = Some(lbl);
                                info.preview_engine = Some(eng);
                            }
                        }
                    }
                }
                let _ = RegCloseKey(hkey);
            }
        }
    }
    info
}

pub fn cache_dir() -> PathBuf {
    crate::settings::config_dir().join("mcbe-types")
}

fn cache_slug(pkg: &str, ver: &str) -> String {
    let mut s = String::with_capacity(pkg.len() + ver.len() + 2);
    for ch in pkg.chars().chain(std::iter::once('@')).chain(ver.chars()) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '@') {
            s.push(ch);
        } else {
            s.push('_');
        }
    }
    format!("{s}.d.ts.z")
}

pub fn is_cached(pkg: &str, ver: &str) -> bool {
    cache_dir().join(cache_slug(pkg, ver)).is_file()
}

pub fn cache_total_bytes() -> u64 {
    let Ok(rd) = std::fs::read_dir(cache_dir()) else {
        return 0;
    };
    rd.flatten()
        .filter_map(|e| e.metadata().ok().map(|m| m.len()))
        .sum()
}

fn curl() -> Command {
    let mut cmd = Command::new("curl.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

pub fn refresh_npm_dist_tags() -> Result<Vec<PkgVersions>, String> {
    let mut updated = catalog();
    for entry in &mut updated {
        let enc = entry.name.replace('/', "%2f");
        let url = format!("https://registry.npmjs.org/-/package/{enc}/dist-tags");
        let out = curl()
            .args(["-s", "-f", "-L", "--max-time", "8", &url])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("curl error: {e}"))?;
        if !out.status.success() {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) else {
            continue;
        };
        let get = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        if let Some(latest) = get("latest") {
            entry.official_npm = latest.clone();
            entry.official_manifest = manifest_version_for(&latest);
        }
        if let Some(prev) = get("rc").or_else(|| get("beta")) {
            entry.preview_npm = prev.clone();
            entry.preview_manifest = manifest_version_for(&prev);
        }
    }
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());
    reg.catalog = updated.clone();
    reg.index = None;
    Ok(updated)
}

/// Extracts `*.d.ts` declaration contents in memory from an npm `.tgz` tarball without touching disk.
pub fn extract_dts_from_tgz(tgz: &[u8]) -> Result<String, String> {
    if tgz.len() < 18 || tgz[0] != 0x1f || tgz[1] != 0x8b || tgz[2] != 0x08 {
        return Err("Invalid gzip tarball header".into());
    }
    let flg = tgz[3];
    let mut pos = 10usize;
    if flg & 0x04 != 0 {
        if pos + 2 > tgz.len() {
            return Err("Truncated gzip extra".into());
        }
        let xlen = u16::from_le_bytes([tgz[pos], tgz[pos + 1]]) as usize;
        pos += 2 + xlen;
    }
    for bit in [0x08u8, 0x10u8] {
        if flg & bit != 0 {
            while pos < tgz.len() && tgz[pos] != 0 {
                pos += 1;
            }
            pos += 1;
        }
    }
    if flg & 0x02 != 0 {
        pos += 2;
    }
    if pos >= tgz.len().saturating_sub(8) {
        return Err("Truncated gzip stream".into());
    }
    let tar = miniz_oxide::inflate::decompress_to_vec(&tgz[pos..tgz.len() - 8])
        .map_err(|e| format!("Tarball deflate failed: {e:?}"))?;

    let mut dts_out = String::new();
    let mut tp = 0usize;
    while tp + 512 <= tar.len() {
        let hdr = &tar[tp..tp + 512];
        if hdr.iter().all(|&b| b == 0) {
            break;
        }
        let name_end = hdr[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&hdr[..name_end]);
        let pref_end = hdr[345..500].iter().position(|&b| b == 0).unwrap_or(155);
        let full = if pref_end > 0 {
            format!("{}/{}", String::from_utf8_lossy(&hdr[345..345 + pref_end]), name)
        } else {
            name.into_owned()
        };
        let sz_raw = String::from_utf8_lossy(&hdr[124..136]);
        let sz = usize::from_str_radix(sz_raw.trim_matches(|c: char| c == '\0' || c.is_ascii_whitespace()), 8)
            .unwrap_or(0);
        tp += 512;
        if tp + sz > tar.len() {
            break;
        }
        if full.ends_with(".d.ts") && (hdr[156] == 0 || hdr[156] == b'0') {
            if !dts_out.is_empty() {
                dts_out.push('\n');
            }
            dts_out.push_str(&String::from_utf8_lossy(&tar[tp..tp + sz]));
        }
        tp += (sz + 511) & !511;
    }
    if dts_out.is_empty() {
        return Err("No .d.ts declarations found in tarball".into());
    }
    Ok(dts_out)
}

pub fn hydrate_package_types(pkg: &str, npm_version: &str) -> Result<usize, String> {
    let short = pkg.rsplit('/').next().unwrap_or(pkg);
    let url = format!("https://registry.npmjs.org/{pkg}/-/{short}-{npm_version}.tgz");
    let out = curl()
        .args(["-s", "-f", "-L", "--max-time", "25", &url])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("curl error: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Failed to download {pkg}@{npm_version} (exit {})",
            out.status.code().unwrap_or(-1)
        ));
    }
    let dts = extract_dts_from_tgz(&out.stdout)?;
    let compressed = miniz_oxide::deflate::compress_to_vec(dts.as_bytes(), 6);
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(cache_slug(pkg, npm_version));
    let tmp = dir.join(format!("{}.tmp", cache_slug(pkg, npm_version)));
    std::fs::write(&tmp, &compressed).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    registry().write().unwrap_or_else(|e| e.into_inner()).index = None;
    Ok(compressed.len())
}

fn load_cached_dts(pkg: &str, ver: &str) -> Option<String> {
    let raw = std::fs::read(cache_dir().join(cache_slug(pkg, ver))).ok()?;
    let bytes = miniz_oxide::inflate::decompress_to_vec(&raw).ok()?;
    String::from_utf8(bytes).ok()
}

fn strip_generic_and_union(raw: &str) -> String {
    let s = raw.split(';').next().unwrap_or(raw).trim();
    let s = s
        .strip_prefix("Promise<")
        .and_then(|x| x.strip_suffix('>'))
        .unwrap_or(s);
    let main = s
        .split('|')
        .map(str::trim)
        .find(|part| !matches!(*part, "undefined" | "null" | "void"))
        .unwrap_or(s);
    main.split('<')
        .next()
        .unwrap_or(main)
        .trim_end_matches("[]")
        .trim()
        .to_string()
}

fn format_jsdoc(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        let t = line
            .trim()
            .trim_start_matches("/**")
            .trim_end_matches("*/")
            .trim_start_matches('*')
            .trim();
        if t.is_empty() || t.starts_with("@remarks") {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(t);
    }
    out
}

fn parse_member_into(td: &mut TypeDef, raw: &str, doc: String) {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed == "}"
        || trimmed.starts_with("private ")
        || trimmed.starts_with("protected ")
    {
        return;
    }
    if td.kind == "enum" {
        let name = trimmed.split(['=', ',']).next().unwrap_or("").trim().to_string();
        if !name.is_empty() {
            td.members.push(MemberDef {
                name,
                kind: CompletionItemKind::ENUM_MEMBER,
                detail: trimmed.trim_end_matches(',').to_string(),
                doc,
                return_type: String::new(),
            });
        }
        return;
    }
    let body = trimmed.strip_prefix("readonly ").unwrap_or(trimmed).trim();
    if let Some((n, rest)) = body.split_once('(') {
        let name = n.split('<').next().unwrap_or(n).trim().to_string();
        if !name.is_empty() && name != "constructor" && !name.starts_with('[') {
            let ret = rest
                .rsplit_once("):")
                .map(|(_, r)| strip_generic_and_union(r))
                .unwrap_or_default();
            td.members.push(MemberDef {
                name,
                kind: CompletionItemKind::METHOD,
                detail: format!("({}", rest.trim_end_matches(';')),
                doc,
                return_type: ret,
            });
        }
    } else if let Some((n, ty)) = body.split_once(':') {
        let name = n.trim_end_matches('?').trim().to_string();
        if !name.is_empty() && !name.starts_with('[') {
            let rty = strip_generic_and_union(ty);
            td.members.push(MemberDef {
                name,
                kind: CompletionItemKind::PROPERTY,
                detail: rty.clone(),
                doc,
                return_type: rty,
            });
        }
    }
}

pub fn parse_dts(dts: &str) -> ModuleSymbols {
    let mut mod_sym = ModuleSymbols::default();
    let mut doc_buf: Vec<String> = Vec::new();
    let mut in_doc = false;
    let mut cur_type: Option<(String, TypeDef)> = None;
    let mut brace_depth = 0i32;

    for raw in dts.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if in_doc {
            doc_buf.push(trimmed.to_string());
            if trimmed.contains("*/") {
                in_doc = false;
            }
            continue;
        }
        if trimmed.starts_with("/**") {
            doc_buf.clear();
            doc_buf.push(trimmed.to_string());
            if !trimmed.contains("*/") {
                in_doc = true;
            }
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }

        let opens = trimmed.bytes().filter(|&b| b == b'{').count() as i32;
        let closes = trimmed.bytes().filter(|&b| b == b'}').count() as i32;

        if brace_depth == 0 {
            let doc = format_jsdoc(&doc_buf);
            doc_buf.clear();
            let rest = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
            if let Some(after) = rest
                .strip_prefix("class ")
                .or_else(|| rest.strip_prefix("interface "))
                .or_else(|| rest.strip_prefix("enum "))
            {
                let is_enum = rest.starts_with("enum ");
                let kind_lbl = if is_enum {
                    "enum"
                } else if rest.starts_with("interface ") {
                    "interface"
                } else {
                    "class"
                };
                let head = after.split('{').next().unwrap_or("").trim();
                let mut parts = head.split_whitespace();
                let name = parts
                    .next()
                    .unwrap_or("")
                    .split('<')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let extends = head
                    .split_once(" extends ")
                    .map(|(_, e)| strip_generic_and_union(e.split(" implements ").next().unwrap_or(e)));
                if !name.is_empty() {
                    mod_sym.exports.push(SymbolEntry {
                        name: name.clone(),
                        kind: if is_enum { CompletionItemKind::ENUM } else { CompletionItemKind::CLASS },
                        detail: format!("{kind_lbl} {name}"),
                        doc: doc.clone(),
                        type_name: name.clone(),
                    });
                    if opens > 0 {
                        let mut td = TypeDef {
                            kind: kind_lbl,
                            extends,
                            doc,
                            members: Vec::new(),
                        };
                        if opens <= closes {
                            if let (Some(lb), Some(rb)) = (trimmed.find('{'), trimmed.rfind('}')) {
                                if lb < rb {
                                    let sep = if td.kind == "enum" { ',' } else { ';' };
                                    for part in trimmed[lb + 1..rb].split(sep) {
                                        parse_member_into(&mut td, part, String::new());
                                    }
                                }
                            }
                            mod_sym.types.insert(name, td);
                        } else {
                            cur_type = Some((name, td));
                        }
                    }
                }
            } else if let Some(after) = rest.strip_prefix("const ") {
                if let Some((n, ty)) = after.split_once(':') {
                    let name = n.trim().to_string();
                    let t_clean = strip_generic_and_union(ty);
                    mod_sym.exports.push(SymbolEntry {
                        name,
                        kind: CompletionItemKind::CONSTANT,
                        detail: format!("const: {t_clean}"),
                        doc,
                        type_name: t_clean,
                    });
                }
            } else if let Some(after) = rest.strip_prefix("function ") {
                if let Some((n, sig)) = after.split_once('(') {
                    let name = n.split('<').next().unwrap_or(n).trim().to_string();
                    let ret = sig
                        .rsplit_once("):")
                        .map(|(_, r)| strip_generic_and_union(r))
                        .unwrap_or_default();
                    mod_sym.exports.push(SymbolEntry {
                        name: name.clone(),
                        kind: CompletionItemKind::FUNCTION,
                        detail: format!("function ({sig}"),
                        doc,
                        type_name: ret,
                    });
                }
            }
        } else if brace_depth == 1 {
            if let Some((_, ref mut td)) = cur_type {
                if trimmed == "}" || trimmed.starts_with("private ") || trimmed.starts_with("protected ") {
                    doc_buf.clear();
                } else {
                    let doc = format_jsdoc(&doc_buf);
                    doc_buf.clear();
                    parse_member_into(td, trimmed, doc);
                }
            }
        }

        brace_depth += opens - closes;
        if brace_depth <= 0 {
            brace_depth = 0;
            if let Some((name, td)) = cur_type.take() {
                mod_sym.types.insert(name, td);
            }
        }
    }
    mod_sym
}

fn builtin_catalog(ch: McbeChannel) -> TypeIndex {
    let preview = ch == McbeChannel::Preview;
    let mut modules = HashMap::new();

    let server_dts = concat!(
        "/** Singleton representing the current Minecraft world. */\n",
        "export const world: World;\n",
        "/** Singleton for tick scheduling, after/before system events, and watchdogs. */\n",
        "export const system: System;\n",
        "export enum Direction { Down = 'Down', Up = 'Up', North = 'North', South = 'South', West = 'West', East = 'East' }\n",
        "export enum GameMode { Survival = 'Survival', Creative = 'Creative', Adventure = 'Adventure', Spectator = 'Spectator' }\n",
        "export enum EquipmentSlot { Mainhand = 'Mainhand', Offhand = 'Offhand', Head = 'Head', Chest = 'Chest', Legs = 'Legs', Feet = 'Feet' }\n",
        "export enum EntityDamageCause {\n",
        "  none = 'none', fire = 'fire', lava = 'lava', fall = 'fall',\n",
        "  drowning = 'drowning', explosion = 'explosion', void = 'void',\n",
        "  magic = 'magic', entityAttack = 'entityAttack',\n",
        "}\n",
        "export class World {\n",
        "  readonly afterEvents: WorldAfterEvents;\n",
        "  readonly beforeEvents: WorldBeforeEvents;\n",
        "  readonly scoreboard: Scoreboard;\n",
        "  /** Broadcasts a chat message to all players in the world. */\n",
        "  sendMessage(message: string): void;\n",
        "  /** Returns a Dimension instance (`overworld`, `nether`, `the_end`). */\n",
        "  getDimension(dimensionId: string): Dimension;\n",
        "  /** Returns all currently connected players. */\n",
        "  getAllPlayers(): Player[];\n",
        "  /** Queries players matching optional filters. */\n",
        "  getPlayers(options?: EntityQueryOptions): Player[];\n",
        "  /** Reads a world dynamic property by identifier. */\n",
        "  getDynamicProperty(identifier: string): boolean | number | string | Vector3 | undefined;\n",
        "  /** Persists a world dynamic property across restarts. */\n",
        "  setDynamicProperty(identifier: string, value?: boolean | number | string | Vector3): void;\n",
        "  /** Plays a sound audible to all players. */\n",
        "  playSound(soundId: string, location: Vector3): void;\n",
        "}\n",
        "export class System {\n",
        "  readonly currentTick: number;\n",
        "  readonly afterEvents: SystemAfterEvents;\n",
        "  readonly beforeEvents: SystemBeforeEvents;\n",
        "  /** Schedules a callback on the next game tick. */\n",
        "  run(callback: () => void): number;\n",
        "  /** Repeats callback every `tickInterval` ticks. */\n",
        "  runInterval(callback: () => void, tickInterval?: number): number;\n",
        "  /** Runs callback once after `tickDelay` ticks. */\n",
        "  runTimeout(callback: () => void, tickDelay?: number): number;\n",
        "  /** Runs a generator job across ticks using unused frame budget. */\n",
        "  runJob(generator: Generator<void, void, void>): number;\n",
        "  /** Cancels a scheduled run/runInterval/runTimeout id. */\n",
        "  clearRun(runId: number): void;\n",
        "}\n",
        "export class WorldAfterEvents {\n",
        "  readonly worldLoad: WorldLoadAfterEventSignal;\n",
        "  readonly playerSpawn: PlayerSpawnAfterEventSignal;\n",
        "  readonly playerJoin: PlayerJoinAfterEventSignal;\n",
        "  readonly playerLeave: PlayerLeaveAfterEventSignal;\n",
        "  readonly playerBreakBlock: PlayerBreakBlockAfterEventSignal;\n",
        "  readonly playerPlaceBlock: PlayerPlaceBlockAfterEventSignal;\n",
        "  readonly itemUse: ItemUseAfterEventSignal;\n",
        "  readonly entityHitEntity: EntityHitEntityAfterEventSignal;\n",
        "  readonly entityDie: EntityDieAfterEventSignal;\n",
        "  readonly chatSend: ChatSendAfterEventSignal;\n",
        "}\n",
        "export class WorldBeforeEvents {\n",
        "  readonly chatSend: ChatSendBeforeEventSignal;\n",
        "  readonly playerBreakBlock: PlayerBreakBlockBeforeEventSignal;\n",
        "  readonly playerInteractWithBlock: PlayerInteractWithBlockBeforeEventSignal;\n",
        "  readonly itemUse: ItemUseBeforeEventSignal;\n",
        "}\n",
        "export class SystemAfterEvents {\n",
        "  readonly scriptEventReceive: ScriptEventCommandMessageAfterEventSignal;\n",
        "}\n",
        "export class SystemBeforeEvents {\n",
        "  readonly startup: StartupEventSignal;\n",
        "  readonly watchdogTerminate: WatchdogTerminateBeforeEventSignal;\n",
        "}\n",
        "export class WorldLoadAfterEventSignal { subscribe(cb: (arg: WorldLoadAfterEvent) => void): void; unsubscribe(cb: (arg: WorldLoadAfterEvent) => void): void; }\n",
        "export class PlayerSpawnAfterEventSignal { subscribe(cb: (arg: PlayerSpawnAfterEvent) => void): void; }\n",
        "export class PlayerBreakBlockAfterEventSignal { subscribe(cb: (arg: PlayerBreakBlockAfterEvent) => void): void; }\n",
        "export class ItemUseAfterEventSignal { subscribe(cb: (arg: ItemUseAfterEvent) => void): void; }\n",
        "export class ChatSendBeforeEventSignal { subscribe(cb: (arg: ChatSendBeforeEvent) => void): void; }\n",
        "export class PlayerSpawnAfterEvent { readonly player: Player; readonly initialSpawn: boolean; }\n",
        "export class PlayerBreakBlockAfterEvent { readonly player: Player; readonly block: Block; readonly brokenBlockPermutation: BlockPermutation; readonly dimension: Dimension; }\n",
        "export class ItemUseAfterEvent { readonly source: Player; readonly itemStack: ItemStack; }\n",
        "export class ChatSendBeforeEvent { readonly sender: Player; readonly message: string; cancel: boolean; }\n",
        "export class Entity {\n",
        "  readonly id: string;\n",
        "  readonly typeId: string;\n",
        "  readonly dimension: Dimension;\n",
        "  readonly location: Vector3;\n",
        "  nameTag: string;\n",
        "  runCommand(commandString: string): CommandResult;\n",
        "  applyDamage(amount: number, options?: EntityApplyDamageOptions): boolean;\n",
        "  applyImpulse(vector: Vector3): void;\n",
        "  teleport(location: Vector3, teleportOptions?: TeleportOptions): void;\n",
        "  getComponent(componentId: string): EntityComponent;\n",
        "  getTags(): string[];\n",
        "  addTag(tag: string): boolean;\n",
        "  removeTag(tag: string): boolean;\n",
        "  hasTag(tag: string): boolean;\n",
        "  kill(): boolean;\n",
        "  remove(): void;\n",
        "}\n",
        "export class Player extends Entity {\n",
        "  readonly name: string;\n",
        "  readonly level: number;\n",
        "  readonly onScreenDisplay: ScreenDisplay;\n",
        "  readonly camera: Camera;\n",
        "  readonly inputPermissions: InputPermissions;\n",
        "  sendMessage(message: string): void;\n",
        "  getGameMode(): GameMode;\n",
        "  setGameMode(gameMode?: GameMode): void;\n",
        "  playSound(soundId: string, soundOptions?: PlayerSoundOptions): void;\n",
        "  addExperience(amount: number): number;\n",
        "  isOp(): boolean;\n",
        "}\n",
        "export class ScreenDisplay {\n",
        "  setActionBar(text: string): void;\n",
        "  setTitle(title: string, options?: TitleDisplayOptions): void;\n",
        "  updateSubtitle(subtitle: string): void;\n",
        "}\n",
        "export class Dimension {\n",
        "  readonly id: string;\n",
        "  getBlock(location: Vector3): Block;\n",
        "  getEntities(options?: EntityQueryOptions): Entity[];\n",
        "  spawnEntity(identifier: string, location: Vector3): Entity;\n",
        "  spawnItem(itemStack: ItemStack, location: Vector3): Entity;\n",
        "  spawnParticle(effectName: string, location: Vector3, molangVariables?: MolangVariableMap): void;\n",
        "  runCommand(commandString: string): CommandResult;\n",
        "  playSound(soundId: string, location: Vector3): void;\n",
        "}\n",
        "export class Block {\n",
        "  readonly typeId: string;\n",
        "  readonly location: Vector3;\n",
        "  readonly dimension: Dimension;\n",
        "  readonly permutation: BlockPermutation;\n",
        "  setType(blockType: string): void;\n",
        "  setPermutation(permutation: BlockPermutation): void;\n",
        "}\n",
        "export class BlockPermutation {\n  readonly type: BlockType;\n  matches(blockName: string): boolean;\n}\n",
        "export class ItemStack {\n",
        "  constructor(itemTypeId: string, amount?: number);\n",
        "  typeId: string; amount: number; nameTag?: string;\n",
        "  getLore(): string[];\n",
        "  setLore(loreList?: string[]): void;\n",
        "  clone(): ItemStack;\n",
        "}\n",
        "export class Container {\n",
        "  readonly size: number; readonly emptySlotsCount: number;\n",
        "  getItem(slot: number): ItemStack;\n",
        "  setItem(slot: number, item?: ItemStack): void;\n",
        "  addItem(item: ItemStack): ItemStack;\n",
        "}\n",
        "export class MolangVariableMap {\n",
        "  setFloat(name: string, value: number): void;\n",
        "  setVector3(name: string, vector: Vector3): void;\n",
        "  setColorRGBA(name: string, color: RGBA): void;\n",
        "}\n",
    );

    let mut srv = parse_dts(server_dts);
    if preview {
        let preview_dts = concat!(
            "export enum CommandPermissionLevel { Any = 0, GameDirectors = 1, Admin = 2, Host = 3, Owner = 4 }\n",
            "export enum CustomCommandParamType { Boolean = 0, Integer = 1, Float = 2, String = 3, EntitySelector = 4, PlayerSelector = 5, Location = 6 }\n",
            "export class CustomCommandRegistry {\n",
            "  /** Registers a custom slash command in Preview/Beta 2.12+. */\n",
            "  registerCommand(customCommand: CustomCommand, callback: (origin: CustomCommandOrigin, ...args: any[]) => void): void;\n",
            "  registerEnum(name: string, values: string[]): void;\n",
            "}\n",
            "export class StartupEvent {\n",
            "  readonly blockComponentRegistry: BlockComponentRegistry;\n",
            "  readonly itemComponentRegistry: ItemComponentRegistry;\n",
            "  readonly customCommandRegistry: CustomCommandRegistry;\n",
            "}\n",
            "export class StartupEventSignal { subscribe(cb: (arg: StartupEvent) => void): void; }\n",
            "export class Camera { fade(fadeCameraOptions?: CameraFadeOptions): void; setCamera(cameraPreset: string): void; clear(): void; }\n",
        );
        let pmod = parse_dts(preview_dts);
        srv.exports.extend(pmod.exports);
        srv.types.extend(pmod.types);
    } else {
        let stable_cam = parse_dts("export class Camera { setCamera(cameraPreset: string): void; clear(): void; }");
        srv.types.extend(stable_cam.types);
    }
    modules.insert("@minecraft/server".into(), srv);

    let ui_dts = concat!(
        "export class ActionFormData {\n",
        "  title(titleText: string): ActionFormData;\n",
        "  body(bodyText: string): ActionFormData;\n",
        "  button(text: string, iconPath?: string): ActionFormData;\n",
        "  show(player: Player): Promise<ActionFormResponse>;\n",
        "}\n",
        "export class ActionFormResponse { readonly canceled: boolean; readonly selection?: number; readonly cancelationReason?: FormCancelationReason; }\n",
        "export class ModalFormData {\n",
        "  title(titleText: string): ModalFormData;\n",
        "  textField(label: string, placeholderText: string, defaultValue?: string): ModalFormData;\n",
        "  toggle(label: string, defaultValue?: boolean): ModalFormData;\n",
        "  slider(label: string, minimumValue: number, maximumValue: number, valueStep: number, defaultValue?: number): ModalFormData;\n",
        "  dropdown(label: string, options: string[], defaultValueIndex?: number): ModalFormData;\n",
        "  show(player: Player): Promise<ModalFormResponse>;\n",
        "}\n",
        "export class ModalFormResponse { readonly canceled: boolean; readonly formValues?: (string | number | boolean)[]; }\n",
        "export class MessageFormData {\n",
        "  title(titleText: string): MessageFormData;\n",
        "  body(bodyText: string): MessageFormData;\n",
        "  button1(text: string): MessageFormData;\n",
        "  button2(text: string): MessageFormData;\n",
        "  show(player: Player): Promise<MessageFormResponse>;\n",
        "}\n",
        "export class MessageFormResponse { readonly canceled: boolean; readonly selection?: number; }\n",
        "export enum FormCancelationReason { UserClosed = 'UserClosed', UserBusy = 'UserBusy' }\n",
    );
    modules.insert("@minecraft/server-ui".into(), parse_dts(ui_dts));
    modules.insert(
        "@minecraft/server-gametest".into(),
        parse_dts(concat!(
            "export function register(\n",
            "  testClassName: string, testName: string, callback: (test: Test) => void,\n",
            "): void;\n",
            "export class Test {\n",
            "  succeed(): void;\n",
            "  fail(errorMessage: string): void;\n",
            "  spawn(entityType: string, blockLocation: Vector3): Entity;\n",
            "}\n",
        )),
    );
    modules.insert(
        "@minecraft/server-net".into(),
        parse_dts(concat!(
            "export const http: HttpClient;\n",
            "export class HttpClient {\n",
            "  get(uri: string): Promise<HttpResponse>;\n",
            "  request(config: HttpRequest): Promise<HttpResponse>;\n",
            "}\n",
            "export class HttpResponse { readonly status: number; readonly body: string; }\n",
        )),
    );
    modules.insert(
        "@minecraft/server-admin".into(),
        parse_dts(concat!(
            "export const variables: ServerVariables;\n",
            "export const secrets: ServerSecrets;\n",
            "export class ServerVariables { get(name: string): any; }\n",
            "export class ServerSecrets { get(name: string): SecretStringValue; }\n",
        )),
    );
    modules.insert(
        "@minecraft/common".into(),
        parse_dts("export class UnsupportedScriptError extends Error {}\nexport class EngineError extends Error {}"),
    );
    modules.insert(
        "@minecraft/debug-utilities".into(),
        parse_dts("export const debugDrawer: DebugDrawer;\nexport class DebugDrawer { addShape(shape: any): void; removeAll(): void; }"),
    );

    TypeIndex {
        channel: Some(ch),
        modules,
    }
}

pub fn set_workspace_context(
    active: bool,
    channel: McbeChannel,
    pkg_versions: HashMap<String, String>,
) {
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());
    let changed = reg.channel != channel || reg.pkg_versions != pkg_versions;
    reg.active = active;
    reg.channel = channel;
    reg.pkg_versions = pkg_versions;
    if !active || changed {
        reg.index = None;
    }
}

fn ensure_index() -> Arc<TypeIndex> {
    {
        let reg = registry().read().unwrap_or_else(|e| e.into_inner());
        if let Some(idx) = &reg.index {
            return Arc::clone(idx);
        }
    }
    let (ch, catalog_copy, versions) = {
        let reg = registry().read().unwrap_or_else(|e| e.into_inner());
        (reg.channel, reg.catalog.clone(), reg.pkg_versions.clone())
    };
    let mut idx = builtin_catalog(ch);
    for entry in catalog_copy {
        let ver = versions
            .get(entry.name)
            .cloned()
            .unwrap_or_else(|| entry.npm_for(ch).to_string());
        if let Some(dts) = load_cached_dts(entry.name, &ver) {
            let parsed = parse_dts(&dts);
            if !parsed.exports.is_empty() || !parsed.types.is_empty() {
                idx.modules.insert(entry.name.to_string(), parsed);
            }
        }
    }
    let arc = Arc::new(idx);
    registry().write().unwrap_or_else(|e| e.into_inner()).index = Some(Arc::clone(&arc));
    arc
}

pub fn byte_to_lsp_pos(source: &str, offset: usize) -> Position {
    let safe = offset.min(source.len());
    let head = &source[..safe];
    let line = head.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = head.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col: u32 = head[line_start..].chars().map(|c| c.len_utf16() as u32).sum();
    Position::new(line, col)
}

fn make_item(
    label: String,
    kind: CompletionItemKind,
    detail: String,
    doc: String,
    range: Range,
    new_text: String,
) -> CompletionItem {
    CompletionItem {
        label,
        kind: Some(kind),
        detail: (!detail.is_empty()).then_some(detail),
        documentation: (!doc.is_empty()).then_some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc,
        })),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text })),
        ..Default::default()
    }
}

fn find_type<'a>(idx: &'a TypeIndex, name: &str) -> Option<&'a TypeDef> {
    idx.modules.values().find_map(|m| m.types.get(name))
}

fn find_export<'a>(idx: &'a TypeIndex, name: &str) -> Option<&'a SymbolEntry> {
    idx.modules
        .values()
        .find_map(|m| m.exports.iter().find(|e| e.name == name))
}

fn resolve_expr_type(idx: &TypeIndex, source: &str, chain: &[&str], offset: usize) -> Option<String> {
    if chain.is_empty() {
        return None;
    }
    let head = chain[0].trim();
    let mut cur = if let Some(inner) = head.strip_prefix("new ") {
        inner.split('(').next().unwrap_or(inner).trim().to_string()
    } else if let Some(sym) = find_export(idx, head) {
        sym.type_name.clone()
    } else {
        infer_local_var_type(idx, source, head, offset)?
    };

    for seg in &chain[1..] {
        let name = seg.split('(').next().unwrap_or(seg).trim();
        let mut cursor = Some(cur.as_str());
        let mut next_ty = None;
        for _ in 0..4 {
            let Some(tname) = cursor else { break };
            let Some(td) = find_type(idx, tname) else { break };
            if let Some(m) = td.members.iter().find(|m| m.name == name) {
                next_ty = Some(m.return_type.clone());
                break;
            }
            cursor = td.extends.as_deref();
        }
        cur = next_ty.filter(|t| !t.is_empty())?;
    }
    Some(cur)
}

fn infer_local_var_type(idx: &TypeIndex, source: &str, ident: &str, offset: usize) -> Option<String> {
    let head = &source[..offset.min(source.len())];
    // 1. Callback param pattern: `.subscribe((ident) =>` or `.subscribe(ident =>`
    if let Some(sub_pos) = head.rfind(".subscribe(") {
        let after = &head[sub_pos + 11..];
        let param = after
            .trim_start()
            .trim_start_matches('(')
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        if param == ident {
            let before_sub = &head[..sub_pos];
            let expr = before_sub
                .rsplit(['\n', ';', '{', '}', '=', '>', ',', '('])
                .next()
                .unwrap_or("")
                .trim();
            let segs: Vec<&str> = expr.split('.').map(str::trim).filter(|s| !s.is_empty()).collect();
            if let Some(sig_ty) = resolve_expr_type(idx, source, &segs, sub_pos) {
                if let Some(td) = find_type(idx, &sig_ty) {
                    if let Some(sub_m) = td.members.iter().find(|m| m.name == "subscribe") {
                        if let Some(arg_ty) = sub_m
                            .detail
                            .split(':')
                            .nth(2)
                            .or_else(|| sub_m.detail.split(':').nth(1))
                        {
                            let clean = arg_ty.split(')').next().unwrap_or(arg_ty).trim();
                            if !clean.is_empty() {
                                return Some(clean.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    // 2. Variable declaration: `const/let ident = ...`
    for line in head.lines().rev() {
        let t = line.trim();
        for kw in ["const ", "let ", "var "] {
            if let Some(rest) = t.strip_prefix(kw) {
                if let Some((lhs, rhs)) = rest.split_once('=') {
                    let var = lhs.split(':').next().unwrap_or(lhs).trim();
                    if var == ident {
                        let clean_rhs = rhs.trim().trim_end_matches(';').trim();
                        if let Some(after_new) = clean_rhs.strip_prefix("new ") {
                            let cls = after_new.split('(').next().unwrap_or("").trim();
                            if !cls.is_empty() {
                                return Some(cls.to_string());
                            }
                        }
                        let segs: Vec<&str> = clean_rhs.split('.').map(str::trim).filter(|s| !s.is_empty()).collect();
                        if let Some(t) = resolve_expr_type(idx, source, &segs, offset) {
                            return Some(t);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn complete_script(source: &str, byte_offset: usize, _file_path: &Path) -> Vec<CompletionItem> {
    let idx = ensure_index();
    let off = byte_offset.min(source.len());
    let head = &source[..off];
    let cur_pos = byte_to_lsp_pos(source, off);
    let line_start = head.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_head = &head[line_start..];

    // 1. Module specifier: from "@minecraft/..."
    if let Some(qpos) = line_head.rfind(['"', '\'']) {
        let before_q = &line_head[..qpos];
        if before_q.contains("from") || before_q.contains("import") {
            let typed = &line_head[qpos + 1..];
            let range = Range::new(byte_to_lsp_pos(source, line_start + qpos + 1), cur_pos);
            return catalog()
                .into_iter()
                .filter(|c| c.name.contains(typed) || typed.is_empty())
                .map(|c| {
                    let ch = idx.channel.unwrap_or(McbeChannel::Official);
                    let detail = format!("{} · {}", c.npm_for(ch), c.summary);
                    make_item(
                        c.name.to_string(),
                        CompletionItemKind::MODULE,
                        detail,
                        c.summary.to_string(),
                        range,
                        c.name.to_string(),
                    )
                })
                .collect();
        }
    }

    // 2. Named import: import { ... } from "@minecraft/..."
    if line_head.contains("import") && line_head.contains('{') && !line_head.contains('}') {
        let tail_line = source[off..].lines().next().unwrap_or("");
        let mod_name = [
            "@minecraft/server-ui",
            "@minecraft/server-gametest",
            "@minecraft/server-net",
            "@minecraft/server-admin",
            "@minecraft/debug-utilities",
            "@minecraft/common",
            "@minecraft/server",
        ]
        .into_iter()
        .find(|m| tail_line.contains(m))
        .unwrap_or("@minecraft/server");
        let word_start = head
            .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);
        let typed = &head[word_start..];
        let range = Range::new(byte_to_lsp_pos(source, word_start), cur_pos);
        if let Some(ms) = idx.modules.get(mod_name) {
            return ms
                .exports
                .iter()
                .filter(|e| e.name.to_ascii_lowercase().starts_with(&typed.to_ascii_lowercase()))
                .take(80)
                .map(|e| make_item(e.name.clone(), e.kind, e.detail.clone(), e.doc.clone(), range, e.name.clone()))
                .collect();
        }
    }

    // Current identifier prefix
    let word_start = head
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let typed = &head[word_start..];
    let range = Range::new(byte_to_lsp_pos(source, word_start), cur_pos);

    // 3. Dot member completion: `expr.prefix`
    if word_start > 0 && head.as_bytes()[word_start - 1] == b'.' {
        let before_dot = &head[..word_start - 1];
        let expr = before_dot
            .rsplit(['\n', ';', '{', '}', '=', '>', ',', '!', '&', '|', ':'])
            .next()
            .unwrap_or("")
            .trim()
            .trim_start_matches('(')
            .trim();
        let segs: Vec<&str> = expr.split('.').map(str::trim).filter(|s| !s.is_empty()).collect();
        if let Some(ty) = resolve_expr_type(&idx, source, &segs, off) {
            let mut items = Vec::new();
            let mut cursor = Some(ty.as_str());
            let mut seen = std::collections::HashSet::new();
            for _ in 0..4 {
                let Some(tname) = cursor else { break };
                let Some(td) = find_type(&idx, tname) else { break };
                for m in &td.members {
                    if (typed.is_empty() || m.name.to_ascii_lowercase().starts_with(&typed.to_ascii_lowercase()))
                        && seen.insert(m.name.clone())
                    {
                        items.push(make_item(
                            m.name.clone(),
                            m.kind,
                            m.detail.clone(),
                            m.doc.clone(),
                            range,
                            m.name.clone(),
                        ));
                    }
                }
                cursor = td.extends.as_deref();
            }
            if !items.is_empty() {
                return items;
            }
        }
        return Vec::new();
    }

    // 4. Top-level MCBE symbols (when `@minecraft/` is imported OR workspace is active)
    if !is_workspace_active() && !source.contains("@minecraft/") {
        return Vec::new();
    }
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let low = typed.to_ascii_lowercase();
    for m in idx.modules.values() {
        for e in &m.exports {
            if (low.is_empty() || e.name.to_ascii_lowercase().starts_with(&low)) && seen.insert(e.name.clone()) {
                items.push(make_item(
                    e.name.clone(),
                    e.kind,
                    e.detail.clone(),
                    e.doc.clone(),
                    range,
                    e.name.clone(),
                ));
                if items.len() >= 80 {
                    return items;
                }
            }
        }
    }
    items
}

pub fn complete_manifest(source: &str, byte_offset: usize) -> Vec<CompletionItem> {
    let off = byte_offset.min(source.len());
    let head = &source[..off];
    let line_start = head.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_head = &head[line_start..];
    let cur_pos = byte_to_lsp_pos(source, off);
    let Some(qpos) = line_head.rfind('"') else {
        return Vec::new();
    };
    let typed = &line_head[qpos + 1..];
    let range = Range::new(byte_to_lsp_pos(source, line_start + qpos + 1), cur_pos);
    let ch = registry().read().unwrap_or_else(|e| e.into_inner()).channel;
    let list = catalog();

    if line_head[..qpos].contains("\"module_name\"") {
        return list
            .into_iter()
            .filter(|c| typed.is_empty() || c.name.contains(typed))
            .map(|c| {
                make_item(
                    c.name.to_string(),
                    CompletionItemKind::MODULE,
                    format!("manifest {} · npm {}", c.manifest_for(ch), c.npm_for(ch)),
                    c.summary.to_string(),
                    range,
                    c.name.to_string(),
                )
            })
            .collect();
    }
    if line_head[..qpos].contains("\"version\"") {
        return list
            .into_iter()
            .map(|c| {
                let ver = c.manifest_for(ch).to_string();
                make_item(
                    format!("{} ({})", ver, c.name),
                    CompletionItemKind::VALUE,
                    format!("npm {}", c.npm_for(ch)),
                    c.summary.to_string(),
                    range,
                    ver,
                )
            })
            .collect();
    }
    if line_head[..qpos].contains("\"type\"") {
        return ["data", "resources", "script", "skin_pack", "world_template"]
            .into_iter()
            .map(|t| make_item(t.into(), CompletionItemKind::ENUM_MEMBER, "module type".into(), String::new(), range, t.into()))
            .collect();
    }
    Vec::new()
}

pub fn hover_script(source: &str, byte_offset: usize) -> Option<Hover> {
    if !is_workspace_active() && !source.contains("@minecraft/") {
        return None;
    }
    let idx = ensure_index();
    let off = byte_offset.min(source.len());
    let start = source[..off]
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = off
        + source[off..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(source.len() - off);
    if start >= end {
        return None;
    }
    let word = &source[start..end];
    let range = Range::new(byte_to_lsp_pos(source, start), byte_to_lsp_pos(source, end));

    if start > 0 && source.as_bytes()[start - 1] == b'.' {
        let before = &source[..start - 1];
        let expr = before
            .rsplit(['\n', ';', '{', '}', '=', ',', '('])
            .next()
            .unwrap_or("")
            .trim();
        let segs: Vec<&str> = expr.split('.').map(str::trim).filter(|s| !s.is_empty()).collect();
        if let Some(ty) = resolve_expr_type(&idx, source, &segs, start) {
            if let Some(td) = find_type(&idx, &ty) {
                if let Some(m) = td.members.iter().find(|m| m.name == word) {
                    let md = if m.doc.is_empty() {
                        format!("```ts\n{}.{}: {}\n```", ty, m.name, m.detail)
                    } else {
                        format!("```ts\n{}.{}: {}\n```\n{}", ty, m.name, m.detail, m.doc)
                    };
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: md,
                        }),
                        range: Some(range),
                    });
                }
            }
        }
    }

    if let Some(sym) = find_export(&idx, word) {
        let md = if sym.doc.is_empty() {
            format!("```ts\n{} ({})\n```", sym.name, sym.detail)
        } else {
            format!("```ts\n{} ({})\n```\n{}", sym.name, sym.detail, sym.doc)
        };
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: Some(range),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_appx_official_and_preview_versions() {
        assert_eq!(
            parse_appx_mcbe_version("Microsoft.MinecraftUWP_1.26.6501.0_x64__8wekyb3d8bbwe"),
            Some((McbeChannel::Official, "1.26.65".into(), [1, 26, 65]))
        );
        assert_eq!(
            parse_appx_mcbe_version("Microsoft.MinecraftWindowsBeta_1.26.7026.0_x64__8wekyb3d8bbwe"),
            Some((McbeChannel::Preview, "1.26.70-preview.26".into(), [1, 26, 70]))
        );
        assert_eq!(manifest_version_for("2.11.0"), "2.11.0");
        assert_eq!(manifest_version_for("2.12.0-rc.1.26.70-preview.26"), "2.12.0-beta");
    }

    #[test]
    fn extracts_dts_from_synthetic_tgz_and_parses_symbols() {
        let dts = "/** Root world */\nexport const world: World;\nexport class World {\n  sendMessage(msg: string): void;\n}\n";
        let mut tar = vec![0u8; 512];
        let name = b"package/index.d.ts";
        tar[..name.len()].copy_from_slice(name);
        let sz_oct = format!("{:011o}\0", dts.len());
        tar[124..136].copy_from_slice(sz_oct.as_bytes());
        tar[156] = b'0';
        tar.extend_from_slice(dts.as_bytes());
        let rem = (512 - (dts.len() % 512)) % 512;
        tar.resize(tar.len() + rem + 1024, 0);

        let def = miniz_oxide::deflate::compress_to_vec(&tar, 4);
        let mut tgz = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0, 0x03];
        tgz.extend_from_slice(&def);
        tgz.extend_from_slice(&[0u8; 8]);

        let extracted = extract_dts_from_tgz(&tgz).unwrap();
        assert!(extracted.contains("export class World"));
        let parsed = parse_dts(&extracted);
        assert!(parsed.exports.iter().any(|e| e.name == "world" && e.type_name == "World"));
        assert!(parsed.types.get("World").unwrap().members.iter().any(|m| m.name == "sendMessage"));
    }

    #[test]
    fn completes_script_and_manifest_by_channel() {
        set_workspace_context(true, McbeChannel::Preview, HashMap::new());
        let s1 = "import { world } from \"@minecraft/server\";\nworld.afterEvents.";
        let hits = complete_script(s1, s1.len(), Path::new("scripts/main.js"));
        assert!(hits.iter().any(|c| c.label == "worldLoad"));

        let s2 = "import { world } from \"@minecraft/server\";\nworld.afterEvents.playerBreakBlock.subscribe((ev) => ev.";
        let ev_hits = complete_script(s2, s2.len(), Path::new("scripts/main.js"));
        assert!(ev_hits.iter().any(|c| c.label == "brokenBlockPermutation"));

        let s3 = "import { System } from \"@minecraft/server\";\nnew CustomCommandRegistry().";
        let prev_hits = complete_script(s3, s3.len(), Path::new("scripts/main.js"));
        assert!(prev_hits.iter().any(|c| c.label == "registerCommand"));

        set_workspace_context(true, McbeChannel::Official, HashMap::new());
        let off_hits = complete_script(s3, s3.len(), Path::new("scripts/main.js"));
        assert!(!off_hits.iter().any(|c| c.label == "registerCommand"));

        let man = "{\n  \"dependencies\": [{ \"module_name\": \"@minecraft/\"";
        let man_hits = complete_manifest(man, man.len());
        assert!(man_hits.iter().any(|c| c.label == "@minecraft/server"));

        let hov_src = "import { world } from \"@minecraft/server\";\nworld.sendMessage(\"hi\");";
        assert!(hover_script(hov_src, hov_src.find("sendMessage").unwrap() + 2).is_some());
        set_workspace_context(false, McbeChannel::Official, HashMap::new());
    }
}
