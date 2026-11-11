//! What the agent is told about the workspace before the first message (and
//! again when it changes): its folders, what kind of project each is (from the
//! manifests in it), the languages and layout, the git state and the open tabs.
//! Built from the explorer's file list and a few small files, in well under a
//! millisecond, so the agent starts with the picture a person gets at a glance
//! instead of spending tool calls on `ls` and `cat package.json`.

use crate::scm::RepoStatus;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Cap on the overview, so it never crowds out the question.
const MAX_CHARS: usize = 14000;
/// Of that, the file map (about 2,000 tokens): every file of a small project, the
/// first ones of each folder in a large one.
const MAP_CHARS: usize = 7000;
/// Folders whose files are never worth listing.
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build", ".venv", "venv", "__pycache__", ".next", ".cache", "vendor", ".idea", ".vscode", "out", "coverage"];

fn read_small(p: &Path) -> Option<String> {
    let meta = std::fs::metadata(p).ok()?;
    if meta.len() > 256 * 1024 {
        return None;
    }
    std::fs::read_to_string(p).ok()
}

fn first_n(items: impl IntoIterator<Item = String>, n: usize) -> String {
    let v: Vec<String> = items.into_iter().collect();
    let more = v.len().saturating_sub(n);
    let mut s = v.into_iter().take(n).collect::<Vec<_>>().join(", ");
    if more > 0 {
        let _ = write!(s, " (+{more})");
    }
    s
}

/// Keys of a `[section]` in a TOML file (rough: one `key = ...` per line).
fn toml_keys(text: &str, section: &str) -> Vec<String> {
    let mut on = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            on = t == format!("[{section}]");
            continue;
        }
        if on {
            if let Some((k, _)) = t.split_once('=') {
                let k = k.trim().trim_matches('"');
                if !k.is_empty() && !k.starts_with('#') {
                    out.push(k.to_string());
                }
            }
        }
    }
    out
}

fn toml_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut on = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            on = t == format!("[{section}]");
            continue;
        }
        if on {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

/// Package name from a requirements / dependency spec line.
fn req_name(line: &str) -> Option<String> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') || t.starts_with('-') {
        return None;
    }
    let end = t.find(|c: char| "=<>~!;[ @".contains(c)).unwrap_or(t.len());
    Some(t[..end].to_string()).filter(|s| !s.is_empty())
}

