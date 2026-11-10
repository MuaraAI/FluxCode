//! Extensions: what FluxCode can build, run, pack and preview, as small manifests.
//!
//! An extension is one JSON file (`extension.json`): which projects it is for (a file
//! that must exist, maybe with some text in it), the tasks it adds (a command line, or
//! a native step written in Rust for what a shell line can't do, like packing a
//! Bedrock add-on), the tools it needs (with a winget id to install them), and notes
//! that tell an AI agent how such a project works. There is no extension runtime: a
//! manifest is data, so installing one costs a few KB and nothing runs until you
//! click a task.
//!
//! The catalog ships inside FluxCode (`extensions/*.json`, compiled in). More can be
//! installed from a file into `~/.fluxcode/extensions/<id>/extension.json`. Installed
//! ids are kept in the settings.

pub mod bedrock;
pub mod compile;
pub mod java;
pub mod view;
pub mod zip;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The built-in catalog, in the order the gallery shows it.
const BUILTIN: &[&str] = &[
    include_str!("../../extensions/html-preview.json"),
    include_str!("../../extensions/paper.json"),
    include_str!("../../extensions/bedrock.json"),
    include_str!("../../extensions/java-gradle.json"),
    include_str!("../../extensions/java-maven.json"),
    include_str!("../../extensions/node.json"),
    include_str!("../../extensions/rust.json"),
    include_str!("../../extensions/python.json"),
    include_str!("../../extensions/go.json"),
    include_str!("../../extensions/cmake.json"),
    include_str!("../../extensions/dotnet.json"),
];

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Rule {
    /// A file name or the end of a path (`src/main/resources/plugin.yml`); the last
    /// part may be `*.ext`.
    pub file: String,
    /// Text the file must contain (all of it).
    #[serde(default)]
    pub contains: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct TaskDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: String,
    /// The command line, run in the project folder. `{gradle}`, `{mvn}`, `{python}`,
    /// `{npm}` and `{name}` are filled in.
    #[serde(default)]
    pub run: String,
    /// A step FluxCode does itself (`paper.deploy`, `bedrock.pack`, ...).
    #[serde(default)]
    pub native: String,
    /// Shown only when one of these files is in the project folder (`a|b|*.csproj`).
    #[serde(default)]
    pub when: String,
    /// Hidden when one of these is there.
    #[serde(default)]
    pub unless: String,
    /// Shown only when the build file (build.gradle.kts, build.gradle, pom.xml or
    /// package.json) has this text, like a plugin id.
    #[serde(default)]
    pub uses: String,
    /// A native step to run after the command succeeds.
    #[serde(default)]
    pub then: String,
    /// Run with the JDK the project asks for (JAVA_HOME).
    #[serde(default)]
    pub java: bool,
    /// The project's main button.
    #[serde(default)]
    pub primary: bool,
    /// Runs in the Terminal (it keeps running and reads what you type).
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Need {
    /// The program looked up on PATH (`java`, `node`, `cargo`).
    pub cmd: String,
    pub label: String,
    /// `winget install -e --id <this>` installs it on Windows.
    #[serde(default)]
    pub winget: String,
    /// Where to get it otherwise.
    #[serde(default)]
    pub url: String,
    /// Only needed when the project has one of these files.
    #[serde(default)]
    pub when: String,
    /// Not needed when the project has one of these (a wrapper script, say).
    #[serde(default)]
    pub unless: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub publisher: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub icon: String,
    pub summary: String,
    /// Markdown for the details page.
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub detect: Vec<Rule>,
    /// Marker files of a project's top folder, most important first: the first
    /// one found going up from the matched file decides.
    #[serde(default)]
    pub root: Vec<String>,
    /// Packs found side by side become one project (a Bedrock add-on).
    #[serde(default)]
    pub group_siblings: bool,
    /// Extensions this one stands in for on the same folder (Paper over plain Gradle).
    #[serde(default)]
    pub replaces: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<TaskDef>,
    #[serde(default)]
    pub needs: Vec<Need>,
    /// New-project templates FluxCode can write (`paper`, `bedrock`).
    #[serde(default)]
    pub templates: Vec<String>,
    /// What an AI agent should know about such a project.
    #[serde(default)]
    pub agent: String,
    /// Installed on a fresh setup.
    #[serde(default)]
    pub default: bool,
    /// Where an installed-from-file manifest lives (None: built in).
    #[serde(skip)]
    pub source: Option<PathBuf>,
}

impl Manifest {
    pub fn builtin(&self) -> bool {
        self.source.is_none()
    }
}

pub fn parse(text: &str) -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if m.id.is_empty() || !m.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
        return Err(format!("\"id\" must be letters, digits, '-' or '.', got {:?}", m.id));
    }
    if m.name.trim().is_empty() {
        return Err("\"name\" is empty".into());
    }
    Ok(m)
}

