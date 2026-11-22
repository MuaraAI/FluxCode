
use super::market;
use super::vsx_theme;
use crate::theme::{self, Palette};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    pub themes: Vec<(String, String)>,
    pub languages: Vec<String>,
    pub failed: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lang {
    pub token: &'static str,
    pub name: &'static str,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
}

static APPLIED: RwLock<Option<HashMap<String, Applied>>> = RwLock::new(None);
static LANGS: RwLock<Vec<Lang>> = RwLock::new(Vec::new());
static BUILT: Mutex<Option<HashMap<(PathBuf, String), &'static Palette>>> = Mutex::new(None);

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn load() {
    let mut applied = HashMap::new();
    let mut palettes = Vec::new();
    let mut langs = Vec::new();
    for ext in market::installed_list() {
        let Some(pkg) = read_pkg(&ext.folder) else {
            continue;
        };
        let mut a = Applied::default();
        let mut nls = None;
        for t in list(&pkg, "themes") {
            match theme(&ext, t, &mut nls) {
                Ok(p) => {
                    a.themes.push((p.id.to_string(), p.name.to_string()));
                    palettes.push(p);
                }
                Err(e) => a.failed.push(e),
            }
        }
        for l in list(&pkg, "languages") {
            if let Some(lang) = language_of(l, &mut |s| market::localized(&ext.folder, s, &mut nls))
            {
                if !a.languages.contains(&lang.name.to_string()) {
                    a.languages.push(lang.name.to_string());
                }
                langs.push(lang);
            }
        }
        if a != Applied::default() {
            applied.insert(ext.id.clone(), a);
        }
    }
    theme::set_extra(palettes);
    *LANGS.write().unwrap_or_else(|e| e.into_inner()) = langs;
    *APPLIED.write().unwrap_or_else(|e| e.into_inner()) = Some(applied);
}

pub fn applied(id: &str) -> Applied {
    APPLIED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(id).cloned())
        .unwrap_or_default()
}

fn read_pkg(folder: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(folder.join("package.json")).ok()?).ok()
}

fn list<'a>(pkg: &'a Value, key: &str) -> &'a [Value] {
    pkg.get("contributes")
        .and_then(|c| c.get(key))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub fn theme_id(ext_id: &str, theme: &Value, label: &str) -> String {
    let own = theme.get("id").and_then(Value::as_str).unwrap_or(label);
    format!("vsx:{ext_id}/{own}")
}

fn theme(
    ext: &market::Installed,
    t: &Value,
    nls: &mut Option<Value>,
) -> Result<&'static Palette, String> {
    let rel = t.get("path").and_then(Value::as_str).unwrap_or("");
    let raw_label = t.get("label").and_then(Value::as_str).unwrap_or("");
    let label = match market::localized(&ext.folder, raw_label, nls) {
        l if l.trim().is_empty() || l.starts_with('%') => Path::new(rel)
            .file_stem()
            .map_or_else(|| ext.display.clone(), |s| s.to_string_lossy().into_owned()),
        l => l,
    };
    if rel.is_empty() || rel.contains("..") || Path::new(rel).is_absolute() {
        return Err(format!("{label}: no theme file"));
    }
    let path = ext.folder.join(rel);
    let key = (path.clone(), label.clone());
    if let Some(p) = BUILT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .get(&key)
    {
        return Ok(p);
    }
    let file = vsx_theme::read_theme(&ext.folder, &path).map_err(|e| format!("{label}: {e}"))?;
    let ui = t.get("uiTheme").and_then(Value::as_str).unwrap_or("");
    let dark = vsx_theme::is_dark(ui, &file);
    let p = palette(
        &file,
        dark,
        theme_id(&ext.id, t, &label),
        label,
        &ext.display,
    );
    BUILT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key, p);
    Ok(p)
}

pub fn palette(
    file: &vsx_theme::ThemeFile,
    dark: bool,
    id: String,
    name: String,
    from: &str,
) -> &'static Palette {
    let k = vsx_theme::colors(file, dark);
    let syntax = vsx_theme::syntax(file, k.editor, k.fg).map(|s| &*Box::leak(Box::new(s)));
    let note = crate::trf!("VS Code theme from {ext}"; ext => from);
    Box::leak(Box::new(Palette {
        id: leak(id),
        name: leak(name),
        note: leak(note),
        dark,
        title: k.title,
        side: k.side,
        editor: k.editor,
        elevated: k.elevated,
        hover: k.hover,
        border: k.border,
        border_strong: k.border_strong,
        fg: k.fg,
        strong: k.strong,
        muted: k.muted,
        dim: k.dim,
        accent: k.accent,
        on_accent: k.on_accent,
        accent_text: k.accent_text,
        blue: k.blue,
        green: k.green,
        warn: k.warn,
        danger: k.danger,
        folder: k.folder,
        select: k.select,
        selection: k.selection,
        syntax,
    }))
}