/// One line per thing the folder is, read from its manifests.
pub fn project_kinds(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let has = |n: &str| root.join(n).exists();
    if let Some(t) = read_small(&root.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            let name = v["name"].as_str().unwrap_or("(unnamed)");
            let keys = |k: &str| v[k].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
            let mut line = format!("Node.js package \"{name}\"");
            let scripts = keys("scripts");
            if !scripts.is_empty() {
                let _ = write!(line, "; npm scripts: {}", first_n(scripts, 10));
            }
            let mut deps = keys("dependencies");
            deps.extend(keys("devDependencies"));
            if !deps.is_empty() {
                let _ = write!(line, "; dependencies: {}", first_n(deps, 14));
            }
            out.push(line);
        }
    }
    for (file, what) in [
        ("tsconfig.json", "TypeScript (tsconfig.json)"),
        ("vite.config.ts", "Vite"),
        ("vite.config.js", "Vite"),
        ("next.config.js", "Next.js"),
        ("next.config.mjs", "Next.js"),
        ("next.config.ts", "Next.js"),
        ("nuxt.config.ts", "Nuxt"),
        ("svelte.config.js", "SvelteKit"),
        ("astro.config.mjs", "Astro"),
        ("tailwind.config.js", "Tailwind CSS"),
        ("tailwind.config.ts", "Tailwind CSS"),
        ("manage.py", "Django (manage.py)"),
        ("Dockerfile", "Docker (Dockerfile)"),
        ("docker-compose.yml", "Docker Compose"),
        ("compose.yaml", "Docker Compose"),
        ("tauri.conf.json", "Tauri"),
        ("pubspec.yaml", "Flutter / Dart (pubspec.yaml)"),
        ("build.gradle", "Gradle"),
        ("build.gradle.kts", "Gradle (Kotlin)"),
        ("CMakeLists.txt", "CMake"),
        ("Gemfile", "Ruby (Gemfile)"),
        ("mix.exs", "Elixir (mix.exs)"),
        ("deno.json", "Deno"),
        ("bun.lockb", "Bun"),
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "Yarn"),
    ] {
        if has(file) && !out.iter().any(|l| l == what) {
            out.push(what.to_string());
        }
    }
    if let Some(t) = read_small(&root.join("Cargo.toml")) {
        let name = toml_value(&t, "package", "name").unwrap_or_else(|| "workspace".into());
        let deps = toml_keys(&t, "dependencies");
        let mut line = format!("Rust crate \"{name}\"");
        if !deps.is_empty() {
            let _ = write!(line, "; dependencies: {}", first_n(deps, 14));
        }
        out.push(line);
    }
    if let Some(t) = read_small(&root.join("pyproject.toml")) {
        let name = toml_value(&t, "project", "name").or_else(|| toml_value(&t, "tool.poetry", "name")).unwrap_or_else(|| "(unnamed)".into());
        let mut deps: Vec<String> = Vec::new();
        if let Some(i) = t.find("dependencies = [") {
            let rest = &t[i + 16..];
            let body = &rest[..rest.find(']').unwrap_or(rest.len())];
            deps = body.split(',').filter_map(|d| req_name(d.trim().trim_matches(['"', '\'', '\n', ' ']))).collect();
        }
        deps.extend(toml_keys(&t, "tool.poetry.dependencies").into_iter().filter(|d| d != "python"));
        let mut line = format!("Python project \"{name}\" (pyproject.toml)");
        if !deps.is_empty() {
            let _ = write!(line, "; dependencies: {}", first_n(deps, 14));
        }
        out.push(line);
    }
    if let Some(t) = read_small(&root.join("requirements.txt")) {
        let deps: Vec<String> = t.lines().filter_map(req_name).collect();
        out.push(format!("Python requirements.txt: {}", first_n(deps, 16)));
    }
    if let Some(t) = read_small(&root.join("go.mod")) {
        let module = t.lines().find_map(|l| l.strip_prefix("module ")).unwrap_or("?").trim();
        out.push(format!("Go module {module}"));
    }
    if let Some(t) = read_small(&root.join("composer.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            let req = v["require"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
            out.push(format!("PHP Composer package {}; requires: {}", v["name"].as_str().unwrap_or("(unnamed)"), first_n(req, 12)));
        }
    }
    if let Some(t) = read_small(&root.join("pom.xml")) {
        let id = t.split("<artifactId>").nth(1).and_then(|s| s.split('<').next()).unwrap_or("?");
        out.push(format!("Maven project {id}"));
    }
    if let Some(t) = read_small(&root.join("plugin.yml")).or_else(|| read_small(&root.join("src/main/resources/plugin.yml"))) {
        let name = t.lines().find_map(|l| l.strip_prefix("name:")).unwrap_or("?").trim();
        out.push(format!("Minecraft (Bukkit/Paper) plugin {name}"));
    }
    if has("server.properties") {
        out.push("Minecraft server folder (server.properties)".into());
    }
    if let Some(t) = read_small(&root.join("Makefile")) {
        let targets: Vec<String> = t
            .lines()
            .filter_map(|l| l.split_once(':').map(|(a, _)| a))
            .filter(|a| !a.is_empty() && !a.starts_with(['\t', '.', '#', ' ']) && !a.contains(['=', '$', ' ']))
            .map(String::from)
            .collect();
        if !targets.is_empty() {
            out.push(format!("Makefile targets: {}", first_n(targets, 10)));
        }
    }
    if let Ok(rd) = std::fs::read_dir(root.join(".github").join("workflows")) {
        let wf: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        if !wf.is_empty() {
            out.push(format!("GitHub Actions workflows: {}", first_n(wf, 8)));
        }
    }
    for v in [".venv", "venv", "env"] {
        if root.join(v).join("pyvenv.cfg").is_file() {
            out.push(format!("Python virtual environment in {v}/"));
            break;
        }
    }
    out
}

/// Title and first paragraph of the README, if there is one.
fn readme(root: &Path) -> Option<String> {
    let t = ["README.md", "readme.md", "README.MD", "README.txt", "README"].iter().find_map(|n| read_small(&root.join(n)))?;
    let mut out = String::new();
    for line in t.lines().map(str::trim) {
        if line.is_empty() {
            if out.len() > 60 {
                break;
            }
            continue;
        }
        if line.starts_with("<") || line.starts_with("[![") || line.starts_with("![") {
            continue;
        }
        out.push_str(line.trim_start_matches('#').trim());
        out.push(' ');
        if out.len() > 360 {
            break;
        }
    }
    let out = out.trim().to_string();
    (!out.is_empty()).then(|| out.chars().take(400).collect())
}

