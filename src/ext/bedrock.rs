
use super::Project;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Behavior,
    Resource,
    Skin,
    World,
    Other,
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub dir: PathBuf,
    pub name: String,
    pub uuid: String,
    pub kind: Kind,
    pub manifest: Value,
}

pub fn strip_comments(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let (mut i, mut in_str) = (0, false);
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            in_str = c != b'"';
            i += 1;
        } else if c == b'"' {
            in_str = true;
            out.push(c);
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

fn read_manifest(dir: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    serde_json::from_str(&strip_comments(&text)).ok()
}

fn kind_of(m: &Value) -> Kind {
    let types: Vec<&str> = m.get("modules").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.get("type").and_then(|t| t.as_str())).collect()).unwrap_or_default();
    if types.iter().any(|t| *t == "data" || *t == "script" || *t == "javascript") {
        Kind::Behavior
    } else if types.contains(&"resources") {
        Kind::Resource
    } else if types.contains(&"skin_pack") {
        Kind::Skin
    } else if types.contains(&"world_template") {
        Kind::World
    } else {
        Kind::Other
    }
}

pub fn plain_name(s: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
        } else if c == '§' {
            skip = true;
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

pub fn packs(dir: &Path) -> Vec<Pack> {
    let mut dirs = vec![dir.to_path_buf()];
    if !dir.join("manifest.json").is_file() {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.join("manifest.json").is_file()).collect())
            .unwrap_or_default();
        for sub in ["behavior_packs", "resource_packs", "development_behavior_packs", "development_resource_packs"] {
            if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
                found.extend(rd.flatten().map(|e| e.path()).filter(|p| p.join("manifest.json").is_file()));
            }
        }
        found.sort();
        found.dedup();
        dirs = found;
    }
    dirs.into_iter()
        .filter_map(|d| {
            let m = read_manifest(&d)?;
            let header = m.get("header")?;
            let mut name = plain_name(header.get("name").and_then(|v| v.as_str()).unwrap_or(""));
            if name == "pack.name" {
                name = std::fs::read_to_string(d.join("texts").join("en_US.lang"))
                    .ok()
                    .and_then(|t| t.lines().find_map(|l| l.strip_prefix("pack.name=").map(|v| plain_name(v.trim()))))
                    .unwrap_or_default();
            }
            let uuid = header.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Some(Pack { name: if name.is_empty() { d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() } else { name }, uuid, kind: kind_of(&m), manifest: m, dir: d })
        })
        .collect()
}

pub fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5 && parts.iter().zip([8, 4, 4, 4, 12]).all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn version_ok(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.len() == 3 && a.iter().all(|x| x.as_u64().is_some()),
        Value::String(s) => {
            let core = s.split(['-', '+']).next().unwrap_or("");
            let n: Vec<&str> = core.split('.').collect();
            n.len() == 3 && n.iter().all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
        }
        _ => false,
    }
}

pub fn validate(list: &[Pack]) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut seen: Vec<(String, PathBuf)> = Vec::new();
    let all_uuids: Vec<String> = list.iter().map(|p| p.uuid.to_ascii_lowercase()).collect();
    for p in list {
        let mut bad = |m: String| out.push((p.dir.clone(), m));
        let m = &p.manifest;
        match m.get("format_version").and_then(|v| v.as_u64()) {
            Some(1..=3) => {}
            _ => bad("format_version should be 2 (or 3)".into()),
        }
        let Some(h) = m.get("header") else {
            bad("There is no \"header\"".into());
            continue;
        };
        if h.get("name").and_then(|v| v.as_str()).is_none_or(|s| s.trim().is_empty()) {
            bad("header.name is empty".into());
        }
        let mut uuid_check = |u: Option<&str>, what: &str, bad: &mut dyn FnMut(String)| match u {
            Some(u) if is_uuid(u) => {
                let l = u.to_ascii_lowercase();
                if let Some((_, other)) = seen.iter().find(|(s, _)| *s == l) {
                    bad(format!("{what} {u} is used twice (also in {}): every pack and module needs its own", other.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
                }
                seen.push((l, p.dir.clone()));
            }
            Some(u) => bad(format!("{what} \"{u}\" is not a UUID (8-4-4-4-12 hex digits)")),
            None => bad(format!("{what} is missing")),
        };
        uuid_check(h.get("uuid").and_then(|v| v.as_str()), "header.uuid", &mut bad);
        if !h.get("version").is_some_and(version_ok) {
            bad("header.version should be [major, minor, patch]".into());
        }
        if p.kind != Kind::Skin && !h.get("min_engine_version").is_some_and(version_ok) {
            bad("header.min_engine_version should be [1, 21, 0] or similar".into());
        }
        match m.get("modules").and_then(|v| v.as_array()) {
            Some(mods) if !mods.is_empty() => {
                for (i, md) in mods.iter().enumerate() {
                    let t = md.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if !["data", "resources", "script", "javascript", "skin_pack", "world_template", "client_data", "interface"].contains(&t) {
                        bad(format!("modules[{i}].type \"{t}\" is not a pack type"));
                    }
                    uuid_check(md.get("uuid").and_then(|v| v.as_str()), &format!("modules[{i}].uuid"), &mut bad);
                    if !md.get("version").is_some_and(version_ok) {
                        bad(format!("modules[{i}].version should be [major, minor, patch]"));
                    }
                    if t == "script" {
                        match md.get("entry").and_then(|v| v.as_str()) {
                            Some(e) if p.dir.join(e).is_file() => {}
                            Some(e) => bad(format!("modules[{i}].entry {e} doesn't exist (build the scripts first?)")),
                            None => bad(format!("modules[{i}] is a script module without \"entry\"")),
                        }
                    }
                }
            }
            _ => bad("\"modules\" is empty".into()),
        }
        for (i, d) in m.get("dependencies").and_then(|v| v.as_array()).cloned().unwrap_or_default().iter().enumerate() {
            if let Some(u) = d.get("uuid").and_then(|v| v.as_str()) {
                if !is_uuid(u) {
                    bad(format!("dependencies[{i}].uuid \"{u}\" is not a UUID"));
                } else if !all_uuids.contains(&u.to_ascii_lowercase()) {
                    bad(format!("dependencies[{i}] needs pack {u}, which isn't in this add-on (fine if it's installed on its own)"));
                }
            } else if d.get("module_name").is_none() {
                bad(format!("dependencies[{i}] needs a uuid or a module_name"));
            }
        }
    }
    out
}

fn skip_file(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.starts_with('.') || l == "thumbs.db" || l == "node_modules" || l.ends_with(".psd") || l.ends_with(".blend") || l == "tsconfig.json" || l == "package-lock.json"
}

fn add_dir(z: &mut super::zip::Zip, dir: &Path, prefix: &str) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let n = e.file_name().to_string_lossy().into_owned();
        if skip_file(&n) {
            continue;
        }
        let p = e.path();
        let name = if prefix.is_empty() { n.clone() } else { format!("{prefix}/{n}") };
        if p.is_dir() {
            add_dir(z, &p, &name)?;
        } else {
            z.add(&name, &std::fs::read(&p).map_err(|e| e.to_string())?);
        }
    }
    Ok(())
}

