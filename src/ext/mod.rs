pub mod bedrock;
pub mod compile;
pub mod host;
pub mod java;
pub mod market;
mod market_view;
pub mod mcbe_types;
pub mod mcbe_view;
pub mod native;
pub mod native_webview;
pub mod view;
pub mod vsc_registry;
pub mod vsc_view;
pub mod vsx_icons;
pub mod vsx_theme;
pub mod webview_bridge;
pub mod zip;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    pub file: String,
    #[serde(default)]
    pub contains: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct TaskDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub run: String,
    #[serde(default)]
    pub native: String,
    #[serde(default)]
    pub when: String,
    #[serde(default)]
    pub unless: String,
    #[serde(default)]
    pub uses: String,
    #[serde(default)]
    pub then: String,
    #[serde(default)]
    pub java: bool,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Need {
    pub cmd: String,
    pub label: String,
    #[serde(default)]
    pub winget: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub when: String,
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
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub detect: Vec<Rule>,
    #[serde(default)]
    pub root: Vec<String>,
    #[serde(default)]
    pub group_siblings: bool,
    #[serde(default)]
    pub replaces: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<TaskDef>,
    #[serde(default)]
    pub needs: Vec<Need>,
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub default: bool,
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
    if m.id.is_empty()
        || !m
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return Err(format!(
            "\"id\" must be letters, digits, '-' or '.', got {:?}",
            m.id
        ));
    }
    if m.name.trim().is_empty() {
        return Err("\"name\" is empty".into());
    }
    Ok(m)
}

pub fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn user_dir() -> PathBuf {
    home().join(".fluxcode").join("extensions")
}

