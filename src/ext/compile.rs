//! FluxCompile: how FluxCode builds a Java project.
//!
//! The compiling itself is the project's own build (Gradle or Maven, as its authors set
//! it up), because only that build knows its dependencies, shading and resources.
//! FluxCompile decides how that build runs and what you see of it:
//! - the JDK the project asks for (`java::best_jdk`), on JAVA_HOME and PATH;
//! - Gradle with its local build cache (unless the project turns it off); parallel
//!   projects and the configuration cache only when the project turns them on itself,
//!   since forcing them breaks builds that wire projects together or use plugins that
//!   don't support them (Gradle's docs; EssentialsX wires its projects with
//!   afterEvaluate, for one). The daemon stays warm between builds (Gradle's default);
//!   FluxCompile never changes its JVM settings, which would start a second daemon;
//! - the output folded: one progress line instead of a line per Gradle task (failed
//!   tasks still show), then one line saying what was built, read from the jar;
//! - the plugin's own name and version (from the plugin.yml inside the jar) for the
//!   file it deploys and to find older copies in the server's plugins folder.

use super::java::{self, Server};
use super::Project;
use std::path::{Path, PathBuf};

/// `key=value` from a project's gradle.properties.
fn gradle_property(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("gradle.properties")).ok()?;
    text.lines().map(str::trim).find_map(|l| l.strip_prefix(key).and_then(|r| r.trim_start().strip_prefix('=')).map(|v| v.trim().to_string()))
}

/// Flags FluxCompile adds to a Gradle build in `dir`: the local build cache unless the
/// project turns it off, plain console output. What the project's gradle.properties
/// turns on (parallel, configuration cache, ...) Gradle uses by itself.
pub fn gradle_flags(dir: &Path) -> String {
    let mut f = Vec::new();
    if gradle_property(dir, "org.gradle.caching").as_deref() != Some("false") {
        f.push("--build-cache");
    }
    f.push("--console=plain");
    f.join(" ")
}

/// What FluxCompile does, for the extension page.
pub const ABOUT: &str = "the project's JDK, Gradle's local build cache (unless the project turns it off), the project's own parallel and configuration-cache settings, the output folded";

/// `name` and `version` from the plugin.yml (or paper-plugin.yml) inside a jar.
pub fn jar_plugin(jar: &Path) -> Option<(String, String)> {
    let bytes = std::fs::read(jar).ok()?;
    let yml = ["paper-plugin.yml", "plugin.yml", "velocity-plugin.json", "bungee.yml"].iter().find_map(|n| super::zip::read_entry(&bytes, n))?;
    let text = String::from_utf8_lossy(&yml);
    let get = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(&format!("{key}:")))
            .map(|l| l[key.len() + 1..].trim().trim_matches(['\'', '"']).to_string())
            .filter(|v| !v.is_empty())
    };
    Some((get("name")?, get("version").unwrap_or_default()))
}

/// `KiwEssentials`, `0.1.0-SNAPSHOT` → `KiwEssentials-0.1.0-SNAPSHOT.jar`.
pub fn jar_name(name: &str, version: &str) -> String {
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || "._-+".contains(c) { c } else { '_' }).collect::<String>();
    if version.is_empty() || version.contains("${") { format!("{}.jar", safe(name)) } else { format!("{}-{}.jar", safe(name), safe(version)) }
}

/// The plugin's name as it calls itself: from the built jar, else its plugin.yml.
pub fn plugin_name(dir: &Path) -> Option<String> {
    let yml = java::plugin_yml(dir);
    let built = java::built_jar(dir, yml.as_ref().map(|y| y.name.as_str()).unwrap_or("")).and_then(|j| jar_plugin(&j)).map(|(n, _)| n);
    built.or_else(|| yml.map(|y| y.name)).filter(|n| !n.is_empty())
}

fn short(p: &Path, base: &Path) -> String {
    let parent = base.parent().unwrap_or(base);
    p.strip_prefix(parent).map(|r| r.display().to_string()).unwrap_or_else(|_| p.display().to_string())
}

/// After a build: what was built, from the jar (for the Output panel).
pub fn summary(p: &Project, took: &str) -> Option<String> {
    let yml = java::plugin_yml(&p.dir);
    let jar = java::built_jar(&p.dir, yml.as_ref().map(|y| y.name.as_str()).unwrap_or(""))?;
    let kb = std::fs::metadata(&jar).map(|m| m.len() / 1024).unwrap_or(0);
    let what = match jar_plugin(&jar) {
        Some((n, v)) if !v.is_empty() => format!("{n} {v}"),
        Some((n, _)) => n,
        None => jar.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
    };
    let rel = jar.strip_prefix(&p.dir).map(|r| r.display().to_string()).unwrap_or_default();
    Some(format!("{what} · {kb} KB · {took} · {rel}"))
}