/// Folders and files at the top of `root`, with how many files each folder holds.
fn layout(root: &Path, files: &[String]) -> String {
    let rs = root.to_string_lossy().to_string();
    let mut dirs: BTreeMap<String, usize> = BTreeMap::new();
    let mut top: Vec<String> = Vec::new();
    for f in files {
        let Some(rest) = f.strip_prefix(&rs) else { continue };
        let rest = rest.trim_start_matches(['\\', '/']);
        match rest.split_once(['\\', '/']) {
            Some((d, _)) => *dirs.entry(d.to_string()).or_default() += 1,
            None if !rest.is_empty() => top.push(rest.to_string()),
            None => {}
        }
    }
    let mut out = String::new();
    let shown: Vec<String> = dirs
        .iter()
        .filter(|(d, _)| !d.starts_with('.') || d.as_str() == ".github")
        .map(|(d, n)| format!("{d}/ ({n})"))
        .collect();
    if !shown.is_empty() {
        let _ = writeln!(out, "  folders: {}", first_n(shown, 24));
    }
    if !top.is_empty() {
        let _ = writeln!(out, "  files: {}", first_n(top, 24));
    }
    out
}

/// Every file under `root`, one line per folder: `src/agent/: activity.rs, menu.rs`.
/// With it the agent opens the right file at once instead of spending its first turns
/// listing folders. Over the budget, each folder keeps its first files and says how
/// many more it has.
fn file_map(root: &Path, files: &[String], budget: usize) -> String {
    let rs = root.to_string_lossy().to_string();
    let mut by_dir: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for f in files {
        let Some(rest) = f.strip_prefix(&rs) else { continue };
        let rest = rest.trim_start_matches(['\\', '/']);
        if rest.is_empty() {
            continue;
        }
        let (dir, name) = match rest.rfind(['\\', '/']) {
            Some(k) => (&rest[..k], &rest[k + 1..]),
            None => ("", rest),
        };
        if dir.split(['\\', '/']).any(|part| SKIP_DIRS.contains(&part) || (part.starts_with('.') && part != ".github")) {
            continue;
        }
        by_dir.entry(dir.replace('\\', "/")).or_default().push(name);
    }
    if by_dir.is_empty() {
        return String::new();
    }
    let total: usize = by_dir.values().map(Vec::len).sum();
    // Files per folder: all of them if they fit, else as many as the budget allows.
    let mut per = usize::MAX;
    loop {
        let size: usize = by_dir.iter().map(|(d, v)| d.len() + 4 + v.iter().take(per).map(|n| n.len() + 2).sum::<usize>() + 10).sum();
        if size <= budget || per <= 3 {
            break;
        }
        per = if per == usize::MAX { by_dir.values().map(Vec::len).max().unwrap_or(1) / 2 } else { per * 2 / 3 };
    }
    let mut out = format!("  file map ({total} files; open what you need directly, no need to list folders):\n");
    for (d, mut names) in by_dir {
        if out.len() > budget {
            out.push_str("    … (more folders: use your search tools)\n");
            break;
        }
        names.sort_unstable();
        let shown = first_n(names.iter().map(|n| n.to_string()), per.min(names.len()).max(1));
        let label = if d.is_empty() { "./".to_string() } else { format!("{d}/") };
        let _ = writeln!(out, "    {label}: {shown}");
    }
    out
}

/// Counts of files per language (Plain Text left out), largest first.
fn languages(files: &[String]) -> String {
    let mut by: BTreeMap<&'static str, usize> = BTreeMap::new();
    for f in files {
        let (_, label) = crate::files::language_for(Path::new(f), "");
        if label != "Plain Text" {
            *by.entry(label).or_default() += 1;
        }
    }
    let mut v: Vec<(&str, usize)> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    first_n(v.into_iter().map(|(l, n)| format!("{l} {n}")), 8)
}

pub struct Inputs<'a> {
    pub roots: &'a [PathBuf],
    pub files: &'a [String],
    pub git: Option<&'a RepoStatus>,
    /// Open tabs, relative, the active one first with its cursor line.
    pub tabs: Vec<String>,
    pub python: Option<&'a str>,
}

