//! Java projects: the JDK a build asks for, Paper test servers, deploying a plugin,
//! the Gradle wrapper, and a new Paper plugin.
//!
//! The JDK matters: a Gradle toolchain set to 21 on a PC whose `java` is 25 makes
//! Gradle look for a 21 (or download one). FluxCode reads the version the build asks
//! for and runs it with a JDK of that version from the usual install folders, Gradle's
//! own, or a `tools/jdk-*` folder next to the project.

use super::Project;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Jdk {
    pub home: PathBuf,
    pub major: u32,
}

/// `JAVA_VERSION="21.0.5"` in a JDK's `release` file → 21 (and "1.8.0" → 8).
pub fn release_major(text: &str) -> Option<u32> {
    let line = text.lines().find(|l| l.starts_with("JAVA_VERSION="))?;
    let v = line.split('=').nth(1)?.trim().trim_matches('"');
    let mut parts = v.split(['.', '_', '+', '-']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn jdk_at(home: &Path) -> Option<Jdk> {
    let javac = home.join("bin").join(if cfg!(windows) { "javac.exe" } else { "javac" });
    if !javac.is_file() {
        return None;
    }
    let major = std::fs::read_to_string(home.join("release")).ok().and_then(|t| release_major(&t))?;
    Some(Jdk { home: home.to_path_buf(), major })
}

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
}

/// Every JDK found: JAVA_HOME, the usual install folders, Gradle's and IntelliJ's
/// downloads, and `tools/jdk*` or `jdk*` folders in the project's parents.
pub fn jdks(near: Option<&Path>) -> Vec<Jdk> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        dirs.push(PathBuf::from(h));
    }
    let home = super::home();
    // Folders whose every subfolder may be a JDK.
    let mut parents: Vec<PathBuf> = vec![home.join(".jdks"), home.join(".gradle").join("jdks"), home.join(".sdkman").join("candidates").join("java")];
    // Folders where only `jdk*` subfolders are looked at (a project's parents).
    let mut nearby: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for base in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(base).map(PathBuf::from) {
                for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "Amazon Corretto", "BellSoft", "Semeru", "OpenJDK"] {
                    parents.push(pf.join(vendor));
                }
            }
        }
    } else {
        parents.push(PathBuf::from("/usr/lib/jvm"));
        parents.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    }
    if let Some(mut d) = near {
        for _ in 0..6 {
            parents.push(d.join("tools"));
            nearby.push(d.to_path_buf());
            match d.parent() {
                Some(p) => d = p,
                None => break,
            }
        }
    }
    for p in parents {
        for c in children(&p) {
            dirs.push(c.join("Contents").join("Home"));
            dirs.push(c);
        }
    }
    for p in nearby {
        for c in children(&p) {
            if c.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().starts_with("jdk")) {
                dirs.push(c);
            }
        }
    }
    let mut out: Vec<Jdk> = Vec::new();
    for d in dirs {
        if let Some(j) = jdk_at(&d) {
            if !out.iter().any(|o| o.home == j.home) {
                out.push(j);
            }
        }
    }
    out
}

/// The JDK for a build: the version it asks for, else the newest.
pub fn best_jdk(project: Option<&Project>) -> Option<Jdk> {
    let all = jdks(project.map(|p| p.dir.as_path()));
    let want = project.and_then(|p| wanted(&p.dir));
    match want {
        Some(v) => all.iter().find(|j| j.major == v).cloned(),
        None => all.into_iter().max_by_key(|j| j.major),
    }
}