pub fn language_of(l: &Value, localize: &mut dyn FnMut(&str) -> String) -> Option<Lang> {
    let id = l.get("id").and_then(Value::as_str)?.trim();
    let strings = |k: &str| -> Vec<String> {
        l.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let extensions: Vec<String> = strings("extensions")
        .into_iter()
        .map(|e| {
            if e.starts_with('.') {
                e
            } else {
                format!(".{e}")
            }
        })
        .collect();
    let filenames = strings("filenames");
    if id.is_empty() || (extensions.is_empty() && filenames.is_empty()) {
        return None;
    }
    let name = l
        .get("aliases")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .find(|s| !s.trim().is_empty())
        })
        .map(localize)
        .filter(|s| !s.trim().is_empty() && !s.starts_with('%'))
        .unwrap_or_else(|| id.to_string());
    let token = extensions
        .iter()
        .map(|e| e.rsplit('.').next().unwrap_or(""))
        .find(|e| !e.is_empty())
        .unwrap_or(id)
        .to_string();
    Some(Lang {
        token: leak(token),
        name: leak(name),
        extensions,
        filenames,
    })
}

pub fn language(name: &str, multi_dot: bool) -> Option<(&'static str, &'static str)> {
    let langs = LANGS.read().unwrap_or_else(|e| e.into_inner());
    lookup(&langs, name, multi_dot)
}

