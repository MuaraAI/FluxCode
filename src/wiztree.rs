use crate::files;
use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState},
    popover::Popover,
    v_flex, Disableable, Icon, Selectable, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WizTab {
    Tree,
    Extensions,
    Largest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WizSort {
    Size,
    Files,
    Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WizMinSize {
    All,
    Kb100,
    Mb1,
    Mb10,
}

impl WizMinSize {
    fn bytes(self) -> u64 {
        match self {
            Self::All => 0,
            Self::Kb100 => 100 * 1024,
            Self::Mb1 => 1024 * 1024,
            Self::Mb10 => 10 * 1024 * 1024,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Kb100 => "100 KB+",
            Self::Mb1 => "1 MB+",
            Self::Mb10 => "10 MB+",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WizMapHeight {
    Compact,
    Normal,
    Tall,
}

impl WizMapHeight {
    fn px(self) -> f32 {
        match self {
            Self::Compact => 165.,
            Self::Normal => 230.,
            Self::Tall => 310.,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Normal => "Normal",
            Self::Tall => "Tall",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WizConfig {
    pub show_map: bool,
    pub show_hidden: bool,
    pub skip_caches: bool,
    pub sort: WizSort,
    pub min_size: WizMinSize,
    pub map_height: WizMapHeight,
}

impl Default for WizConfig {
    fn default() -> Self {
        Self {
            show_map: true,
            show_hidden: true,
            skip_caches: false,
            sort: WizSort::Size,
            min_size: WizMinSize::All,
            map_height: WizMapHeight::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScanOpts {
    pub show_hidden: bool,
    pub skip_caches: bool,
}

impl Default for ScanOpts {
    fn default() -> Self {
        Self {
            show_hidden: true,
            skip_caches: false,
        }
    }
}

const CACHE_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    "__pycache__",
    ".next",
    ".nuxt",
    ".gradle",
    ".venv",
    "venv",
    "vendor",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileCategory {
    Code,
    Media,
    Build,
    Data,
    Temp,
    Other,
}

impl FileCategory {
    pub fn from_ext(ext: &str) -> Self {
        match ext {
            "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "go" | "c" | "h"
            | "cpp" | "cc" | "hpp" | "java" | "kt" | "cs" | "swift" | "zig" | "dart" | "rb"
            | "php" | "lua" | "sh" | "ps1" | "html" | "css" | "scss" | "vue" | "svelte" => {
                Self::Code
            }
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "ico" | "bmp" | "mp4" | "mov"
            | "webm" | "mp3" | "wav" | "ogg" | "flac" | "ttf" | "otf" | "woff" | "woff2" => {
                Self::Media
            }
            "rlib" | "rmeta" | "exe" | "dll" | "so" | "dylib" | "pdb" | "wasm" | "o" | "a"
            | "obj" | "lib" | "class" | "jar" | "apk" | "zip" | "tar" | "gz" | "xz" | "7z"
            | "dmg" | "msi" | "node" => Self::Build,
            "json" | "jsonc" | "toml" | "yaml" | "yml" | "xml" | "md" | "flux" | "lock" | "csv"
            | "sql" | "sqlite" | "db" | "dat" | "pack" | "idx" | "txt" | "pdf" => Self::Data,
            "log" | "tmp" | "map" | "dmp" | "cache" | "bak" | "old" => Self::Temp,
            _ => Self::Other,
        }
    }

    fn color(self) -> Hsla {
        match self {
            Self::Code => c(theme::BLUE),
            Self::Media => c(theme::GREEN),
            Self::Build => c(theme::WARN),
            Self::Data => c(theme::ACCENT),
            Self::Temp => c(theme::DANGER),
            Self::Other => c(theme::FG_MUTED),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::Media => "Media",
            Self::Build => "Build & Bin",
            Self::Data => "Data",
            Self::Temp => "Logs & Temp",
            Self::Other => "Other",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DiskNode {
    pub name: String,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub size: u64,
    pub files: u64,
    pub folders: u64,
    pub is_dir: bool,
    pub ext_id: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtStat {
    pub ext: String,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct LargestFile {
    pub node: u32,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct DiskTree {
    pub root: PathBuf,
    pub nodes: Vec<DiskNode>,
    pub ext_names: Vec<String>,
    pub extensions: Vec<ExtStat>,
    pub largest: Vec<LargestFile>,
    pub elapsed_ms: u64,
    pub skipped: u64,
}

impl DiskTree {
    pub fn total_size(&self) -> u64 {
        self.nodes.first().map_or(0, |n| n.size)
    }

    pub fn total_files(&self) -> u64 {
        self.nodes.first().map_or(0, |n| n.files)
    }

    pub fn total_folders(&self) -> u64 {
        self.nodes.first().map_or(0, |n| n.folders)
    }

    pub fn ext_str(&self, ext_id: u16) -> &str {
        self.ext_names
            .get(ext_id as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn full_path(&self, id: u32) -> PathBuf {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            let Some(node) = self.nodes.get(i as usize) else {
                break;
            };
            if node.parent.is_none() {
                break;
            }
            parts.push(&node.name);
            cur = node.parent;
        }
        let mut out = self.root.clone();
        for part in parts.into_iter().rev() {
            out.push(part);
        }
        out
    }

    pub fn rel_path(&self, id: u32) -> String {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            let Some(node) = self.nodes.get(i as usize) else {
                break;
            };
            if node.parent.is_none() {
                break;
            }
            parts.push(&node.name);
            cur = node.parent;
        }
        parts.reverse();
        parts.join("/")
    }

    pub fn breadcrumbs(&self, id: u32) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            chain.push(i);
            cur = self.nodes.get(i as usize).and_then(|n| n.parent);
        }
        chain.reverse();
        chain
    }

    /// Compact, token-efficient summary of folder/file sizes and waste hotspots for AI agents.
    pub fn ai_summary(&self, focus: u32, max_chars: usize) -> String {
        use std::fmt::Write;
        let fid = (focus as usize).min(self.nodes.len().saturating_sub(1)) as u32;
        let Some(fnode) = self.nodes.get(fid as usize) else {
            return String::new();
        };
        let total = fnode.size.max(1) as f64;
        let focus_rel = if fid == 0 {
            self.nodes[0].name.clone()
        } else {
            self.rel_path(fid)
        };
        let mut out = String::with_capacity(max_chars.min(4096));
        let _ = writeln!(
            out,
            "[FluxCode Built-in WizTree Disk Index - ALREADY SCANNED]\nWizTree disk map for {focus_rel} ({}): {} total, {} files, {} folders (scanned in {} ms):",
            self.full_path(fid).display(),
            format_bytes(fnode.size),
            format_count(fnode.files),
            format_count(fnode.folders),
            self.elapsed_ms,
        );
        if fid != 0 {
            let root_total = self.total_size().max(1) as f64;
            let root_pct = (fnode.size as f64 / root_total * 100.).clamp(0., 100.);
            let _ = writeln!(
                out,
                "Scope: focused subfolder ({root_pct:.1}% of workspace root {} = {})",
                self.nodes[0].name,
                format_bytes(self.total_size()),
            );
        }

        // Single iterative walk over focused subtree for categories, extensions, largest files, waste and tiny files.
        let mut cat_bytes = [0u64; 6];
        let mut cat_files = [0u64; 6];
        let mut ext_map: HashMap<u16, (u64, u64)> = HashMap::new();
        let mut sub_largest: Vec<(u32, u64)> = Vec::new();
        let mut waste_dirs: Vec<(u32, u64, &'static str)> = Vec::new();
        let mut tiny_files = 0u64;
        let mut tiny_bytes = 0u64;
        let mut small_dirs: Vec<(u32, u64)> = Vec::new();

        let mut stack = vec![(fid, 0u8)];
        while let Some((nid, depth)) = stack.pop() {
            let Some(n) = self.nodes.get(nid as usize) else {
                continue;
            };
            if n.is_dir {
                if nid != fid {
                    let lower = n.name.to_ascii_lowercase();
                    if CACHE_DIRS.iter().any(|&c| c == lower) && n.size > 0 {
                        waste_dirs.push((nid, n.size, "build/cache folder"));
                    } else if depth <= 2 && n.size > 0 && (n.size as f64 / total) >= 0.25 {
                        waste_dirs.push((nid, n.size, "dominant folder"));
                    }
                    if depth == 1 {
                        small_dirs.push((nid, n.size));
                    }
                }
                for &cid in &n.children {
                    stack.push((cid, depth.saturating_add(1)));
                }
            } else {
                let ext = self.ext_str(n.ext_id);
                let cat = FileCategory::from_ext(ext);
                let cix = match cat {
                    FileCategory::Code => 0,
                    FileCategory::Media => 1,
                    FileCategory::Build => 2,
                    FileCategory::Data => 3,
                    FileCategory::Temp => 4,
                    FileCategory::Other => 5,
                };
                cat_bytes[cix] = cat_bytes[cix].saturating_add(n.size);
                cat_files[cix] = cat_files[cix].saturating_add(1);
                let slot = ext_map.entry(n.ext_id).or_insert((0, 0));
                slot.0 = slot.0.saturating_add(n.size);
                slot.1 = slot.1.saturating_add(1);
                if n.size < 1024 {
                    tiny_files += 1;
                    tiny_bytes = tiny_bytes.saturating_add(n.size);
                }
                if n.size > 0 {
                    sub_largest.push((nid, n.size));
                }
            }
        }

        // Category breakdown sorted largest first.
        let cats = [
            (FileCategory::Code, cat_bytes[0], cat_files[0]),
            (FileCategory::Media, cat_bytes[1], cat_files[1]),
            (FileCategory::Build, cat_bytes[2], cat_files[2]),
            (FileCategory::Data, cat_bytes[3], cat_files[3]),
            (FileCategory::Temp, cat_bytes[4], cat_files[4]),
            (FileCategory::Other, cat_bytes[5], cat_files[5]),
        ];
        let mut cat_list: Vec<_> = cats.into_iter().filter(|(_, b, f)| *b > 0 || *f > 0).collect();
        cat_list.sort_by_key(|(_, b, _)| std::cmp::Reverse(*b));
        if !cat_list.is_empty() {
            let parts: Vec<String> = cat_list
                .into_iter()
                .map(|(cat, b, f)| {
                    let pct = (b as f64 / total * 100.).clamp(0., 100.);
                    format!("{} {} ({pct:.1}%, {} files)", cat.label(), format_bytes(b), format_count(f))
                })
                .collect();
            let _ = writeln!(out, "Categories: {}", parts.join(" | "));
        }

        // Folder hierarchy flow (largest to smallest, up to depth 3 for significant branches).
        out.push_str("Folder hierarchy (largest -> smallest):\n");
        let mut tree_lines = 0usize;
        self.write_ai_branch(fid, 1, total, &mut out, &mut tree_lines, 32);

        // Lightweight / smallest folders and tiny files summary so the agent sees both ends of the scale.
        small_dirs.sort_by_key(|(_, s)| *s);
        let light: Vec<String> = small_dirs
            .iter()
            .filter(|(_, s)| (*s as f64 / total) < 0.05)
            .take(5)
            .filter_map(|&(nid, s)| {
                let n = self.nodes.get(nid as usize)?;
                Some(format!("{}/ ({}, {} files)", n.name, format_bytes(s), format_count(n.files)))
            })
            .collect();
        if !light.is_empty() || tiny_files > 0 {
            let mut line = String::from("Smallest / lightweight: ");
            if !light.is_empty() {
                line.push_str(&light.join(", "));
            }
            if tiny_files > 0 {
                if !light.is_empty() {
                    line.push_str("; ");
                }
                let _ = write!(line, "{} tiny files <1 KB ({} total)", format_count(tiny_files), format_bytes(tiny_bytes));
            }
            let _ = writeln!(out, "{line}");
        }

        // Heaviest / waste hotspots (cache/build dirs, dominant dirs, large temp/log files, huge single files).
        sub_largest.sort_by_key(|&(_, s)| std::cmp::Reverse(s));
        let mut hotspots: Vec<(u64, String)> = Vec::new();
        waste_dirs.sort_by_key(|&(_, s, _)| std::cmp::Reverse(s));
        let mut seen_hot: HashSet<u32> = HashSet::new();
        for (nid, sz, why) in waste_dirs.into_iter().take(5) {
            if seen_hot.insert(nid) {
                let pct = (sz as f64 / total * 100.).clamp(0., 100.);
                let rel = self.rel_path(nid);
                hotspots.push((sz, format!("{rel}/: {} ({pct:.1}%, {why})", format_bytes(sz))));
            }
        }
        for &(nid, sz) in sub_largest.iter().take(20) {
            let n = &self.nodes[nid as usize];
            let cat = FileCategory::from_ext(self.ext_str(n.ext_id));
            let pct = (sz as f64 / total * 100.).clamp(0., 100.);
            let why = if cat == FileCategory::Temp && (pct >= 1.0 || sz >= 256 * 1024) {
                Some("log/temp file")
            } else if pct >= 5.0 || sz >= 5 * 1024 * 1024 {
                Some("large file")
            } else {
                None
            };
            if let Some(why) = why {
                if seen_hot.insert(nid) {
                    let rel = self.rel_path(nid);
                    hotspots.push((sz, format!("{rel}: {} ({pct:.1}%, {why})", format_bytes(sz))));
                }
            }
        }
        hotspots.sort_by_key(|(sz, _)| std::cmp::Reverse(*sz));
        if !hotspots.is_empty() {
            out.push_str("Heaviest & potential waste hotspots:\n");
            for (_, desc) in hotspots.into_iter().take(8) {
                let _ = writeln!(out, "  - {desc}");
            }
        }

        // Top extensions in scope.
        let mut exts: Vec<(u16, u64, u64)> = ext_map.into_iter().map(|(id, (b, f))| (id, b, f)).collect();
        exts.sort_by_key(|&(_, b, _)| std::cmp::Reverse(b));
        if !exts.is_empty() {
            let parts: Vec<String> = exts
                .into_iter()
                .take(10)
                .map(|(id, b, f)| {
                    let name = if id == 0 {
                        "(no ext)".to_string()
                    } else {
                        format!(".{}", self.ext_str(id))
                    };
                    let pct = (b as f64 / total * 100.).clamp(0., 100.);
                    format!("{name} {} ({pct:.1}%, {})", format_bytes(b), format_count(f))
                })
                .collect();
            let _ = writeln!(out, "Top extensions: {}", parts.join(", "));
        }

        // Largest files in scope.
        if !sub_largest.is_empty() {
            out.push_str("Largest files:\n");
            for (rank, &(nid, sz)) in sub_largest.iter().take(12).enumerate() {
                let pct = (sz as f64 / total * 100.).clamp(0., 100.);
                let _ = writeln!(out, "  {}. {} ({}, {pct:.1}%)", rank + 1, self.rel_path(nid), format_bytes(sz));
            }
        }

        out.push_str(
            "Instructions for AI agent: This workspace is already indexed natively by FluxCode WizTree above. Answer disk usage, folder size, waste, and largest-file questions directly from this summary with ZERO tool calls (do not run PowerShell, Get-ChildItem, Get-Process, find, du, or grep). Only if the user specifically asks to drill deeper into a subfolder not expanded above, run at most ONE single command: `fluxcode --wiztree <subfolder>`.",
        );
        if out.len() > max_chars && max_chars > 64 {
            let cut = (0..=max_chars - 32).rev().find(|&i| out.is_char_boundary(i)).unwrap_or(0);
            out.truncate(cut);
            out.push_str("\n...(truncated WizTree summary)");
        }
        out
    }

    fn write_ai_branch(
        &self,
        parent: u32,
        depth: usize,
        root_total: f64,
        out: &mut String,
        lines: &mut usize,
        max_lines: usize,
    ) {
        use std::fmt::Write;
        if depth > 3 || *lines >= max_lines {
            return;
        }
        let Some(pnode) = self.nodes.get(parent as usize) else {
            return;
        };
        let mut dirs: Vec<u32> = Vec::new();
        let mut files: Vec<u32> = Vec::new();
        for &cid in &pnode.children {
            if let Some(c) = self.nodes.get(cid as usize) {
                if c.is_dir {
                    dirs.push(cid);
                } else {
                    files.push(cid);
                }
            }
        }
        let indent = "  ".repeat(depth);
        let max_dirs = match depth {
            1 => 12,
            2 => 6,
            _ => 4,
        };
        let mut shown = 0usize;
        for &did in &dirs {
            if *lines >= max_lines || shown >= max_dirs {
                break;
            }
            let d = &self.nodes[did as usize];
            let pct = (d.size as f64 / root_total * 100.).clamp(0., 100.);
            if depth > 1 && shown >= 2 && pct < 1.5 {
                break;
            }
            let _ = writeln!(
                out,
                "{indent}{}/  {} ({pct:.1}%, {} files)",
                d.name,
                format_bytes(d.size),
                format_count(d.files),
            );
            *lines += 1;
            shown += 1;
            if depth < 3 && pct >= 4.0 && !d.children.is_empty() {
                self.write_ai_branch(did, depth + 1, root_total, out, lines, max_lines);
            }
        }
        if dirs.len() > shown && *lines < max_lines {
            let rest_bytes: u64 = dirs[shown..].iter().map(|&id| self.nodes[id as usize].size).sum();
            let _ = writeln!(
                out,
                "{indent}(+{} smaller folders, {} total)",
                dirs.len() - shown,
                format_bytes(rest_bytes),
            );
            *lines += 1;
        }
        if depth == 1 && !files.is_empty() && *lines < max_lines {
            let fbytes: u64 = files.iter().map(|&id| self.nodes[id as usize].size).sum();
            let sample: Vec<String> = files
                .iter()
                .take(4)
                .map(|&id| {
                    let f = &self.nodes[id as usize];
                    format!("{} ({})", f.name, format_bytes(f.size))
                })
                .collect();
            let more = files.len().saturating_sub(sample.len());
            let suffix = if more > 0 { format!(", +{more} more") } else { String::new() };
            let _ = writeln!(
                out,
                "{indent}[direct files: {}, {} total: {}{suffix}]",
                format_count(files.len() as u64),
                format_bytes(fbytes),
                sample.join(", "),
            );
            *lines += 1;
        }
    }
}

pub(crate) fn scan_quick(root: &Path) -> Result<DiskTree, String> {
    scan_tree(root, ScanOpts::default(), &Arc::new(ScanProgress::new()))
}

pub(crate) fn cli(args: &[String]) -> i32 {
    let dir = args
        .first()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    match scan_quick(&dir) {
        Ok(tree) => {
            use std::io::Write;
            let _ = writeln!(std::io::stdout(), "{}", tree.ai_summary(0, 6000));
            0
        }
        Err(e) => {
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "wiztree: {e}");
            1
        }
    }
}

pub(crate) struct ScanProgress {
    pub files: AtomicU64,
    pub folders: AtomicU64,
    pub bytes: AtomicU64,
    pub cancel: AtomicBool,
    pub started: Instant,
}

impl ScanProgress {
    fn new() -> Self {
        Self {
            files: AtomicU64::new(0),
            folders: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            cancel: AtomicBool::new(false),
            started: Instant::now(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TreemapTile {
    pub node: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub category: FileCategory,
    pub is_dir: bool,
    pub depth: u8,
}

#[derive(Default)]
pub(crate) struct FolderHistory {
    pub back: Vec<u32>,
    pub forward: Vec<u32>,
}

impl FolderHistory {
    pub fn push(&mut self, from: u32, to: u32) {
        if from == to {
            return;
        }
        self.back.push(from);
        if self.back.len() > 80 {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    pub fn go_back(&mut self, cur: u32) -> Option<u32> {
        let prev = self.back.pop()?;
        self.forward.push(cur);
        Some(prev)
    }

    pub fn go_forward(&mut self, cur: u32) -> Option<u32> {
        let next = self.forward.pop()?;
        self.back.push(cur);
        Some(next)
    }

    pub fn clear(&mut self) {
        self.back.clear();
        self.forward.clear();
    }
}

type TilesCache = std::rc::Rc<std::cell::RefCell<(f32, f32, u32, Vec<TreemapTile>)>>;
type ToggleFn = Box<dyn Fn(&bool, &mut Window, &mut App)>;

pub(crate) struct WizTreeState {
    pub page: bool,
    pub target: Option<PathBuf>,
    pub tree: Option<Arc<DiskTree>>,
    pub focus: u32,
    pub history: FolderHistory,
    pub tab: WizTab,
    pub cfg: WizConfig,
    pub settings_open: bool,
    pub settings_anim: u64,
    pub open_seq: u64,
    pub nav_seq: u64,
    pub nav_dir: i8,
    pub expanded: HashSet<u32>,
    pub scanning: Option<Arc<ScanProgress>>,
    pub error: Option<String>,
    pub filter: Entity<InputState>,
    pub hovered_tile: Option<u32>,
    pub tiles_cache: TilesCache,
    pub map_origin: std::rc::Rc<std::cell::Cell<Point<Pixels>>>,
    pub scan_gen: u64,
    pub pin_agent: bool,
    _sub: Subscription,
}

impl WizTreeState {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let filter = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr("Filter files, folders, or extensions..."))
        });
        let _sub = cx.subscribe(&filter, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            page: false,
            target: None,
            tree: None,
            focus: 0,
            history: FolderHistory::default(),
            tab: WizTab::Tree,
            cfg: WizConfig::default(),
            settings_open: false,
            settings_anim: 0,
            open_seq: 0,
            nav_seq: 0,
            nav_dir: 0,
            expanded: HashSet::new(),
            scanning: None,
            error: None,
            filter,
            hovered_tile: None,
            tiles_cache: std::rc::Rc::new(std::cell::RefCell::new((0., 0., u32::MAX, Vec::new()))),
            map_origin: std::rc::Rc::new(std::cell::Cell::new(point(px(0.), px(0.)))),
            scan_gen: 0,
            pin_agent: false,
            _sub,
        }
    }

    pub fn cancel_scan(&mut self) {
        if let Some(p) = self.scanning.take() {
            p.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn invalidate_map(&self) {
        self.tiles_cache.borrow_mut().2 = u32::MAX;
    }

    pub fn scan_opts(&self) -> ScanOpts {
        ScanOpts {
            show_hidden: self.cfg.show_hidden,
            skip_caches: self.cfg.skip_caches,
        }
    }
}

struct WorkQueue {
    dirs: VecDeque<(u32, PathBuf)>,
    active: usize,
    done: bool,
}

struct Arena {
    nodes: Vec<DiskNode>,
    ext_map: HashMap<String, u16>,
    ext_names: Vec<String>,
    skipped: u64,
}

impl Arena {
    fn intern_ext(&mut self, name: &str) -> u16 {
        let Some((_, raw)) = name.rsplit_once('.') else {
            return 0;
        };
        if raw.is_empty() || raw.len() > 16 || raw == name {
            return 0;
        }
        let lower = raw.to_ascii_lowercase();
        if let Some(&id) = self.ext_map.get(&lower) {
            return id;
        }
        if self.ext_names.len() >= u16::MAX as usize {
            return 0;
        }
        let id = self.ext_names.len() as u16;
        self.ext_names.push(lower.clone());
        self.ext_map.insert(lower, id);
        id
    }
}

struct LocalEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
}

fn entry_file_size(md: &std::fs::Metadata) -> u64 {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        md.file_size()
    }
    #[cfg(not(windows))]
    {
        md.len()
    }
}

fn should_skip_entry(name: &str, is_dir: bool, opts: ScanOpts) -> bool {
    if !opts.show_hidden && name.starts_with('.') {
        return true;
    }
    if opts.skip_caches && is_dir {
        let lower = name.to_ascii_lowercase();
        if CACHE_DIRS.iter().any(|&c| c == lower) {
            return true;
        }
    }
    false
}

pub(crate) fn scan_tree(
    root: &Path,
    opts: ScanOpts,
    progress: &Arc<ScanProgress>,
) -> Result<DiskTree, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a readable folder.", root.display()));
    }
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let root_node = DiskNode {
        name: root_name,
        parent: None,
        children: Vec::new(),
        size: 0,
        files: 0,
        folders: 0,
        is_dir: true,
        ext_id: 0,
    };
    let arena = Arc::new(Mutex::new(Arena {
        nodes: vec![root_node],
        ext_map: HashMap::new(),
        ext_names: vec![String::new()],
        skipped: 0,
    }));
    let queue = Arc::new((
        Mutex::new(WorkQueue {
            dirs: VecDeque::from([(0u32, root.to_path_buf())]),
            active: 0,
            done: false,
        }),
        Condvar::new(),
    ));
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 6);
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let arena = arena.clone();
        let queue = queue.clone();
        let progress = progress.clone();
        handles.push(std::thread::spawn(move || {
            let (lock, cvar) = &*queue;
            loop {
                if progress.cancel.load(Ordering::Relaxed) {
                    if let Ok(mut q) = lock.lock() {
                        q.done = true;
                        q.dirs.clear();
                        cvar.notify_all();
                    }
                    break;
                }
                let item = {
                    let Ok(mut q) = lock.lock() else { break };
                    while q.dirs.is_empty() && !q.done {
                        if q.active == 0 {
                            q.done = true;
                            cvar.notify_all();
                            break;
                        }
                        match cvar.wait(q) {
                            Ok(g) => q = g,
                            Err(_) => return,
                        }
                    }
                    if q.done {
                        break;
                    }
                    if let Some(next) = q.dirs.pop_front() {
                        q.active += 1;
                        next
                    } else {
                        continue;
                    }
                };

                let (parent_id, dir_path) = item;
                let mut local: Vec<LocalEntry> = Vec::new();
                let mut local_skipped: u64 = 0;
                let mut local_files: u64 = 0;
                let mut local_folders: u64 = 0;
                let mut local_bytes: u64 = 0;

                match std::fs::read_dir(&dir_path) {
                    Ok(rd) => {
                        for res in rd {
                            if progress.cancel.load(Ordering::Relaxed) {
                                break;
                            }
                            let Ok(entry) = res else {
                                local_skipped += 1;
                                continue;
                            };
                            let Ok(ft) = entry.file_type() else {
                                local_skipped += 1;
                                continue;
                            };
                            if ft.is_symlink() {
                                continue;
                            }
                            let name = entry.file_name().to_string_lossy().into_owned();
                            let is_dir = ft.is_dir();
                            if should_skip_entry(&name, is_dir, opts) {
                                continue;
                            }
                            if is_dir {
                                local_folders += 1;
                                local.push(LocalEntry {
                                    name,
                                    path: entry.path(),
                                    is_dir: true,
                                    size: 0,
                                });
                            } else if ft.is_file() {
                                let size =
                                    entry.metadata().map(|m| entry_file_size(&m)).unwrap_or(0);
                                local_files += 1;
                                local_bytes = local_bytes.saturating_add(size);
                                local.push(LocalEntry {
                                    name,
                                    path: PathBuf::new(),
                                    is_dir: false,
                                    size,
                                });
                            }
                        }
                    }
                    Err(_) => {
                        local_skipped += 1;
                    }
                }

                if local_files > 0 {
                    progress.files.fetch_add(local_files, Ordering::Relaxed);
                }
                if local_folders > 0 {
                    progress.folders.fetch_add(local_folders, Ordering::Relaxed);
                }
                if local_bytes > 0 {
                    progress.bytes.fetch_add(local_bytes, Ordering::Relaxed);
                }

                let mut subdirs: Vec<(u32, PathBuf)> = Vec::new();
                if let Ok(mut a) = arena.lock() {
                    a.skipped = a.skipped.saturating_add(local_skipped);
                    let mut child_ids = Vec::with_capacity(local.len());
                    for item in local {
                        let id = a.nodes.len() as u32;
                        let ext_id = if item.is_dir {
                            0
                        } else {
                            a.intern_ext(&item.name)
                        };
                        a.nodes.push(DiskNode {
                            name: item.name,
                            parent: Some(parent_id),
                            children: Vec::new(),
                            size: item.size,
                            files: u64::from(!item.is_dir),
                            folders: 0,
                            is_dir: item.is_dir,
                            ext_id,
                        });
                        child_ids.push(id);
                        if item.is_dir {
                            subdirs.push((id, item.path));
                        }
                    }
                    if let Some(p) = a.nodes.get_mut(parent_id as usize) {
                        p.children = child_ids;
                    }
                }

                if let Ok(mut q) = lock.lock() {
                    q.active = q.active.saturating_sub(1);
                    if !progress.cancel.load(Ordering::Relaxed) {
                        for sub in subdirs {
                            q.dirs.push_back(sub);
                        }
                    }
                    if q.dirs.is_empty() && q.active == 0 {
                        q.done = true;
                    }
                    cvar.notify_all();
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    if progress.cancel.load(Ordering::Relaxed) {
        return Err("Scan cancelled.".into());
    }

    let mut a = arena.lock().map_err(|_| "Scan lock failed.".to_string())?;
    let mut nodes = std::mem::take(&mut a.nodes);
    let ext_names = std::mem::take(&mut a.ext_names);
    let skipped = a.skipped;
    drop(a);

    rollup_tree(&mut nodes);

    let mut ext_totals: Vec<(u64, u64)> = vec![(0, 0); ext_names.len()];
    let mut largest: Vec<LargestFile> = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        if !node.is_dir {
            if let Some(slot) = ext_totals.get_mut(node.ext_id as usize) {
                slot.0 = slot.0.saturating_add(node.size);
                slot.1 = slot.1.saturating_add(1);
            }
            if node.size > 0 {
                largest.push(LargestFile {
                    node: idx as u32,
                    size: node.size,
                });
            }
        }
    }
    let mut extensions: Vec<ExtStat> = ext_totals
        .into_iter()
        .enumerate()
        .filter(|(_, (_, count))| *count > 0)
        .map(|(i, (bytes, files))| ExtStat {
            ext: if i == 0 {
                "(no extension)".into()
            } else {
                format!(".{}", ext_names[i])
            },
            bytes,
            files,
        })
        .collect();
    extensions.sort_by_key(|e| std::cmp::Reverse(e.bytes));

    largest.sort_by_key(|f| std::cmp::Reverse(f.size));
    largest.truncate(100);

    Ok(DiskTree {
        root: root.to_path_buf(),
        nodes,
        ext_names,
        extensions,
        largest,
        elapsed_ms: progress.started.elapsed().as_millis() as u64,
        skipped,
    })
}

fn rollup_tree(nodes: &mut [DiskNode]) {
    if nodes.is_empty() {
        return;
    }
    // Iterative post-order traversal avoids stack overflow on deeply nested node_modules trees.
    let mut order: Vec<u32> = Vec::with_capacity(nodes.len());
    let mut stack: Vec<u32> = vec![0];
    while let Some(id) = stack.pop() {
        order.push(id);
        if let Some(node) = nodes.get(id as usize) {
            for &child in &node.children {
                stack.push(child);
            }
        }
    }
    for &id in order.iter().rev() {
        let idx = id as usize;
        if !nodes[idx].is_dir {
            continue;
        }
        let mut children = std::mem::take(&mut nodes[idx].children);
        let (mut size, mut files, mut folders) = (0u64, 0u64, 0u64);
        for &cid in &children {
            let c = &nodes[cid as usize];
            size = size.saturating_add(c.size);
            files = files.saturating_add(c.files);
            folders = folders
                .saturating_add(c.folders)
                .saturating_add(u64::from(c.is_dir));
        }
        children.sort_by_key(|&cid| std::cmp::Reverse(nodes[cid as usize].size));
        nodes[idx].size = size;
        nodes[idx].files = files;
        nodes[idx].folders = folders;
        nodes[idx].children = children;
    }
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = 1024. * 1024.;
    const GB: f64 = 1024. * 1024. * 1024.;
    const TB: f64 = 1024. * 1024. * 1024. * 1024.;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else if b < TB {
        format!("{:.2} GB", b / GB)
    } else {
        format!("{:.2} TB", b / TB)
    }
}

fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn worst_ratio(row: &[(u32, f32)], short_side: f32) -> f32 {
    if row.is_empty() || short_side <= 0. {
        return f32::INFINITY;
    }
    let sum: f32 = row.iter().map(|&(_, a)| a).sum();
    let max_a = row.iter().map(|&(_, a)| a).fold(0.0f32, f32::max);
    let min_a = row.iter().map(|&(_, a)| a).fold(f32::INFINITY, f32::min);
    if min_a <= 0. || sum <= 0. {
        return f32::INFINITY;
    }
    let s2 = short_side * short_side;
    ((s2 * max_a) / (sum * sum)).max((sum * sum) / (s2 * min_a))
}

fn squarify_rect(
    items: &[(u32, u64)],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Vec<(u32, f32, f32, f32, f32)> {
    let mut out = Vec::new();
    if items.is_empty() || w <= 1. || h <= 1. {
        return out;
    }
    let total: f64 = items.iter().map(|&(_, s)| s as f64).sum();
    if total <= 0. {
        return out;
    }
    let area = (w as f64) * (h as f64);
    let scaled: Vec<(u32, f32)> = items
        .iter()
        .filter(|&&(_, s)| s > 0)
        .map(|&(id, s)| (id, ((s as f64 / total) * area) as f32))
        .filter(|&(_, a)| a > 0.25)
        .collect();
    if scaled.is_empty() {
        return out;
    }

    let (mut rx, mut ry, mut rw, mut rh) = (x, y, w, h);
    let mut i = 0usize;
    while i < scaled.len() && rw > 0.5 && rh > 0.5 {
        let short = rw.min(rh);
        let mut row: Vec<(u32, f32)> = vec![scaled[i]];
        i += 1;
        while i < scaled.len() {
            let cur_worst = worst_ratio(&row, short);
            let mut next = row.clone();
            next.push(scaled[i]);
            if worst_ratio(&next, short) <= cur_worst {
                row = next;
                i += 1;
            } else {
                break;
            }
        }
        let row_sum: f32 = row.iter().map(|&(_, a)| a).sum();
        if rw >= rh {
            let strip_w = (row_sum / rh).clamp(0., rw);
            let mut cy = ry;
            for &(id, a) in &row {
                let ch = if strip_w > 0. {
                    (a / strip_w).clamp(0., ry + rh - cy)
                } else {
                    0.
                };
                if strip_w >= 1. && ch >= 1. {
                    out.push((id, rx, cy, strip_w, ch));
                }
                cy += ch;
            }
            rx += strip_w;
            rw = (rw - strip_w).max(0.);
        } else {
            let strip_h = (row_sum / rw).clamp(0., rh);
            let mut cx = rx;
            for &(id, a) in &row {
                let cw = if strip_h > 0. {
                    (a / strip_h).clamp(0., rx + rw - cx)
                } else {
                    0.
                };
                if cw >= 1. && strip_h >= 1. {
                    out.push((id, cx, ry, cw, strip_h));
                }
                cx += cw;
            }
            ry += strip_h;
            rh = (rh - strip_h).max(0.);
        }
    }
    out
}

fn dominant_category(tree: &DiskTree, id: u32) -> FileCategory {
    let Some(node) = tree.nodes.get(id as usize) else {
        return FileCategory::Other;
    };
    if !node.is_dir {
        return FileCategory::from_ext(tree.ext_str(node.ext_id));
    }
    let mut cur = node;
    for _ in 0..4 {
        let Some(&first) = cur.children.first() else {
            break;
        };
        let child = &tree.nodes[first as usize];
        if !child.is_dir {
            return FileCategory::from_ext(tree.ext_str(child.ext_id));
        }
        cur = child;
    }
    FileCategory::Other
}

pub(crate) fn layout_treemap(tree: &DiskTree, focus: u32, w: f32, h: f32) -> Vec<TreemapTile> {
    let Some(parent) = tree.nodes.get(focus as usize) else {
        return Vec::new();
    };
    let items: Vec<(u32, u64)> = parent
        .children
        .iter()
        .take(180)
        .filter_map(|&cid| {
            let sz = tree.nodes.get(cid as usize)?.size;
            (sz > 0).then_some((cid, sz))
        })
        .collect();
    let top_rects = squarify_rect(&items, 0., 0., w, h);
    let mut tiles = Vec::with_capacity(top_rects.len() * 2);
    for (id, x, y, tw, th) in top_rects {
        let node = &tree.nodes[id as usize];
        let cat = dominant_category(tree, id);
        tiles.push(TreemapTile {
            node: id,
            x,
            y,
            w: tw,
            h: th,
            category: cat,
            is_dir: node.is_dir,
            depth: 0,
        });
        if node.is_dir && tw >= 76. && th >= 48. && !node.children.is_empty() {
            let sub_items: Vec<(u32, u64)> = node
                .children
                .iter()
                .take(36)
                .filter_map(|&cid| {
                    let sz = tree.nodes.get(cid as usize)?.size;
                    (sz > 0).then_some((cid, sz))
                })
                .collect();
            let pad = 3.;
            let head = 17.;
            let iw = (tw - pad * 2.).max(0.);
            let ih = (th - head - pad).max(0.);
            for (sid, sx, sy, sw, sh) in squarify_rect(&sub_items, x + pad, y + head, iw, ih) {
                let snode = &tree.nodes[sid as usize];
                tiles.push(TreemapTile {
                    node: sid,
                    x: sx,
                    y: sy,
                    w: sw,
                    h: sh,
                    category: dominant_category(tree, sid),
                    is_dir: snode.is_dir,
                    depth: 1,
                });
            }
        }
    }
    tiles
}

struct TreeRow {
    id: u32,
    depth: usize,
    parent_size: u64,
}

fn sorted_children(tree: &DiskTree, children: &[u32], sort: WizSort, min_bytes: u64) -> Vec<u32> {
    let mut list: Vec<u32> = children
        .iter()
        .copied()
        .filter(|&cid| {
            tree.nodes
                .get(cid as usize)
                .is_some_and(|n| n.size >= min_bytes)
        })
        .collect();
    match sort {
        WizSort::Size => {}
        WizSort::Files => list.sort_by_key(|&cid| {
            let n = &tree.nodes[cid as usize];
            (std::cmp::Reverse(n.files), std::cmp::Reverse(n.size))
        }),
        WizSort::Name => list.sort_by(|&a, &b| {
            let na = &tree.nodes[a as usize];
            let nb = &tree.nodes[b as usize];
            nb.is_dir.cmp(&na.is_dir).then_with(|| {
                na.name
                    .to_ascii_lowercase()
                    .cmp(&nb.name.to_ascii_lowercase())
            })
        }),
    }
    list
}

fn flatten_visible_rows(
    tree: &DiskTree,
    focus: u32,
    expanded: &HashSet<u32>,
    query: &str,
    sort: WizSort,
    min_bytes: u64,
    limit: usize,
) -> Vec<TreeRow> {
    let mut out = Vec::new();
    let Some(root) = tree.nodes.get(focus as usize) else {
        return out;
    };
    if query.is_empty() {
        let initial = sorted_children(tree, &root.children, sort, min_bytes);
        let mut stack: Vec<(u32, usize, u64)> = initial
            .into_iter()
            .rev()
            .map(|cid| (cid, 0usize, root.size.max(1)))
            .collect();
        while let Some((id, depth, parent_size)) = stack.pop() {
            out.push(TreeRow {
                id,
                depth,
                parent_size,
            });
            if out.len() >= limit {
                break;
            }
            let node = &tree.nodes[id as usize];
            if node.is_dir && expanded.contains(&id) {
                let psize = node.size.max(1);
                let kids = sorted_children(tree, &node.children, sort, min_bytes);
                for cid in kids.into_iter().rev() {
                    stack.push((cid, depth + 1, psize));
                }
            }
        }
        return out;
    }

    let q = query.to_ascii_lowercase();
    let mut matches: Vec<u32> = Vec::new();
    let mut stack = vec![focus];
    while let Some(id) = stack.pop() {
        let node = &tree.nodes[id as usize];
        if id != focus && node.size >= min_bytes && node.name.to_ascii_lowercase().contains(&q) {
            matches.push(id);
        }
        for &cid in &node.children {
            stack.push(cid);
        }
    }
    let ordered = sorted_children(tree, &matches, sort, min_bytes);
    let denom = root.size.max(1);
    for id in ordered.into_iter().take(limit) {
        out.push(TreeRow {
            id,
            depth: 0,
            parent_size: denom,
        });
    }
    out
}

impl Workbench {
    pub(crate) fn open_wiztree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.wiztree.page {
            if let Some(p) = self.active_doc_path() {
                self.push_nav_history(crate::workbench::NavTarget::File(p));
            }
            self.wiztree.open_seq = self.wiztree.open_seq.wrapping_add(1);
        }
        self.settings_open = false;
        self.ext.page = None;
        self.whats_new = None;
        self.agent_full = false;
        self.wiztree.page = true;
        let want = self.wiztree.target.clone().or_else(|| self.root.clone());
        let needs_scan = match (&want, &self.wiztree.tree, &self.wiztree.scanning) {
            (Some(dir), Some(t), None) => t.root != *dir,
            (Some(_), None, None) => true,
            _ => false,
        };
        if needs_scan {
            if let Some(dir) = want {
                self.wiztree_scan(dir, window, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_wiztree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.wiztree.page && !self.settings_open {
            self.wiztree.page = false;
            self.wiztree.settings_open = false;
            self.wiztree.cancel_scan();
            cx.notify();
        } else {
            self.open_wiztree(window, cx);
        }
    }

    pub(crate) fn wiztree_goto(&mut self, target: u32, dir: i8, cx: &mut Context<Self>) {
        let max_id = self
            .wiztree
            .tree
            .as_ref()
            .map_or(0, |t| t.nodes.len().saturating_sub(1) as u32);
        let next = target.min(max_id);
        if next == self.wiztree.focus {
            return;
        }
        self.wiztree.history.push(self.wiztree.focus, next);
        self.nav_forward.clear();
        self.wiztree.focus = next;
        self.wiztree.hovered_tile = None;
        self.wiztree.nav_dir = dir;
        self.wiztree.nav_seq = self.wiztree.nav_seq.wrapping_add(1);
        self.wiztree.invalidate_map();
        cx.notify();
    }

    pub(crate) fn wiztree_back(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(prev) = self.wiztree.history.go_back(self.wiztree.focus) else {
            return false;
        };
        self.wiztree.focus = prev;
        self.wiztree.hovered_tile = None;
        self.wiztree.nav_dir = -1;
        self.wiztree.nav_seq = self.wiztree.nav_seq.wrapping_add(1);
        self.wiztree.invalidate_map();
        cx.notify();
        true
    }

    pub(crate) fn wiztree_forward(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(next) = self.wiztree.history.go_forward(self.wiztree.focus) else {
            return false;
        };
        self.wiztree.focus = next;
        self.wiztree.hovered_tile = None;
        self.wiztree.nav_dir = 1;
        self.wiztree.nav_seq = self.wiztree.nav_seq.wrapping_add(1);
        self.wiztree.invalidate_map();
        cx.notify();
        true
    }

    pub(crate) fn wiztree_up(&mut self, cx: &mut Context<Self>) -> bool {
        let parent = self
            .wiztree
            .tree
            .as_ref()
            .and_then(|t| t.nodes.get(self.wiztree.focus as usize))
            .and_then(|n| n.parent);
        let Some(pid) = parent else { return false };
        self.wiztree_goto(pid, -1, cx);
        true
    }

    fn wiztree_open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        // Record WizTree in workbench history so Mouse Back returns straight to this WizTree view.
        self.push_nav_history(crate::workbench::NavTarget::WizTree);
        self.nav_suppress_path = Some(path.clone());
        self.nav_replaying = true;
        self.jump_to_file_smooth(path, None, window, cx);
        self.nav_replaying = false;
    }

    pub(crate) fn wiztree_ask_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.wiztree.tree.is_none() {
            return;
        }
        self.wiztree.pin_agent = true;
        self.agent.off_ctx.remove("wiztree");
        self.on_show_agent(&crate::workbench::ShowAgent, window, cx);
        let h = self.agent_input_focus(cx);
        h.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn wiztree_copy_ai_summary(&mut self, cx: &mut Context<Self>) {
        let Some(tree) = &self.wiztree.tree else {
            return;
        };
        let summary = tree.ai_summary(self.wiztree.focus, 4000);
        cx.write_to_clipboard(ClipboardItem::new_string(summary));
        self.status = tr("Copied WizTree summary for AI").into();
        cx.notify();
    }

    pub(crate) fn wiztree_pick_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Select Folder for WizTree").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = paths.await.ok()?.ok()??.into_iter().next()?;
            this.update_in(cx, |this, window, cx| {
                this.wiztree_scan(path, window, cx);
            })
            .ok()
        })
        .detach();
    }

    pub(crate) fn wiztree_scan(
        &mut self,
        dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wiztree.cancel_scan();
        self.wiztree.scan_gen = self.wiztree.scan_gen.wrapping_add(1);
        let gen = self.wiztree.scan_gen;
        let opts = self.wiztree.scan_opts();
        let progress = Arc::new(ScanProgress::new());
        self.wiztree.target = Some(dir.clone());
        self.wiztree.scanning = Some(progress.clone());
        self.wiztree.error = None;
        self.wiztree.hovered_tile = None;
        self.wiztree.invalidate_map();
        cx.notify();

        let progress_poll = progress.clone();
        cx.spawn_in(window, async move |this, cx| {
            while !progress_poll.cancel.load(Ordering::Relaxed) {
                cx.background_executor()
                    .timer(Duration::from_millis(65))
                    .await;
                let still_running = this
                    .update(cx, |this, cx| {
                        let active =
                            this.wiztree.scan_gen == gen && this.wiztree.scanning.is_some();
                        if active {
                            cx.notify();
                        }
                        active
                    })
                    .unwrap_or(false);
                if !still_running {
                    break;
                }
            }
        })
        .detach();

        let progress_bg = progress.clone();
        let task = cx.background_spawn(async move { scan_tree(&dir, opts, &progress_bg) });
        cx.spawn_in(window, async move |this, cx| {
            let res = task.await;
            _ = this.update(cx, |this, cx| {
                if this.wiztree.scan_gen != gen {
                    return;
                }
                this.wiztree.scanning = None;
                match res {
                    Ok(tree) => {
                        this.wiztree.focus = 0;
                        this.wiztree.history.clear();
                        this.wiztree.expanded.clear();
                        this.wiztree.hovered_tile = None;
                        this.wiztree.nav_dir = 0;
                        this.wiztree.nav_seq = this.wiztree.nav_seq.wrapping_add(1);
                        this.wiztree.tree = Some(Arc::new(tree));
                        this.wiztree.error = None;
                        this.wiztree.invalidate_map();
                    }
                    Err(e) if e != "Scan cancelled." => {
                        this.wiztree.error = Some(e);
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_wiztree_settings_popover(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.wiztree.settings_open;
        let anim_seq = self.wiztree.settings_anim;
        let cfg = self.wiztree.cfg;
        let weak_open = cx.entity().downgrade();
        let weak_body = cx.entity().downgrade();

        let trigger = Button::new("wt-settings-btn")
            .icon(Icon::new(gpui_kit::assets::IconName::Settings).size(theme::u(14.)))
            .ghost()
            .xsmall()
            .selected(open)
            .tooltip(tr("WizTree Settings"));

        Popover::new("wt-settings-pop")
            .anchor(Anchor::TopRight)
            .open(open)
            .appearance(false)
            .on_open_change(move |next_open, _, cx| {
                _ = weak_open.update(cx, |this, cx| {
                    this.wiztree.settings_open = *next_open;
                    if *next_open {
                        this.wiztree.settings_anim = this.wiztree.settings_anim.wrapping_add(1);
                    }
                    cx.notify();
                });
            })
            .trigger(trigger)
            .content(move |_, _, cx| {
                let w_map = weak_body.clone();
                let w_hid = weak_body.clone();
                let w_cac = weak_body.clone();
                let w_sort = weak_body.clone();
                let w_min = weak_body.clone();
                let w_ht = weak_body.clone();

                let toggle_row = |id: &'static str,
                                  title: &'static str,
                                  desc: &'static str,
                                  on: bool,
                                  on_flip: ToggleFn| {
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .py_1()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(c(theme::FG_STRONG))
                                            .child(tr(title)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(c(theme::FG_DIM))
                                            .child(tr(desc)),
                                    ),
                            )
                            .child(
                                crate::ui::McToggle::new(id)
                                    .small()
                                    .checked(on)
                                    .accessibility_label(tr(title))
                                    .on_click(move |v, w, cx| on_flip(v, w, cx)),
                            )
                    };

                let sort_seg = crate::ui::SegmentedControl::new(
                    "wt-cfg-sort",
                    [
                        (WizSort::Size, "wt-s-size", tr("Size")),
                        (WizSort::Files, "wt-s-files", tr("Files")),
                        (WizSort::Name, "wt-s-name", tr("Name")),
                    ]
                    .into_iter()
                    .map(|(s, id, label)| {
                        let w = w_sort.clone();
                        crate::ui::SegItem::new(id, label, cfg.sort == s, move |_, _, cx| {
                            _ = w.update(cx, |this, cx| {
                                this.wiztree.cfg.sort = s;
                                this.wiztree.nav_dir = 0;
                                this.wiztree.nav_seq = this.wiztree.nav_seq.wrapping_add(1);
                                cx.notify();
                            });
                        })
                    }),
                );

                let min_seg = crate::ui::SegmentedControl::new(
                    "wt-cfg-min",
                    [
                        WizMinSize::All,
                        WizMinSize::Kb100,
                        WizMinSize::Mb1,
                        WizMinSize::Mb10,
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let w = w_min.clone();
                        crate::ui::SegItem::new(
                            SharedString::from(format!("wt-m-{i}")),
                            tr(m.label()),
                            cfg.min_size == m,
                            move |_, _, cx| {
                                _ = w.update(cx, |this, cx| {
                                    this.wiztree.cfg.min_size = m;
                                    cx.notify();
                                });
                            },
                        )
                    }),
                );

                let ht_seg = crate::ui::SegmentedControl::new(
                    "wt-cfg-ht",
                    [
                        WizMapHeight::Compact,
                        WizMapHeight::Normal,
                        WizMapHeight::Tall,
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, mh)| {
                        let w = w_ht.clone();
                        crate::ui::SegItem::new(
                            SharedString::from(format!("wt-h-{i}")),
                            tr(mh.label()),
                            cfg.map_height == mh,
                            move |_, _, cx| {
                                _ = w.update(cx, |this, cx| {
                                    this.wiztree.cfg.map_height = mh;
                                    this.wiztree.invalidate_map();
                                    cx.notify();
                                });
                            },
                        )
                    }),
                );

                let panel = v_flex()
                    .id("wt-settings-card")
                    .w(theme::u(290.))
                    .max_w(relative(0.92))
                    .p_3()
                    .gap_2()
                    .glass_pop()
                    .occlude()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(theme::FG_STRONG))
                            .child(tr("WizTree Settings")),
                    )
                    .child(toggle_row(
                        "wt-sw-map",
                        "Show Treemap",
                        "Visual disk map at the bottom",
                        cfg.show_map,
                        Box::new(move |_, _, cx| {
                            _ = w_map.update(cx, |this, cx| {
                                this.wiztree.cfg.show_map = !this.wiztree.cfg.show_map;
                                cx.notify();
                            });
                        }),
                    ))
                    .child(toggle_row(
                        "wt-sw-hidden",
                        "Include hidden files",
                        "Scan dotfiles like .git and .env",
                        cfg.show_hidden,
                        Box::new(move |_, w, cx| {
                            _ = w_hid.update(cx, |this, cx| {
                                this.wiztree.cfg.show_hidden = !this.wiztree.cfg.show_hidden;
                                if let Some(dir) = this.wiztree.target.clone() {
                                    this.wiztree_scan(dir, w, cx);
                                }
                                cx.notify();
                            });
                        }),
                    ))
                    .child(toggle_row(
                        "wt-sw-cache",
                        "Skip build caches",
                        "Skip node_modules, target, dist, build",
                        cfg.skip_caches,
                        Box::new(move |_, w, cx| {
                            _ = w_cac.update(cx, |this, cx| {
                                this.wiztree.cfg.skip_caches = !this.wiztree.cfg.skip_caches;
                                if let Some(dir) = this.wiztree.target.clone() {
                                    this.wiztree_scan(dir, w, cx);
                                }
                                cx.notify();
                            });
                        }),
                    ))
                    .child(div().h(px(1.)).bg(c(theme::BORDER)))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(c(theme::FG_MUTED))
                                    .child(tr("Sort by")),
                            )
                            .child(sort_seg),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(c(theme::FG_MUTED))
                                    .child(tr("Minimum size")),
                            )
                            .child(min_seg),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(c(theme::FG_MUTED))
                                    .child(tr("Treemap height")),
                            )
                            .child(ht_seg),
                    );

                if cx.reduce_motion() {
                    panel.into_any_element()
                } else {
                    panel
                        .with_animation(
                            SharedString::from(format!("wt-cfg-in-{anim_seq}")),
                            Animation::new(Duration::from_millis(150))
                                .with_easing(gpui_kit::ease_out_quint()),
                            |el, t| el.opacity(0.2 + 0.8 * t).mt(px((1. - t) * -6.)),
                        )
                        .into_any_element()
                }
            })
            .into_any_element()
    }

    pub(crate) fn render_wiztree(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let scanning = self.wiztree.scanning.as_ref();
        let tree = self.wiztree.tree.as_ref();
        let can_workspace = self
            .root
            .as_ref()
            .is_some_and(|r| self.wiztree.target.as_ref() != Some(r));
        let motion = !cx.reduce_motion();

        let summary: SharedString = if let Some(p) = scanning {
            let files = format_count(p.files.load(Ordering::Relaxed));
            let folders = format_count(p.folders.load(Ordering::Relaxed));
            let size = format_bytes(p.bytes.load(Ordering::Relaxed));
            let ms = p.started.elapsed().as_millis();
            crate::trf!(
                "Scanning... {files} files · {folders} folders · {size} ({ms} ms)";
                files => files,
                folders => folders,
                size => size,
                ms => ms
            )
            .into()
        } else if let Some(t) = tree {
            let files = format_count(t.total_files());
            let folders = format_count(t.total_folders());
            let size = format_bytes(t.total_size());
            let ms = t.elapsed_ms;
            let base = crate::trf!(
                "{files} files · {folders} folders · {size} in {ms} ms";
                files => files,
                folders => folders,
                size => size,
                ms => ms
            );
            if t.skipped > 0 {
                format!(
                    "{base} · {}",
                    crate::trf!("{n} skipped"; n => format_count(t.skipped))
                )
                .into()
            } else {
                base.into()
            }
        } else {
            tr("Fast Rust folder and disk usage analyzer").into()
        };

        let header = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_SIDE))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2p5()
                    .items_center()
                    .child(
                        Icon::new(gpui_kit::assets::IconName::HardDrive)
                            .flex_none()
                            .size(theme::u(17.))
                            .text_color(c(theme::ACCENT_TEXT)),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(tr("Disk Usage (WizTree)")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .truncate()
                                    .child(summary.clone()),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .flex_wrap()
                    .gap_1p5()
                    .items_center()
                    .when(can_workspace, |h| {
                        h.child(
                            Button::new("wt-ws")
                                .label(tr("Workspace"))
                                .xsmall()
                                .ghost()
                                .on_click(cx.listener(|this, _, w, cx| {
                                    if let Some(root) = this.root.clone() {
                                        this.wiztree_scan(root, w, cx);
                                    }
                                })),
                        )
                    })
                    .when(tree.is_some() && scanning.is_none(), |h| {
                        h.child(
                            Button::new("wt-ask-agent")
                                .icon(
                                    Icon::new(gpui_kit::assets::IconName::MessageSquareCode)
                                        .size(theme::u(13.)),
                                )
                                .label(tr("Ask Agent"))
                                .xsmall()
                                .outline()
                                .tooltip(tr(
                                    "Attach this WizTree disk summary to the AI agent",
                                ))
                                .on_click(cx.listener(|this, _, w, cx| {
                                    this.wiztree_ask_agent(w, cx);
                                })),
                        )
                        .child(
                            Button::new("wt-copy-ai")
                                .icon(
                                    Icon::new(gpui_kit::assets::IconName::Copy)
                                        .size(theme::u(13.)),
                                )
                                .xsmall()
                                .ghost()
                                .tooltip(tr("Copy compact WizTree summary for AI"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.wiztree_copy_ai_summary(cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("wt-pick")
                            .label(tr("Pick Folder..."))
                            .xsmall()
                            .outline()
                            .on_click(
                                cx.listener(|this, _, w, cx| this.wiztree_pick_folder(w, cx)),
                            ),
                    )
                    .child(if scanning.is_some() {
                        Button::new("wt-cancel")
                            .label(tr("Cancel"))
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.wiztree.cancel_scan();
                                cx.notify();
                            }))
                    } else {
                        Button::new("wt-rescan")
                            .label(tr("Rescan"))
                            .xsmall()
                            .primary()
                            .disabled(self.wiztree.target.is_none() && self.root.is_none())
                            .on_click(cx.listener(|this, _, w, cx| {
                                if let Some(dir) =
                                    this.wiztree.target.clone().or_else(|| this.root.clone())
                                {
                                    this.wiztree_scan(dir, w, cx);
                                }
                            }))
                    })
                    .child(self.render_wiztree_settings_popover(cx))
                    .child(
                        Button::new("wt-close")
                            .icon(Icon::new(gpui_kit::assets::IconName::X).size(theme::u(14.)))
                            .xsmall()
                            .ghost()
                            .tooltip(tr("Close"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.wiztree.page = false;
                                this.wiztree.settings_open = false;
                                this.wiztree.cancel_scan();
                                cx.notify();
                            })),
                    ),
            );

        let body: AnyElement = if let Some(err) = &self.wiztree.error {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .p_6()
                .child(
                    Icon::new(gpui_kit::assets::IconName::TriangleAlert)
                        .size(theme::u(28.))
                        .text_color(c(theme::DANGER)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(c(theme::DANGER))
                        .text_center()
                        .max_w(theme::u(420.))
                        .child(err.clone()),
                )
                .child(
                    Button::new("wt-err-pick")
                        .label(tr("Pick Folder..."))
                        .small()
                        .primary()
                        .on_click(cx.listener(|this, _, w, cx| this.wiztree_pick_folder(w, cx))),
                )
                .into_any_element()
        } else if tree.is_none() && scanning.is_some() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .p_6()
                .child(
                    gpui_kit::component::spinner::Spinner::new()
                        .icon(gpui_kit::assets::IconName::LoaderCircle)
                        .large()
                        .color(c(theme::ACCENT_TEXT)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(theme::FG_STRONG))
                        .child(tr("Indexing folder structure...")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .child(summary),
                )
                .into_any_element()
        } else if let Some(t) = tree {
            self.render_wiztree_content(t, cx)
        } else {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .p_6()
                .child(
                    Icon::new(gpui_kit::assets::IconName::HardDrive)
                        .size(theme::u(32.))
                        .text_color(c(theme::FG_DIM)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(theme::FG_STRONG))
                        .child(tr("No folder scanned yet")),
                )
                .child(
                    div()
                        .max_w(theme::u(360.))
                        .text_xs()
                        .text_center()
                        .text_color(c(theme::FG_DIM))
                        .child(tr(
                            "Open a project folder or pick any directory on your PC to inspect disk usage, largest files, and extension breakdown.",
                        )),
                )
                .child(
                    Button::new("wt-empty-pick")
                        .label(tr("Pick Folder..."))
                        .small()
                        .primary()
                        .on_click(cx.listener(|this, _, w, cx| this.wiztree_pick_folder(w, cx))),
                )
                .into_any_element()
        };

        let page = v_flex()
            .id("wiztree-page")
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(c(theme::BG_EDITOR))
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, w, cx| {
                    cx.stop_propagation();
                    this.workbench_nav_back(w, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, w, cx| {
                    cx.stop_propagation();
                    this.workbench_nav_forward(w, cx);
                }),
            )
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                let k = e.keystroke.key.as_str();
                if k == "escape" {
                    this.wiztree.page = false;
                    this.wiztree.settings_open = false;
                    this.wiztree.cancel_scan();
                    cx.notify();
                } else if e.keystroke.modifiers.alt && k == "left" {
                    cx.stop_propagation();
                    this.workbench_nav_back(w, cx);
                } else if e.keystroke.modifiers.alt && k == "right" {
                    cx.stop_propagation();
                    this.workbench_nav_forward(w, cx);
                } else if e.keystroke.modifiers.alt && k == "up" {
                    cx.stop_propagation();
                    this.wiztree_up(cx);
                }
            }))
            .child(header)
            .child(body);

        if !motion {
            page.into_any_element()
        } else {
            let seq = self.wiztree.open_seq;
            page.with_animation(
                SharedString::from(format!("wt-open-{seq}")),
                Animation::new(Duration::from_millis(165)).with_easing(gpui_kit::ease_out_quint()),
                |el, t| el.opacity(0.35 + 0.65 * t),
            )
            .into_any_element()
        }
    }

    fn render_wiztree_content(&self, tree: &Arc<DiskTree>, cx: &mut Context<Self>) -> AnyElement {
        let focus = self
            .wiztree
            .focus
            .min(tree.nodes.len().saturating_sub(1) as u32);
        let crumbs = tree.breadcrumbs(focus);
        let query = self.wiztree.filter.read(cx).value().trim().to_string();
        let can_back = !self.wiztree.history.back.is_empty() || !self.nav_back.is_empty();
        let can_fwd = !self.wiztree.history.forward.is_empty() || !self.nav_forward.is_empty();
        let can_up = tree
            .nodes
            .get(focus as usize)
            .and_then(|n| n.parent)
            .is_some();
        let motion = !cx.reduce_motion();
        let nav_seq = self.wiztree.nav_seq;
        let nav_dir = self.wiztree.nav_dir;

        let nav_btn = |id: &'static str,
                       icon: gpui_kit::assets::IconName,
                       tip: &'static str,
                       enabled: bool| {
            Button::new(id)
                .icon(Icon::new(icon).size(theme::u(13.)))
                .ghost()
                .xsmall()
                .disabled(!enabled)
                .tooltip(tr(tip))
        };

        let crumb_bar = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_1p5()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .child(
                h_flex()
                    .min_w_0()
                    .flex_wrap()
                    .gap_1()
                    .items_center()
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_0p5()
                            .pr_1()
                            .mr_1()
                            .border_r_1()
                            .border_color(c(theme::BORDER))
                            .child(
                                nav_btn(
                                    "wt-nav-back",
                                    gpui_kit::assets::IconName::ArrowLeft,
                                    "Back (Mouse Back / Alt+Left)",
                                    can_back,
                                )
                                .on_click(cx.listener(
                                    |this, _, w, cx| {
                                        this.workbench_nav_back(w, cx);
                                    },
                                )),
                            )
                            .child(
                                nav_btn(
                                    "wt-nav-fwd",
                                    gpui_kit::assets::IconName::ArrowRight,
                                    "Forward (Mouse Forward / Alt+Right)",
                                    can_fwd,
                                )
                                .on_click(cx.listener(
                                    |this, _, w, cx| {
                                        this.workbench_nav_forward(w, cx);
                                    },
                                )),
                            )
                            .child(
                                nav_btn(
                                    "wt-nav-up",
                                    gpui_kit::assets::IconName::ArrowUp,
                                    "Parent folder (Alt+Up)",
                                    can_up,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.wiztree_up(cx);
                                    },
                                )),
                            ),
                    )
                    .children(crumbs.iter().enumerate().map(|(i, &cid)| {
                        let name = tree.nodes[cid as usize].name.clone();
                        let last = i + 1 == crumbs.len();
                        h_flex()
                            .gap_1()
                            .items_center()
                            .when(i > 0, |h| {
                                h.child(div().text_xs().text_color(c(theme::FG_DIM)).child("/"))
                            })
                            .child(
                                div()
                                    .id(("wt-crumb", cid as usize))
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded(px(4.))
                                    .text_xs()
                                    .font_weight(if last {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(c(if last {
                                        theme::FG_STRONG
                                    } else {
                                        theme::FG_MUTED
                                    }))
                                    .when(!last, |d| {
                                        d.cursor_pointer()
                                            .hover(|s| {
                                                s.bg(c(theme::BG_HOVER))
                                                    .text_color(c(theme::FG_STRONG))
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.wiztree_goto(cid, -1, cx);
                                            }))
                                    })
                                    .child(name),
                            )
                    })),
            )
            .child(
                h_flex()
                    .flex_none()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(crate::ui::SegmentedControl::new(
                        "seg-wiztree-tab",
                        [
                            (WizTab::Tree, "wt-tab-tree", tr("Tree")),
                            (WizTab::Extensions, "wt-tab-ext", tr("Extensions")),
                            (WizTab::Largest, "wt-tab-big", tr("Largest Files")),
                        ]
                        .into_iter()
                        .map(|(tab, id, label)| {
                            crate::ui::SegItem::new(
                                id,
                                label,
                                self.wiztree.tab == tab,
                                cx.listener(move |this, _, _, cx| {
                                    if this.wiztree.tab != tab {
                                        this.wiztree.tab = tab;
                                        this.wiztree.nav_dir = 0;
                                        this.wiztree.nav_seq = this.wiztree.nav_seq.wrapping_add(1);
                                        cx.notify();
                                    }
                                }),
                            )
                        }),
                    ))
                    .child(
                        div()
                            .w(theme::u(190.))
                            .min_w(theme::u(120.))
                            .child(crate::ui::McInput::new(&self.wiztree.filter).xsmall().cleanable(true)),
                    ),
            );

        let table: AnyElement = match self.wiztree.tab {
            WizTab::Tree => self.render_wiztree_tree(tree, focus, &query, cx),
            WizTab::Extensions => self.render_wiztree_extensions(tree, &query),
            WizTab::Largest => self.render_wiztree_largest(tree, &query, cx),
        };

        let table_wrap: AnyElement = if !motion || nav_seq == 0 {
            v_flex()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(table)
                .into_any_element()
        } else {
            let dx = match nav_dir {
                d if d > 0 => 10.,
                d if d < 0 => -10.,
                _ => 0.,
            };
            let dy = if nav_dir == 0 { 5. } else { 0. };
            v_flex()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(table)
                .with_animation(
                    SharedString::from(format!("wt-nav-{nav_seq}")),
                    Animation::new(Duration::from_millis(165))
                        .with_easing(gpui_kit::ease_out_quint()),
                    move |el, t| {
                        el.opacity(0.25 + 0.75 * t)
                            .ml(px((1. - t) * dx))
                            .mt(px((1. - t) * dy))
                    },
                )
                .into_any_element()
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(crumb_bar)
            .child(table_wrap)
            .when(self.wiztree.cfg.show_map, |v| {
                v.child(self.render_wiztree_treemap(tree, focus, cx))
            })
            .into_any_element()
    }

    fn render_wiztree_tree(
        &self,
        tree: &Arc<DiskTree>,
        focus: u32,
        query: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cfg = self.wiztree.cfg;
        let rows = flatten_visible_rows(
            tree,
            focus,
            &self.wiztree.expanded,
            query,
            cfg.sort,
            cfg.min_size.bytes(),
            260,
        );
        let hovered = self.wiztree.hovered_tile;

        let col_header = h_flex()
            .w_full()
            .min_w_0()
            .h(theme::u(22.))
            .px_3()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_SIDE))
            .text_size(px(10.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(theme::FG_DIM))
            .child(div().flex_1().min_w_0().pl(theme::u(18.)).child(tr("Name")))
            .child(
                div()
                    .flex_none()
                    .w(theme::u(112.))
                    .text_right()
                    .child(tr("Share")),
            )
            .child(
                div()
                    .flex_none()
                    .w(theme::u(76.))
                    .text_right()
                    .child(tr("Size")),
            )
            .child(
                div()
                    .flex_none()
                    .w(theme::u(60.))
                    .text_right()
                    .child(tr("Files")),
            )
            .child(div().flex_none().w(theme::u(48.)))
            .child(div().flex_none().w(theme::u(20.)));

        if rows.is_empty() {
            return v_flex()
                .size_full()
                .child(col_header)
                .child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(if query.is_empty() && cfg.min_size == WizMinSize::All {
                            tr("This folder is empty.")
                        } else {
                            tr("No matching files or folders.")
                        }),
                )
                .into_any_element();
        }

        v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(col_header)
            .child(
                v_flex()
                    .id("wt-tree-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_0p5()
                    .children(rows.into_iter().map(|row| {
                        let node = &tree.nodes[row.id as usize];
                        let pct = ((node.size as f64 / row.parent_size.max(1) as f64) * 100.)
                            .clamp(0., 100.) as f32;
                        let expanded = self.wiztree.expanded.contains(&row.id);
                        let is_dir = node.is_dir;
                        let is_hov = hovered == Some(row.id);
                        let rid = row.id;
                        let path = tree.full_path(rid);
                        let path_reveal = path.clone();
                        let row_icon = if is_dir {
                            files::folder_icon_el(&node.name, expanded, row.depth == 0, theme::u(14.))
                        } else {
                            files::file_icon_el(&node.name, theme::u(14.))
                        };
                        let bar_col = dominant_category(tree, rid).color();
                        let indent = (row.depth as f32 * 16.).min(160.);
                        let display_name = if query.is_empty() {
                            node.name.clone()
                        } else {
                            tree.rel_path(rid)
                        };

                        h_flex()
                            .id(("wt-row", rid as usize))
                            .w_full()
                            .min_w_0()
                            .h(theme::u(28.))
                            .px_3()
                            .gap_2()
                            .items_center()
                            .cursor_pointer()
                            .when(is_hov, |d| d.bg(c(theme::BG_HOVER)))
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                                if *entered {
                                    if this.wiztree.hovered_tile != Some(rid) {
                                        this.wiztree.hovered_tile = Some(rid);
                                        cx.notify();
                                    }
                                } else if this.wiztree.hovered_tile == Some(rid) {
                                    this.wiztree.hovered_tile = None;
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                if is_dir {
                                    if !this.wiztree.expanded.remove(&rid) {
                                        this.wiztree.expanded.insert(rid);
                                    }
                                    cx.notify();
                                } else {
                                    this.wiztree_open_file(path.clone(), w, cx);
                                }
                            }))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .pl(px(indent))
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(theme::u(14.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(is_dir, |d| {
                                                d.child(
                                                    Icon::new(if expanded {
                                                        gpui_kit::assets::IconName::ChevronDown
                                                    } else {
                                                        gpui_kit::assets::IconName::ChevronRight
                                                    })
                                                    .size(theme::u(12.))
                                                    .text_color(c(theme::FG_DIM)),
                                                )
                                            }),
                                    )
                                    .child(row_icon)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(c(if is_dir {
                                                theme::FG_STRONG
                                            } else {
                                                theme::FG
                                            }))
                                            .child(display_name),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_none()
                                    .w(theme::u(112.))
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .h(px(6.))
                                            .rounded(px(3.))
                                            .bg(c(theme::BG_SIDE))
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative((pct / 100.).clamp(0.02, 1.)))
                                                    .bg(bar_col.opacity(0.85)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(theme::u(38.))
                                            .text_right()
                                            .text_xs()
                                            .text_color(c(theme::FG_DIM))
                                            .child(format!("{pct:.1}%")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(theme::u(76.))
                                    .text_right()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(format_bytes(node.size)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(theme::u(60.))
                                    .text_right()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(if is_dir {
                                        format_count(node.files)
                                    } else {
                                        String::new()
                                    }),
                            )
                            // Keep a fixed 48px Focus slot on both folders and files so columns stay aligned.
                            .child(if is_dir {
                                div()
                                    .id(("wt-zoom", rid as usize))
                                    .flex_none()
                                    .w(theme::u(48.))
                                    .py_0p5()
                                    .rounded(px(4.))
                                    .text_center()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .hover(|s| {
                                        s.bg(c(theme::BG_ELEVATED))
                                            .text_color(c(theme::ACCENT_TEXT))
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.wiztree_goto(rid, 1, cx);
                                    }))
                                    .child(tr("Focus"))
                                    .into_any_element()
                            } else {
                                div().flex_none().w(theme::u(48.)).into_any_element()
                            })
                            .child(
                                div()
                                    .id(("wt-rev", rid as usize))
                                    .flex_none()
                                    .size(theme::u(20.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .text_color(c(theme::FG_DIM))
                                    .hover(|s| {
                                        s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG))
                                    })
                                    .tooltip(|w, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(tr(
                                            "Reveal in File Explorer",
                                        ))
                                        .build(w, cx)
                                    })
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        cx.reveal_path(&path_reveal);
                                    })
                                    .child(
                                        Icon::new(gpui_kit::assets::IconName::ExternalLink)
                                            .size(theme::u(12.)),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }

    fn render_wiztree_extensions(&self, tree: &Arc<DiskTree>, query: &str) -> AnyElement {
        let q = query.to_ascii_lowercase();
        let min_bytes = self.wiztree.cfg.min_size.bytes();
        let total = tree.total_size().max(1) as f64;
        let items: Vec<&ExtStat> = tree
            .extensions
            .iter()
            .filter(|e| {
                e.bytes >= min_bytes && (q.is_empty() || e.ext.to_ascii_lowercase().contains(&q))
            })
            .take(150)
            .collect();
        if items.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(tr("No matching extensions."))
                .into_any_element();
        }
        v_flex()
            .id("wt-ext-list")
            .size_full()
            .overflow_y_scroll()
            .py_1()
            .children(items.into_iter().enumerate().map(|(i, e)| {
                let pct = ((e.bytes as f64 / total) * 100.).clamp(0., 100.) as f32;
                let cat = FileCategory::from_ext(e.ext.trim_start_matches('.'));
                h_flex()
                    .id(("wt-ext", i))
                    .w_full()
                    .min_w_0()
                    .h(theme::u(28.))
                    .px_4()
                    .gap_2p5()
                    .items_center()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(
                        div()
                            .flex_none()
                            .size(px(8.))
                            .rounded_full()
                            .bg(cat.color()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(theme::FG_STRONG))
                            .child(e.ext.clone()),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .w(theme::u(140.))
                            .gap_1p5()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(6.))
                                    .rounded(px(3.))
                                    .bg(c(theme::BG_SIDE))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative((pct / 100.).clamp(0.02, 1.)))
                                            .bg(cat.color().opacity(0.85)),
                                    ),
                            )
                            .child(
                                div()
                                    .w(theme::u(38.))
                                    .text_right()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(format!("{pct:.1}%")),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(80.))
                            .text_right()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(theme::FG_STRONG))
                            .child(format_bytes(e.bytes)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(82.))
                            .text_right()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(crate::trf!("{n} files"; n => format_count(e.files))),
                    )
            }))
            .into_any_element()
    }

    fn render_wiztree_largest(
        &self,
        tree: &Arc<DiskTree>,
        query: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let q = query.to_ascii_lowercase();
        let min_bytes = self.wiztree.cfg.min_size.bytes();
        let total = tree.total_size().max(1) as f64;
        let items: Vec<&LargestFile> = tree
            .largest
            .iter()
            .filter(|f| {
                f.size >= min_bytes
                    && (q.is_empty()
                        || tree.nodes[f.node as usize]
                            .name
                            .to_ascii_lowercase()
                            .contains(&q)
                        || tree.rel_path(f.node).to_ascii_lowercase().contains(&q))
            })
            .collect();
        if items.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(tr("No matching files or folders."))
                .into_any_element();
        }
        v_flex()
            .id("wt-big-list")
            .size_full()
            .overflow_y_scroll()
            .py_1()
            .children(items.into_iter().enumerate().map(|(i, f)| {
                let node = &tree.nodes[f.node as usize];
                let pct = ((f.size as f64 / total) * 100.).clamp(0., 100.) as f32;
                let rel = tree.rel_path(f.node);
                let path = tree.full_path(f.node);
                let path_reveal = path.clone();
                h_flex()
                    .id(("wt-big", i))
                    .w_full()
                    .min_w_0()
                    .h(theme::u(28.))
                    .px_4()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.wiztree_open_file(path.clone(), w, cx);
                    }))
                    .child(files::file_icon_el(&node.name, theme::u(14.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::FG_STRONG))
                            .child(rel),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(44.))
                            .text_right()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(format!("{pct:.1}%")),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(80.))
                            .text_right()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(theme::FG_STRONG))
                            .child(format_bytes(f.size)),
                    )
                    .child(
                        div()
                            .id(("wt-big-rev", i))
                            .flex_none()
                            .size(theme::u(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .text_color(c(theme::FG_DIM))
                            .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(tr(
                                    "Reveal in File Explorer",
                                ))
                                .build(w, cx)
                            })
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                cx.reveal_path(&path_reveal);
                            })
                            .child(
                                Icon::new(gpui_kit::assets::IconName::ExternalLink)
                                    .size(theme::u(12.)),
                            ),
                    )
            }))
            .into_any_element()
    }

    fn render_wiztree_treemap(
        &self,
        tree: &Arc<DiskTree>,
        focus: u32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = self.wiztree.hovered_tile;
        let focus_size = tree.nodes.get(focus as usize).map_or(1, |n| n.size.max(1));
        let hover_info: AnyElement = if let Some(hid) =
            hovered.filter(|&i| (i as usize) < tree.nodes.len())
        {
            let n = &tree.nodes[hid as usize];
            let pct = (n.size as f64 / focus_size as f64 * 100.).clamp(0., 100.);
            let rel = tree.rel_path(hid);
            let title = if n.is_dir {
                format!("{}/", n.name)
            } else {
                n.name.clone()
            };
            let meta = if n.is_dir {
                format!(
                    "{} ({pct:.1}%) · {}",
                    format_bytes(n.size),
                    crate::trf!("{n} files"; n => format_count(n.files))
                )
            } else {
                format!("{} ({pct:.1}%)", format_bytes(n.size))
            };
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .max_w(theme::u(190.))
                        .truncate()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_STRONG))
                        .child(title),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(theme::ACCENT_TEXT))
                        .child(meta),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(rel),
                )
                .into_any_element()
        } else {
            div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(tr(
                        "Hover a block to inspect size, click a folder to zoom in, or click a file to open it.",
                    ))
                    .into_any_element()
        };

        let legend = h_flex()
            .flex_none()
            .flex_wrap()
            .gap_2()
            .items_center()
            .children(
                [
                    FileCategory::Code,
                    FileCategory::Media,
                    FileCategory::Build,
                    FileCategory::Data,
                    FileCategory::Temp,
                ]
                .into_iter()
                .map(|cat| {
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(div().size(px(8.)).rounded(px(2.)).bg(cat.color()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(tr(cat.label())),
                        )
                }),
            );

        let tree_paint = tree.clone();
        let cache_paint = self.wiztree.tiles_cache.clone();
        let origin_paint = self.wiztree.map_origin.clone();
        let bg_sep = c(theme::BG_EDITOR);
        let hi_border = c(theme::FG_STRONG);
        let folder_bg = c(theme::BG_ELEVATED);
        let folder_edge = c(theme::BORDER_STRONG);

        let view_id = cx.entity_id();
        let map_canvas = canvas(
            move |bounds, _, cx| {
                origin_paint.set(bounds.origin);
                let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let mut cache = cache_paint.borrow_mut();
                if cache.2 != focus || (cache.0 - w).abs() > 1. || (cache.1 - h).abs() > 1. {
                    cache.0 = w;
                    cache.1 = h;
                    cache.2 = focus;
                    cache.3 = layout_treemap(&tree_paint, focus, w, h);
                    cx.defer(move |cx| cx.notify(view_id));
                }
            },
            {
                let cache = self.wiztree.tiles_cache.clone();
                move |bounds, _, window, _| {
                    let ox = f32::from(bounds.origin.x);
                    let oy = f32::from(bounds.origin.y);
                    let cache = cache.borrow();
                    for tile in &cache.3 {
                        let pad = if tile.depth == 0 { 1. } else { 0.75 };
                        let tw = (tile.w - pad * 2.).max(0.5);
                        let th = (tile.h - pad * 2.).max(0.5);
                        let rect = Bounds::new(
                            point(px(ox + tile.x + pad), px(oy + tile.y + pad)),
                            size(px(tw), px(th)),
                        );
                        let is_hov = hovered == Some(tile.node);
                        let fill_col = if tile.is_dir && tile.depth == 0 {
                            if is_hov {
                                folder_bg.opacity(1.)
                            } else {
                                folder_bg
                            }
                        } else {
                            tile.category.color().opacity(if is_hov {
                                0.96
                            } else if tile.depth == 1 {
                                0.64
                            } else {
                                0.78
                            })
                        };
                        let border_col = if is_hov {
                            hi_border
                        } else if tile.is_dir && tile.depth == 0 {
                            folder_edge
                        } else {
                            bg_sep.opacity(0.65)
                        };
                        window.paint_quad(PaintQuad {
                            bounds: rect,
                            corner_radii: Corners::all(px(if tile.depth == 0 { 3. } else { 2. })),
                            background: fill_col.into(),
                            border_widths: Edges::all(px(if is_hov { 1.5 } else { 1. })),
                            border_color: border_col,
                            border_style: BorderStyle::Solid,
                        });
                    }
                }
            },
        )
        .size_full();

        let cache_move = self.wiztree.tiles_cache.clone();
        let origin_move = self.wiztree.map_origin.clone();
        let cache_click = self.wiztree.tiles_cache.clone();
        let origin_click = self.wiztree.map_origin.clone();
        let tree_click = tree.clone();

        let labels: Vec<AnyElement> = {
            let cache = self.wiztree.tiles_cache.borrow();
            cache
                .3
                .iter()
                .filter(|t| {
                    (t.depth == 0 && t.w >= 82. && t.h >= 22.)
                        || (t.depth == 1 && t.w >= 96. && t.h >= 34.)
                })
                .take(26)
                .filter_map(|t| {
                    let n = tree.nodes.get(t.node as usize)?;
                    let text = if t.w >= 132. {
                        format!("{} · {}", n.name, format_bytes(n.size))
                    } else {
                        n.name.clone()
                    };
                    Some(
                        div()
                            .absolute()
                            .left(px(t.x + 5.))
                            .top(px(t.y + 2.))
                            .w(px((t.w - 10.).max(0.)))
                            .h(px(15.))
                            .overflow_hidden()
                            .truncate()
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if t.is_dir && t.depth == 0 {
                                c(theme::FG_STRONG)
                            } else {
                                c(theme::ON_ACCENT)
                            })
                            .child(text)
                            .into_any_element(),
                    )
                })
                .collect()
        };

        let map_inner = div()
            .id("wt-treemap")
            .relative()
            .flex_1()
            .w_full()
            .min_h_0()
            .cursor_pointer()
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                let o = origin_move.get();
                let lx = f32::from(ev.position.x - o.x);
                let ly = f32::from(ev.position.y - o.y);
                let hit = hit_test_tile(&cache_move.borrow().3, lx, ly);
                if this.wiztree.hovered_tile != hit {
                    this.wiztree.hovered_tile = hit;
                    cx.notify();
                }
            }))
            .on_hover(cx.listener(|this, entered: &bool, _, cx| {
                if !*entered && this.wiztree.hovered_tile.is_some() {
                    this.wiztree.hovered_tile = None;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                    let o = origin_click.get();
                    let lx = f32::from(ev.position.x - o.x);
                    let ly = f32::from(ev.position.y - o.y);
                    let Some(hit) = hit_test_tile(&cache_click.borrow().3, lx, ly) else {
                        return;
                    };
                    let Some(node) = tree_click.nodes.get(hit as usize) else {
                        return;
                    };
                    if node.is_dir {
                        this.wiztree_goto(hit, 1, cx);
                    } else {
                        let path = tree_click.full_path(hit);
                        this.wiztree_open_file(path, w, cx);
                    }
                }),
            )
            .child(map_canvas)
            .children(labels);

        let motion = !cx.reduce_motion();
        let nav_seq = self.wiztree.nav_seq;
        let map_body: AnyElement = if !motion || nav_seq == 0 {
            map_inner.into_any_element()
        } else {
            map_inner
                .with_animation(
                    SharedString::from(format!("wt-map-{nav_seq}")),
                    Animation::new(Duration::from_millis(165))
                        .with_easing(gpui_kit::ease_out_quint()),
                    |el, t| el.opacity(0.35 + 0.65 * t),
                )
                .into_any_element()
        };

        v_flex()
            .w_full()
            .h(theme::u(self.wiztree.cfg.map_height.px()))
            .flex_none()
            .border_t_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_SIDE))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(c(theme::BORDER))
                    .child(hover_info)
                    .child(legend),
            )
            .child(map_body)
            .into_any_element()
    }
}

