use gpui_kit::SharedString;
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

pub const MATERIAL_EXT_ID: &str = "pkief.material-icon-theme";
pub const MATERIAL_ID: &str = "vsx:pkief.material-icon-theme:material-icon-theme";
pub const BUNDLED_ASSET_PREFIX: &str = "material-icons/";
const MAX_THEME_BYTES: u64 = 8 << 20;
const CACHE_SIZE: usize = 256;
const CACHE_MASK: usize = CACHE_SIZE - 1;
const MAX_CACHED_CHUNKS: usize = 8;

static BUNDLED_JSON_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/material-icons.json.z"));
static BUNDLED_PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/material-icons.pack"));
static BUNDLED_LOGO: &[u8] = include_bytes!("../../assets/material-icons/logo.png");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconThemeMeta {
    pub id: String,
    pub label: String,
    pub ext_id: String,
    pub ext_display: String,
    pub ext_folder: PathBuf,
    pub path: PathBuf,
    pub bundled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconSource {
    Asset(SharedString),
    File(Arc<PathBuf>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconKey(pub u64);

#[derive(Clone, Copy)]
struct ChunkMeta {
    offset: usize,
    comp_len: usize,
    raw_len: usize,
}

#[derive(Clone, Copy)]
struct PackEntry {
    chunk_idx: u16,
    offset: usize,
    len: usize,
}

struct CachedChunk {
    idx: u16,
    raw: Vec<u8>,
    age: u64,
}

struct PackCache {
    tick: u64,
    chunks: Vec<CachedChunk>,
}

struct BundledPack {
    chunks: Vec<ChunkMeta>,
    payload_start: usize,
    entries: HashMap<&'static str, PackEntry>,
    cache: Mutex<PackCache>,
}

impl BundledPack {
    fn parse(raw: &'static [u8]) -> Self {
        let mut cur = 0usize;
        let read_u32 = |c: &mut usize| -> u32 {
            let b = [raw[*c], raw[*c + 1], raw[*c + 2], raw[*c + 3]];
            *c += 4;
            u32::from_le_bytes(b)
        };
        let read_u16 = |c: &mut usize| -> u16 {
            let b = [raw[*c], raw[*c + 1]];
            *c += 2;
            u16::from_le_bytes(b)
        };
        let chunk_count = read_u32(&mut cur) as usize;
        let mut chunks = Vec::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            chunks.push(ChunkMeta {
                offset: read_u32(&mut cur) as usize,
                comp_len: read_u32(&mut cur) as usize,
                raw_len: read_u32(&mut cur) as usize,
            });
        }
        let entry_count = read_u32(&mut cur) as usize;
        let mut entries = HashMap::with_capacity(entry_count);
        for _ in 0..entry_count {
            let nlen = read_u16(&mut cur) as usize;
            let name = std::str::from_utf8(&raw[cur..cur + nlen]).unwrap_or("");
            cur += nlen;
            let chunk_idx = read_u16(&mut cur);
            let offset = read_u32(&mut cur) as usize;
            let len = read_u32(&mut cur) as usize;
            if !name.is_empty() {
                entries.insert(
                    name,
                    PackEntry {
                        chunk_idx,
                        offset,
                        len,
                    },
                );
            }
        }
        Self {
            chunks,
            payload_start: cur,
            entries,
            cache: Mutex::new(PackCache {
                tick: 0,
                chunks: Vec::with_capacity(MAX_CACHED_CHUNKS),
            }),
        }
    }

    fn load_svg(&self, name: &str) -> Option<Vec<u8>> {
        let entry = *self.entries.get(name)?;
        let cm = *self.chunks.get(entry.chunk_idx as usize)?;
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.tick = guard.tick.wrapping_add(1);
        let tick = guard.tick;
        if let Some(hit) = guard.chunks.iter_mut().find(|c| c.idx == entry.chunk_idx) {
            hit.age = tick;
            return hit
                .raw
                .get(entry.offset..entry.offset + entry.len)
                .map(<[u8]>::to_vec);
        }
        drop(guard);

        let start = self.payload_start + cm.offset;
        let comp = BUNDLED_PACK.get(start..start + cm.comp_len)?;
        let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(comp, cm.raw_len + 64).ok()?;
        let svg = raw.get(entry.offset..entry.offset + entry.len)?.to_vec();

        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        guard.tick = guard.tick.wrapping_add(1);
        let tick = guard.tick;
        if !guard.chunks.iter().any(|c| c.idx == entry.chunk_idx) {
            if guard.chunks.len() >= MAX_CACHED_CHUNKS {
                if let Some((evict_ix, _)) =
                    guard.chunks.iter().enumerate().min_by_key(|(_, c)| c.age)
                {
                    guard.chunks.swap_remove(evict_ix);
                }
            }
            guard.chunks.push(CachedChunk {
                idx: entry.chunk_idx,
                raw,
                age: tick,
            });
        }
        Some(svg)
    }
}

fn bundled_pack() -> &'static BundledPack {
    static PACK: OnceLock<BundledPack> = OnceLock::new();
    PACK.get_or_init(|| BundledPack::parse(BUNDLED_PACK))
}

pub fn load_bundled_asset(path: &str) -> Option<Vec<u8>> {
    let name = path.strip_prefix(BUNDLED_ASSET_PREFIX)?;
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    if name == "logo.png" {
        return Some(BUNDLED_LOGO.to_vec());
    }
    bundled_pack().load_svg(name)
}

pub fn bundled_meta() -> IconThemeMeta {
    IconThemeMeta {
        id: MATERIAL_ID.into(),
        label: "Material Icon Theme".into(),
        ext_id: MATERIAL_EXT_ID.into(),
        ext_display: "Material Icon Theme".into(),
        ext_folder: PathBuf::new(),
        path: PathBuf::new(),
        bundled: true,
    }
}

#[derive(Default)]
struct IconMap {
    file: Option<u16>,
    folder: Option<u16>,
    folder_open: Option<u16>,
    root_folder: Option<u16>,
    root_folder_open: Option<u16>,
    file_names: HashMap<String, u16>,
    file_extensions: HashMap<String, u16>,
    folder_names: HashMap<String, u16>,
    folder_names_open: HashMap<String, u16>,
    language_ids: HashMap<String, u16>,
}

pub struct LoadedIconTheme {
    gen_id: u64,
    id: String,
    label: String,
    hides_explorer_arrows: bool,
    assets: Vec<IconSource>,
    dark: IconMap,
    light: Option<IconMap>,
}

#[derive(Default)]
struct IconState {
    installed: Vec<IconThemeMeta>,
    active_id: String,
    loaded: Option<Arc<LoadedIconTheme>>,
}

static STATE: OnceLock<RwLock<IconState>> = OnceLock::new();
static NEXT_GEN: AtomicU64 = AtomicU64::new(1);
static ACTIVE_GEN: AtomicU64 = AtomicU64::new(0);

fn state() -> &'static RwLock<IconState> {
    STATE.get_or_init(|| RwLock::new(IconState::default()))
}