fn lookup(langs: &[Lang], name: &str, multi_dot: bool) -> Option<(&'static str, &'static str)> {
    if !multi_dot {
        if let Some(l) = langs.iter().find(|l| l.filenames.iter().any(|f| f == name)) {
            return Some((l.token, l.name));
        }
    }
    langs
        .iter()
        .flat_map(|l| l.extensions.iter().map(move |e| (l, e)))
        .filter(|(_, e)| {
            (e.matches('.').count() > 1) == multi_dot
                && name.ends_with(e.as_str())
                && name.len() > e.len()
        })
        .max_by_key(|(_, e)| e.len())
        .map(|(l, _)| (l.token, l.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn same(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn languages_take_extensions_names_and_aliases() {
        let l = language_of(
            &json!({ "id": "svelte", "aliases": ["Svelte", "svelte"], "extensions": [".svelte", "SVX"], "filenames": ["Svelte.config"] }),
            &mut same,
        )
        .unwrap();
        assert_eq!((l.token, l.name), ("svelte", "Svelte"));
        assert_eq!(l.extensions, [".svelte", ".svx"]);
        assert_eq!(l.filenames, ["svelte.config"]);
        assert!(language_of(&json!({ "id": "svelte-injection" }), &mut same).is_none());
        let l = language_of(
            &json!({ "id": "prisma", "extensions": [".prisma"] }),
            &mut same,
        )
        .unwrap();
        assert_eq!(l.name, "prisma");
        let l = language_of(
            &json!({ "id": "x", "aliases": ["%x.name%"], "filenames": ["xfile"] }),
            &mut |_| "Ex".into(),
        )
        .unwrap();
        assert_eq!((l.token, l.name), ("x", "Ex"));
    }

    #[test]
    fn lookup_by_file_name_and_longest_ending() {
        let langs = vec![
            language_of(&json!({ "id": "blade", "aliases": ["Blade"], "extensions": [".blade.php"] }), &mut same).unwrap(),
            language_of(&json!({ "id": "svelte", "aliases": ["Svelte"], "extensions": [".svelte"], "filenames": ["svelte.config"] }), &mut same).unwrap(),
        ];
        assert_eq!(
            lookup(&langs, "app.svelte", false),
            Some(("svelte", "Svelte"))
        );
        assert_eq!(
            lookup(&langs, "svelte.config", false),
            Some(("svelte", "Svelte"))
        );
        assert_eq!(
            lookup(&langs, "home.blade.php", true),
            Some(("php", "Blade"))
        );
        assert_eq!(lookup(&langs, "home.blade.php", false), None);
        assert_eq!(lookup(&langs, "app.svelte", true), None);
        assert_eq!(lookup(&langs, ".svelte", false), None);
        assert_eq!(lookup(&langs, "main.rs", false), None);
    }

    #[test]
    fn theme_ids_prefer_the_manifest_id() {
        assert_eq!(
            theme_id(
                "dracula-theme.theme-dracula",
                &json!({ "id": "dracula-soft" }),
                "Dracula Soft"
            ),
            "vsx:dracula-theme.theme-dracula/dracula-soft"
        );
        assert_eq!(
            theme_id("zhuangtongfa.material-theme", &json!({}), "One Dark Pro"),
            "vsx:zhuangtongfa.material-theme/One Dark Pro"
        );
    }

    #[test]
    #[ignore]
    fn marketplace_themes_end_to_end() {
        for (query, id) in [
            ("Dracula Official", "dracula-theme.theme-dracula"),
            ("One Dark Pro", "zhuangtongfa.material-theme"),
        ] {
            let page = market::search(&market::Query::new(query, None, None), 0).expect("search");
            let item = page
                .items
                .iter()
                .find(|i| i.id() == id)
                .unwrap_or_else(|| panic!("{id} not found"));
            market::install(item).expect("install");
        }
        load();
        for id in ["dracula-theme.theme-dracula", "zhuangtongfa.material-theme"] {
            let a = applied(id);
            assert!(
                !a.themes.is_empty(),
                "{id}: no themes applied ({:?})",
                a.failed
            );
            for (tid, name) in &a.themes {
                let p = theme::find(tid).expect("palette");
                let st = p.syntax.expect("code colors");
                let named = ["keyword", "string", "comment", "function", "type", "number"]
                    .iter()
                    .filter(|n| st.syntax.style(n).and_then(|s| s.color).is_some())
                    .count();
                println!(
                    "{name}: dark={} editor=#{:06x} side=#{:06x} fg=#{:06x} accent=#{:06x} selection=#{:06x}, {named}/6 code colors",
                    p.dark, p.editor, p.side, p.fg, p.accent, p.selection
                );
                assert!(named >= 5, "{name}: only {named} of the main code colors");
                for t in [
                    p.fg,
                    p.strong,
                    p.muted,
                    p.dim,
                    p.accent_text,
                    p.blue,
                    p.green,
                    p.warn,
                    p.danger,
                    p.folder,
                ] {
                    for s in [p.title, p.side, p.editor, p.elevated, p.hover, p.select] {
                        assert!(
                            vsx_theme::contrast(t, s) >= 4.5,
                            "{name}: #{t:06x} on #{s:06x}"
                        );
                    }
                }
                assert!(
                    vsx_theme::contrast(p.on_accent, p.accent) >= 5.1,
                    "{name}: text on the accent"
                );
            }
        }
    }

    #[test]
    fn an_installed_theme_extension_is_applied() {
        let dir = std::env::temp_dir().join(format!("fluxcode-native-{}", std::process::id()));
        let ext = dir.join("acme.night-1.0.0");
        std::fs::create_dir_all(ext.join("themes")).unwrap();
        std::fs::write(
            ext.join("package.json"),
            r#"{ "publisher": "acme", "name": "night", "version": "1.0.0", "displayName": "Night",
                 "contributes": { "themes": [{ "label": "Night", "uiTheme": "vs-dark", "path": "./themes/night.json" },
                                             { "label": "Broken", "uiTheme": "vs-dark", "path": "./themes/missing.json" }],
                                  "languages": [{ "id": "nightlang", "aliases": ["Night Lang"], "extensions": [".night"] }] } }"#,
        )
        .unwrap();
        std::fs::write(
            ext.join("themes/night.json"),
            r##"{ "colors": { "editor.background": "#101820", "editor.foreground": "#e0e0e0" },
                 "tokenColors": [{ "scope": "keyword", "settings": { "foreground": "#ff8800" } }] }"##,
        )
        .unwrap();
        let installed = market::Installed {
            id: "acme.night".into(),
            folder: ext.clone(),
            display: "Night".into(),
            ..Default::default()
        };
        let pkg = read_pkg(&ext).unwrap();
        let mut nls = None;
        let p = theme(&installed, &list(&pkg, "themes")[0], &mut nls).unwrap();
        assert_eq!(
            (p.id, p.name, p.dark, p.editor),
            ("vsx:acme.night/Night", "Night", true, 0x101820)
        );
        assert!(p.syntax.and_then(|s| s.syntax.style("keyword")).is_some());
        assert!(std::ptr::eq(
            p,
            theme(&installed, &list(&pkg, "themes")[0], &mut nls).unwrap()
        ));
        assert!(theme(&installed, &list(&pkg, "themes")[1], &mut nls).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