fn file_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let n = n.trim().to_string();
    if n.is_empty() { "addon".into() } else { n }
}

pub fn pack(project: &Project) -> Result<Vec<String>, String> {
    let list = packs(&project.dir);
    if list.is_empty() {
        return Err("No pack here (a folder with manifest.json).".into());
    }
    let out_dir = project.dir.join("dist");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let mut z = super::zip::Zip::default();
    let target = if list.len() == 1 {
        add_dir(&mut z, &list[0].dir, "")?;
        out_dir.join(format!("{}.mcpack", file_name(&list[0].name)))
    } else {
        for p in &list {
            let folder = p.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| file_name(&p.name));
            add_dir(&mut z, &p.dir, &folder)?;
        }
        out_dir.join(format!("{}.mcaddon", file_name(&project.name)))
    };
    let files = z.len();
    let bytes = z.finish();
    std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;
    let shown = target.strip_prefix(&project.dir).map(|r| r.display().to_string()).unwrap_or_else(|_| target.display().to_string());
    Ok(vec![format!("Packed {} files into {shown} ({} KB). Double-click it to import into Minecraft.", files, bytes.len() / 1024 + 1)])
}

pub fn com_mojang_for(channel: McbeChannel) -> Option<PathBuf> {
    let mut c = Vec::new();
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA").map(PathBuf::from) {
            let folder = match channel {
                McbeChannel::Official => "Minecraft Bedrock",
                McbeChannel::Preview => "Minecraft Bedrock Preview",
            };
            c.push(a.join(folder).join("Users").join("Shared").join("games").join("com.mojang"));
        }
        if let Some(l) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            let pkg = match channel {
                McbeChannel::Official => "Microsoft.MinecraftUWP_8wekyb3d8bbwe",
                McbeChannel::Preview => "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe",
            };
            c.push(l.join("Packages").join(pkg).join("LocalState").join("games").join("com.mojang"));
        }
    } else if channel == McbeChannel::Official {
        c.push(super::home().join(".local/share/mcpelauncher/games/com.mojang"));
    }
    c.into_iter().find(|p| p.is_dir())
}

