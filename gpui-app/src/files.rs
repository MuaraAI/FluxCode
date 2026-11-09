//! File system helpers: workspace tree, language detection, file badges.

use gpui_kit::assets::IconName;
use gpui_kit::component::tree::TreeItem;
use std::path::Path;

/// Folders that are never worth walking for the explorer. They are still listed
/// (as folders, dimmed), just not opened.
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".next", "__pycache__", ".venv", "venv"];
/// Upper bound so a huge folder can't stall startup.
const MAX_ENTRIES: usize = 20_000;

/// Plain (Send) folder listing, built on a background thread.
pub struct Node {
    id: String,
    name: String,
    children: Option<Vec<Node>>,
}

/// Every leaf of a scan (files, and skipped folders listed without children).
pub fn flatten(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        match &n.children {
            Some(ch) => flatten(ch, out),
            None => out.push(n.id.clone()),
        }
    }
}

pub fn scan(root: &Path) -> Vec<Node> {
    let mut budget = MAX_ENTRIES;
    walk(root, &mut budget)
}

/// Turn a scan into tree items (UI thread: `TreeItem` is not Send), and collect
/// every folder's path: the tree only calls an item a folder when it has
/// children, so empty and skipped folders would otherwise draw as files.
pub fn to_items(nodes: Vec<Node>, dirs: &mut std::collections::HashSet<String>) -> Vec<TreeItem> {
    nodes
        .into_iter()
        .map(|n| match n.children {
            Some(ch) => {
                dirs.insert(n.id.clone());
                TreeItem::new(n.id, n.name).children(to_items(ch, dirs))
            }
            None => TreeItem::new(n.id, n.name),
        })
        .collect()
}

fn walk(dir: &Path, budget: &mut usize) -> Vec<Node> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<(bool, String, std::path::PathBuf)> = rd
        .flatten()
        .map(|e| {
            let p = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (is_dir, e.file_name().to_string_lossy().into_owned(), p)
        })
        .collect();
    // Folders first, then case-insensitive by name (like VS Code).
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

    let mut out = Vec::with_capacity(entries.len());
    for (is_dir, name, path) in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let id = path.to_string_lossy().into_owned();
        if is_dir {
            let children = if SKIP_DIRS.contains(&name.as_str()) { Vec::new() } else { walk(&path, budget) };
            out.push(Node { id, name, children: Some(children) });
        } else {
            out.push(Node { id, name, children: None });
        }
    }
    out
}

/// gpui-component highlighter language name for a path.
pub fn language_for(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "html" | "htm" | "vue" | "svelte" => "html",
        "css" | "scss" | "less" => "css",
        "json" | "jsonc" | "flx" => "json",
        "md" | "markdown" => "markdown",
        "py" | "pyw" => "python",
        "rs" => "rust",
        "php" => "php",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "sh" | "bash" | "zsh" => "bash",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "cpp",
        "go" => "go",
        "java" => "java",
        "lua" => "lua",
        "sql" => "sql",
        _ => "text",
    }
}

/// Display name for the status bar.
pub fn language_label(lang: &str) -> &'static str {
    match lang {
        "javascript" => "JavaScript",
        "typescript" => "TypeScript",
        "tsx" => "TypeScript JSX",
        "html" => "HTML",
        "css" => "CSS",
        "json" => "JSON",
        "markdown" => "Markdown",
        "python" => "Python",
        "rust" => "Rust",
        "php" => "PHP",
        "toml" => "TOML",
        "yaml" => "YAML",
        "bash" => "Shell",
        "c" => "C",
        "cpp" => "C++",
        "go" => "Go",
        "java" => "Java",
        "lua" => "Lua",
        "sql" => "SQL",
        _ => "Plain Text",
    }
}