fn hit_test_tile(tiles: &[TreemapTile], x: f32, y: f32) -> Option<u32> {
    // Search in reverse so inner child tiles win over their enclosing folder frame.
    tiles
        .iter()
        .rev()
        .find_map(|t| (x >= t.x && x <= t.x + t.w && y >= t.y && y <= t.y + t.h).then_some(t.node))
}

#[cfg(test)]
mod tests {
    use super::{
        flatten_visible_rows, format_bytes, format_count, hit_test_tile, layout_treemap, scan_tree,
        FolderHistory, ScanOpts, ScanProgress, WizSort,
    };
    use std::collections::HashSet;
    use std::sync::Arc;

    #[test]
    fn scans_directory_sizes_counts_and_extensions_accurately() {
        let root = std::env::temp_dir().join(format!("fluxcode-wiztree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("assets").join("img")).unwrap();
        std::fs::write(root.join("src").join("main.rs"), vec![b'a'; 1200]).unwrap();
        std::fs::write(root.join("src").join("lib.rs"), vec![b'b'; 800]).unwrap();
        std::fs::write(
            root.join("assets").join("img").join("logo.png"),
            vec![0u8; 4000],
        )
        .unwrap();
        std::fs::write(root.join("README"), vec![b'x'; 200]).unwrap();

        let tree = scan_tree(&root, ScanOpts::default(), &Arc::new(ScanProgress::new())).unwrap();
        assert_eq!(tree.total_size(), 6200);
        assert_eq!(tree.total_files(), 4);
        assert_eq!(tree.total_folders(), 3);
        assert_eq!(tree.nodes[0].children.len(), 3);
        let first = &tree.nodes[tree.nodes[0].children[0] as usize];
        assert_eq!(first.name, "assets");
        assert_eq!(first.size, 4000);
        assert_eq!(tree.extensions[0].ext, ".png");
        assert_eq!(tree.extensions[0].bytes, 4000);
        assert_eq!(tree.extensions[1].ext, ".rs");
        assert_eq!(tree.extensions[1].bytes, 2000);
        assert_eq!(tree.extensions[1].files, 2);
        assert_eq!(tree.largest[0].size, 4000);
        assert_eq!(tree.rel_path(tree.largest[0].node), "assets/img/logo.png");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_respects_hidden_and_skip_cache_options() {
        let root = std::env::temp_dir().join(format!("fluxcode-wt-opts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("node_modules").join("dep.js"), vec![0u8; 5000]).unwrap();
        std::fs::write(root.join(".git").join("HEAD"), vec![0u8; 100]).unwrap();
        std::fs::write(root.join("app.rs"), vec![0u8; 400]).unwrap();

        let full = scan_tree(&root, ScanOpts::default(), &Arc::new(ScanProgress::new())).unwrap();
        assert_eq!(full.total_size(), 5500);

        let clean = scan_tree(
            &root,
            ScanOpts {
                show_hidden: false,
                skip_caches: true,
            },
            &Arc::new(ScanProgress::new()),
        )
        .unwrap();
        assert_eq!(clean.total_size(), 400);
        assert_eq!(clean.total_files(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn folder_history_back_and_forward_track_navigation() {
        let mut h = FolderHistory::default();
        h.push(0, 3);
        h.push(3, 7);
        assert_eq!(h.go_back(7), Some(3));
        assert_eq!(h.go_back(3), Some(0));
        assert_eq!(h.go_back(0), None);
        assert_eq!(h.go_forward(0), Some(3));
        h.push(3, 9);
        assert_eq!(h.go_forward(9), None);
        assert_eq!(h.go_back(9), Some(3));
    }

    #[test]
    fn tree_rows_respect_sort_and_min_size() {
        let root = std::env::temp_dir().join(format!("fluxcode-wt-sort-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("z_big.bin"), vec![0u8; 2000]).unwrap();
        std::fs::write(root.join("a_small.rs"), vec![0u8; 100]).unwrap();

        let tree = scan_tree(&root, ScanOpts::default(), &Arc::new(ScanProgress::new())).unwrap();
        let by_name = flatten_visible_rows(&tree, 0, &HashSet::new(), "", WizSort::Name, 0, 50);
        assert_eq!(tree.nodes[by_name[0].id as usize].name, "a_small.rs");
        let filtered = flatten_visible_rows(&tree, 0, &HashSet::new(), "", WizSort::Size, 500, 50);
        assert_eq!(filtered.len(), 1);
        assert_eq!(tree.nodes[filtered[0].id as usize].name, "z_big.bin");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn squarified_treemap_tiles_stay_inside_bounds() {
        let root = std::env::temp_dir().join(format!("fluxcode-wt-map-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("big")).unwrap();
        std::fs::write(root.join("big").join("a.bin"), vec![1u8; 5000]).unwrap();
        std::fs::write(root.join("big").join("b.rs"), vec![2u8; 2500]).unwrap();
        std::fs::write(root.join("small.json"), vec![3u8; 1000]).unwrap();
        let tree = scan_tree(&root, ScanOpts::default(), &Arc::new(ScanProgress::new())).unwrap();
        let tiles = layout_treemap(&tree, 0, 600., 220.);
        assert!(!tiles.is_empty());
        for t in &tiles {
            assert!(t.x.is_finite() && t.y.is_finite() && t.w > 0. && t.h > 0.);
            assert!(t.x >= -0.5 && t.y >= -0.5);
            assert!(t.x + t.w <= 600.5 && t.y + t.h <= 220.5);
        }
        assert!(hit_test_tile(&tiles, 10., 10.).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn format_bytes_and_counts_are_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_count(1234567), "1,234,567");
    }

    #[test]
    fn ai_summary_captures_hierarchy_hotspots_and_lightweight_items() {
        let root = std::env::temp_dir().join(format!("fluxcode-wt-ai-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("node_modules").join("bigpkg")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("logs")).unwrap();
        std::fs::write(root.join("node_modules").join("bigpkg").join("bundle.js"), vec![b'a'; 80_000]).unwrap();
        std::fs::write(root.join("logs").join("debug.log"), vec![b'b'; 12_000]).unwrap();
        std::fs::write(root.join("src").join("main.rs"), vec![b'c'; 600]).unwrap();
        std::fs::write(root.join("README.md"), vec![b'd'; 120]).unwrap();

        let tree = scan_tree(&root, ScanOpts::default(), &Arc::new(ScanProgress::new())).unwrap();
        let summary = tree.ai_summary(0, 4000);
        assert!(summary.contains("WizTree disk map"), "{summary}");
        assert!(summary.contains("node_modules/"), "{summary}");
        assert!(summary.contains("build/cache folder"), "{summary}");
        assert!(summary.contains("logs/debug.log"), "{summary}");
        assert!(summary.contains("Smallest / lightweight:"), "{summary}");
        assert!(summary.contains("Largest files:"), "{summary}");
        assert!(!summary.contains('\u{2014}'), "no em dash in AI summary");

        let tight = tree.ai_summary(0, 200);
        assert!(tight.len() <= 200, "len {}", tight.len());

        let nm_id = tree.nodes[0].children.iter().copied().find(|&id| tree.nodes[id as usize].name == "node_modules").unwrap();
        let focused = tree.ai_summary(nm_id, 4000);
        assert!(focused.contains("focused subfolder"), "{focused}");
        assert!(focused.contains("bigpkg/"), "{focused}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