/// The Java version a build asks for: a Gradle toolchain or `release`, Maven's
/// `maven.compiler.release` / `release` / `java.version`.
pub fn wanted(dir: &Path) -> Option<u32> {
    let num = |s: &str| -> Option<u32> {
        let d: String = s.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
        d.parse().ok().filter(|n: &u32| (8..=40).contains(n))
    };
    for f in ["build.gradle.kts", "build.gradle"] {
        let Ok(t) = std::fs::read_to_string(dir.join(f)) else { continue };
        for key in ["JavaLanguageVersion.of(", "release.set(", "JavaVersion.VERSION_", "sourceCompatibility"] {
            if let Some(i) = t.find(key) {
                if let Some(n) = num(&t[i + key.len()..(i + key.len() + 12).min(t.len())]) {
                    return Some(n);
                }
            }
        }
    }
    let t = std::fs::read_to_string(dir.join("pom.xml")).ok()?;
    for key in ["<maven.compiler.release>", "<release>", "<java.version>", "<maven.compiler.target>"] {
        if let Some(i) = t.find(key) {
            if let Some(n) = num(&t[i + key.len()..(i + key.len() + 8).min(t.len())]) {
                return Some(n);
            }
        }
    }
    None
}

/// The Temurin JDK winget id for a version (21 → EclipseAdoptium.Temurin.21.JDK).
pub fn winget_jdk(major: u32) -> String {
    format!("EclipseAdoptium.Temurin.{major}.JDK")
}

// ------------------------------------------------------------------ Paper

/// `name:`, `main:` and `api-version:` from a plugin's plugin.yml / paper-plugin.yml.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PluginYml {
    pub name: String,
    pub main: String,
    pub api: String,
    pub path: PathBuf,
}

fn yml_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with(&format!("{key}:")))
        .map(|l| l[key.len() + 1..].trim().trim_matches(['\'', '"']).to_string())
        .filter(|v| !v.is_empty())
}

/// The plugin.yml under a project (the first one, skipping build output).
pub fn plugin_yml(dir: &Path) -> Option<PluginYml> {
    fn find(d: &Path, depth: u32) -> Option<PathBuf> {
        if depth > 7 {
            return None;
        }
        for name in ["paper-plugin.yml", "plugin.yml"] {
            let p = d.join(name);
            if p.is_file() && d.ends_with("resources") {
                return Some(p);
            }
        }
        let mut subs = children(d);
        subs.sort();
        for s in subs {
            let n = s.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if ["build", ".gradle", ".git", "target", "node_modules", "out", "bin"].contains(&n.as_str()) {
                continue;
            }
            if let Some(p) = find(&s, depth + 1) {
                return Some(p);
            }
        }
        None
    }
    let path = find(dir, 0)?;
    let text = std::fs::read_to_string(&path).ok()?;
    Some(PluginYml { name: yml_value(&text, "name").unwrap_or_default(), main: yml_value(&text, "main").unwrap_or_default(), api: yml_value(&text, "api-version").unwrap_or_default(), path })
}

/// A Paper (or Purpur, Folia, Spigot) server: its folder and jar.
#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub dir: PathBuf,
    pub jar: String,
}

impl Server {
    /// `paper-26.3-244.jar` → "26.3"; `paper-1.21.11-132.jar` → "1.21.11".
    pub fn version(&self) -> Option<String> {
        let stem = self.jar.trim_end_matches(".jar");
        let mut parts = stem.split('-').skip(1);
        let v = parts.next()?;
        v.chars().next().filter(char::is_ascii_digit).map(|_| v.to_string())
    }
}

/// A server in this folder: a `plugins` folder and a server jar.
pub fn server_at(dir: &Path) -> Option<Server> {
    if !dir.join("plugins").is_dir() {
        return None;
    }
    let rd = std::fs::read_dir(dir).ok()?;
    let mut jars: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l.ends_with(".jar") && ["paper", "purpur", "folia", "spigot", "pufferfish", "leaf"].iter().any(|k| l.starts_with(k))
        })
        .collect();
    jars.sort();
    Some(Server { dir: dir.to_path_buf(), jar: jars.pop()? })
}

/// The test server for a plugin project: a parent folder that is one (the plugin's
/// source kept inside the server's folder), else one in the workspace.
pub fn find_server(project: &Path, roots: &[PathBuf]) -> Option<Server> {
    let mut d = project.parent();
    for _ in 0..6 {
        let dir = d?;
        if let Some(s) = server_at(dir) {
            return Some(s);
        }
        d = dir.parent();
    }
    for r in roots {
        if let Some(s) = server_at(r) {
            return Some(s);
        }
        for c in children(r) {
            if let Some(s) = server_at(&c) {
                return Some(s);
            }
        }
    }
    None
}

