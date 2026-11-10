//! Minecraft Bedrock add-ons: check the manifests, pack `.mcpack` / `.mcaddon`, copy
//! the packs into the game's development folders, and write a new add-on.

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

/// Bedrock reads manifests with `//` and `/* */` comments: drop them (not inside strings).
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
    // Only ASCII bytes were dropped, so the rest is still UTF-8.
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

/// `§l§6My Pack` → `My Pack`.
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

/// The packs of an add-on: the folder itself, or the packs right inside it.
pub fn packs(dir: &Path) -> Vec<Pack> {
    let mut dirs = vec![dir.to_path_buf()];
    if !dir.join("manifest.json").is_file() {
        dirs = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.join("manifest.json").is_file()).collect()).unwrap_or_default();
        dirs.sort();
    }
    dirs.into_iter()
        .filter_map(|d| {
            let m = read_manifest(&d)?;
            let header = m.get("header")?;
            let mut name = plain_name(header.get("name").and_then(|v| v.as_str()).unwrap_or(""));
            // `pack.name` is a key in texts/en_US.lang.
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

/// Problems in an add-on's manifests: (pack folder, message). Empty: all good.
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

/// Pack an add-on into `<dir>/dist`: one `.mcpack`, or a `.mcaddon` with every pack.
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

/// The game's com.mojang folders, newest layout first (the GDK build since 1.21.120 keeps
/// them under %APPDATA%\Minecraft Bedrock; the Store build under LocalState).
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

/// Copy each pack into development_behavior_packs / development_resource_packs, so the
/// game loads the current files (reopen the world to see changes).
pub fn deploy(project: &Project) -> Result<Vec<String>, String> {
    let list = packs(&project.dir);
    let base = com_mojang().into_iter().next().ok_or("Minecraft Bedrock's data folder (com.mojang) isn't on this PC: start the game once, then try again.")?;
    let mut lines = Vec::new();
    for p in &list {
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

/// For the agent: the packs, their kind and how they depend on each other.
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
            let script = k.manifest.get("modules").and_then(|m| m.as_array()).and_then(|a| a.iter().find(|x| x.get("type").and_then(|t| t.as_str()) == Some("script"))).and_then(|x| x.get("entry")).and_then(|e| e.as_str());
            match script {
                Some(e) => format!("{} ({kind} pack, script entry {e})", k.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                None => format!("{} ({kind} pack)", k.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            }
        })
        .collect();
    let target = com_mojang().into_iter().next().map(|b| format!(" Deploy copies into {}.", b.display())).unwrap_or_default();
    format!(" Packs: {}.{target}", names.join(", "))
}

// ------------------------------------------------------------------ new add-on

/// A random version 4 UUID.
pub fn uuid4() -> String {
    let mut b: [u8; 16] = crate::collab::net::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Write `parent/<Name>/BP` and `RP`: a behavior pack that needs its resource pack, a
/// function to try (`/function hello`), and the pack names in en_US.lang.
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
}