/// The user's home folder.
pub fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default()
}

/// `~/.fluxcode/extensions`.
pub fn user_dir() -> PathBuf {
    home().join(".fluxcode").join("extensions")
}

/// The built-in manifests, then the ones installed from files.
pub fn catalog() -> Vec<Manifest> {
    let mut all: Vec<Manifest> = BUILTIN.iter().map(|t| parse(t).expect("built-in extension manifest")).collect();
    if let Ok(rd) = std::fs::read_dir(user_dir()) {
        for e in rd.flatten() {
            let p = e.path().join("extension.json");
            if let Some(mut m) = std::fs::read_to_string(&p).ok().and_then(|t| parse(&t).ok()) {
                if all.iter().any(|b| b.id == m.id) {
                    continue;
                }
                m.source = Some(p);
                all.push(m);
            }
        }
    }
    all
}

/// Copy a manifest file into the extensions folder; returns its id.
pub fn install_file(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let m = parse(&text)?;
    if BUILTIN.iter().any(|t| parse(t).map(|b| b.id == m.id).unwrap_or(false)) {
        return Err(format!("{} is built in already", m.id));
    }
    let dir = user_dir().join(&m.id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("extension.json"), text).map_err(|e| e.to_string())?;
    Ok(m.id)
}

pub fn remove_file(m: &Manifest) {
    if let Some(dir) = m.source.as_ref().and_then(|p| p.parent()) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ------------------------------------------------------------------ detection

/// A folder an extension is for.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub ext: String,
    pub dir: PathBuf,
    pub name: String,
}

/// Build output and dependency folders: never a project's own files.
const SKIP: &[&str] = &["node_modules", "build", "target", ".gradle", ".git", "dist", "out", "bin", "obj", ".venv", "venv", "__pycache__", "libraries", "cache"];

fn skipped(path: &str) -> bool {
    path.split(['/', '\\']).any(|seg| SKIP.iter().any(|s| seg.eq_ignore_ascii_case(s)))
}

/// Does `path` (any separators) end with `pattern` (`/`, last part may be `*.ext`)?
pub fn matches(path: &str, pattern: &str) -> bool {
    let segs: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    let pat: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    if pat.is_empty() || pat.len() > segs.len() {
        return false;
    }
    let tail = &segs[segs.len() - pat.len()..];
    tail.iter().zip(&pat).all(|(s, p)| match p.strip_prefix('*') {
        Some(suffix) => s.to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase()),
        None => s.eq_ignore_ascii_case(p),
    })
}

/// Files in `dir` matching one of `a|b|*.x`.
pub fn has_any(dir: &Path, alts: &str) -> bool {
    alts.split('|').map(str::trim).filter(|a| !a.is_empty()).any(|a| match a.strip_prefix('*') {
        Some(suffix) => std::fs::read_dir(dir)
            .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase())))
            .unwrap_or(false),
        None => dir.join(a).exists(),
    })
}

fn rule_hits(rule: &Rule, path: &str) -> bool {
    if !matches(path, &rule.file) {
        return false;
    }
    if rule.contains.is_empty() {
        return true;
    }
    let Ok(meta) = std::fs::metadata(path) else { return false };
    if meta.len() > 512 * 1024 {
        return false;
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    rule.contains.iter().all(|c| text.contains(c.as_str()))
}

/// The project folder for a matched file: the first root marker found going up
/// (inside `top`), else the file's own folder.
fn project_dir(m: &Manifest, file: &Path, top: &Path) -> PathBuf {
    let start = file.parent().unwrap_or(file);
    for marker in &m.root {
        let mut d = Some(start);
        while let Some(dir) = d {
            if has_any(dir, marker) {
                return dir.to_path_buf();
            }
            if dir == top {
                break;
            }
            d = dir.parent().filter(|p| p.starts_with(top));
        }
    }
    start.to_path_buf()
}

/// Every project each extension is for, from the workspace's file list. Blocking:
/// it reads the few files whose rule needs their text.
pub fn detect(catalog: &[Manifest], roots: &[PathBuf], files: &[String]) -> Vec<Project> {
    let mut found: Vec<Project> = Vec::new();
    for m in catalog {
        let mut mine: Vec<Project> = Vec::new();
        for f in files {
            if skipped(f) || !m.detect.iter().any(|r| rule_hits(r, f)) {
                continue;
            }
            let file = Path::new(f);
            let Some(top) = roots.iter().find(|r| file.starts_with(r)) else { continue };
            let dir = project_dir(m, file, top);
            if !mine.iter().any(|p| p.dir == dir) {
                let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.display().to_string());
                mine.push(Project { ext: m.id.clone(), dir, name });
            }
            if mine.len() >= 24 {
                break;
            }
        }
        if m.group_siblings {
            mine = group(mine);
        }
        if m.id == "paper" {
            for p in &mut mine {
                if let Some(n) = compile::plugin_name(&p.dir) {
                    p.name = n;
                }
            }
        }
        found.extend(mine);
    }
    // Paper over plain Gradle on the same folder, and so on.
    let shadowed: Vec<(String, PathBuf)> = found
        .iter()
        .flat_map(|p| catalog.iter().find(|m| m.id == p.ext).map(|m| m.replaces.iter().map(|r| (r.clone(), p.dir.clone())).collect::<Vec<_>>()).unwrap_or_default())
        .collect();
    found.retain(|p| !shadowed.iter().any(|(id, dir)| *id == p.ext && *dir == p.dir));
    found
}