pub fn normalize_id(id: &str) -> &str {
    match id.trim() {
        "" | "fluxcode" | "default" | "material-icon-theme" => MATERIAL_ID,
        other => other,
    }
}

#[derive(Clone)]
struct CacheSlot {
    gen_id: u64,
    flags: u8,
    hash: u64,
    len: u8,
    name: [u8; 64],
    result: Option<IconSource>,
}

impl Default for CacheSlot {
    fn default() -> Self {
        Self {
            gen_id: 0,
            flags: 0,
            hash: 0,
            len: 0,
            name: [0; 64],
            result: None,
        }
    }
}

thread_local! {
    static ICON_CACHE: RefCell<Vec<CacheSlot>> = RefCell::new(vec![CacheSlot::default(); CACHE_SIZE]);
}

#[inline]
fn fnv1a_lower(flags: u8, lower: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ (flags as u64);
    for &b in lower {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[inline]
fn lower_ascii_stack<'a>(s: &str, buf: &'a mut [u8; 128]) -> Option<&'a str> {
    if !s.is_ascii() || s.len() > buf.len() {
        return None;
    }
    for (dst, &src) in buf.iter_mut().zip(s.as_bytes()) {
        *dst = src.to_ascii_lowercase();
    }
    std::str::from_utf8(&buf[..s.len()]).ok()
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
    target: &mut HashMap<String, u16>,
    obj: Option<&Value>,
    defs: &HashMap<String, u16>,
    strip_leading_dot: bool,
) {
    let Some(map) = obj.and_then(Value::as_object) else {
        return;
    };
    for (k, v) in map {
        let Some(def_id) = v.as_str() else { continue };
        let Some(&idx) = defs.get(def_id) else {
            continue;
        };
        let mut key = k.trim().to_ascii_lowercase();
        if strip_leading_dot {
            while let Some(rest) = key.strip_prefix('.') {
                key = rest.to_string();
            }
        }
        if !key.is_empty() {
            target.insert(key, idx);
        }
    }
}