/// Copy the built plugin into the server's plugins folder as `<Name>-<version>.jar`,
/// replacing every other jar there that is the same plugin (by the name inside it).
pub fn deploy(project: &Project, roots: &[PathBuf]) -> Result<Vec<String>, String> {
    let yml = java::plugin_yml(&project.dir);
    let declared = yml.as_ref().map(|y| y.name.clone()).unwrap_or_default();
    let jar = java::built_jar(&project.dir, &declared).ok_or("No built jar yet: run Build first (it lands in build/libs or target).")?;
    let server: Server = java::find_server(&project.dir, roots).ok_or(
        "No test server found. Keep the plugin's folder inside a Paper server's folder (one with a plugins folder and paper-*.jar), or open both in one workspace.",
    )?;
    let (name, version) = jar_plugin(&jar).unwrap_or_else(|| (if declared.is_empty() { project.name.clone() } else { declared.clone() }, String::new()));
    let target = jar_name(&name, &version);
    let plugins = server.dir.join("plugins");
    let mut lines = Vec::new();
    let busy = |n: &str| format!("{n} is in use: the server is running. Type stop in the Terminal, then deploy again.");
    for e in std::fs::read_dir(&plugins).map_err(|e| e.to_string())?.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n == target || !n.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        let same = jar_plugin(&e.path()).is_some_and(|(other, _)| other.eq_ignore_ascii_case(&name));
        if same {
            std::fs::remove_file(e.path()).map_err(|_| busy(&n))?;
            lines.push(format!("Removed the older {n}"));
        }
    }
    std::fs::copy(&jar, plugins.join(&target)).map_err(|e| if e.raw_os_error() == Some(32) { busy(&target) } else { e.to_string() })?;
    let kb = std::fs::metadata(&jar).map(|m| m.len() / 1024).unwrap_or(0);
    let v = if version.is_empty() { String::new() } else { format!(" {version}") };
    lines.insert(0, format!("{name}{v} → {} ({kb} KB)", short(&plugins.join(&target), &server.dir)));
    lines.push(format!("Start {} (or restart it) to load it.", server.jar));
    if !mojang_mapped(&jar) && server.version().is_some_and(|v| !v.starts_with("1.") || v.as_str() >= "1.20.5") {
        lines.push("Tip: add `paperweight-mappings-namespace: mojang` to the jar's manifest; Paper then skips remapping this plugin each time it changes (LuckPerms, WorldEdit and ViaVersion do).".into());
    }
    Ok(lines)
}

/// The jar says it uses Mojang's names, so Paper loads it without remapping.
pub fn mojang_mapped(jar: &Path) -> bool {
    std::fs::read(jar)
        .ok()
        .and_then(|b| super::zip::read_entry(&b, "META-INF/MANIFEST.MF"))
        .is_some_and(|m| String::from_utf8_lossy(&m).contains("paperweight-mappings-namespace"))
}

/// Gradle's per-task lines are folded into one progress line while a build runs.
pub fn is_task_line(t: &str) -> bool {
    t.starts_with("> Task ") && !t.contains("FAILED")
}

/// `> Task :main:compileJava UP-TO-DATE` → `:main:compileJava`.
pub fn task_name(t: &str) -> &str {
    t.trim_start_matches("> Task ").split_whitespace().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_follow_the_project() {
        let d = std::env::temp_dir().join(format!("fx-flags-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(gradle_flags(&d), "--build-cache --console=plain");
        std::fs::write(d.join("gradle.properties"), "org.gradle.jvmargs=-Xmx2G\norg.gradle.caching = false\n").unwrap();
        assert_eq!(gradle_flags(&d), "--console=plain");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn names_from_the_jar() {
        let d = std::env::temp_dir().join(format!("fx-compile-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let mut z = super::super::zip::Zip::default();
        z.add("plugin.yml", b"name: KiwEssentials\nversion: '0.1.0-SNAPSHOT'\nmain: dev.kiw.Main\n");
        z.add("dev/kiw/Main.class", &[0xCA, 0xFE, 0xBA, 0xBE]);
        let jar = d.join("x.jar");
        std::fs::write(&jar, z.finish()).unwrap();
        assert_eq!(jar_plugin(&jar), Some(("KiwEssentials".into(), "0.1.0-SNAPSHOT".into())));
        assert_eq!(jar_name("KiwEssentials", "0.1.0-SNAPSHOT"), "KiwEssentials-0.1.0-SNAPSHOT.jar");
        assert_eq!(jar_name("My Plugin", "${version}"), "My_Plugin.jar");
        assert!(is_task_line("> Task :main:compileJava UP-TO-DATE"));
        assert!(!is_task_line("> Task :main:compileJava FAILED"));
        assert_eq!(task_name("> Task :main:compileJava UP-TO-DATE"), ":main:compileJava");
        let _ = std::fs::remove_dir_all(&d);
    }
}