/// The plugin jar a build made: the newest jar in a `libs` (Gradle) or `target`
/// (Maven) folder, not sources or javadoc, named after the plugin when one is.
pub fn built_jar(dir: &Path, plugin: &str) -> Option<PathBuf> {
    fn walk(d: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        if depth > 6 {
            return;
        }
        let in_out = d.ends_with("libs") || d.ends_with("target");
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if p.is_dir() {
                if ["src", ".gradle", ".git", "node_modules", "classes", "tmp", "generated", "resources"].contains(&n.as_str()) {
                    continue;
                }
                walk(&p, depth + 1, out);
            } else if in_out && n.ends_with(".jar") && !["-sources", "-javadoc", "-plain", "original-"].iter().any(|k| n.contains(k)) {
                // A multi-project build's top project makes an empty jar (just a manifest)
                // with the plugin's name: never that one.
                if std::fs::metadata(&p).is_ok_and(|m| m.len() >= 1024) {
                    out.push(p);
                }
            }
        }
    }
    let mut jars = Vec::new();
    walk(&dir.join("build"), 0, &mut jars);
    walk(&dir.join("target"), 0, &mut jars);
    for c in children(dir) {
        walk(&c.join("build"), 0, &mut jars);
        walk(&c.join("target"), 0, &mut jars);
    }
    // The jars of the newest build (written within a minute of the newest one), and of
    // those the largest: the shaded jar, not the thin one shadow leaves next to it.
    let newest = |list: Vec<PathBuf>| {
        let time = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let last = list.iter().filter_map(time).max()?;
        list.iter()
            .filter(|p| time(p).is_some_and(|t| last.duration_since(t).map(|d| d.as_secs() < 60).unwrap_or(true)))
            .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .cloned()
    };
    let lower = plugin.to_ascii_lowercase();
    let named: Vec<PathBuf> = jars.iter().filter(|p| !lower.is_empty() && p.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().starts_with(&lower))).cloned().collect();
    if named.is_empty() { newest(jars) } else { newest(named) }
}

/// The command line that starts a server in the Terminal, with the newest JDK.
pub fn server_line(server: &Server) -> String {
    let java = best_jdk(None).map(|j| j.home.join("bin").join("java").display().to_string()).unwrap_or_else(|| "java".into());
    let cd = if cfg!(windows) { format!("cd /d \"{}\"", server.dir.display()) } else { format!("cd \"{}\"", server.dir.display()) };
    format!("{cd} && \"{java}\" -Xms1G -Xmx2G -XX:+UseG1GC -jar {} --nogui", server.jar)
}

/// For the agent: the JDK, the plugin and the test server.
pub fn facts(p: &Project) -> String {
    let mut s = String::new();
    match (wanted(&p.dir), best_jdk(Some(p))) {
        (Some(w), Some(j)) => s.push_str(&format!(" Java {w} (JDK at {}).", j.home.display())),
        (Some(w), None) => s.push_str(&format!(" Asks for Java {w}, which isn't installed.")),
        (None, Some(j)) => s.push_str(&format!(" JDK {}.", j.major)),
        _ => {}
    }
    if p.ext == "paper" {
        if let Some(y) = plugin_yml(&p.dir) {
            s.push_str(&format!(" plugin.yml: name {}, main {}, api-version {}.", y.name, y.main, if y.api.is_empty() { "unset" } else { &y.api }));
        }
        match find_server(&p.dir, &[]) {
            Some(sv) => s.push_str(&format!(" Test server: {} ({}); Deploy copies the jar into its plugins folder, with the server stopped.", sv.dir.display(), sv.jar)),
            None => s.push_str(" No test server next to it."),
        }
    }
    s
}

// ------------------------------------------------------------------ Gradle wrapper

/// The current Gradle version, from services.gradle.org (blocking).
fn current_gradle() -> Result<String, String> {
    let out = crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-s", "-S", "-f", "-L", "--max-time", "20", "https://services.gradle.org/versions/current"])
        .output()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| "services.gradle.org did not answer".to_string())?;
    v.get("version").and_then(|v| v.as_str()).map(str::to_string).ok_or_else(|| "no version from services.gradle.org".into())
}

