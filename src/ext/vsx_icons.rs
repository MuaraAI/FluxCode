use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

pub const DEFAULT_ID: &str = "";
const MAX_THEME_BYTES: u64 = 8 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconThemeMeta {
    pub id: String,
    pub label: String,
    pub ext_id: String,
    pub ext_display: String,
    pub ext_folder: PathBuf,
    pub path: PathBuf,
}

#[derive(Default)]
struct IconMap {
    file: Option<Arc<PathBuf>>,
    folder: Option<Arc<PathBuf>>,
    folder_open: Option<Arc<PathBuf>>,
    root_folder: Option<Arc<PathBuf>>,
    root_folder_open: Option<Arc<PathBuf>>,
    file_names: HashMap<String, Arc<PathBuf>>,
    file_extensions: HashMap<String, Arc<PathBuf>>,
    folder_names: HashMap<String, Arc<PathBuf>>,
    folder_names_open: HashMap<String, Arc<PathBuf>>,
    language_ids: HashMap<String, Arc<PathBuf>>,
}

pub struct LoadedIconTheme {
    id: String,
    label: String,
    hides_explorer_arrows: bool,
    canon_ext_folder: PathBuf,
    dark: IconMap,
    light: Option<IconMap>,
    verified: Mutex<HashMap<PathBuf, bool>>,
}

#[derive(Default)]
struct IconState {
    installed: Vec<IconThemeMeta>,
    active_id: String,
    loaded: Option<Arc<LoadedIconTheme>>,
}

static STATE: OnceLock<RwLock<IconState>> = OnceLock::new();

fn state() -> &'static RwLock<IconState> {
    STATE.get_or_init(|| RwLock::new(IconState::default()))
}

fn normalize_id(id: &str) -> &str {
    match id.trim() {
        "" | "fluxcode" | "default" => DEFAULT_ID,
        other => other,
    }
}