/// Packs in the same folder become one project named after that folder.
fn group(list: Vec<Project>) -> Vec<Project> {
    let mut out: Vec<Project> = Vec::new();
    for p in &list {
        let parent = p.dir.parent().map(Path::to_path_buf);
        let siblings = list.iter().filter(|q| q.dir.parent().map(Path::to_path_buf) == parent).count();
        let (dir, name) = match parent {
            Some(par) if siblings > 1 => (par.clone(), par.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            _ => (p.dir.clone(), p.name.clone()),
        };
        if !out.iter().any(|o| o.dir == dir) {
            out.push(Project { ext: p.ext.clone(), dir, name });
        }
    }
    out
}

// ------------------------------------------------------------------ tasks

/// A task ready to run for one project.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub label: String,
    pub icon: String,
    /// The command line, filled in (empty for a native step).
    pub line: String,
    pub native: String,
    pub then: String,
    pub java: bool,
    pub primary: bool,
    pub terminal: bool,
}

fn wrapper(dir: &Path, name: &str, fallback: &str) -> String {
    if dir.join(name).is_file() || dir.join(format!("{name}.bat")).is_file() || dir.join(format!("{name}.cmd")).is_file() {
        if cfg!(windows) {
            let bat = if dir.join(format!("{name}.cmd")).is_file() { format!("{name}.cmd") } else { format!("{name}.bat") };
            format!(".\\{bat}")
        } else {
            format!("./{name}")
        }
    } else {
        fallback.to_string()
    }
}

/// npm, pnpm, yarn or bun, from the lock file.
pub fn package_manager(dir: &Path) -> &'static str {
    if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

fn fill(line: &str, p: &Project) -> String {
    line.replace("{fluxcompile}", &compile::gradle_flags(&p.dir))
        .replace("{gradle}", &wrapper(&p.dir, "gradlew", "gradle"))
        .replace("{mvn}", &wrapper(&p.dir, "mvnw", "mvn"))
        .replace("{python}", &crate::proc::python(Some(&p.dir)))
        .replace("{npm}", package_manager(&p.dir))
        .replace("{name}", &p.name)
}

/// A build file in `dir` (or a folder right inside it) mentions `text`.
pub fn uses(dir: &Path, text: &str) -> bool {
    let files = ["build.gradle.kts", "build.gradle", "pom.xml", "package.json", "settings.gradle.kts", "gradle/libs.versions.toml"];
    let mut dirs = vec![dir.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(dir) {
        dirs.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !skipped(&p.to_string_lossy())));
    }
    dirs.iter().any(|d| files.iter().any(|f| std::fs::read_to_string(d.join(f)).is_ok_and(|t| t.contains(text))))
}