pub fn com_mojang() -> Vec<PathBuf> {
    let mut c = Vec::new();
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA").map(PathBuf::from) {
            c.push(a.join("Minecraft Bedrock").join("Users").join("Shared").join("games").join("com.mojang"));
            c.push(a.join("Minecraft Bedrock Preview").join("Users").join("Shared").join("games").join("com.mojang"));
        }
        if let Some(l) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            for pkg in ["Microsoft.MinecraftUWP_8wekyb3d8bbwe", "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe"] {
                c.push(l.join("Packages").join(pkg).join("LocalState").join("games").join("com.mojang"));
            }
        }
    } else {
        c.push(super::home().join(".local/share/mcpelauncher/games/com.mojang"));
    }
    c.into_iter().filter(|p| p.is_dir()).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McbeChannel {
    Official,
    Preview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenMode {
    BothSynced,
    BehaviorOnly,
    ResourceOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackDep {
    pub uuid: String,
    pub version: [u32; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleDep {
    pub module_name: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct PackInfo {
    pub pack: Pack,
    pub rel_dir: String,
    pub version: [u32; 3],
    pub script_entry: Option<String>,
    pub pack_deps: Vec<PackDep>,
    pub module_deps: Vec<ModuleDep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncKind {
    Synced,
    VersionMismatch,
    MissingCounterpart,
    UnlinkedPair,
    WorldPackMismatch,
    DuplicateUuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncItem {
    pub kind: SyncKind,
    pub title: String,
    pub detail: String,
    pub bp_dir: Option<PathBuf>,
    pub rp_dir: Option<PathBuf>,
    pub fixable: bool,
}

#[derive(Clone, Debug)]
pub struct McbeWorkspace {
    pub root: PathBuf,
    pub is_world_folder: bool,
    pub packs: Vec<PackInfo>,
    pub sync_items: Vec<SyncItem>,
}

#[derive(Clone, Debug)]
pub struct GenerateSpec {
    pub name: String,
    pub description: String,
    pub mode: GenMode,
    pub min_engine_version: [u32; 3],
    pub with_script_api: bool,
    pub script_deps: Vec<(String, String)>,
}

pub fn deploy_packs(list: &[Pack], channel: McbeChannel) -> Result<Vec<String>, String> {
    let base = com_mojang_for(channel).ok_or_else(|| match channel {
        McbeChannel::Official => "Minecraft Bedrock (Official) com.mojang folder was not found on this PC.".to_string(),
        McbeChannel::Preview => "Minecraft Bedrock Preview com.mojang folder was not found on this PC.".to_string(),
    })?;
    deploy_to_base(list, &base)
}

fn deploy_to_base(list: &[Pack], base: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for p in list {
        let sub = match p.kind {
            Kind::Behavior => "development_behavior_packs",
            Kind::Resource => "development_resource_packs",
            Kind::Skin => "development_skin_packs",
            _ => continue,
        };
        let folder = p.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| file_name(&p.name));
        let to = base.join(sub).join(&folder);
        let _ = std::fs::remove_dir_all(&to);
        let n = copy_dir(&p.dir, &to).map_err(|e| e.to_string())?;
        lines.push(format!("{}: {n} files to {}", p.name, to.display()));
    }
    if lines.is_empty() {
        return Err("No behavior or resource pack to copy.".into());
    }
    lines.push("Reopen the world in Minecraft to load the changes.".into());
    Ok(lines)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if skip_file(&name) {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            n += copy_dir(&p, &to.join(&name))?;
        } else {
            std::fs::copy(&p, to.join(&name))?;
            n += 1;
        }
    }
    Ok(n)
}

pub fn deploy(project: &Project) -> Result<Vec<String>, String> {
    let list = packs(&project.dir);
    let base = com_mojang().into_iter().next().ok_or("Minecraft Bedrock's data folder (com.mojang) isn't on this PC: start the game once, then try again.")?;
    deploy_to_base(&list, &base)
}

fn parse_ver3(v: Option<&Value>) -> Option<[u32; 3]> {
    match v? {
        Value::Array(a) if a.len() == 3 => Some([
            a[0].as_u64()? as u32,
            a[1].as_u64()? as u32,
            a[2].as_u64()? as u32,
        ]),
        Value::String(s) => {
            let core = s.split(['-', '+']).next()?;
            let mut it = core.split('.');
            Some([
                it.next()?.parse().ok()?,
                it.next()?.parse().ok()?,
                it.next()?.parse().ok()?,
            ])
        }
        _ => None,
    }
}

fn discover_pack_dirs(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 || out.len() >= 64 {
        return;
    }
    if root.join("manifest.json").is_file() {
        out.push(root.to_path_buf());
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else { return };
    let mut subdirs: Vec<PathBuf> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let low = name.to_ascii_lowercase();
            if matches!(low.as_str(), "node_modules" | ".git" | "dist" | "target" | "db" | "build" | ".vscode") || low.starts_with('.') {
                return None;
            }
            let p = e.path();
            p.is_dir().then_some(p)
        })
        .collect();
    subdirs.sort();
    for d in subdirs {
        discover_pack_dirs(&d, depth + 1, out);
    }
}

fn read_pack_at(d: PathBuf) -> Option<Pack> {
    let m = read_manifest(&d)?;
    let header = m.get("header")?;
    let mut name = plain_name(header.get("name").and_then(|v| v.as_str()).unwrap_or(""));
    if name == "pack.name" {
        name = std::fs::read_to_string(d.join("texts").join("en_US.lang"))
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("pack.name=").map(|v| plain_name(v.trim()))))
            .unwrap_or_default();
    }
    let uuid = header.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
    Some(Pack {
        name: if name.is_empty() {
            d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            name
        },
        uuid,
        kind: kind_of(&m),
        manifest: m,
        dir: d,
    })
}

fn pack_info(root: &Path, p: Pack) -> PackInfo {
    let h = p.manifest.get("header");
    let version = parse_ver3(h.and_then(|x| x.get("version"))).unwrap_or([1, 0, 0]);
    let script_entry = p
        .manifest
        .get("modules")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find(|m| matches!(m.get("type").and_then(Value::as_str), Some("script" | "javascript"))))
        .and_then(|m| m.get("entry"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut pack_deps = Vec::new();
    let mut module_deps = Vec::new();
    if let Some(deps) = p.manifest.get("dependencies").and_then(Value::as_array) {
        for d in deps {
            if let Some(u) = d.get("uuid").and_then(Value::as_str) {
                pack_deps.push(PackDep {
                    uuid: u.to_string(),
                    version: parse_ver3(d.get("version")).unwrap_or([1, 0, 0]),
                });
            } else if let Some(m) = d.get("module_name").and_then(Value::as_str) {
                let ver = d
                    .get("version")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| parse_ver3(d.get("version")).map(|v| format!("{}.{}.{}", v[0], v[1], v[2])))
                    .unwrap_or_default();
                module_deps.push(ModuleDep {
                    module_name: m.to_string(),
                    version: ver,
                });
            }
        }
    }
    let rel_dir = p
        .dir
        .strip_prefix(root)
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| p.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    PackInfo {
        pack: p,
        rel_dir,
        version,
        script_entry,
        pack_deps,
        module_deps,
    }
}

fn read_world_pack_refs(path: &Path) -> Vec<(String, [u32; 3])> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&strip_comments(&text)) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|e| {
            let uuid = e.get("pack_id").and_then(Value::as_str)?.to_ascii_lowercase();
            let ver = parse_ver3(e.get("version")).unwrap_or([1, 0, 0]);
            Some((uuid, ver))
        })
        .collect()
}