pub fn catalog() -> Vec<Manifest> {
    let mut all: Vec<Manifest> = BUILTIN
        .iter()
        .map(|t| parse(t).expect("built-in extension manifest"))
        .collect();
    if let Ok(rd) = std::fs::read_dir(user_dir()) {
        for e in rd.flatten() {
            let p = e.path().join("extension.json");
            if let Some(mut m) = std::fs::read_to_string(&p)
                .ok()
                .and_then(|t| parse(&t).ok())
            {
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

pub fn remove_file(m: &Manifest) {
    if let Some(dir) = m.source.as_ref().and_then(|p| p.parent()) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub ext: String,
    pub dir: PathBuf,
    pub name: String,
}

const SKIP: &[&str] = &[
    "node_modules",
    "build",
    "target",
    ".gradle",
    ".git",
    "dist",
    "out",
    "bin",
    "obj",
    ".venv",
    "venv",
    "__pycache__",
    "libraries",
    "cache",
];

fn skipped(path: &str) -> bool {
    path.split(['/', '\\'])
        .any(|seg| SKIP.iter().any(|s| seg.eq_ignore_ascii_case(s)))
}

pub fn matches(path: &str, pattern: &str) -> bool {
    let segs: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    let pat: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    if pat.is_empty() || pat.len() > segs.len() {
        return false;
    }
    let tail = &segs[segs.len() - pat.len()..];
    tail.iter()
        .zip(&pat)
        .all(|(s, p)| match p.strip_prefix('*') {
            Some(suffix) => s
                .to_ascii_lowercase()
                .ends_with(&suffix.to_ascii_lowercase()),
            None => s.eq_ignore_ascii_case(p),
        })
}

pub fn has_any(dir: &Path, alts: &str) -> bool {
    alts.split('|')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .any(|a| match a.strip_prefix('*') {
            Some(suffix) => std::fs::read_dir(dir)
                .map(|rd| {
                    rd.flatten().any(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .ends_with(&suffix.to_ascii_lowercase())
                    })
                })
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
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() > 512 * 1024 {
        return false;
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    rule.contains.iter().all(|c| text.contains(c.as_str()))
}

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

pub fn detect(catalog: &[Manifest], roots: &[PathBuf], files: &[String]) -> Vec<Project> {
    let mut found: Vec<Project> = Vec::new();
    for m in catalog {
        let mut mine: Vec<Project> = Vec::new();
        for f in files {
            if skipped(f) || !m.detect.iter().any(|r| rule_hits(r, f)) {
                continue;
            }
            let file = Path::new(f);
            let Some(top) = roots.iter().find(|r| file.starts_with(r)) else {
                continue;
            };
            let dir = project_dir(m, file, top);
            if !mine.iter().any(|p| p.dir == dir) {
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| dir.display().to_string());
                mine.push(Project {
                    ext: m.id.clone(),
                    dir,
                    name,
                });
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
    let shadowed: Vec<(String, PathBuf)> = found
        .iter()
        .flat_map(|p| {
            catalog
                .iter()
                .find(|m| m.id == p.ext)
                .map(|m| {
                    m.replaces
                        .iter()
                        .map(|r| (r.clone(), p.dir.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();
    found.retain(|p| {
        !shadowed
            .iter()
            .any(|(id, dir)| *id == p.ext && *dir == p.dir)
    });
    found
}

fn group(list: Vec<Project>) -> Vec<Project> {
    let mut out: Vec<Project> = Vec::new();
    for p in &list {
        let parent = p.dir.parent().map(Path::to_path_buf);
        let siblings = list
            .iter()
            .filter(|q| q.dir.parent().map(Path::to_path_buf) == parent)
            .count();
        let (dir, name) = match parent {
            Some(par) if siblings > 1 => (
                par.clone(),
                par.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            _ => (p.dir.clone(), p.name.clone()),
        };
        if !out.iter().any(|o| o.dir == dir) {
            out.push(Project {
                ext: p.ext.clone(),
                dir,
                name,
            });
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub line: String,
    pub native: String,
    pub then: String,
    pub java: bool,
    pub primary: bool,
    pub terminal: bool,
}

fn wrapper(dir: &Path, name: &str, fallback: &str) -> String {
    if dir.join(name).is_file()
        || dir.join(format!("{name}.bat")).is_file()
        || dir.join(format!("{name}.cmd")).is_file()
    {
        if cfg!(windows) {
            let bat = if dir.join(format!("{name}.cmd")).is_file() {
                format!("{name}.cmd")
            } else {
                format!("{name}.bat")
            };
            format!(".\\{bat}")
        } else {
            format!("./{name}")
        }
    } else {
        fallback.to_string()
    }
}

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

pub fn uses(dir: &Path, text: &str) -> bool {
    let files = [
        "build.gradle.kts",
        "build.gradle",
        "pom.xml",
        "package.json",
        "settings.gradle.kts",
        "gradle/libs.versions.toml",
    ];
    let mut dirs = vec![dir.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(dir) {
        dirs.extend(
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir() && !skipped(&p.to_string_lossy())),
        );
    }
    dirs.iter().any(|d| {
        files
            .iter()
            .any(|f| std::fs::read_to_string(d.join(f)).is_ok_and(|t| t.contains(text)))
    })
}

pub fn npm_scripts(dir: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    v.get("scripts")
        .and_then(|s| s.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

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
                    icon: if primary {
                        "play".into()
                    } else if s.contains("build") {
                        "hammer".into()
                    } else if s.contains("test") {
                        "flask-conical".into()
                    } else {
                        "terminal".into()
                    },
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
            line: if t.run.is_empty() {
                String::new()
            } else {
                fill(&t.run, p)
            },
            native: t.native.clone(),
            then: t.then.clone(),
            java: t.java,
            primary: t.primary,
            terminal: t.terminal,
        });
    }
    out
}

pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(|e| e.to_ascii_lowercase())
            .collect()
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

#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Found(String),
    Missing,
}

pub fn check_needs(m: &Manifest, projects: &[&Project]) -> Vec<(Need, Tool)> {
    m.needs
        .iter()
        .filter(|n| {
            projects.is_empty()
                || projects.iter().any(|p| {
                    (n.when.is_empty() || has_any(&p.dir, &n.when))
                        && (n.unless.is_empty() || !has_any(&p.dir, &n.unless))
                })
        })
        .map(|n| {
            let mut need = n.clone();
            let found = if n.cmd == "java" {
                match projects
                    .iter()
                    .find_map(|p| java::wanted(&p.dir).map(|w| (w, *p)))
                {
                    Some((w, p)) => {
                        need.label = format!("Java {w} (JDK)");
                        need.winget = java::winget_jdk(w);
                        java::best_jdk(Some(p))
                            .map(|j| format!("JDK {} · {}", j.major, j.home.display()))
                    }
                    None => java::best_jdk(None)
                        .map(|j| format!("JDK {} · {}", j.major, j.home.display())),
                }
            } else {
                which(&n.cmd).map(|p| p.display().to_string())
            };
            (need, found.map(Tool::Found).unwrap_or(Tool::Missing))
        })
        .collect()
}

pub fn agent_notes(
    catalog: &[Manifest],
    installed: &[String],
    found: &[Project],
    top: &[PathBuf],
) -> Option<String> {
    let mut s = String::new();
    let mut noted: Vec<&str> = Vec::new();
    for p in found {
        if !installed.contains(&p.ext) {
            continue;
        }
        let Some(m) = catalog.iter().find(|m| m.id == p.ext) else {
            continue;
        };
        let rel = top
            .iter()
            .find_map(|r| p.dir.strip_prefix(r).ok())
            .map(|r| r.display().to_string())
            .unwrap_or_else(|| p.dir.display().to_string());
        let rel = if rel.is_empty() { ".".to_string() } else { rel };
        s.push_str(&format!("- {} ({rel}): {}.", p.name, m.name));
        let ts = tasks(m, p);
        let cmds: Vec<String> = ts
            .iter()
            .filter(|t| !t.line.is_empty())
            .take(4)
            .map(|t| format!("{}: `{}`", t.label, t.line))
            .collect();
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

pub fn default_installed() -> Vec<String> {
    BUILTIN
        .iter()
        .filter_map(|t| parse(t).ok())
        .filter(|m| m.default)
        .map(|m| m.id)
        .collect()
}

pub struct Ext {
    pub catalog: Vec<Manifest>,
    pub found: Vec<Project>,
    pub tasks: Vec<Vec<Task>>,
    pub query: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    pub new_name: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    pub filter: Filter,
    pub page: Option<String>,
    pub needs: HashMap<String, Vec<(Need, Tool)>>,
    pub after: Option<(Project, String)>,
    pub last_cwd: Option<PathBuf>,
    pub scanning: bool,
    pub note: Option<String>,
    pub running: Option<(usize, String)>,
    pub fold_scan: usize,
    pub fold_count: usize,
    pub fold_line: Option<usize>,
    pub market: market::Market,
    pub _query_sub: gpui_kit::Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Market,
    Installed,
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
            assert!(
                !m.summary.contains('\u{2014}') && !m.about.contains('\u{2014}'),
                "{}",
                m.id
            );
            for t in &m.tasks {
                assert!(
                    !t.run.is_empty() || !t.native.is_empty(),
                    "{}:{}",
                    m.id,
                    t.id
                );
            }
        }
        assert!(default_installed().contains(&"html-preview".to_string()));
    }

    #[test]
    fn path_patterns() {
        assert!(matches(
            "C:\\a\\proj\\src\\main\\resources\\plugin.yml",
            "src/main/resources/plugin.yml"
        ));
        assert!(!matches(
            "C:\\a\\plugin.yml",
            "src/main/resources/plugin.yml"
        ));
        assert!(matches("C:\\a\\App.csproj", "*.csproj"));
        assert!(matches("/x/Cargo.toml", "cargo.toml"));
        assert!(skipped("C:\\a\\node_modules\\x\\package.json"));
        assert!(!skipped("C:\\a\\web\\package.json"));
    }

    #[test]
    fn finds_a_gradle_project_at_its_settings_file() {
        let root = std::env::temp_dir().join(format!("fx-ext-{}", std::process::id()));
        let module = root
            .join("proj")
            .join("main")
            .join("src")
            .join("main")
            .join("resources");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(root.join("proj").join("settings.gradle.kts"), "").unwrap();
        std::fs::write(root.join("proj").join("main").join("build.gradle.kts"), "").unwrap();
        std::fs::write(module.join("plugin.yml"), "name: X\nmain: a.X\n").unwrap();
        let files = vec![
            module.join("plugin.yml").to_string_lossy().into_owned(),
            root.join("proj")
                .join("settings.gradle.kts")
                .to_string_lossy()
                .into_owned(),
        ];
        let cat = catalog();
        let found = detect(&cat, std::slice::from_ref(&root), &files);
        let paper: Vec<&Project> = found.iter().filter(|p| p.ext == "paper").collect();
        assert_eq!(paper.len(), 1, "{found:?}");
        assert_eq!(paper[0].dir, root.join("proj"));
        assert!(!found
            .iter()
            .any(|p| p.ext == "java-gradle" && p.dir == root.join("proj")));
        let t = tasks(cat.iter().find(|m| m.id == "paper").unwrap(), paper[0]);
        assert!(t.iter().any(|t| t.primary), "{t:?}");
        assert_eq!(
            agent_notes(&cat, &[], &found, std::slice::from_ref(&root)),
            None
        );
        let notes = agent_notes(
            &cat,
            &["paper".to_string()],
            &found,
            std::slice::from_ref(&root),
        )
        .unwrap();
        assert!(
            notes.contains("Paper Plugins")
                && notes.contains("build -x test")
                && notes.contains("plugin.yml: name X"),
            "{notes}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
