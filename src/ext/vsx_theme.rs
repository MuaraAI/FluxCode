
use gpui_kit::component::highlighter::HighlightThemeStyle;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

const MAX_INCLUDE: u8 = 6;
const MAX_FILE: u64 = 8 << 20;

#[derive(Default, Debug, Clone)]
pub struct ThemeFile {
    pub colors: Map<String, Value>,
    pub rules: Vec<Value>,
    pub kind: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Colors {
    pub title: u32,
    pub side: u32,
    pub editor: u32,
    pub elevated: u32,
    pub hover: u32,
    pub border: u32,
    pub border_strong: u32,
    pub fg: u32,
    pub strong: u32,
    pub muted: u32,
    pub dim: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub accent_text: u32,
    pub blue: u32,
    pub green: u32,
    pub warn: u32,
    pub danger: u32,
    pub folder: u32,
    pub select: u32,
    pub selection: u32,
}


pub fn parse_jsonc(text: &str) -> Option<Value> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let stripped = super::bedrock::strip_comments(text);
    serde_json::from_str(&drop_trailing_commas(&stripped)).ok()
}

fn drop_trailing_commas(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let (mut i, mut in_str) = (0, false);
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            in_str = c != b'"';
        } else if c == b'"' {
            in_str = true;
            out.push(c);
        } else if c == b',' {
            let next = b[i + 1..].iter().find(|c| !c.is_ascii_whitespace());
            if !matches!(next, Some(b'}') | Some(b']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn inside(root: &Path, base: &Path, rel: &str) -> Option<PathBuf> {
    let joined = base.join(rel);
    let (root, path) = (root.canonicalize().ok()?, joined.canonicalize().ok()?);
    path.starts_with(&root).then_some(path)
}

fn read_text(path: &Path) -> Result<String, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_FILE {
        return Err(format!("{} is larger than 8 MB", path.display()));
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

pub fn read_theme(root: &Path, path: &Path) -> Result<ThemeFile, String> {
    let mut out = ThemeFile::default();
    read_into(root, path, &mut out, 0)?;
    Ok(out)
}

fn read_into(root: &Path, path: &Path, out: &mut ThemeFile, depth: u8) -> Result<(), String> {
    if depth > MAX_INCLUDE {
        return Ok(());
    }
    let v = parse_jsonc(&read_text(path)?)
        .ok_or_else(|| format!("{} is not a JSON theme", path.display()))?;
    let dir = path.parent().unwrap_or(root);
    if let Some(inc) = v.get("include").and_then(Value::as_str) {
        if let Some(p) = inside(root, dir, inc) {
            let _ = read_into(root, &p, out, depth + 1);
        }
    }
    if let Some(Value::Object(c)) = v.get("colors") {
        for (k, val) in c {
            out.colors.insert(k.clone(), val.clone());
        }
    }
    match v.get("tokenColors") {
        Some(Value::Array(a)) => out.rules.extend(a.iter().cloned()),
        Some(Value::String(rel)) if !rel.to_ascii_lowercase().ends_with(".tmtheme") => {
            if let Some(p) = inside(root, dir, rel) {
                if let Some(v) = read_text(&p).ok().and_then(|t| parse_jsonc(&t)) {
                    let rules = v.get("tokenColors").unwrap_or(&v);
                    if let Value::Array(a) = rules {
                        out.rules.extend(a.iter().cloned());
                    }
                }
            }
        }
        _ => {}
    }
    if let Some(Value::Array(a)) = v.get("settings") {
        out.rules.extend(a.iter().cloned());
    }
    if let Some(t) = v.get("type").and_then(Value::as_str) {
        out.kind = Some(t.to_string());
    }
    Ok(())
}

pub fn is_dark(ui_theme: &str, file: &ThemeFile) -> bool {
    match ui_theme {
        "vs" | "hc-light" => return false,
        "vs-dark" | "hc-black" => return true,
        _ => {}
    }
    match file.kind.as_deref() {
        Some("light") | Some("hcLight") => false,
        Some("dark") | Some("hc") => true,
        _ => file
            .colors
            .get("editor.background")
            .and_then(Value::as_str)
            .and_then(parse_color)
            .is_none_or(|(c, _)| luminance(c) < 0.4),
    }
}


pub fn parse_color(s: &str) -> Option<(u32, f32)> {
    let h = s.trim().strip_prefix('#')?;
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = |s: &str| u32::from_str_radix(s, 16).ok();
    let wide = |c: char| {
        let v = c.to_digit(16).unwrap_or(0);
        v * 16 + v
    };
    match h.len() {
        3 | 4 => {
            let cs: Vec<u32> = h.chars().map(wide).collect();
            let a = cs.get(3).map_or(1., |a| *a as f32 / 255.);
            Some(((cs[0] << 16) | (cs[1] << 8) | cs[2], a))
        }
        6 => Some((n(h)?, 1.)),
        8 => Some((n(&h[..6])?, n(&h[6..])? as f32 / 255.)),
        _ => None,
    }
}

pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, s: u32| ((c >> s) & 0xff) as f32;
    let one = |s: u32| (ch(a, s) * (1. - t) + ch(b, s) * t).round().clamp(0., 255.) as u32;
    (one(16) << 16) | (one(8) << 8) | one(0)
}

pub fn luminance(h: u32) -> f64 {
    let channel = |shift: u32| {
        let c = ((h >> shift) & 0xff) as f64 / 255.;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn saturation(c: u32) -> f32 {
    let ch = |s: u32| ((c >> s) & 0xff) as f32 / 255.;
    let (r, g, b) = (ch(16), ch(8), ch(0));
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    let l = (hi + lo) / 2.;
    if hi == lo {
        0.
    } else {
        (hi - lo) / (1. - (2. * l - 1.).abs())
    }
}

pub fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn readable(fg: u32, surfaces: &[u32], min: f64, dark: bool) -> u32 {
    let target = if dark { 0xffffff } else { 0x000000 };
    let ok = |c: u32| surfaces.iter().all(|s| contrast(c, *s) >= min);
    (0..=20)
        .map(|i| mix(fg, target, i as f32 / 20.))
        .find(|c| ok(*c))
        .unwrap_or(target)
}

struct Keys<'a>(&'a Map<String, Value>);

impl Keys<'_> {
    fn get(&self, keys: &[&str], under: u32) -> Option<u32> {
        keys.iter().find_map(|k| {
            let (c, a) = parse_color(self.0.get(*k)?.as_str()?)?;
            (a > 0.02).then(|| mix(under, c, a))
        })
    }
}

pub fn colors(file: &ThemeFile, dark: bool) -> Colors {
    let k = Keys(&file.colors);
    let (global_fg, global_bg) = global_colors(&file.rules);
    let editor = k
        .get(&["editor.background"], 0)
        .or(global_bg)
        .unwrap_or(if dark { 0x1e1e1e } else { 0xffffff });
    let fg = k
        .get(&["editor.foreground", "foreground"], editor)
        .or(global_fg)
        .unwrap_or(if dark { 0xd4d4d4 } else { 0x333333 });
    let side = k.get(&["sideBar.background"], editor).unwrap_or(editor);
    let title = k
        .get(
            &["titleBar.activeBackground", "activityBar.background"],
            side,
        )
        .unwrap_or(side);
    let elevated = k
        .get(
            &[
                "editorWidget.background",
                "menu.background",
                "dropdown.background",
                "quickInput.background",
            ],
            editor,
        )
        .unwrap_or_else(|| mix(editor, fg, 0.06));
    let hover = k
        .get(&["list.hoverBackground"], side)
        .filter(|h| contrast(*h, side) > 1.04)
        .unwrap_or_else(|| mix(side, fg, 0.08));
    let select = k
        .get(
            &[
                "list.activeSelectionBackground",
                "list.inactiveSelectionBackground",
            ],
            side,
        )
        .filter(|s| contrast(*s, side) > 1.04)
        .unwrap_or_else(|| mix(side, fg, 0.12));
    let border = k
        .get(
            &[
                "sideBar.border",
                "editorGroup.border",
                "panel.border",
                "tab.border",
            ],
            side,
        )
        .filter(|b| contrast(*b, side) > 1.15)
        .unwrap_or_else(|| mix(side, fg, 0.12));
    let border_strong = k
        .get(&["input.border", "dropdown.border", "contrastBorder"], side)
        .filter(|b| contrast(*b, side) > contrast(border, side))
        .unwrap_or_else(|| mix(side, fg, 0.22));
    let strong = mix(fg, if dark { 0xffffff } else { 0x000000 }, 0.45);
    let muted = k
        .get(&["descriptionForeground"], editor)
        .unwrap_or_else(|| mix(fg, editor, 0.25));
    let dim = k
        .get(&["editorLineNumber.foreground"], editor)
        .unwrap_or_else(|| mix(fg, editor, 0.35));
    let candidates: [(&str, &str); 6] = [
        ("activityBarBadge.background", "activityBarBadge.foreground"),
        ("button.background", "button.foreground"),
        (
            "statusBarItem.remoteBackground",
            "statusBarItem.remoteForeground",
        ),
        ("progressBar.background", ""),
        ("focusBorder", ""),
        ("tab.activeBorder", ""),
    ];
    let found: Vec<(u32, Option<u32>)> = candidates
        .iter()
        .filter_map(|(bg, on)| {
            let a = k.get(&[bg], side)?;
            Some((a, k.get(&[on], a)))
        })
        .collect();
    let (accent, accent_on) = found
        .iter()
        .find(|(a, _)| saturation(*a) >= 0.3 && contrast(*a, side) >= 1.5)
        .or_else(|| found.iter().find(|(a, _)| contrast(*a, side) >= 1.5))
        .or(found.first())
        .copied()
        .unwrap_or((if dark { 0x0e639c } else { 0x005fb8 }, None));
    let accent_text = k.get(&["textLink.foreground"], editor).unwrap_or(accent);
    let blue = k
        .get(&["terminal.ansiBrightBlue", "terminal.ansiBlue"], editor)
        .unwrap_or(0x569cd6);
    let green = k
        .get(
            &[
                "terminal.ansiGreen",
                "gitDecoration.untrackedResourceForeground",
            ],
            editor,
        )
        .unwrap_or(0x6a9955);
    let warn = k
        .get(
            &[
                "editorWarning.foreground",
                "list.warningForeground",
                "terminal.ansiYellow",
            ],
            editor,
        )
        .unwrap_or(0xcca700);
    let danger = k
        .get(
            &[
                "errorForeground",
                "editorError.foreground",
                "terminal.ansiRed",
            ],
            editor,
        )
        .unwrap_or(0xf14c4c);
    let folder = k
        .get(&["terminal.ansiYellow", "editorWarning.foreground"], editor)
        .unwrap_or(warn);
    let selection = k
        .get(&["editor.selectionBackground"], editor)
        .unwrap_or_else(|| mix(editor, accent, 0.3));

    let surfaces = [title, side, editor, elevated, hover, select];
    let text = |c: u32| readable(c, &surfaces, 4.5, dark);
    let (accent, on_accent) = accent_pair(accent, accent_on);
    Colors {
        title,
        side,
        editor,
        elevated,
        hover,
        border,
        border_strong,
        fg: text(fg),
        strong: text(strong),
        muted: text(muted),
        dim: text(dim),
        accent,
        on_accent,
        accent_text: text(accent_text),
        blue: text(blue),
        green: text(green),
        warn: text(warn),
        danger: text(danger),
        folder: text(folder),
        select,
        selection,
    }
}

fn accent_pair(accent: u32, on: Option<u32>) -> (u32, u32) {
    if let Some(on) = on.filter(|on| contrast(*on, accent) >= 5.1) {
        return (accent, on);
    }
    let on = if contrast(0xffffff, accent) >= contrast(0x000000, accent) {
        0xffffff
    } else {
        0x000000
    };
    let away = if on == 0xffffff { 0x000000 } else { 0xffffff };
    let accent = (0..=20)
        .map(|i| mix(accent, away, i as f32 / 20.))
        .find(|a| contrast(on, *a) >= 5.1)
        .unwrap_or(away);
    (accent, on)
}

fn global_colors(rules: &[Value]) -> (Option<u32>, Option<u32>) {
    let mut out = (None, None);
    for r in rules.iter().filter(|r| r.get("scope").is_none()) {
        let s = r.get("settings");
        let col = |k: &str| {
            s.and_then(|s| s.get(k))
                .and_then(Value::as_str)
                .and_then(parse_color)
        };
        if let Some((c, _)) = col("foreground") {
            out.0 = Some(c);
        }
        if let Some((c, _)) = col("background") {
            out.1 = Some(c);
        }
    }
    out
}


const SCOPES: &[(&str, &[&str])] = &[
    (
        "comment",
        &["comment.line.double-slash", "comment.block", "comment"],
    ),
    ("comment.doc", &["comment.block.documentation"]),
    ("string", &["string.quoted.double", "string"]),
    ("string.escape", &["constant.character.escape"]),
    ("string.regex", &["string.regexp"]),
    ("number", &["constant.numeric.integer", "constant.numeric"]),
    ("boolean", &["constant.language.boolean"]),
    (
        "constant",
        &[
            "constant.language",
            "support.constant",
            "variable.other.constant",
            "constant.other",
        ],
    ),
    ("operator", &["keyword.operator"]),
    ("preproc", &["keyword.control.import", "meta.preprocessor"]),
    (
        "keyword",
        &[
            "keyword.control",
            "storage.type",
            "keyword",
            "storage.modifier",
        ],
    ),
    (
        "type",
        &[
            "entity.name.type",
            "support.type",
            "entity.name.class",
            "storage.type.primitive",
        ],
    ),
    (
        "function",
        &[
            "entity.name.function",
            "support.function",
            "meta.function-call",
        ],
    ),
    ("tag", &["entity.name.tag"]),
    ("attribute", &["entity.other.attribute-name"]),
    ("label", &["entity.name.label"]),
    ("title", &["markup.heading", "entity.name.section"]),
    ("variable.special", &["variable.language"]),
    (
        "property",
        &[
            "variable.other.property",
            "variable.other.object.property",
            "support.variable.property",
            "meta.object-literal.key",
        ],
    ),
    (
        "variable",
        &["variable.other.readwrite", "variable.other", "variable"],
    ),
    ("enum", &["entity.name.type.enum"]),
    ("variant", &["variable.other.enummember"]),
    ("emphasis.strong", &["markup.bold"]),
    ("emphasis", &["markup.italic"]),
    ("link_uri", &["markup.underline.link"]),
    ("link_text", &["string.other.link"]),
    ("text.literal", &["markup.inline.raw", "markup.raw"]),
    (
        "punctuation.list_marker",
        &["punctuation.definition.list.begin.markdown", "markup.list"],
    ),
    (
        "punctuation.bracket",
        &["punctuation.section.block", "meta.brace"],
    ),
    (
        "punctuation.delimiter",
        &["punctuation.separator", "punctuation.terminator"],
    ),
    ("punctuation", &["punctuation"]),
];

fn selectors(rule: &Value) -> Vec<String> {
    let raw: Vec<&str> = match rule.get("scope") {
        Some(Value::String(s)) => s.split(',').collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .flat_map(|s| s.split(','))
            .collect(),
        _ => Vec::new(),
    };
    raw.into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.contains(' ') && !s.starts_with('-'))
        .map(String::from)
        .collect()
}

fn score(selector: &str, scope: &str) -> usize {
    let hit = scope == selector
        || scope
            .strip_prefix(selector)
            .is_some_and(|rest| rest.starts_with('.'));
    if hit {
        selector.split('.').count()
    } else {
        0
    }
}

pub fn resolve(rules: &[Value], scope: &str) -> (Option<(u32, f32)>, Option<String>) {
    let mut fg: (usize, Option<(u32, f32)>) = (0, None);
    let mut style: (usize, Option<String>) = (0, None);
    for rule in rules {
        let Some(settings) = rule.get("settings") else {
            continue;
        };
        let best = selectors(rule)
            .iter()
            .map(|s| score(s, scope))
            .max()
            .unwrap_or(0);
        if best == 0 {
            continue;
        }
        if let Some(c) = settings
            .get("foreground")
            .and_then(Value::as_str)
            .and_then(parse_color)
        {
            if best >= fg.0 {
                fg = (best, Some(c));
            }
        }
        if let Some(s) = settings.get("fontStyle").and_then(Value::as_str) {
            if best >= style.0 {
                style = (best, Some(s.to_string()));
            }
        }
    }
    (fg.1, style.1)
}

fn hex(c: u32) -> String {
    format!("#{c:06x}")
}

pub fn syntax(file: &ThemeFile, editor: u32, fg: u32) -> Option<HighlightThemeStyle> {
    let mut syntax = Map::new();
    for (name, scopes) in SCOPES {
        let found = scopes.iter().find_map(|s| match resolve(&file.rules, s) {
            (Some(c), st) => Some((c, st)),
            _ => None,
        });
        let Some(((c, a), st)) = found else {
            continue;
        };
        let color = mix(editor, c, a);
        if color == fg && st.as_deref().unwrap_or("").is_empty() {
            continue;
        }
        let mut s = json!({ "color": hex(color) });
        let st = st.unwrap_or_default();
        if st.contains("italic") {
            s["font_style"] = json!("italic");
        } else if st.contains("underline") {
            s["font_style"] = json!("underline");
        }
        if st.contains("bold") {
            s["font_weight"] = json!(700);
        }
        syntax.insert(name.to_string(), s);
    }
    let k = Keys(&file.colors);
    let mut style = json!({ "syntax": syntax });
    let status: [(&str, &[&str]); 8] = [
        ("error", &["editorError.foreground", "errorForeground"]),
        ("warning", &["editorWarning.foreground"]),
        ("info", &["editorInfo.foreground"]),
        ("hint", &["editorHint.foreground"]),
        ("success", &["terminal.ansiGreen"]),
        (
            "created",
            &[
                "editorGutter.addedBackground",
                "gitDecoration.addedResourceForeground",
            ],
        ),
        (
            "modified",
            &[
                "editorGutter.modifiedBackground",
                "gitDecoration.modifiedResourceForeground",
            ],
        ),
        (
            "deleted",
            &[
                "editorGutter.deletedBackground",
                "gitDecoration.deletedResourceForeground",
            ],
        ),
    ];
    for (name, keys) in status {
        if let Some(c) = k.get(keys, editor) {
            style[name] = json!(hex(c));
        }
    }
    serde_json::from_value(style).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> ThemeFile {
        let v = parse_jsonc(text).unwrap();
        ThemeFile {
            colors: v
                .get("colors")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            rules: v
                .get("tokenColors")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            kind: v.get("type").and_then(Value::as_str).map(String::from),
        }
    }

    const DRACULA: &str = r##"{
        "type": "dark",
        "colors": {
            "editor.background": "#282A36",
            "editor.foreground": "#F8F8F2",
            "sideBar.background": "#21222C",
            "titleBar.activeBackground": "#21222C",
            "list.hoverBackground": "#44475A75",
            "list.activeSelectionBackground": "#44475A",
            "editorLineNumber.foreground": "#6272A4",
            "editor.selectionBackground": "#44475A",
            "button.background": "#44475A",
            "button.foreground": "#F8F8F2",
            "activityBarBadge.background": "#FF79C6",
            "activityBarBadge.foreground": "#F8F8F2",
            "textLink.foreground": "#8BE9FD",
            "errorForeground": "#FF5555",
            "terminal.ansiGreen": "#50FA7B",
            "terminal.ansiYellow": "#F1FA8C",
            "editorGutter.addedBackground": "#50FA7B80",
        },
        "tokenColors": [
            { "scope": ["comment", "punctuation.definition.comment"], "settings": { "foreground": "#6272A4" } },
            { "scope": "string", "settings": { "foreground": "#F1FA8C" } },
            { "scope": "keyword, storage.type, storage.modifier", "settings": { "foreground": "#FF79C6" } },
            { "scope": "keyword.operator", "settings": { "foreground": "#FF79C6" } },
            { "scope": ["entity.name.function", "support.function"], "settings": { "foreground": "#50FA7B" } },
            { "scope": "constant.numeric", "settings": { "foreground": "#BD93F9" } },
            { "scope": "entity.name.type, support.type", "settings": { "fontStyle": "italic", "foreground": "#8BE9FD" } },
            { "scope": "source.python keyword", "settings": { "foreground": "#000000" } },
            { "scope": "variable.parameter", "settings": { "fontStyle": "italic", "foreground": "#FFB86C" } },
        ],
    }"##;

    #[test]
    fn reads_json_with_comments_and_trailing_commas() {
        let v = parse_jsonc("\u{feff}{ \"a\": [1, 2,], // x\n \"b\": \"//, ]\", }").unwrap();
        assert_eq!(v, json!({ "a": [1, 2], "b": "//, ]" }));
    }

    #[test]
    fn parses_hex_colors_with_alpha() {
        assert_eq!(parse_color("#282A36"), Some((0x282a36, 1.)));
        assert_eq!(parse_color("#fff"), Some((0xffffff, 1.)));
        let (c, a) = parse_color("#44475A80").unwrap();
        assert_eq!(c, 0x44475a);
        assert!((a - 0.502).abs() < 0.01);
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#12345"), None);
    }

    #[test]
    fn dracula_becomes_a_dark_palette_with_its_own_surfaces() {
        let f = file(DRACULA);
        assert!(is_dark("vs-dark", &f));
        assert!(is_dark("", &f));
        let c = colors(&f, true);
        assert_eq!(c.editor, 0x282a36);
        assert_eq!(c.side, 0x21222c);
        assert_eq!(c.title, 0x21222c);
        assert_eq!(c.fg, 0xf8f8f2);
        assert_eq!(c.select, 0x44475a);
        assert_eq!(c.hover, mix(0x21222c, 0x44475a, 0x75 as f32 / 255.));
        assert_eq!(c.selection, 0x44475a);
        assert_eq!(c.accent_text, 0x8be9fd);
        assert_eq!((c.accent, c.on_accent), (0xff79c6, 0x000000));
    }

    #[test]
    fn a_grey_only_theme_keeps_its_grey_accent() {
        let f = file(
            r##"{ "colors": { "editor.background": "#1e1e1e", "button.background": "#5a5a5a", "button.foreground": "#ffffff" } }"##,
        );
        let c = colors(&f, true);
        assert_eq!(saturation(c.accent), 0.);
        assert!(contrast(c.on_accent, c.accent) >= 5.1);
    }

    #[test]
    fn text_colors_are_lifted_to_readable_contrast() {
        let c = colors(&file(DRACULA), true);
        assert!(contrast(0x6272a4, c.editor) < 4.5);
        let surfaces = [c.title, c.side, c.editor, c.elevated, c.hover, c.select];
        for t in [
            c.fg,
            c.strong,
            c.muted,
            c.dim,
            c.accent_text,
            c.blue,
            c.green,
            c.warn,
            c.danger,
            c.folder,
        ] {
            for s in surfaces {
                assert!(contrast(t, s) >= 4.5, "{t:06x} on {s:06x}");
            }
        }
        assert!(contrast(c.on_accent, c.accent) >= 5.1);
    }

    #[test]
    fn a_light_theme_without_colors_gets_defaults() {
        let f = file(r#"{ "type": "light", "tokenColors": [] }"#);
        assert!(!is_dark("", &f));
        let c = colors(&f, false);
        assert_eq!(c.editor, 0xffffff);
        assert!(contrast(c.fg, c.editor) >= 4.5);
        assert!(contrast(c.on_accent, c.accent) >= 5.1);
        assert_ne!(c.border, c.side);
    }

    #[test]
    fn scope_rules_resolve_like_vs_code() {
        let f = file(DRACULA);
        assert_eq!(resolve(&f.rules, "keyword.control").0, Some((0xff79c6, 1.)));
        assert_eq!(
            resolve(&f.rules, "keyword.operator").0,
            Some((0xff79c6, 1.))
        );
        assert_eq!(
            resolve(&f.rules, "entity.name.type").1.as_deref(),
            Some("italic")
        );
        assert_eq!(resolve(&f.rules, "keywords").0, None);
        let rules = vec![
            json!({ "scope": "string", "settings": { "foreground": "#111111" } }),
            json!({ "scope": "string", "settings": { "foreground": "#222222" } }),
            json!({ "scope": "string.quoted", "settings": { "fontStyle": "bold" } }),
        ];
        assert_eq!(
            resolve(&rules, "string.quoted.double"),
            (Some((0x222222, 1.)), Some("bold".into()))
        );
    }

    #[test]
    fn code_colors_map_onto_highlight_names() {
        let f = file(DRACULA);
        let st = syntax(&f, 0x282a36, 0xf8f8f2).expect("style");
        let color = |name: &str| st.syntax.style(name).and_then(|s| s.color);
        let want = |hex: u32| Some(gpui_kit::Hsla::from(gpui_kit::rgb(hex)));
        assert_eq!(color("keyword"), want(0xff79c6));
        assert_eq!(color("string"), want(0xf1fa8c));
        assert_eq!(color("comment"), want(0x6272a4));
        assert_eq!(color("function"), want(0x50fa7b));
        assert_eq!(color("number"), want(0xbd93f9));
        assert_eq!(color("type"), want(0x8be9fd));
        let ty = st.syntax.style("type").unwrap();
        assert_eq!(ty.font_style, Some(gpui_kit::FontStyle::Italic));
        assert_eq!(color("tag"), None);
    }

    #[test]
    fn includes_merge_base_first() {
        let dir = std::env::temp_dir().join(format!("fluxcode-vsx-theme-{}", std::process::id()));
        let themes = dir.join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::write(
            themes.join("base.json"),
            r##"{ "colors": { "editor.background": "#101010", "editor.foreground": "#eeeeee" },
                 "tokenColors": [{ "scope": "string", "settings": { "foreground": "#00ff00" } }] }"##,
        )
        .unwrap();
        std::fs::write(
            themes.join("top.json"),
            r##"{ "include": "./base.json", "colors": { "editor.background": "#202020" },
                 "tokenColors": [{ "scope": "string", "settings": { "foreground": "#ff0000" } }] }"##,
        )
        .unwrap();
        std::fs::write(
            themes.join("escape.json"),
            r#"{ "include": "../../outside.json" }"#,
        )
        .unwrap();
        let f = read_theme(&dir, &themes.join("top.json")).unwrap();
        assert_eq!(f.colors["editor.background"], "#202020");
        assert_eq!(f.colors["editor.foreground"], "#eeeeee");
        assert_eq!(resolve(&f.rules, "string").0, Some((0xff0000, 1.)));
        assert!(read_theme(&dir, &themes.join("escape.json"))
            .unwrap()
            .colors
            .is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