/// The explorer's, tabs' and search's file icon: one Lucide outline per kind of
/// file (code, data, text, image, config, script, lock, archive, ...), tinted
/// with the language's color, so every row lines up and reads the same way.
/// `None` means the neutral text color.
pub fn icon_for(name: &str) -> (IconName, Option<u32>) {
    let lower = name.to_ascii_lowercase();
    let ext = if lower.contains('.') { lower.rsplit('.').next().unwrap_or("") } else { "" };
    match (lower.as_str(), ext) {
        ("package-lock.json" | "cargo.lock" | "yarn.lock" | "pnpm-lock.yaml" | "composer.lock", _) | (_, "lock") => (IconName::FileLock, None),
        ("license" | "licence" | "copying", _) => (IconName::FileText, Some(0xd9a74a)),
        ("dockerfile" | "makefile" | ".gitignore" | ".gitattributes" | ".editorconfig" | ".npmrc" | ".env", _) => (IconName::FileCog, None),
        (_, "js" | "mjs" | "cjs") => (IconName::FileCode, Some(0xe8c547)),
        (_, "jsx") => (IconName::FileCode, Some(0x61dafb)),
        (_, "ts" | "mts" | "cts" | "tsx") => (IconName::FileCode, Some(0x3b8eea)),
        (_, "html" | "htm" | "vue" | "svelte") => (IconName::FileCode, Some(0xe8704a)),
        (_, "css" | "scss" | "less") => (IconName::FileCode, Some(0x5aa7e8)),
        (_, "py" | "pyw" | "pyi") => (IconName::FileCode, Some(0x4f9fd8)),
        (_, "rs") => (IconName::FileCode, Some(0xdea584)),
        (_, "php") => (IconName::FileCode, Some(0x8c8fd8)),
        (_, "c" | "h" | "cpp" | "hpp" | "cc" | "cs") => (IconName::FileCode, Some(0x6c9bd2)),
        (_, "go") => (IconName::FileCode, Some(0x4fd1c5)),
        (_, "java" | "kt") => (IconName::FileCode, Some(0xe76f51)),
        (_, "lua" | "rb" | "swift" | "dart" | "sql") => (IconName::FileCode, Some(0x5a7fe8)),
        (_, "json" | "jsonc" | "json5" | "webmanifest") => (IconName::FileBraces, Some(0xd9a74a)),
        (_, "md" | "markdown" | "mdx" | "txt" | "rst" | "log") => (IconName::FileText, Some(0x8fb8ff)),
        (_, "flx") => (IconName::FileText, Some(0xc792ea)),
        (_, "toml" | "yml" | "yaml" | "ini" | "cfg" | "conf" | "env" | "xml") => (IconName::FileCog, None),
        (_, "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd") => (IconName::FileTerminal, Some(0x5fd38d)),
        (_, "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" | "bmp" | "avif") => (IconName::FileImage, Some(0xc792ea)),
        (_, "zip" | "7z" | "rar" | "gz" | "tgz" | "tar" | "whl" | "jar") => (IconName::FileArchive, None),
        (_, "mp3" | "wav" | "ogg" | "flac") => (IconName::FileMusic, None),
        (_, "mp4" | "mov" | "webm" | "mkv") => (IconName::FileVideoCamera, None),
        (_, "ttf" | "otf" | "woff" | "woff2") => (IconName::FileType, None),
        (_, "csv" | "tsv" | "xlsx") => (IconName::FileSpreadsheet, Some(0x5fd38d)),
        (_, "pem" | "key" | "crt") => (IconName::FileKey, None),
        _ => (IconName::File, None),
    }
}

/// Folder icon: `.git` gets its own, everything else the plain folder.
pub fn folder_icon(name: &str, open: bool) -> IconName {
    match name {
        ".git" => IconName::FolderGit,
        _ if open => IconName::FolderOpen,
        _ => IconName::Folder,
    }
}

// The extra Lucide icons above, embedded next to gpui-kit's default set
// (about 0.5 KiB of SVG each); see `main.rs` for the asset source.
gpui_kit::assets::icon_assets!(pub FileIcons, [
    File, FileCode, FileBraces, FileText, FileImage, FileLock, FileCog, FileTerminal, FileArchive,
    FileMusic, FileVideoCamera, FileType, FileSpreadsheet, FileKey, FolderGit, Clock,
    // Source Control
    GitBranch, GitCommitHorizontal, GitPullRequest, CloudUpload, RefreshCcw, Rocket, FileDiff, Undo2
]);