/// Add the Gradle wrapper (gradlew, gradlew.bat, the 49 KB wrapper jar) from Gradle's
/// repository at the current release, so a project builds without Gradle installed.
pub fn add_wrapper(dir: &Path) -> Result<String, String> {
    let v = current_gradle()?;
    let curl = if cfg!(windows) { "curl.exe" } else { "curl" };
    std::fs::create_dir_all(dir.join("gradle").join("wrapper")).map_err(|e| e.to_string())?;
    for f in ["gradlew", "gradlew.bat", "gradle/wrapper/gradle-wrapper.jar"] {
        let url = format!("https://raw.githubusercontent.com/gradle/gradle/v{v}/{f}");
        let to = dir.join(f.replace('/', std::path::MAIN_SEPARATOR_STR));
        let out = crate::proc::command(curl).args(["-s", "-S", "-f", "-L", "--max-time", "60", "-o"]).arg(&to).arg(&url).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("Could not download {f}: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
    }
    let props = format!(
        "distributionBase=GRADLE_USER_HOME\ndistributionPath=wrapper/dists\ndistributionUrl=https\\://services.gradle.org/distributions/gradle-{v}-bin.zip\nnetworkTimeout=10000\nvalidateDistributionUrl=true\nzipStoreBase=GRADLE_USER_HOME\nzipStorePath=wrapper/dists\n"
    );
    std::fs::write(dir.join("gradle").join("wrapper").join("gradle-wrapper.properties"), props).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir.join("gradlew"), std::fs::Permissions::from_mode(0o755));
    }
    Ok(v)
}

// ------------------------------------------------------------------ new Paper plugin

/// `my cool-plugin` → `MyCoolPlugin`.
pub fn class_name(name: &str) -> String {
    let s: String = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect();
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) { format!("P{s}") } else { s }
}

/// The paper-api coordinate, Java version and api-version for a server version:
/// "26.3" → 26.3.build.+ on Java 25 (Paper's docs), "1.21.11" → the -R0.1-SNAPSHOT scheme on 21.
pub fn paper_target(version: Option<&str>) -> (String, u32, String) {
    match version {
        Some(v) if v.starts_with("1.") => (format!("io.papermc.paper:paper-api:{v}-R0.1-SNAPSHOT"), 21, v.to_string()),
        Some(v) => (format!("io.papermc.paper:paper-api:{v}.build.+"), 25, v.to_string()),
        None => ("io.papermc.paper:paper-api:26.3.build.+".into(), 25, "26.3".into()),
    }
}