pub fn analyze_workspace(roots: &[PathBuf]) -> Option<McbeWorkspace> {
    let root = roots.first()?.clone();
    let is_world_folder = root.join("world_behavior_packs.json").is_file()
        || root.join("world_resource_packs.json").is_file()
        || root.join("level.dat").is_file();
    let mut dirs = Vec::new();
    for r in roots {
        discover_pack_dirs(r, 0, &mut dirs);
    }
    let mut seen_paths = std::collections::HashSet::new();
    let mut infos = Vec::new();
    for d in dirs {
        let key = d.canonicalize().unwrap_or_else(|_| d.clone());
        if !seen_paths.insert(key) {
            continue;
        }
        if let Some(p) = read_pack_at(d) {
            infos.push(pack_info(&root, p));
        }
    }
    if infos.is_empty() && !is_world_folder {
        return None;
    }

    let world_bp_uuids = read_world_pack_refs(&root.join("world_behavior_packs.json"));
    let world_rp_uuids = read_world_pack_refs(&root.join("world_resource_packs.json"));
    let mut sync_items = Vec::new();

    // 1. Check duplicate header UUIDs
    let mut seen_uuids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for info in &infos {
        let u = info.pack.uuid.to_ascii_lowercase();
        if u.is_empty() {
            continue;
        }
        if let Some(prev_name) = seen_uuids.get(&u) {
            sync_items.push(SyncItem {
                kind: SyncKind::DuplicateUuid,
                title: format!("Duplicate UUID: {}", info.pack.name),
                detail: format!("Shares header.uuid {u} with {prev_name}"),
                bp_dir: (info.pack.kind == Kind::Behavior).then(|| info.pack.dir.clone()),
                rp_dir: (info.pack.kind == Kind::Resource).then(|| info.pack.dir.clone()),
                fixable: true,
            });
        } else {
            seen_uuids.insert(u, info.pack.name.clone());
        }
    }

    let rps: Vec<&PackInfo> = infos.iter().filter(|i| i.pack.kind == Kind::Resource).collect();
    let bps: Vec<&PackInfo> = infos.iter().filter(|i| i.pack.kind == Kind::Behavior).collect();
    let mut linked_rp_uuids = std::collections::HashSet::new();

    // 2. Check BP -> RP dependencies & versions
    for bp in &bps {
        if bp.pack_deps.is_empty() {
            if let Some(rp) = rps.first() {
                sync_items.push(SyncItem {
                    kind: SyncKind::UnlinkedPair,
                    title: format!("{} <-> {}", bp.pack.name, rp.pack.name),
                    detail: "Behavior Pack is not linked to Resource Pack yet".into(),
                    bp_dir: Some(bp.pack.dir.clone()),
                    rp_dir: Some(rp.pack.dir.clone()),
                    fixable: true,
                });
            }
            continue;
        }
        for dep in &bp.pack_deps {
            let dep_u = dep.uuid.to_ascii_lowercase();
            if let Some(rp) = infos.iter().find(|p| p.pack.uuid.eq_ignore_ascii_case(&dep_u)) {
                linked_rp_uuids.insert(rp.pack.uuid.to_ascii_lowercase());
                if rp.version != dep.version {
                    sync_items.push(SyncItem {
                        kind: SyncKind::VersionMismatch,
                        title: format!("{} -> {}", bp.pack.name, rp.pack.name),
                        detail: format!(
                            "BP expects {:?}, RP is {:?}",
                            dep.version, rp.version
                        ),
                        bp_dir: Some(bp.pack.dir.clone()),
                        rp_dir: Some(rp.pack.dir.clone()),
                        fixable: true,
                    });
                } else {
                    let rev_ok = rp.pack_deps.iter().any(|rd| {
                        rd.uuid.eq_ignore_ascii_case(&bp.pack.uuid) && rd.version == bp.version
                    });
                    if rev_ok || rp.pack_deps.is_empty() {
                        sync_items.push(SyncItem {
                            kind: SyncKind::Synced,
                            title: format!("{} <-> {}", bp.pack.name, rp.pack.name),
                            detail: format!("Synced v{}.{}.{}", bp.version[0], bp.version[1], bp.version[2]),
                            bp_dir: Some(bp.pack.dir.clone()),
                            rp_dir: Some(rp.pack.dir.clone()),
                            fixable: false,
                        });
                    } else {
                        sync_items.push(SyncItem {
                            kind: SyncKind::VersionMismatch,
                            title: format!("{} -> {}", rp.pack.name, bp.pack.name),
                            detail: "RP dependency on BP does not match current BP version".into(),
                            bp_dir: Some(bp.pack.dir.clone()),
                            rp_dir: Some(rp.pack.dir.clone()),
                            fixable: true,
                        });
                    }
                }
            } else {
                sync_items.push(SyncItem {
                    kind: SyncKind::MissingCounterpart,
                    title: format!("{} misses pack {}", bp.pack.name, dep.uuid),
                    detail: "Required dependency pack is missing from this workspace (installing one side alone will fail)".into(),
                    bp_dir: Some(bp.pack.dir.clone()),
                    rp_dir: None,
                    fixable: !rps.is_empty(),
                });
            }
        }
    }

    // 3. Check RP -> missing BP
    for rp in &rps {
        if linked_rp_uuids.contains(&rp.pack.uuid.to_ascii_lowercase()) {
            continue;
        }
        for dep in &rp.pack_deps {
            if !infos.iter().any(|p| p.pack.uuid.eq_ignore_ascii_case(&dep.uuid)) {
                sync_items.push(SyncItem {
                    kind: SyncKind::MissingCounterpart,
                    title: format!("{} misses pack {}", rp.pack.name, dep.uuid),
                    detail: "Resource Pack depends on a missing Behavior Pack UUID".into(),
                    bp_dir: None,
                    rp_dir: Some(rp.pack.dir.clone()),
                    fixable: !bps.is_empty(),
                });
            }
        }
    }

    // 4. World pack registry sync check (when editing a world folder directly)
    if is_world_folder {
        for (bp_id, _) in &world_bp_uuids {
            if let Some(bp) = bps.iter().find(|p| p.pack.uuid.eq_ignore_ascii_case(bp_id)) {
                for dep in &bp.pack_deps {
                    if let Some(rp) = rps.iter().find(|r| r.pack.uuid.eq_ignore_ascii_case(&dep.uuid)) {
                        let in_world = world_rp_uuids.iter().any(|(id, ver)| {
                            id.eq_ignore_ascii_case(&rp.pack.uuid) && *ver == rp.version
                        });
                        if !in_world {
                            sync_items.push(SyncItem {
                                kind: SyncKind::WorldPackMismatch,
                                title: format!("World JSON missing {}", rp.pack.name),
                                detail: "world_behavior_packs.json activates this BP, but its paired RP is missing/outdated in world_resource_packs.json".into(),
                                bp_dir: Some(bp.pack.dir.clone()),
                                rp_dir: Some(rp.pack.dir.clone()),
                                fixable: true,
                            });
                        }
                    }
                }
            }
        }
    }

    Some(McbeWorkspace {
        root,
        is_world_folder,
        packs: infos,
        sync_items,
    })
}