pub fn overview(i: &Inputs) -> String {
    let mut s = String::from(
        "Workspace overview from FluxCode, the editor the user is working in. Use it to understand what they mean; \
         read files with your tools when you need the details.\n",
    );
    for (n, root) in i.roots.iter().enumerate() {
        let name = root.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
        let role = if n == 0 { "main folder, your working directory" } else { "added folder" };
        let _ = writeln!(s, "\nFolder {name} ({role}): {}", root.display());
        for k in project_kinds(root) {
            let _ = writeln!(s, "  - {k}");
        }
        if let Some(r) = readme(root) {
            let _ = writeln!(s, "  README: {r}");
        }
        s.push_str(&layout(root, i.files));
        s.push_str(&file_map(root, i.files, MAP_CHARS / i.roots.len().max(1)));
    }
    let _ = writeln!(s, "\nFiles: {} in total; by language: {}", i.files.len(), languages(i.files));
    if let Some(py) = i.python {
        let _ = writeln!(s, "Python used by Run (F5): {py}");
    }
    if let Some(g) = i.git {
        let branch = g.branch.as_deref().unwrap_or("(detached)");
        let mut line = format!("Git: branch {branch}");
        if let Some(up) = &g.upstream {
            let _ = write!(line, ", tracking {up} ({} ahead, {} behind)", g.ahead, g.behind);
        }
        let changed: Vec<String> = g.conflicts.iter().chain(&g.staged).chain(&g.changes).map(|c| format!("{} {}", c.code, c.path)).collect();
        if changed.is_empty() {
            line.push_str("; no uncommitted changes");
        } else {
            let _ = write!(line, "; uncommitted: {}", first_n(changed, 15));
        }
        let _ = writeln!(s, "{line}");
    }
    if !i.tabs.is_empty() {
        let _ = writeln!(s, "Open in the editor: {}", first_n(i.tabs.clone(), 12));
    }
    if s.len() > MAX_CHARS {
        let cut = (0..=MAX_CHARS).rev().find(|&c| s.is_char_boundary(c)).unwrap_or(0);
        s.truncate(cut);
        s.push_str("\n(overview cut short)\n");
    }
    s
}

/// Changes when the workspace does: folders, file count, branch, number of changes
/// (not the tabs: the open file goes with each message anyway).
pub fn signature(i: &Inputs) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    i.roots.hash(&mut h);
    i.files.len().hash(&mut h);
    if let Some(g) = i.git {
        g.branch.hash(&mut h);
        g.count().hash(&mut h);
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::{overview, project_kinds, req_name, toml_keys, Inputs};
    use std::path::PathBuf;

    #[test]
    fn manifests() {
        let dir = std::env::temp_dir().join(format!("fx-ws-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("package.json"), r#"{"name":"kopi","scripts":{"dev":"vite","build":"vite build"},"dependencies":{"react":"18"}}"#).unwrap();
        std::fs::write(dir.join("requirements.txt"), "flask==3.0\n# comment\nrequests>=2\n").unwrap();
        std::fs::write(dir.join("README.md"), "# Kopi\n\nAplikasi kasir kopi.\n").unwrap();
        let kinds = project_kinds(&dir);
        assert!(kinds[0].starts_with("Node.js package \"kopi\"; npm scripts: dev, build; dependencies: react"), "{kinds:?}");
        assert!(kinds.iter().any(|k| k == "Python requirements.txt: flask, requests"), "{kinds:?}");
        let files = vec![dir.join("src").join("app.py").to_string_lossy().into_owned(), dir.join("package.json").to_string_lossy().into_owned()];
        let roots = vec![dir.clone()];
        let o = overview(&Inputs { roots: &roots, files: &files, git: None, tabs: vec!["src\\app.py (active, line 3)".into()], python: None });
        assert!(o.contains("README: Kopi Aplikasi kasir kopi."), "{o}");
        assert!(o.contains("folders: src/ (1)"), "{o}");
        assert!(o.contains("    src/: app.py\n"), "{o}");
        assert!(o.contains("    ./: package.json\n"), "{o}");
        assert!(o.contains("by language: JSON 1, Python 1") || o.contains("by language: Python 1, JSON 1"), "{o}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = PathBuf::new();
    }

    #[test]
    fn small_parsers() {
        assert_eq!(req_name("Django>=4 ; python_version > '3'"), Some("Django".into()));
        assert_eq!(req_name("-r base.txt"), None);
        assert_eq!(toml_keys("[package]\nname = \"a\"\n[dependencies]\nserde = \"1\"\ngpui = { version = \"0.9\" }\n", "dependencies"), vec!["serde", "gpui"]);
    }
}
