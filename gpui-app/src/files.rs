//! File system helpers: workspace tree, language detection, file badges.

use gpui_kit::component::tree::TreeItem;
use std::path::Path;

/// Folders that are never worth walking for the explorer.
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".next", "__pycache__", ".venv", "venv"];
/// Upper bound so a huge folder can't stall startup.
const MAX_ENTRIES: usize = 20_000;

/// Plain (Send) folder listing, built on a background thread.
pub struct Node {
    id: String,
    name: String,
    children: Option<Vec<Node>>,
}

pub fn scan(root: &Path) -> Vec<Node> {
    let mut budget = MAX_ENTRIES;
    walk(root, &mut budget)
}

/// Turn a scan into tree items (UI thread: `TreeItem` is not Send).
pub fn to_items(nodes: Vec<Node>) -> Vec<TreeItem> {
    nodes
        .into_iter()
        .map(|n| match n.children {
            Some(ch) => TreeItem::new(n.id, n.name).children(to_items(ch)),
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

/// Small text badge + colour for a file in the explorer and tabs
/// (cheap stand-in for FluxCode's SVG file icons).
pub fn badge(name: &str) -> (&'static str, u32) {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "js" | "mjs" | "cjs" => ("JS", 0xe8c547),
        "jsx" => ("JSX", 0x61dafb),
        "ts" | "mts" | "cts" => ("TS", 0x3b8eea),
        "tsx" => ("TSX", 0x3b8eea),
        "html" | "htm" => ("<>", 0xe8704a),
        "css" | "scss" | "less" => ("#", 0x5aa7e8),
        "json" | "jsonc" => ("{}", 0xd9a74a),
        "md" | "markdown" => ("M", 0x8fb8ff),
        "py" | "pyw" => ("PY", 0x4f9fd8),
        "rs" => ("RS", 0xdea584),
        "php" => ("PHP", 0x8c8fd8),
        "toml" | "yml" | "yaml" | "ini" | "cfg" => ("⚙", 0x949aa4),
        "sh" | "bash" | "ps1" | "bat" | "cmd" => ("$", 0x5fd38d),
        "c" | "h" | "cpp" | "hpp" | "cc" => ("C", 0x6c9bd2),
        "go" => ("GO", 0x4fd1c5),
        "java" => ("J", 0xe76f51),
        "lua" => ("LUA", 0x5a7fe8),
        "flx" => ("FX", 0xc792ea),
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" => ("IMG", 0xc792ea),
        "lock" => ("🔒", 0x949aa4),
        _ => ("·", 0x6b717c),
    }
}