pub fn sync_pair(bp_dir: &Path, rp_dir: &Path) -> Result<String, String> {
    let mut bp_val = read_manifest(bp_dir).ok_or("Cannot read BP manifest.json")?;
    let mut rp_val = read_manifest(rp_dir).ok_or("Cannot read RP manifest.json")?;
    let bp_uuid = bp_val.pointer("/header/uuid").and_then(Value::as_str).ok_or("BP missing header.uuid")?.to_string();
    let rp_uuid = rp_val.pointer("/header/uuid").and_then(Value::as_str).ok_or("RP missing header.uuid")?.to_string();
    let bp_ver = parse_ver3(bp_val.pointer("/header/version")).unwrap_or([1, 0, 0]);
    let rp_ver = parse_ver3(rp_val.pointer("/header/version")).unwrap_or([1, 0, 0]);

    let update_dep = |doc: &mut Value, target_uuid: &str, target_ver: [u32; 3]| {
        let ver_json = serde_json::json!([target_ver[0], target_ver[1], target_ver[2]]);
        let obj = doc.as_object_mut();
        let Some(root) = obj else { return };
        let deps = root.entry("dependencies").or_insert_with(|| serde_json::json!([]));
        let Some(arr) = deps.as_array_mut() else { return };
        arr.retain(|d| {
            d.get("uuid")
                .and_then(Value::as_str)
                .is_none_or(|u| !u.eq_ignore_ascii_case(target_uuid))
        });
        arr.push(serde_json::json!({ "uuid": target_uuid, "version": ver_json }));
    };

    update_dep(&mut bp_val, &rp_uuid, rp_ver);
    update_dep(&mut rp_val, &bp_uuid, bp_ver);

    let bp_txt = serde_json::to_string_pretty(&bp_val).map_err(|e| e.to_string())?;
    let rp_txt = serde_json::to_string_pretty(&rp_val).map_err(|e| e.to_string())?;
    std::fs::write(bp_dir.join("manifest.json"), format!("{bp_txt}\n")).map_err(|e| e.to_string())?;
    std::fs::write(rp_dir.join("manifest.json"), format!("{rp_txt}\n")).map_err(|e| e.to_string())?;
    Ok("Synced BP and RP manifest dependencies".into())
}

pub fn sync_world_packs(world_root: &Path, packs: &[PackInfo]) -> Result<String, String> {
    let write_reg = |file: &str, kind: Kind, existing: Vec<(String, [u32; 3])>| -> Result<usize, String> {
        let mut entries = existing;
        for info in packs.iter().filter(|p| p.pack.kind == kind) {
            let uuid = info.pack.uuid.clone();
            if uuid.is_empty() {
                continue;
            }
            if let Some(pos) = entries.iter().position(|(u, _)| u.eq_ignore_ascii_case(&uuid)) {
                entries[pos].1 = info.version;
            } else {
                entries.push((uuid, info.version));
            }
        }
        let arr: Vec<Value> = entries
            .iter()
            .map(|(id, v)| serde_json::json!({ "pack_id": id, "version": [v[0], v[1], v[2]] }))
            .collect();
        let text = serde_json::to_string_pretty(&arr).map_err(|e| e.to_string())?;
        std::fs::write(world_root.join(file), format!("{text}\n")).map_err(|e| e.to_string())?;
        Ok(arr.len())
    };
    let bp_existing = read_world_pack_refs(&world_root.join("world_behavior_packs.json"));
    let rp_existing = read_world_pack_refs(&world_root.join("world_resource_packs.json"));
    let b = write_reg("world_behavior_packs.json", Kind::Behavior, bp_existing)?;
    let r = write_reg("world_resource_packs.json", Kind::Resource, rp_existing)?;
    Ok(format!("Synced world pack JSON ({b} BP, {r} RP)"))
}