fn clean_join(root: &Path, base: &Path, raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }
    let rel = Path::new(&normalized);
    if rel.is_absolute() {
        return None;
    }
    let mut stack: Vec<std::ffi::OsString> = base
        .strip_prefix(root)
        .ok()?
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_os_string()),
            _ => None,
        })
        .collect();
    for c in rel.components() {
        match c {
            Component::CurDir => {}
            Component::Normal(s) => stack.push(s.to_os_string()),
            Component::ParentDir => {
                stack.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if stack.is_empty() {
        return None;
    }
    let mut out = root.to_path_buf();
    for seg in stack {
        out.push(seg);
    }
    Some(out)
}

fn insert_mapped(
    target: &mut HashMap<String, Arc<PathBuf>>,
    obj: Option<&Value>,
    defs: &HashMap<String, Arc<PathBuf>>,
    strip_leading_dot: bool,
) {
    let Some(map) = obj.and_then(Value::as_object) else {
        return;
    };
    for (k, v) in map {
        let Some(def_id) = v.as_str() else { continue };
        let Some(path) = defs.get(def_id) else { continue };
        let mut key = k.trim().to_ascii_lowercase();
        if strip_leading_dot {
            while let Some(rest) = key.strip_prefix('.') {
                key = rest.to_string();
            }
        }
        if !key.is_empty() {
            target.insert(key, Arc::clone(path));
        }
    }
}

fn build_map(obj: &Map<String, Value>, defs: &HashMap<String, Arc<PathBuf>>) -> IconMap {
    let def = |key: &str| {
        obj.get(key)
            .and_then(Value::as_str)
            .and_then(|id| defs.get(id))
            .map(Arc::clone)
    };
    let mut map = IconMap {
        file: def("file"),
        folder: def("folder"),
        folder_open: def("folderExpanded"),
        root_folder: def("rootFolder"),
        root_folder_open: def("rootFolderExpanded"),
        ..Default::default()
    };
    insert_mapped(&mut map.file_names, obj.get("fileNames"), defs, false);
    insert_mapped(
        &mut map.file_extensions,
        obj.get("fileExtensions"),
        defs,
        true,
    );
    insert_mapped(&mut map.folder_names, obj.get("folderNames"), defs, false);
    insert_mapped(
        &mut map.folder_names_open,
        obj.get("folderNamesExpanded"),
        defs,
        false,
    );
    insert_mapped(&mut map.language_ids, obj.get("languageIds"), defs, false);
    map
}

fn vscode_lang_alias(tok: &str) -> Option<&'static str> {
    Some(match tok {
        "tsx" => "typescriptreact",
        "bash" => "shellscript",
        "cs" => "csharp",
        "fs" => "fsharp",
        "kt" => "kotlin",
        "rb" => "ruby",
        "pl" => "perl",
        "hs" | "lhs" => "haskell",
        "ex" => "elixir",
        "erl" => "erlang",
        "clj" => "clojure",
        "jl" => "julia",
        "cr" => "crystal",
        "pas" => "pascal",
        "m" => "objective-c",
        "mm" => "objective-cpp",
        "Dockerfile" => "dockerfile",
        "Makefile" => "makefile",
        "gitignore" => "ignore",
        "tf" => "terraform",
        "text" => "plaintext",
        "j2" => "jinja",
        "tex" => "latex",
        _ => return None,
    })
}

impl LoadedIconTheme {
    pub fn load(meta: &IconThemeMeta) -> Result<Self, String> {
        let len = std::fs::metadata(&meta.path)
            .map_err(|e| e.to_string())?
            .len();
        if len > MAX_THEME_BYTES {
            return Err(format!("{} is larger than 8 MB", meta.path.display()));
        }
        let text = std::fs::read_to_string(&meta.path).map_err(|e| e.to_string())?;
        let val = super::vsx_theme::parse_jsonc(&text)
            .ok_or_else(|| format!("{} is not valid icon theme JSON", meta.path.display()))?;
        let root_obj = val
            .as_object()
            .ok_or_else(|| format!("{} must be a JSON object", meta.path.display()))?;
        let hides_explorer_arrows = root_obj
            .get("hidesExplorerArrows")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let canon_ext_folder = meta
            .ext_folder
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let canon_path = meta.path.canonicalize().map_err(|e| e.to_string())?;
        let json_dir = canon_path.parent().unwrap_or(&canon_ext_folder);

        let mut shared_paths: HashMap<PathBuf, Arc<PathBuf>> = HashMap::new();
        let mut defs: HashMap<String, Arc<PathBuf>> = HashMap::new();
        if let Some(def_map) = root_obj.get("iconDefinitions").and_then(Value::as_object) {
            for (def_id, def_val) in def_map {
                let Some(raw_path) = def_val.get("iconPath").and_then(Value::as_str) else {
                    continue;
                };
                let Some(resolved) = clean_join(&canon_ext_folder, json_dir, raw_path) else {
                    continue;
                };
                let arc = shared_paths
                    .entry(resolved.clone())
                    .or_insert_with(|| Arc::new(resolved))
                    .clone();
                defs.insert(def_id.clone(), arc);
            }
        }

        let dark = build_map(root_obj, &defs);
        let light = root_obj
            .get("light")
            .and_then(Value::as_object)
            .map(|o| build_map(o, &defs));

        Ok(Self {
            id: meta.id.clone(),
            label: meta.label.clone(),
            hides_explorer_arrows,
            canon_ext_folder,
            dark,
            light,
            verified: Mutex::new(HashMap::new()),
        })
    }

    fn verify(&self, candidate: &Arc<PathBuf>) -> Option<Arc<PathBuf>> {
        if let Some(&ok) = self
            .verified
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(candidate.as_ref())
        {
            return ok.then(|| Arc::clone(candidate));
        }
        let ok = candidate.is_file()
            && candidate
                .canonicalize()
                .is_ok_and(|c| c.starts_with(&self.canon_ext_folder));
        self.verified
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((**candidate).clone(), ok);
        ok.then(|| Arc::clone(candidate))
    }

    fn lookup_in_maps<'a>(&'a self, dark: bool, mut f: impl FnMut(&'a IconMap) -> Option<&'a Arc<PathBuf>>) -> Option<Arc<PathBuf>> {
        if !dark {
            if let Some(light) = &self.light {
                if let Some(p) = f(light).and_then(|c| self.verify(c)) {
                    return Some(p);
                }
            }
        }
        f(&self.dark).and_then(|c| self.verify(c))
    }

    pub fn resolve_file(&self, name: &str, dark: bool) -> Option<Arc<PathBuf>> {
        if !name.is_empty() {
            let lower = name.to_ascii_lowercase();
            if let Some(p) = self.lookup_in_maps(dark, |m| m.file_names.get(&lower)) {
                return Some(p);
            }
            for (dot, _) in lower.match_indices('.') {
                if dot == 0 || dot + 1 >= lower.len() {
                    continue;
                }
                let ext = &lower[dot + 1..];
                if let Some(p) = self.lookup_in_maps(dark, |m| m.file_extensions.get(ext)) {
                    return Some(p);
                }
            }
            let (lang, _) = crate::files::language_for(Path::new(&lower), "");
            let lang_lower = lang.to_ascii_lowercase();
            if let Some(p) = self.lookup_in_maps(dark, |m| {
                m.language_ids
                    .get(&lang_lower)
                    .or_else(|| vscode_lang_alias(lang).and_then(|a| m.language_ids.get(a)))
            }) {
                return Some(p);
            }
        }
        self.lookup_in_maps(dark, |m| m.file.as_ref())
    }

    pub fn resolve_folder(&self, name: &str, open: bool, root: bool, dark: bool) -> Option<Arc<PathBuf>> {
        if root {
            if open {
                if let Some(p) = self.lookup_in_maps(dark, |m| {
                    m.root_folder_open.as_ref().or(m.root_folder.as_ref())
                }) {
                    return Some(p);
                }
            } else if let Some(p) = self.lookup_in_maps(dark, |m| m.root_folder.as_ref()) {
                return Some(p);
            }
        }
        if !name.is_empty() {
            let lower = name.to_ascii_lowercase();
            if let Some(p) = self.lookup_in_maps(dark, |m| {
                if open {
                    m.folder_names_open
                        .get(&lower)
                        .or_else(|| m.folder_names.get(&lower))
                } else {
                    m.folder_names.get(&lower)
                }
            }) {
                return Some(p);
            }
        }
        self.lookup_in_maps(dark, |m| {
            if open {
                m.folder_open.as_ref().or(m.folder.as_ref())
            } else {
                m.folder.as_ref()
            }
        })
    }
}

pub fn set_installed(metas: Vec<IconThemeMeta>) {
    let reload_meta = {
        let mut s = state().write().unwrap_or_else(|e| e.into_inner());
        s.installed = metas;
        let meta = if s.active_id.is_empty() {
            None
        } else {
            s.installed.iter().find(|m| m.id == s.active_id).cloned()
        };
        if meta.is_none() {
            s.active_id.clear();
            s.loaded = None;
        }
        meta
    };
    if let Some(meta) = reload_meta {
        match LoadedIconTheme::load(&meta) {
            Ok(loaded) => {
                let mut s = state().write().unwrap_or_else(|e| e.into_inner());
                if s.active_id == meta.id {
                    s.loaded = Some(Arc::new(loaded));
                }
            }
            Err(_) => {
                let mut s = state().write().unwrap_or_else(|e| e.into_inner());
                if s.active_id == meta.id {
                    s.active_id.clear();
                    s.loaded = None;
                }
            }
        }
    }
}

pub fn list() -> Vec<IconThemeMeta> {
    state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .installed
        .clone()
}

pub fn active_id() -> String {
    state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .active_id
        .clone()
}

pub fn active_label() -> String {
    state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .as_ref()
        .map(|l| l.label.clone())
        .unwrap_or_default()
}

pub fn is_active() -> bool {
    state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .is_some()
}

pub fn hides_explorer_arrows() -> bool {
    state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .as_ref()
        .is_some_and(|l| l.hides_explorer_arrows)
}

pub fn set_active(id: &str) -> Result<Option<String>, String> {
    let id = normalize_id(id);
    if id.is_empty() {
        let mut s = state().write().unwrap_or_else(|e| e.into_inner());
        s.active_id.clear();
        s.loaded = None;
        return Ok(None);
    }
    let meta = {
        let s = state().read().unwrap_or_else(|e| e.into_inner());
        if s.active_id == id {
            if let Some(loaded) = &s.loaded {
                return Ok(Some(loaded.label.clone()));
            }
        }
        s.installed.iter().find(|m| m.id == id).cloned()
    };
    let Some(meta) = meta else {
        let mut s = state().write().unwrap_or_else(|e| e.into_inner());
        s.active_id.clear();
        s.loaded = None;
        return Err(format!("Icon theme {id} is not installed"));
    };
    match LoadedIconTheme::load(&meta) {
        Ok(loaded) => {
            let label = loaded.label.clone();
            let mut s = state().write().unwrap_or_else(|e| e.into_inner());
            s.active_id = loaded.id.clone();
            s.loaded = Some(Arc::new(loaded));
            Ok(Some(label))
        }
        Err(e) => {
            let mut s = state().write().unwrap_or_else(|e| e.into_inner());
            s.active_id.clear();
            s.loaded = None;
            Err(e)
        }
    }
}

pub fn file_icon(name: &str, dark: bool) -> Option<Arc<PathBuf>> {
    let loaded = state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .as_ref()
        .map(Arc::clone)?;
    loaded.resolve_file(name, dark)
}

pub fn folder_icon(name: &str, open: bool, root: bool, dark: bool) -> Option<Arc<PathBuf>> {
    let loaded = state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .as_ref()
        .map(Arc::clone)?;
    loaded.resolve_folder(name, open, root, dark)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_svg(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"/>").unwrap();
    }

    #[test]
    fn parses_resolves_light_dark_and_rejects_traversal() {
        let dir = std::env::temp_dir().join(format!("fluxcode-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ext = dir.join("acme.icons-1.0.0");
        let theme_json = ext.join("themes/dim.json");
        let outside = dir.join("escape.svg");
        write_svg(&outside);
        write_svg(&ext.join("themes/dark/file.svg"));
        write_svg(&ext.join("themes/dark/folder.svg"));
        write_svg(&ext.join("themes/dark/folder-open.svg"));
        write_svg(&ext.join("themes/dark/root.svg"));
        write_svg(&ext.join("themes/dark/root-open.svg"));
        write_svg(&ext.join("themes/dark/folder-src.svg"));
        write_svg(&ext.join("themes/dark/folder-src-open.svg"));
        write_svg(&ext.join("themes/dark/js.svg"));
        write_svg(&ext.join("themes/dark/ts.svg"));
        write_svg(&ext.join("themes/dark/ts-def.svg"));
        write_svg(&ext.join("themes/dark/pkg.svg"));
        write_svg(&ext.join("themes/dark/rust.svg"));
        write_svg(&ext.join("themes/light/js.svg"));
        write_svg(&ext.join("themes/light/file.svg"));

        std::fs::write(
            &theme_json,
            r#"{
              // JSONC comment & trailing commas allowed
              "hidesExplorerArrows": true,
              "file": "f",
              "folder": "fld",
              "folderExpanded": "fld_o",
              "rootFolder": "rfld",
              "rootFolderExpanded": "rfld_o",
              "iconDefinitions": {
                "f": { "iconPath": "./dark/file.svg" },
                "fld": { "iconPath": "./dark/folder.svg" },
                "fld_o": { "iconPath": "./dark/folder-open.svg" },
                "rfld": { "iconPath": "./dark/root.svg" },
                "rfld_o": { "iconPath": "./dark/root-open.svg" },
                "fld_src": { "iconPath": "./dark/folder-src.svg" },
                "fld_src_o": { "iconPath": "./dark/folder-src-open.svg" },
                "js": { "iconPath": "./dark/js.svg" },
                "ts": { "iconPath": "./dark/ts.svg" },
                "d_ts": { "iconPath": "./dark/ts-def.svg" },
                "pkg": { "iconPath": "./dark/pkg.svg" },
                "rs": { "iconPath": "./dark/rust.svg" },
                "l_js": { "iconPath": "./light/js.svg" },
                "l_f": { "iconPath": "./light/file.svg" },
                "bad_traversal": { "iconPath": "../../escape.svg" },
                "bad_missing": { "iconPath": "./dark/does-not-exist.svg" },
                "font_only": { "fontCharacter": "\\E001" }
              },
              "fileNames": {
                "package.json": "pkg",
                ".gitignore": "bad_traversal",
                "broken.cfg": "bad_missing"
              },
              "fileExtensions": {
                "js": "js",
                "ts": "ts",
                "d.ts": "d_ts"
              },
              "folderNames": {
                "src": "fld_src"
              },
              "folderNamesExpanded": {
                "src": "fld_src_o"
              },
              "languageIds": {
                "rust": "rs",
              },
              "light": {
                "file": "l_f",
                "fileExtensions": {
                  "js": "l_js",
                },
              },
            }"#,
        )
        .unwrap();

        let meta = IconThemeMeta {
            id: "vsx:acme.icons/dim".into(),
            label: "Acme Dim".into(),
            ext_id: "acme.icons".into(),
            ext_display: "Acme".into(),
            ext_folder: ext.clone(),
            path: theme_json,
        };
        let loaded = LoadedIconTheme::load(&meta).unwrap();
        assert!(loaded.hides_explorer_arrows);

        // Priority: fileNames > longest multi-dot extension > languageIds > default file
        assert!(loaded.resolve_file("PACKAGE.JSON", true).unwrap().ends_with("pkg.svg"));
        assert!(loaded.resolve_file("types.d.ts", true).unwrap().ends_with("ts-def.svg"));
        assert!(loaded.resolve_file("index.ts", true).unwrap().ends_with("ts.svg"));
        assert!(loaded.resolve_file("lib.rs", true).unwrap().ends_with("rust.svg"));
        assert!(loaded.resolve_file("unknown.xyz", true).unwrap().ends_with("dark\\file.svg") || loaded.resolve_file("unknown.xyz", true).unwrap().ends_with("dark/file.svg"));

        // Dark vs light switch
        let js_dark = loaded.resolve_file("app.js", true).unwrap();
        let js_light = loaded.resolve_file("app.js", false).unwrap();
        assert!(js_dark.to_string_lossy().contains("dark") && js_dark.ends_with("js.svg"));
        assert!(js_light.to_string_lossy().contains("light") && js_light.ends_with("js.svg"));
        assert!(loaded.resolve_file("unknown.xyz", false).unwrap().to_string_lossy().contains("light"));

        // Traversal and missing file fall back cleanly
        let gi = loaded.resolve_file(".gitignore", true).unwrap();
        assert!(!gi.ends_with("escape.svg"));
        let broken = loaded.resolve_file("broken.cfg", true).unwrap();
        assert!(broken.ends_with("file.svg"));

        // Folder resolution: root, named open/closed, generic open/closed
        assert!(loaded.resolve_folder("src", false, true, true).unwrap().ends_with("root.svg"));
        assert!(loaded.resolve_folder("src", true, true, true).unwrap().ends_with("root-open.svg"));
        assert!(loaded.resolve_folder("SRC", false, false, true).unwrap().ends_with("folder-src.svg"));
        assert!(loaded.resolve_folder("src", true, false, true).unwrap().ends_with("folder-src-open.svg"));
        assert!(loaded.resolve_folder("other", true, false, true).unwrap().ends_with("folder-open.svg"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_when_json_path_is_canonicalized_and_ext_folder_is_not() {
        let dir = std::env::temp_dir().join(format!("fluxcode-icons-win-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ext = dir.join("thang-nm.flow-icons-2.0.10");
        let dawn_json = ext.join("dawn.json");
        write_svg(&ext.join("dawn/file.svg"));
        write_svg(&ext.join("dawn/folder_gray.svg"));
        write_svg(&ext.join("dawn/folder_gray_open.svg"));
        write_svg(&ext.join("dawn/javascript.svg"));
        std::fs::write(
            &dawn_json,
            r#"{
              "file": "file",
              "folder": "folder_gray",
              "folderExpanded": "folder_gray_open",
              "iconDefinitions": {
                "file": { "iconPath": "./dawn/file.svg" },
                "folder_gray": { "iconPath": "./dawn/folder_gray.svg" },
                "folder_gray_open": { "iconPath": "./dawn/folder_gray_open.svg" },
                "javascript": { "iconPath": "./dawn/javascript.svg" }
              },
              "fileExtensions": { "js": "javascript" }
            }"#,
        )
        .unwrap();

        // Simulate Windows native.rs passing canonicalized `path` while `ext_folder` is plain.
        let meta = IconThemeMeta {
            id: "vsx:thang-nm.flow-icons:flow-dawn".into(),
            label: "Flow Dawn".into(),
            ext_id: "thang-nm.flow-icons".into(),
            ext_display: "Flow Icons".into(),
            ext_folder: ext,
            path: dawn_json.canonicalize().unwrap(),
        };
        let loaded = LoadedIconTheme::load(&meta).unwrap();
        assert!(loaded.resolve_file("index.js", true).unwrap().ends_with("javascript.svg"));
        assert!(loaded.resolve_folder("scripts", false, false, true).unwrap().ends_with("folder_gray.svg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