/// `package.json` scripts, in file order.
pub fn npm_scripts(dir: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    v.get("scripts").and_then(|s| s.as_object()).map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

/// The tasks a project gets from its extension (checks files on disk: off the UI thread
/// when there are many).
pub fn tasks(m: &Manifest, p: &Project) -> Vec<Task> {
    let mut out = Vec::new();
    for t in &m.tasks {
        if !t.when.is_empty() && !has_any(&p.dir, &t.when) {
            continue;
        }
        if !t.unless.is_empty() && has_any(&p.dir, &t.unless) {
            continue;
        }
        if !t.uses.is_empty() && !uses(&p.dir, &t.uses) {
            continue;
        }
        if t.native == "npm.scripts" {
            let pm = package_manager(&p.dir);
            for s in npm_scripts(&p.dir) {
                let primary = s == "dev" || s == "start";
                out.push(Task {
                    id: format!("npm:{s}"),
                    label: format!("{pm} run {s}"),
                    icon: if primary { "play".into() } else if s.contains("build") { "hammer".into() } else if s.contains("test") { "flask-conical".into() } else { "terminal".into() },
                    line: format!("{pm} run {s}"),
                    native: String::new(),
                    then: String::new(),
                    java: false,
                    primary,
                    terminal: primary || s.contains("watch") || s == "serve" || s == "preview",
                });
            }
            continue;
        }
        out.push(Task {
            id: t.id.clone(),
            label: t.label.clone(),
            icon: t.icon.clone(),
            line: if t.run.is_empty() { String::new() } else { fill(&t.run, p) },
            native: t.native.clone(),
            then: t.then.clone(),
            java: t.java,
            primary: t.primary,
            terminal: t.terminal,
        });
    }
    out
}

// ------------------------------------------------------------------ tools

/// A program on PATH (with PATHEXT on Windows), without starting a process.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into()).split(';').map(|e| e.to_ascii_lowercase()).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for e in &exts {
            let p = dir.join(format!("{cmd}{e}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Where a needed tool is, or why it isn't there.
#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Found(String),
    Missing,
}

/// Look up an extension's tools (for the projects it is for, when it has any).
pub fn check_needs(m: &Manifest, projects: &[&Project]) -> Vec<(Need, Tool)> {
    m.needs
        .iter()
        .filter(|n| {
            projects.is_empty()
                || projects.iter().any(|p| (n.when.is_empty() || has_any(&p.dir, &n.when)) && (n.unless.is_empty() || !has_any(&p.dir, &n.unless)))
        })
        .map(|n| {
            let mut need = n.clone();
            let found = if n.cmd == "java" {
                // The version the folder's projects ask for, when one does: that JDK, and its installer.
                match projects.iter().find_map(|p| java::wanted(&p.dir).map(|w| (w, *p))) {
                    Some((w, p)) => {
                        need.label = format!("Java {w} (JDK)");
                        need.winget = java::winget_jdk(w);
                        java::best_jdk(Some(p)).map(|j| format!("JDK {} · {}", j.major, j.home.display()))
                    }
                    None => java::best_jdk(None).map(|j| format!("JDK {} · {}", j.major, j.home.display())),
                }
            } else {
                which(&n.cmd).map(|p| p.display().to_string())
            };
            (need, found.map(Tool::Found).unwrap_or(Tool::Missing))
        })
        .collect()
}

// ------------------------------------------------------------------ agent context

/// What the agent is told about the projects installed extensions are for: which
/// tool builds each, with which command, and the extension's notes.
pub fn agent_notes(catalog: &[Manifest], installed: &[String], found: &[Project], top: &[PathBuf]) -> Option<String> {
    let mut s = String::new();
    let mut noted: Vec<&str> = Vec::new();
    for p in found {
        if !installed.contains(&p.ext) {
            continue;
        }
        let Some(m) = catalog.iter().find(|m| m.id == p.ext) else { continue };
        let rel = top.iter().find_map(|r| p.dir.strip_prefix(r).ok()).map(|r| r.display().to_string()).unwrap_or_else(|| p.dir.display().to_string());
        let rel = if rel.is_empty() { ".".to_string() } else { rel };
        s.push_str(&format!("- {} ({rel}): {}.", p.name, m.name));
        let ts = tasks(m, p);
        let cmds: Vec<String> = ts.iter().filter(|t| !t.line.is_empty()).take(4).map(|t| format!("{}: `{}`", t.label, t.line)).collect();
        if !cmds.is_empty() {
            s.push_str(&format!(" {}.", cmds.join("; ")));
        }
        match m.id.as_str() {
            "paper" | "java-gradle" | "java-maven" => s.push_str(&java::facts(p)),
            "bedrock" => s.push_str(&bedrock::facts(p)),
            _ => {}
        }
        s.push('\n');
        if !m.agent.is_empty() && !noted.contains(&m.id.as_str()) {
            noted.push(&m.id);
        }
    }
    if s.is_empty() {
        return None;
    }
    let mut out = String::from("\nProjects and how FluxCode builds them (its extensions; the user runs these from the Run view):\n");
    out.push_str(&s);
    for id in noted {
        if let Some(m) = catalog.iter().find(|m| m.id == id) {
            out.push_str(&format!("{} notes: {}\n", m.name, m.agent));
        }
    }
    Some(out)
}

/// Installed ids, the defaults on a fresh setup.
pub fn default_installed() -> Vec<String> {
    BUILTIN.iter().filter_map(|t| parse(t).ok()).filter(|m| m.default).map(|m| m.id).collect()
}

/// UI state of the Extensions view and page.
pub struct Ext {
    pub catalog: Vec<Manifest>,
    pub found: Vec<Project>,
    /// Tasks per found project (same order), worked out off the UI thread.
    pub tasks: Vec<Vec<Task>>,
    pub query: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    /// The name box of the details page's New project.
    pub new_name: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    pub filter: Filter,
    /// The details page in the editor area.
    pub page: Option<String>,
    pub needs: HashMap<String, Vec<(Need, Tool)>>,
    /// A native step to run when the running task succeeds.
    pub after: Option<(Project, String)>,
    /// The folder the last task ran in (links in its output resolve there).
    pub last_cwd: Option<PathBuf>,
    pub scanning: bool,
    pub note: Option<String>,
    /// (project, task id) running in the Output panel.
    pub running: Option<(usize, String)>,
    /// FluxCompile's folding of Gradle's task lines: where the scan is, how many were
    /// folded, and the progress line that stands for them.
    pub fold_scan: usize,
    pub fold_count: usize,
    pub fold_line: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Installed,
    Here,
    All,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_manifest_parses_and_is_unique() {
        let all: Vec<Manifest> = BUILTIN.iter().map(|t| parse(t).unwrap()).collect();
        let mut ids: Vec<&str> = all.iter().map(|m| m.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for m in &all {
            assert!(!m.summary.is_empty() && !m.tasks.is_empty(), "{}", m.id);
            assert!(!m.summary.contains('\u{2014}') && !m.about.contains('\u{2014}'), "{}", m.id);
            for t in &m.tasks {
                assert!(!t.run.is_empty() || !t.native.is_empty(), "{}:{}", m.id, t.id);
            }
        }
        assert!(default_installed().contains(&"html-preview".to_string()));
    }

    #[test]
    fn path_patterns() {
        assert!(matches("C:\\a\\proj\\src\\main\\resources\\plugin.yml", "src/main/resources/plugin.yml"));
        assert!(!matches("C:\\a\\plugin.yml", "src/main/resources/plugin.yml"));
        assert!(matches("C:\\a\\App.csproj", "*.csproj"));
        assert!(matches("/x/Cargo.toml", "cargo.toml"));
        assert!(skipped("C:\\a\\node_modules\\x\\package.json"));
        assert!(!skipped("C:\\a\\web\\package.json"));
    }

    #[test]
    fn finds_a_gradle_project_at_its_settings_file() {
        let root = std::env::temp_dir().join(format!("fx-ext-{}", std::process::id()));
        let module = root.join("proj").join("main").join("src").join("main").join("resources");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(root.join("proj").join("settings.gradle.kts"), "").unwrap();
        std::fs::write(root.join("proj").join("main").join("build.gradle.kts"), "").unwrap();
        std::fs::write(module.join("plugin.yml"), "name: X\nmain: a.X\n").unwrap();
        let files = vec![module.join("plugin.yml").to_string_lossy().into_owned(), root.join("proj").join("settings.gradle.kts").to_string_lossy().into_owned()];
        let cat = catalog();
        let found = detect(&cat, std::slice::from_ref(&root), &files);
        // Paper takes the folder; plain Gradle steps aside for it.
        let paper: Vec<&Project> = found.iter().filter(|p| p.ext == "paper").collect();
        assert_eq!(paper.len(), 1, "{found:?}");
        assert_eq!(paper[0].dir, root.join("proj"));
        assert!(!found.iter().any(|p| p.ext == "java-gradle" && p.dir == root.join("proj")));
        let t = tasks(cat.iter().find(|m| m.id == "paper").unwrap(), paper[0]);
        assert!(t.iter().any(|t| t.primary), "{t:?}");
        // The agent is told what it is and how it builds, only once it's installed.
        assert_eq!(agent_notes(&cat, &[], &found, std::slice::from_ref(&root)), None);
        let notes = agent_notes(&cat, &["paper".to_string()], &found, std::slice::from_ref(&root)).unwrap();
        assert!(notes.contains("Paper Plugins") && notes.contains("build -x test") && notes.contains("plugin.yml: name X"), "{notes}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