pub fn regenerate_duplicate_uuids(packs: &[PackInfo]) -> Result<Vec<String>, String> {
    let mut seen = std::collections::HashSet::new();
    let mut fixed = Vec::new();
    for info in packs {
        let mut doc = read_manifest(&info.pack.dir).ok_or_else(|| format!("Cannot read {}/manifest.json", info.rel_dir))?;
        let mut changed = false;
        let cur = info.pack.uuid.to_ascii_lowercase();
        if !is_uuid(&cur) || !seen.insert(cur) {
            let fresh = uuid4();
            if let Some(h) = doc.get_mut("header").and_then(Value::as_object_mut) {
                h.insert("uuid".into(), Value::String(fresh.clone()));
                seen.insert(fresh.to_ascii_lowercase());
                changed = true;
            }
        }
        if let Some(mods) = doc.get_mut("modules").and_then(Value::as_array_mut) {
            for m in mods {
                let mu = m.get("uuid").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
                if !is_uuid(&mu) || !seen.insert(mu) {
                    let fresh = uuid4();
                    if let Some(obj) = m.as_object_mut() {
                        obj.insert("uuid".into(), Value::String(fresh.clone()));
                        seen.insert(fresh.to_ascii_lowercase());
                        changed = true;
                    }
                }
            }
        }
        if changed {
            let txt = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
            std::fs::write(info.pack.dir.join("manifest.json"), format!("{txt}\n")).map_err(|e| e.to_string())?;
            fixed.push(info.pack.name.clone());
        }
    }
    Ok(fixed)
}

pub fn upsert_script_dep(bp_dir: &Path, module_name: &str, version: &str) -> Result<String, String> {
    let mut doc = read_manifest(bp_dir).ok_or("Cannot read BP manifest.json")?;
    let Some(root) = doc.as_object_mut() else {
        return Err("manifest.json root is not an object".into());
    };
    let deps = root.entry("dependencies").or_insert_with(|| serde_json::json!([]));
    let Some(arr) = deps.as_array_mut() else {
        return Err("dependencies is not an array".into());
    };
    arr.retain(|d| d.get("module_name").and_then(Value::as_str) != Some(module_name));
    arr.push(serde_json::json!({ "module_name": module_name, "version": version }));

    let has_script = root
        .get("modules")
        .and_then(Value::as_array)
        .is_some_and(|ms| ms.iter().any(|m| matches!(m.get("type").and_then(Value::as_str), Some("script" | "javascript"))));
    if !has_script {
        let mods = root.entry("modules").or_insert_with(|| serde_json::json!([]));
        if let Some(ms) = mods.as_array_mut() {
            ms.push(serde_json::json!({
                "type": "script",
                "language": "javascript",
                "uuid": uuid4(),
                "version": [1, 0, 0],
                "entry": "scripts/main.js"
            }));
        }
        let scripts_dir = bp_dir.join("scripts");
        let _ = std::fs::create_dir_all(&scripts_dir);
        let main_js = scripts_dir.join("main.js");
        if !main_js.exists() {
            let _ = std::fs::write(&main_js, "import { world, system } from \"@minecraft/server\";\n\nworld.afterEvents.worldLoad.subscribe(() => {\n  console.warn(\"Script API ready\");\n});\n");
        }
    }
    let txt = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(bp_dir.join("manifest.json"), format!("{txt}\n")).map_err(|e| e.to_string())?;
    Ok(format!("Updated {module_name} @ {version}"))
}