/// Write a new Paper plugin into `parent/<Name>`; returns its folder.
pub fn new_plugin(parent: &Path, name: &str, server: Option<&Server>) -> Result<PathBuf, String> {
    let class = class_name(name);
    if class.is_empty() {
        return Err("Give the plugin a name (letters and digits).".into());
    }
    let dir = parent.join(&class);
    if dir.exists() {
        return Err(format!("{} already exists", dir.display()));
    }
    let pkg = format!("dev.{}", class.to_ascii_lowercase());
    let (dep, java, api) = paper_target(server.and_then(|s| s.version()).as_deref());
    let src = dir.join("src").join("main").join("java").join("dev").join(class.to_ascii_lowercase());
    let res = dir.join("src").join("main").join("resources");
    std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&res).map_err(|e| e.to_string())?;
    let w = |p: PathBuf, t: String| std::fs::write(p, t).map_err(|e| e.to_string());
    w(dir.join("settings.gradle.kts"), format!("rootProject.name = \"{class}\"\n"))?;
    w(
        dir.join("build.gradle.kts"),
        format!(
            "plugins {{\n    java\n}}\n\ngroup = \"{pkg}\"\nversion = \"1.0.0\"\n\nrepositories {{\n    mavenCentral()\n    maven(\"https://repo.papermc.io/repository/maven-public/\")\n}}\n\ndependencies {{\n    compileOnly(\"{dep}\")\n}}\n\njava {{\n    toolchain.languageVersion.set(JavaLanguageVersion.of({java}))\n}}\n\ntasks.withType<JavaCompile>().configureEach {{\n    options.encoding = \"UTF-8\"\n}}\n\ntasks.processResources {{\n    filesMatching(\"plugin.yml\") {{\n        expand(\"version\" to project.version)\n    }}\n}}\n"
        ),
    )?;
    w(
        res.join("plugin.yml"),
        format!("name: {class}\nversion: '${{version}}'\nmain: {pkg}.{class}\napi-version: '{api}'\ndescription: A Paper plugin.\ncommands:\n  hello:\n    description: Says hello.\n    usage: /hello\n"),
    )?;
    w(
        src.join(format!("{class}.java")),
        format!(
            "package {pkg};\n\nimport org.bukkit.command.Command;\nimport org.bukkit.command.CommandSender;\nimport org.bukkit.plugin.java.JavaPlugin;\n\npublic final class {class} extends JavaPlugin {{\n    @Override\n    public void onEnable() {{\n        getLogger().info(\"{class} is on.\");\n    }}\n\n    @Override\n    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {{\n        if (command.getName().equalsIgnoreCase(\"hello\")) {{\n            sender.sendMessage(\"Hello from {class}!\");\n            return true;\n        }}\n        return false;\n    }}\n}}\n"
        ),
    )?;
    w(dir.join(".gitignore"), ".gradle/\nbuild/\n.idea/\n*.iml\n".into())?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_jdk_versions() {
        assert_eq!(release_major("IMPLEMENTOR=\"Eclipse\"\nJAVA_VERSION=\"21.0.5\"\n"), Some(21));
        assert_eq!(release_major("JAVA_VERSION=\"1.8.0_402\""), Some(8));
        assert_eq!(release_major("JAVA_VERSION=\"25\""), Some(25));
    }

    #[test]
    fn server_versions_and_targets() {
        let s = Server { dir: PathBuf::new(), jar: "paper-26.3-244.jar".into() };
        assert_eq!(s.version().as_deref(), Some("26.3"));
        let old = Server { dir: PathBuf::new(), jar: "paper-1.21.11-132.jar".into() };
        assert_eq!(old.version().as_deref(), Some("1.21.11"));
        assert_eq!(paper_target(Some("26.3")), ("io.papermc.paper:paper-api:26.3.build.+".into(), 25, "26.3".into()));
        assert_eq!(paper_target(Some("1.21.11")).1, 21);
        assert_eq!(class_name("my cool-plugin"), "MyCoolPlugin");
    }

    #[test]
    fn wanted_java_from_gradle_and_maven() {
        let d = std::env::temp_dir().join(format!("fx-java-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("build.gradle.kts"), "subprojects {\n  toolchain.languageVersion.set(JavaLanguageVersion.of(21))\n}").unwrap();
        assert_eq!(wanted(&d), Some(21));
        std::fs::remove_file(d.join("build.gradle.kts")).unwrap();
        std::fs::write(d.join("pom.xml"), "<properties><maven.compiler.release>17</maven.compiler.release></properties>").unwrap();
        assert_eq!(wanted(&d), Some(17));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_new_plugin_has_its_files() {
        let d = std::env::temp_dir().join(format!("fx-paper-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let dir = new_plugin(&d, "hello world", Some(&Server { dir: d.clone(), jar: "paper-26.3-244.jar".into() })).unwrap();
        let yml = std::fs::read_to_string(dir.join("src/main/resources/plugin.yml")).unwrap();
        assert!(yml.contains("main: dev.helloworld.HelloWorld") && yml.contains("api-version: '26.3'"), "{yml}");
        assert!(std::fs::read_to_string(dir.join("build.gradle.kts")).unwrap().contains("26.3.build.+"));
        assert!(dir.join("src/main/java/dev/helloworld/HelloWorld.java").is_file());
        assert_eq!(plugin_yml(&dir).unwrap().name, "HelloWorld");
        let _ = std::fs::remove_dir_all(&d);
    }
}