fn build_map(obj: &Map<String, Value>, defs: &HashMap<String, u16>) -> IconMap {
    let def = |key: &str| {
        obj.get(key)
            .and_then(Value::as_str)
            .and_then(|id| defs.get(id).copied())
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
    pub fn load_bundled() -> Result<Self, String> {
        let json_bytes = miniz_oxide::inflate::decompress_to_vec(BUNDLED_JSON_Z)
            .map_err(|e| format!("Failed to decompress bundled Material Icon Theme: {e:?}"))?;
        let val: Value = serde_json::from_slice(&json_bytes)
            .map_err(|e| format!("Invalid bundled Material Icon Theme JSON: {e}"))?;
        let root_obj = val
            .as_object()
            .ok_or_else(|| "Bundled Material Icon Theme must be a JSON object".to_string())?;
        let hides_explorer_arrows = root_obj
            .get("hidesExplorerArrows")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let pack = bundled_pack();
        let mut assets: Vec<IconSource> = Vec::new();
        let mut shared: HashMap<&'static str, u16> = HashMap::new();
        let mut defs: HashMap<String, u16> = HashMap::new();

        if let Some(def_map) = root_obj.get("iconDefinitions").and_then(Value::as_object) {
            for (def_id, def_val) in def_map {
                let Some(raw_path) = def_val.get("iconPath").and_then(Value::as_str) else {
                    continue;
                };
                let fname = raw_path.rsplit(['/', '\\']).next().unwrap_or("");
                let Some((&static_name, _)) = pack.entries.get_key_value(fname) else {
                    continue;
                };
                let idx = if let Some(&existing) = shared.get(static_name) {
                    existing
                } else {
                    let Ok(idx) = u16::try_from(assets.len()) else {
                        break;
                    };
                    assets.push(IconSource::Asset(SharedString::from(format!(
                        "{BUNDLED_ASSET_PREFIX}{static_name}"
                    ))));
                    shared.insert(static_name, idx);
                    idx
                };
                defs.insert(def_id.clone(), idx);
            }
        }

        let dark = build_map(root_obj, &defs);
        let light = root_obj
            .get("light")
            .and_then(Value::as_object)
            .map(|o| build_map(o, &defs));

        Ok(Self {
            gen_id: NEXT_GEN.fetch_add(1, Ordering::Relaxed),
            id: MATERIAL_ID.into(),
            label: "Material Icon Theme".into(),
            hides_explorer_arrows,
            assets,
            dark,
            light,
        })
    }

    pub fn load(meta: &IconThemeMeta) -> Result<Self, String> {
        if meta.bundled {
            return Self::load_bundled();
        }
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

        let canon_ext_folder = meta.ext_folder.canonicalize().map_err(|e| e.to_string())?;
        let canon_path = meta.path.canonicalize().map_err(|e| e.to_string())?;
        let json_dir = canon_path.parent().unwrap_or(&canon_ext_folder);

        let mut assets: Vec<IconSource> = Vec::new();
        let mut shared_paths: HashMap<PathBuf, u16> = HashMap::new();
        let mut defs: HashMap<String, u16> = HashMap::new();
        if let Some(def_map) = root_obj.get("iconDefinitions").and_then(Value::as_object) {
            for (def_id, def_val) in def_map {
                let Some(raw_path) = def_val.get("iconPath").and_then(Value::as_str) else {
                    continue;
                };
                let Some(resolved) = clean_join(&canon_ext_folder, json_dir, raw_path) else {
                    continue;
                };
                if let Some(&idx) = shared_paths.get(&resolved) {
                    defs.insert(def_id.clone(), idx);
                    continue;
                }
                let ok = resolved.is_file()
                    && resolved
                        .canonicalize()
                        .is_ok_and(|c| c.starts_with(&canon_ext_folder));
                if !ok {
                    continue;
                }
                let Ok(idx) = u16::try_from(assets.len()) else {
                    break;
                };
                assets.push(IconSource::File(Arc::new(resolved.clone())));
                shared_paths.insert(resolved, idx);
                defs.insert(def_id.clone(), idx);
            }
        }

        let dark = build_map(root_obj, &defs);
        let light = root_obj
            .get("light")
            .and_then(Value::as_object)
            .map(|o| build_map(o, &defs));

        Ok(Self {
            gen_id: NEXT_GEN.fetch_add(1, Ordering::Relaxed),
            id: meta.id.clone(),
            label: meta.label.clone(),
            hides_explorer_arrows,
            assets,
            dark,
            light,
        })
    }

    #[inline]
    fn lookup_idx(
        &self,
        dark: bool,
        mut f: impl FnMut(&IconMap) -> Option<u16>,
    ) -> Option<IconSource> {
        let idx = if !dark {
            self.light
                .as_ref()
                .and_then(&mut f)
                .or_else(|| f(&self.dark))
        } else {
            f(&self.dark)
        }?;
        self.assets.get(idx as usize).cloned()
    }

    pub fn resolve_file(&self, name: &str, dark: bool) -> Option<IconSource> {
        if !name.is_empty() {
            let mut stack = [0u8; 128];
            let owned;
            let lower: &str = match lower_ascii_stack(name, &mut stack) {
                Some(s) => s,
                None => {
                    owned = name.to_ascii_lowercase();
                    &owned
                }
            };
            if let Some(s) = self.lookup_idx(dark, |m| m.file_names.get(lower).copied()) {
                return Some(s);
            }
            for (dot, _) in lower.match_indices('.') {
                if dot == 0 || dot + 1 >= lower.len() {
                    continue;
                }
                let ext = &lower[dot + 1..];
                if let Some(s) = self.lookup_idx(dark, |m| m.file_extensions.get(ext).copied()) {
                    return Some(s);
                }
            }
            let (lang, _) = crate::files::language_for(Path::new(lower), "");
            let mut lang_stack = [0u8; 128];
            let lang_owned;
            let lang_lower: &str = match lower_ascii_stack(lang, &mut lang_stack) {
                Some(s) => s,
                None => {
                    lang_owned = lang.to_ascii_lowercase();
                    &lang_owned
                }
            };
            if let Some(s) = self.lookup_idx(dark, |m| {
                m.language_ids.get(lang_lower).copied().or_else(|| {
                    vscode_lang_alias(lang).and_then(|a| m.language_ids.get(a).copied())
                })
            }) {
                return Some(s);
            }
        }
        self.lookup_idx(dark, |m| m.file)
    }

    pub fn resolve_folder(
        &self,
        name: &str,
        open: bool,
        root: bool,
        dark: bool,
    ) -> Option<IconSource> {
        if root {
            if open {
                if let Some(s) = self.lookup_idx(dark, |m| m.root_folder_open.or(m.root_folder)) {
                    return Some(s);
                }
            } else if let Some(s) = self.lookup_idx(dark, |m| m.root_folder) {
                return Some(s);
            }
        }
        if !name.is_empty() {
            let mut stack = [0u8; 128];
            let owned;
            let lower: &str = match lower_ascii_stack(name, &mut stack) {
                Some(s) => s,
                None => {
                    owned = name.to_ascii_lowercase();
                    &owned
                }
            };
            if let Some(s) = self.lookup_idx(dark, |m| {
                if open {
                    m.folder_names_open
                        .get(lower)
                        .copied()
                        .or_else(|| m.folder_names.get(lower).copied())
                } else {
                    m.folder_names.get(lower).copied()
                }
            }) {
                return Some(s);
            }
        }
        self.lookup_idx(dark, |m| {
            if open {
                m.folder_open.or(m.folder)
            } else {
                m.folder
            }
        })
    }
}

fn ensure_bundled_installed(metas: &mut Vec<IconThemeMeta>) {
    if !metas.iter().any(|m| m.ext_id == MATERIAL_EXT_ID) {
        metas.insert(0, bundled_meta());
    }
}

pub fn set_installed(mut metas: Vec<IconThemeMeta>) {
    ensure_bundled_installed(&mut metas);
    let reload_meta = {
        let mut s = state().write().unwrap_or_else(|e| e.into_inner());
        s.installed = metas;
        let target_id = if s.active_id.is_empty() {
            MATERIAL_ID.to_string()
        } else {
            s.active_id.clone()
        };
        let meta = s
            .installed
            .iter()
            .find(|m| m.id == target_id)
            .cloned()
            .or_else(|| {
                s.installed
                    .iter()
                    .find(|m| m.ext_id == MATERIAL_EXT_ID)
                    .cloned()
            })
            .unwrap_or_else(bundled_meta);
        s.active_id = meta.id.clone();
        meta
    };
    let loaded = LoadedIconTheme::load(&reload_meta).or_else(|_| LoadedIconTheme::load_bundled());
    if let Ok(loaded) = loaded {
        let arc = Arc::new(loaded);
        let gen = arc.gen_id;
        let mut s = state().write().unwrap_or_else(|e| e.into_inner());
        if s.active_id == reload_meta.id || s.loaded.is_none() {
            s.active_id = arc.id.clone();
            s.loaded = Some(arc);
            ACTIVE_GEN.store(gen, Ordering::Release);
        }
    }
}

pub fn list() -> Vec<IconThemeMeta> {
    let s = state().read().unwrap_or_else(|e| e.into_inner());
    if s.installed.is_empty() {
        vec![bundled_meta()]
    } else {
        s.installed.clone()
    }
}

pub fn active_id() -> String {
    let s = state().read().unwrap_or_else(|e| e.into_inner());
    if s.active_id.is_empty() {
        MATERIAL_ID.to_string()
    } else {
        s.active_id.clone()
    }
}

pub fn active_label() -> String {
    let s = state().read().unwrap_or_else(|e| e.into_inner());
    s.loaded
        .as_ref()
        .map(|l| l.label.clone())
        .unwrap_or_else(|| "Material Icon Theme".to_string())
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
    let meta = {
        let mut s = state().write().unwrap_or_else(|e| e.into_inner());
        ensure_bundled_installed(&mut s.installed);
        if s.active_id == id {
            if let Some(loaded) = &s.loaded {
                ACTIVE_GEN.store(loaded.gen_id, Ordering::Release);
                return Ok(Some(loaded.label.clone()));
            }
        }
        s.installed.iter().find(|m| m.id == id).cloned()
    };
    let meta = match meta {
        Some(m) => m,
        None if id == MATERIAL_ID => bundled_meta(),
        None => {
            if let Ok(loaded) = LoadedIconTheme::load_bundled() {
                let arc = Arc::new(loaded);
                let gen = arc.gen_id;
                let mut s = state().write().unwrap_or_else(|e| e.into_inner());
                s.active_id = arc.id.clone();
                s.loaded = Some(arc);
                ACTIVE_GEN.store(gen, Ordering::Release);
            }
            return Err(format!("Icon theme {id} is not installed"));
        }
    };
    match LoadedIconTheme::load(&meta) {
        Ok(loaded) => {
            let label = loaded.label.clone();
            let arc = Arc::new(loaded);
            let gen = arc.gen_id;
            let mut s = state().write().unwrap_or_else(|e| e.into_inner());
            s.active_id = arc.id.clone();
            s.loaded = Some(arc);
            ACTIVE_GEN.store(gen, Ordering::Release);
            Ok(Some(label))
        }
        Err(e) => {
            if let Ok(loaded) = LoadedIconTheme::load_bundled() {
                let arc = Arc::new(loaded);
                let gen = arc.gen_id;
                let mut s = state().write().unwrap_or_else(|e| e.into_inner());
                s.active_id = arc.id.clone();
                s.loaded = Some(arc);
                ACTIVE_GEN.store(gen, Ordering::Release);
            }
            Err(e)
        }
    }
}

fn ensure_loaded() -> Option<Arc<LoadedIconTheme>> {
    if let Some(l) = state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .as_ref()
    {
        return Some(Arc::clone(l));
    }
    let _ = set_active(MATERIAL_ID);
    state()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .loaded
        .as_ref()
        .map(Arc::clone)
}

#[inline]
fn cached_lookup(
    name: &str,
    flags: u8,
    resolve: impl FnOnce(&LoadedIconTheme) -> Option<IconSource>,
) -> Option<IconSource> {
    let mut gen = ACTIVE_GEN.load(Ordering::Acquire);
    if gen == 0 {
        let loaded = ensure_loaded()?;
        gen = loaded.gen_id;
        if name.len() > 64 || !name.is_ascii() {
            return resolve(&loaded);
        }
    }
    if name.len() <= 64 && name.is_ascii() {
        let mut lower = [0u8; 64];
        for (dst, &src) in lower[..name.len()].iter_mut().zip(name.as_bytes()) {
            *dst = src.to_ascii_lowercase();
        }
        let hash = fnv1a_lower(flags, &lower[..name.len()]);
        let ix = (hash as usize) & CACHE_MASK;
        let hit = ICON_CACHE.with(|c| {
            let slots = c.borrow();
            let s = &slots[ix];
            if s.gen_id == gen
                && s.flags == flags
                && s.hash == hash
                && s.len as usize == name.len()
                && s.name[..name.len()] == lower[..name.len()]
            {
                Some(s.result.clone())
            } else {
                None
            }
        });
        if let Some(res) = hit {
            return res;
        }
        let loaded = ensure_loaded()?;
        let res = resolve(&loaded);
        ICON_CACHE.with(|c| {
            let mut slots = c.borrow_mut();
            let slot = &mut slots[ix];
            slot.gen_id = loaded.gen_id;
            slot.flags = flags;
            slot.hash = hash;
            slot.len = name.len() as u8;
            slot.name[..name.len()].copy_from_slice(&lower[..name.len()]);
            slot.result = res.clone();
        });
        return res;
    }
    let loaded = ensure_loaded()?;
    resolve(&loaded)
}

pub fn file_icon(name: &str, dark: bool) -> Option<IconSource> {
    let flags = u8::from(dark);
    cached_lookup(name, flags, |loaded| loaded.resolve_file(name, dark))
}

pub fn folder_icon(name: &str, open: bool, root: bool, dark: bool) -> Option<IconSource> {
    let flags = u8::from(dark) | 2 | (u8::from(open) << 2) | (u8::from(root) << 3);
    cached_lookup(name, flags, |loaded| {
        loaded.resolve_folder(name, open, root, dark)
    })
}

pub fn file_icon_key(name: &str, dark: bool) -> IconKey {
    let mut stack = [0u8; 128];
    let lower_bytes: &[u8] = if let Some(s) = lower_ascii_stack(name, &mut stack) {
        s.as_bytes()
    } else {
        name.as_bytes()
    };
    let gen = ACTIVE_GEN.load(Ordering::Relaxed);
    IconKey(fnv1a_lower(u8::from(dark), lower_bytes) ^ gen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_svg(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(
            path,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"/>",
        )
        .unwrap();
    }

    fn ends_with(src: Option<IconSource>, suffix: &str) -> bool {
        match src {
            Some(IconSource::Asset(s)) => s.ends_with(suffix),
            Some(IconSource::File(p)) => {
                p.ends_with(suffix) || p.to_string_lossy().replace('\\', "/").ends_with(suffix)
            }
            None => false,
        }
    }

    fn contains_str(src: Option<IconSource>, needle: &str) -> bool {
        match src {
            Some(IconSource::Asset(s)) => s.contains(needle),
            Some(IconSource::File(p)) => p.to_string_lossy().contains(needle),
            None => false,
        }
    }

    #[test]
    fn bundled_material_icon_theme_loads_and_serves_svgs() {
        let loaded = LoadedIconTheme::load_bundled().expect("bundled material icon theme");
        assert!(ends_with(loaded.resolve_file("main.rs", true), "rust.svg"));
        assert!(ends_with(
            loaded.resolve_file("package.json", true),
            "nodejs.svg"
        ));
        assert!(
            ends_with(loaded.resolve_file("src", true), "file.svg")
                || loaded.resolve_file("unknown.xyz", true).is_some()
        );
        assert!(ends_with(
            loaded.resolve_folder("src", false, false, true),
            "folder-src.svg"
        ));
        assert!(ends_with(
            loaded.resolve_folder("src", true, false, true),
            "folder-src-open.svg"
        ));

        let Some(IconSource::Asset(asset_path)) = loaded.resolve_file("main.rs", true) else {
            panic!("expected bundled asset");
        };
        let bytes = load_bundled_asset(asset_path.as_ref()).expect("load rust.svg bytes");
        assert!(std::str::from_utf8(&bytes).unwrap().contains("<svg"));
        assert!(load_bundled_asset("material-icons/../secret.svg").is_none());
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
            bundled: false,
        };
        let loaded = LoadedIconTheme::load(&meta).unwrap();
        assert!(loaded.hides_explorer_arrows);

        assert!(ends_with(
            loaded.resolve_file("PACKAGE.JSON", true),
            "pkg.svg"
        ));
        assert!(ends_with(
            loaded.resolve_file("types.d.ts", true),
            "ts-def.svg"
        ));
        assert!(ends_with(loaded.resolve_file("index.ts", true), "ts.svg"));
        assert!(ends_with(loaded.resolve_file("lib.rs", true), "rust.svg"));
        assert!(ends_with(
            loaded.resolve_file("unknown.xyz", true),
            "dark/file.svg"
        ));

        let js_dark = loaded.resolve_file("app.js", true);
        let js_light = loaded.resolve_file("app.js", false);
        assert!(contains_str(js_dark.clone(), "dark") && ends_with(js_dark, "js.svg"));
        assert!(contains_str(js_light.clone(), "light") && ends_with(js_light, "js.svg"));
        assert!(contains_str(
            loaded.resolve_file("unknown.xyz", false),
            "light"
        ));

        let gi = loaded.resolve_file(".gitignore", true);
        assert!(!ends_with(gi, "escape.svg"));
        let broken = loaded.resolve_file("broken.cfg", true);
        assert!(ends_with(broken, "file.svg"));

        assert!(ends_with(
            loaded.resolve_folder("src", false, true, true),
            "root.svg"
        ));
        assert!(ends_with(
            loaded.resolve_folder("src", true, true, true),
            "root-open.svg"
        ));
        assert!(ends_with(
            loaded.resolve_folder("SRC", false, false, true),
            "folder-src.svg"
        ));
        assert!(ends_with(
            loaded.resolve_folder("src", true, false, true),
            "folder-src-open.svg"
        ));
        assert!(ends_with(
            loaded.resolve_folder("other", true, false, true),
            "folder-open.svg"
        ));

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

        let meta = IconThemeMeta {
            id: "vsx:thang-nm.flow-icons:flow-dawn".into(),
            label: "Flow Dawn".into(),
            ext_id: "thang-nm.flow-icons".into(),
            ext_display: "Flow Icons".into(),
            ext_folder: ext,
            path: dawn_json.canonicalize().unwrap(),
            bundled: false,
        };
        let loaded = LoadedIconTheme::load(&meta).unwrap();
        assert!(ends_with(
            loaded.resolve_file("index.js", true),
            "javascript.svg"
        ));
        assert!(ends_with(
            loaded.resolve_folder("scripts", false, false, true),
            "folder_gray.svg"
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