pub fn generate_manifests(root: &Path, spec: &GenerateSpec) -> Result<Vec<PathBuf>, String> {
    let clean = spec.name.trim();
    if clean.is_empty() {
        return Err("Give the pack a name first.".into());
    }
    let slug = file_name(clean).replace(' ', "_");
    let use_subfolders = root.join("behavior_packs").is_dir() || root.join("resource_packs").is_dir();
    let bp_dir = if use_subfolders {
        root.join("behavior_packs").join(format!("{slug}_BP"))
    } else if root.join("manifest.json").is_file() || root.join("BP").exists() {
        root.join(format!("{slug}_BP"))
    } else {
        root.join("BP")
    };
    let rp_dir = if use_subfolders {
        root.join("resource_packs").join(format!("{slug}_RP"))
    } else if root.join("manifest.json").is_file() || root.join("RP").exists() {
        root.join(format!("{slug}_RP"))
    } else {
        root.join("RP")
    };
    let desc = if spec.description.trim().is_empty() {
        format!("{clean} Bedrock add-on")
    } else {
        spec.description.trim().to_string()
    };
    let me = spec.min_engine_version;
    let bp_uuid = uuid4();
    let rp_uuid = uuid4();
    let mut written = Vec::new();

    if matches!(spec.mode, GenMode::BothSynced | GenMode::BehaviorOnly) {
        if bp_dir.join("manifest.json").exists() {
            return Err(format!("{} already has manifest.json", bp_dir.display()));
        }
        std::fs::create_dir_all(bp_dir.join("texts")).map_err(|e| e.to_string())?;
        let mut modules = vec![serde_json::json!({ "type": "data", "uuid": uuid4(), "version": [1, 0, 0] })];
        if spec.with_script_api {
            std::fs::create_dir_all(bp_dir.join("scripts")).map_err(|e| e.to_string())?;
            modules.push(serde_json::json!({
                "type": "script",
                "language": "javascript",
                "uuid": uuid4(),
                "version": [1, 0, 0],
                "entry": "scripts/main.js"
            }));
            let stub = if spec.script_deps.iter().any(|(m, _)| m == "@minecraft/server-ui") {
                concat!(
                    "import { world, system } from \"@minecraft/server\";\n",
                    "import { ActionFormData } from \"@minecraft/server-ui\";\n\n",
                    "world.afterEvents.worldLoad.subscribe(() => {\n",
                    "  system.run(() => console.warn(\"MCBE Script + UI ready\"));\n",
                    "});\n",
                )
            } else {
                concat!(
                    "import { world, system } from \"@minecraft/server\";\n\n",
                    "world.afterEvents.worldLoad.subscribe(() => {\n",
                    "  system.run(() => console.warn(\"MCBE Script ready\"));\n",
                    "});\n",
                )
            };
            std::fs::write(bp_dir.join("scripts").join("main.js"), stub).map_err(|e| e.to_string())?;
        }
        let mut deps = Vec::new();
        if spec.mode == GenMode::BothSynced {
            deps.push(serde_json::json!({ "uuid": rp_uuid, "version": [1, 0, 0] }));
        }
        if spec.with_script_api {
            for (mod_name, ver) in &spec.script_deps {
                deps.push(serde_json::json!({ "module_name": mod_name, "version": ver }));
            }
        }
        let bp_json = serde_json::json!({
            "format_version": 2,
            "header": {
                "name": "pack.name",
                "description": "pack.description",
                "uuid": bp_uuid,
                "version": [1, 0, 0],
                "min_engine_version": [me[0], me[1], me[2]]
            },
            "modules": modules,
            "dependencies": deps
        });
        let p = bp_dir.join("manifest.json");
        std::fs::write(&p, format!("{}\n", serde_json::to_string_pretty(&bp_json).unwrap_or_default())).map_err(|e| e.to_string())?;
        std::fs::write(bp_dir.join("texts").join("en_US.lang"), format!("pack.name={clean} BP\npack.description={desc}\n")).map_err(|e| e.to_string())?;
        written.push(p);
    }

    if matches!(spec.mode, GenMode::BothSynced | GenMode::ResourceOnly) {
        if rp_dir.join("manifest.json").exists() {
            return Err(format!("{} already has manifest.json", rp_dir.display()));
        }
        std::fs::create_dir_all(rp_dir.join("texts")).map_err(|e| e.to_string())?;
        let deps = if spec.mode == GenMode::BothSynced {
            vec![serde_json::json!({ "uuid": bp_uuid, "version": [1, 0, 0] })]
        } else {
            Vec::new()
        };
        let rp_json = serde_json::json!({
            "format_version": 2,
            "header": {
                "name": "pack.name",
                "description": "pack.description",
                "uuid": rp_uuid,
                "version": [1, 0, 0],
                "min_engine_version": [me[0], me[1], me[2]]
            },
            "modules": [{ "type": "resources", "uuid": uuid4(), "version": [1, 0, 0] }],
            "dependencies": deps
        });
        let p = rp_dir.join("manifest.json");
        std::fs::write(&p, format!("{}\n", serde_json::to_string_pretty(&rp_json).unwrap_or_default())).map_err(|e| e.to_string())?;
        std::fs::write(rp_dir.join("texts").join("en_US.lang"), format!("pack.name={clean} RP\npack.description={desc}\n")).map_err(|e| e.to_string())?;
        written.push(p);
    }

    Ok(written)
}

pub fn facts(p: &Project) -> String {
    let list = packs(&p.dir);
    if list.is_empty() {
        return String::new();
    }
    let names: Vec<String> = list
        .iter()
        .map(|k| {
            let kind = match k.kind {
                Kind::Behavior => "behavior",
                Kind::Resource => "resource",
                Kind::Skin => "skin",
                Kind::World => "world template",
                Kind::Other => "other",
            };
            let script = k
                .manifest
                .get("modules")
                .and_then(|m| m.as_array())
                .and_then(|a| a.iter().find(|x| x.get("type").and_then(|t| t.as_str()) == Some("script")))
                .and_then(|x| x.get("entry"))
                .and_then(|e| e.as_str());
            match script {
                Some(e) => format!("{} ({kind} pack, script entry {e})", k.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                None => format!("{} ({kind} pack)", k.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            }
        })
        .collect();
    let target = com_mojang().into_iter().next().map(|b| format!(" Deploy copies into {}.", b.display())).unwrap_or_default();
    format!(" Packs: {}.{target}", names.join(", "))
}


pub fn uuid4() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::getrandom(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn new_addon(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the add-on a name.".into());
    }
    let dir = parent.join(file_name(name));
    if dir.exists() {
        return Err(format!("{} already exists", dir.display()));
    }
    let (bp, rp) = (dir.join("BP"), dir.join("RP"));
    for d in [bp.join("functions"), bp.join("texts"), rp.join("texts")] {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let (bp_id, rp_id) = (uuid4(), uuid4());
    let manifest = |header: &str, module: &str, kind: &str, dep: &str, desc: &str| {
        serde_json::to_string_pretty(&serde_json::json!({
            "format_version": 2,
            "header": { "name": "pack.name", "description": desc, "uuid": header, "version": [1, 0, 0], "min_engine_version": [1, 21, 0] },
            "modules": [{ "type": kind, "uuid": module, "version": [1, 0, 0] }],
            "dependencies": [{ "uuid": dep, "version": [1, 0, 0] }]
        }))
        .unwrap_or_default()
    };
    let w = |p: PathBuf, t: String| std::fs::write(p, t).map_err(|e| e.to_string());
    w(bp.join("manifest.json"), manifest(&bp_id, &uuid4(), "data", &rp_id, "pack.description"))?;
    w(rp.join("manifest.json"), manifest(&rp_id, &uuid4(), "resources", &bp_id, "pack.description"))?;
    w(bp.join("functions").join("hello.mcfunction"), format!("say Hello from {name}!\n"))?;
    w(bp.join("texts").join("en_US.lang"), format!("pack.name={name} BP\npack.description=Behavior pack of {name}\n"))?;
    w(rp.join("texts").join("en_US.lang"), format!("pack.name={name} RP\npack.description=Resource pack of {name}\n"))?;
    w(dir.join(".gitignore"), "dist/\n".into())?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_names() {
        assert_eq!(strip_comments("{\"a\": \"//x\", // note\n \"b\": 1 /* c */}"), "{\"a\": \"//x\", \n \"b\": 1 }");
        assert_eq!(plain_name("§l§6My Pack"), "My Pack");
        assert!(is_uuid(&uuid4()));
        assert!(!is_uuid("1234"));
    }

    #[test]
    fn a_new_addon_is_valid_and_packs() {
        let d = std::env::temp_dir().join(format!("fx-bedrock-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let dir = new_addon(&d, "Test Addon").unwrap();
        let list = packs(&dir);
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|p| p.kind == Kind::Behavior) && list.iter().any(|p| p.kind == Kind::Resource));
        assert!(validate(&list).is_empty(), "{:?}", validate(&list));
        let p = Project { ext: "bedrock".into(), dir: dir.clone(), name: "Test Addon".into() };
        let lines = pack(&p).unwrap();
        assert!(dir.join("dist").join("Test Addon.mcaddon").is_file(), "{lines:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn catches_broken_manifests() {
        let m: Value = serde_json::json!({
            "format_version": 2,
            "header": { "name": "X", "uuid": "not-a-uuid", "version": [1, 0], "min_engine_version": [1, 21, 0] },
            "modules": [{ "type": "data", "uuid": "11111111-1111-4111-8111-111111111111", "version": [1, 0, 0] },
                        { "type": "dataa", "uuid": "11111111-1111-4111-8111-111111111111", "version": [1, 0, 0] }]
        });
        let p = Pack { dir: PathBuf::from("BP"), name: "X".into(), uuid: "not-a-uuid".into(), kind: Kind::Behavior, manifest: m };
        let errs: Vec<String> = validate(&[p]).into_iter().map(|(_, e)| e).collect();
        assert!(errs.iter().any(|e| e.contains("header.uuid")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("header.version")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("used twice")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("not a pack type")), "{errs:?}");
    }

    #[test]
    fn analyzes_sync_mismatch_world_json_and_generates_synced_manifests() {
        let d = std::env::temp_dir().join(format!("fx-mcbe-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("behavior_packs")).unwrap();
        std::fs::create_dir_all(d.join("resource_packs")).unwrap();
        std::fs::write(d.join("level.dat"), b"x").unwrap();
        let written = generate_manifests(
            &d,
            &GenerateSpec {
                name: "CoolPack".into(),
                description: "Test sync".into(),
                mode: GenMode::BothSynced,
                min_engine_version: [1, 26, 65],
                with_script_api: true,
                script_deps: vec![("@minecraft/server".into(), "2.11.0".into())],
            },
        )
        .unwrap();
        assert_eq!(written.len(), 2);
        let ws = analyze_workspace(std::slice::from_ref(&d)).unwrap();
        assert!(ws.is_world_folder);
        assert_eq!(ws.packs.len(), 2);
        assert!(ws.sync_items.iter().any(|s| s.kind == SyncKind::Synced));

        let bp_dir = d.join("behavior_packs/CoolPack_BP");
        let rp_dir = d.join("resource_packs/CoolPack_RP");
        let mut rp_json = read_manifest(&rp_dir).unwrap();
        rp_json["header"]["version"] = serde_json::json!([1, 2, 0]);
        std::fs::write(rp_dir.join("manifest.json"), serde_json::to_string_pretty(&rp_json).unwrap()).unwrap();
        let ws2 = analyze_workspace(std::slice::from_ref(&d)).unwrap();
        assert!(ws2.sync_items.iter().any(|s| s.kind == SyncKind::VersionMismatch));

        sync_pair(&bp_dir, &rp_dir).unwrap();
        let ws3 = analyze_workspace(std::slice::from_ref(&d)).unwrap();
        assert!(ws3.sync_items.iter().any(|s| s.kind == SyncKind::Synced));

        let bp_uuid = ws3.packs.iter().find(|p| p.pack.kind == Kind::Behavior).unwrap().pack.uuid.clone();
        std::fs::write(
            d.join("world_behavior_packs.json"),
            serde_json::to_string_pretty(&serde_json::json!([{ "pack_id": bp_uuid, "version": [1, 0, 0] }])).unwrap(),
        )
        .unwrap();
        let ws4 = analyze_workspace(std::slice::from_ref(&d)).unwrap();
        assert!(ws4.sync_items.iter().any(|s| s.kind == SyncKind::WorldPackMismatch));
        sync_world_packs(&d, &ws4.packs).unwrap();
        let ws5 = analyze_workspace(std::slice::from_ref(&d)).unwrap();
        assert!(!ws5.sync_items.iter().any(|s| s.kind == SyncKind::WorldPackMismatch));

        upsert_script_dep(&bp_dir, "@minecraft/server-ui", "2.3.0").unwrap();
        let ws6 = analyze_workspace(std::slice::from_ref(&d)).unwrap();
        let bp_info = ws6.packs.iter().find(|p| p.pack.kind == Kind::Behavior).unwrap();
        assert!(bp_info.module_deps.iter().any(|m| m.module_name == "@minecraft/server-ui" && m.version == "2.3.0"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
