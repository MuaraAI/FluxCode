//! Completion for the GPUI editor: words from the document, language
//! keywords, and Emmet (HTML + CSS abbreviations), ported from `src/complete/`.
//! gpui-component's editor has no snippet tab stops, so Emmet output is
//! inserted as plain text.
//!
//! Inline (ghost-text) completions come from GitHub Copilot through
//! `crate::copilot`: the editor asks for one a short time after typing
//! stops, shows it dimmed after the cursor, and Tab accepts it. The local
//! menu above is untouched; Copilot never appears in it.

use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, EditorState, Rope, RopeExt};
use gpui_kit::{App, AppContext, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, InlineCompletionContext, InlineCompletionResponse, InlineCompletionTriggerKind,
    InsertReplaceEdit,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

pub fn attach(
    editor: &Entity<EditorState>,
    lang: &'static str,
    file_name: &str,
    path: &PathBuf,
    tab_size: usize,
    copilot: crate::copilot::Copilot,
    cx: &mut App,
) {
    if lang == "text" {
        return;
    }
    let file = file_name.to_ascii_lowercase();
    let language_id = crate::copilot::language_id(lang);
    editor.update(cx, |state, cx| {
        state.lsp_mut().completion_provider = Some(Rc::new(Words {
            lang,
            file,
            path: path.clone(),
            language_id,
            tab_size,
            copilot,
        }));
        cx.notify();
    });
}

struct Words {
    lang: &'static str,
    /// Lower-cased file name: picks the JSON schema (manifest.json, package.json, ...).
    file: String,
    /// Full path, for the Copilot server's document sync.
    path: PathBuf,
    language_id: String,
    tab_size: usize,
    copilot: crate::copilot::Copilot,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The slice the word-completion scan reads: 64 KB either side of the cursor,
/// with edges moved to the nearest char boundary.
fn word_scan_window(text: &str, offset: usize) -> (usize, usize) {
    let mut start = offset.saturating_sub(64 * 1024);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (offset + 64 * 1024).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    (start, end)
}

fn is_html(lang: &str) -> bool {
    matches!(lang, "html" | "php" | "markdown")
}

impl CompletionProvider for Words {
    /// Copilot ghost text. The editor calls this after a short debounce
    /// when typing stops, shows the first item dimmed after the cursor,
    /// and accepts it on Tab. The request runs on a background thread;
    /// when Copilot is off, unsigned, or the server is missing, this
    /// quietly returns nothing and the editor behaves as before.
    fn inline_completion(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: InlineCompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<InlineCompletionResponse>> {
        let copilot = self.copilot.clone();
        let path = self.path.clone();
        let language_id = self.language_id.clone();
        let tab_size = self.tab_size;
        let rope = rope.clone();
        let trigger_kind = if trigger.trigger_kind == InlineCompletionTriggerKind::Invoked {
            1
        } else {
            2
        };
        cx.background_spawn(async move {
            // The whole-document copy runs here, on the background
            // executor, so typing never waits on it.
            let text = rope.to_string();
            let item = copilot.inline(&path, &language_id, &text, offset, tab_size, trigger_kind);
            Ok(InlineCompletionResponse::Array(item.into_iter().collect()))
        })
    }

    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let lang = self.lang;
        let file = self.file.clone();
        let start_pos = rope.offset_to_position(offset);
        let rope = rope.clone();
        cx.background_spawn(async move {
            // The whole-document copy runs here, on the background
            // executor, so typing never waits on it.
            let text = rope.to_string();
            let before = &text[..offset.min(text.len())];
            let line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
            let mut items = Vec::new();

            // JSON: keys and values that fit where the cursor is (manifest.json,
            // package.json, tsconfig.json, composer.json). These come first.
            if lang == "json" {
                if let Some(pos) = json_position(before) {
                    // A closing quote right after the cursor (auto-paired) is replaced too.
                    let after_quote = text[offset..].starts_with('"');
                    let end = if after_quote { rope.offset_to_position(offset + 1) } else { start_pos };
                    let (prefix, list, is_key) = match &pos {
                        JsonPos::Key { path, prefix } => (prefix.as_str(), json_keys(&file, path), true),
                        JsonPos::Value { path, key, prefix } => (prefix.as_str(), json_values(&file, path, key), false),
                    };
                    let start = rope.offset_to_position(offset - prefix.len());
                    let lower = prefix.to_lowercase();
                    for k in list.iter().filter(|k| k.to_lowercase().starts_with(&lower) && **k != prefix) {
                        let insert = if is_key { format!("{k}\": ") } else { format!("{k}\"") };
                        let mut it = item(k, &insert, start, start_pos, CompletionItemKind::PROPERTY, if is_key { "key" } else { "value" }, "0");
                        it.text_edit = Some(CompletionTextEdit::InsertAndReplace(InsertReplaceEdit {
                            new_text: insert,
                            insert: lsp_types::Range::new(start, end),
                            replace: lsp_types::Range::new(start, end),
                        }));
                        items.push(it);
                    }
                    if !items.is_empty() {
                        return Ok(CompletionResponse::Array(items));
                    }
                }
            }

            // Emmet: the abbreviation runs back to the last whitespace.
            if is_html(lang) || lang == "css" {
                let abbr = &line[line.rfind(char::is_whitespace).map_or(0, |i| i + 1)..];
                let expansion = if lang == "css" { css_emmet(abbr) } else { html_emmet(abbr, indent_of(line)) };
                if let Some(exp) = expansion {
                    let start = rope.offset_to_position(offset - abbr.len());
                    items.push(item(abbr, &exp, start, start_pos, CompletionItemKind::SNIPPET, "Emmet abbreviation", "0"));
                }
            }

            let prefix_len = line.chars().rev().take_while(|c| is_word(*c) || (lang == "css" && *c == '-')).map(char::len_utf8).sum::<usize>();
            let prefix = &line[line.len() - prefix_len..];
            if prefix.chars().count() >= 1 && !prefix.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                let start = rope.offset_to_position(offset - prefix_len);
                let lower = prefix.to_lowercase();
                let mut seen = BTreeSet::new();
                for kw in keywords(lang) {
                    if kw.len() > prefix.len() && kw.to_lowercase().starts_with(&lower) && seen.insert(kw.to_string()) {
                        items.push(item(kw, kw, start, start_pos, CompletionItemKind::KEYWORD, "", "1"));
                    }
                }
                if prefix.chars().count() >= 2 {
                    // Words only from near the cursor: scanning the whole
                    // document on every keystroke made completion O(file).
                    let (scan_start, scan_end) = word_scan_window(&text, offset);
                    let mut words = BTreeSet::new();
                    for w in text[scan_start..scan_end]
                        .split(|c: char| !(is_word(c) || (lang == "css" && c == '-')))
                    {
                        if w.len() > prefix.len() && w.len() < 60 && w.to_lowercase().starts_with(&lower) && !w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                            words.insert(w);
                        }
                        if words.len() > 200 {
                            break;
                        }
                    }
                    for w in words {
                        if seen.insert(w.to_string()) {
                            items.push(item(w, w, start, start_pos, CompletionItemKind::TEXT, "", "2"));
                        }
                    }
                }
            }
            items.truncate(50);
            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        let Some(c) = new_text.chars().last() else { return false };
        new_text.chars().count() == 1
            && (is_word(c)
                || (self.lang == "json" && matches!(c, '"' | '-'))
                || (is_html(self.lang) && matches!(c, '!' | '>' | '.' | '#' | '*' | '+' | '}')))
    }
}

// ------------------------------------------------------------------ JSON

/// Where the cursor is in a JSON document: typing an object key, or a string value.
#[derive(Debug, PartialEq)]
enum JsonPos {
    /// `path` is the chain of keys to the object being written ("content_scripts[]"
    /// for an object inside the `content_scripts` array); empty at the top level.
    Key { path: Vec<String>, prefix: String },
    Value { path: Vec<String>, key: String, prefix: String },
}

/// Scan the text before the cursor once and work out the cursor's position.
/// Only an open string counts: JSON keys and the values worth suggesting are strings.
fn json_position(before: &str) -> Option<JsonPos> {
    struct Frame {
        obj: bool,
        /// The key this container is the value of (None inside an array).
        key: Option<String>,
    }
    let mut stack: Vec<Frame> = Vec::new();
    let (mut in_str, mut esc) = (false, false);
    let mut cur = String::new();
    let mut expect_key = false; // in an object, after `{` or `,`
    let mut last_key: Option<String> = None; // the key whose value follows `:`
    let mut pending: Option<String> = None; // a string that may become a key at `:`
    let mut str_is_key = false;
    for ch in before.chars() {
        if in_str {
            if esc {
                esc = false;
                cur.push(ch);
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
                if str_is_key {
                    pending = Some(std::mem::take(&mut cur));
                }
                cur.clear();
            } else {
                cur.push(ch);
            }
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                str_is_key = stack.last().is_some_and(|f| f.obj) && expect_key;
                cur.clear();
            }
            '{' | '[' => {
                let key = if stack.last().is_some_and(|f| f.obj) { last_key.take() } else { None };
                stack.push(Frame { obj: ch == '{', key });
                expect_key = ch == '{';
                last_key = None;
            }
            '}' | ']' => {
                stack.pop();
                expect_key = false;
                last_key = None;
            }
            ':' => {
                last_key = pending.take();
                expect_key = false;
            }
            ',' => {
                expect_key = stack.last().is_some_and(|f| f.obj);
                last_key = None;
            }
            _ => {}
        }
    }
    if !in_str || stack.is_empty() {
        return None;
    }
    // Path: object keys, with arrays marked "[]".
    let mut path = Vec::new();
    for (i, f) in stack.iter().enumerate() {
        if i == 0 {
            continue;
        }
        match (&f.key, f.obj) {
            (Some(k), true) => path.push(k.clone()),
            (Some(k), false) => path.push(format!("{k}[]")),
            (None, _) => {}
        }
    }
    let top = stack.last()?;
    if top.obj && str_is_key {
        Some(JsonPos::Key { path, prefix: cur })
    } else if top.obj {
        Some(JsonPos::Value { path, key: last_key?, prefix: cur })
    } else {
        // A string inside an array: its values are the array key's values.
        let key = top.key.clone()?;
        path.pop();
        Some(JsonPos::Value { path, key, prefix: cur })
    }
}

/// Does `path` match `pattern` (dot-separated, `*` matches any one key)?
fn path_is(path: &[String], pattern: &str) -> bool {
    let parts: Vec<&str> = if pattern.is_empty() { Vec::new() } else { pattern.split('.').collect() };
    parts.len() == path.len() && parts.iter().zip(path).all(|(p, k)| *p == "*" || p == k)
}

const MANIFEST_KEYS: &[&str] = &[
    "manifest_version", "name", "version", "description", "icons", "action", "background", "content_scripts",
    "permissions", "host_permissions", "optional_permissions", "optional_host_permissions", "commands",
    "options_page", "options_ui", "side_panel", "web_accessible_resources", "content_security_policy",
    "default_locale", "homepage_url", "short_name", "author", "minimum_chrome_version", "devtools_page",
    "chrome_url_overrides", "omnibox", "externally_connectable", "declarative_net_request",
    "browser_specific_settings", "incognito", "offline_enabled", "storage",
];
const MANIFEST_PERMISSIONS: &[&str] = &[
    "storage", "tabs", "activeTab", "scripting", "contextMenus", "alarms", "notifications", "cookies", "webRequest",
    "declarativeNetRequest", "downloads", "history", "bookmarks", "identity", "sidePanel", "offscreen",
    "clipboardRead", "clipboardWrite", "unlimitedStorage", "webNavigation", "tabGroups", "idle", "commands",
];
const PACKAGE_KEYS: &[&str] = &[
    "name", "version", "description", "main", "module", "types", "type", "private", "license", "author", "scripts",
    "dependencies", "devDependencies", "peerDependencies", "optionalDependencies", "engines", "bin", "files",
    "repository", "keywords", "homepage", "bugs", "browserslist", "workspaces", "exports", "imports", "sideEffects",
    "packageManager", "publishConfig",
];
const TSCONFIG_OPTIONS: &[&str] = &[
    "target", "module", "moduleResolution", "lib", "jsx", "strict", "outDir", "rootDir", "baseUrl", "paths", "types",
    "esModuleInterop", "skipLibCheck", "declaration", "sourceMap", "noEmit", "allowJs", "checkJs", "resolveJsonModule",
    "isolatedModules", "forceConsistentCasingInFileNames", "noUnusedLocals", "noUnusedParameters",
    "noFallthroughCasesInSwitch", "allowImportingTsExtensions", "verbatimModuleSyntax", "incremental", "composite",
];

/// Keys that belong in the object at `path` of a known file.
fn json_keys(file: &str, path: &[String]) -> &'static [&'static str] {
    match file {
        "manifest.json" => match () {
            _ if path_is(path, "") => MANIFEST_KEYS,
            _ if path_is(path, "action") => &["default_popup", "default_title", "default_icon"],
            _ if path_is(path, "background") => &["service_worker", "type", "scripts"],
            _ if path_is(path, "content_scripts[]") => &["matches", "exclude_matches", "js", "css", "run_at", "all_frames", "match_about_blank", "world", "include_globs", "exclude_globs"],
            _ if path_is(path, "commands.*") => &["suggested_key", "description"],
            _ if path_is(path, "commands.*.suggested_key") => &["default", "windows", "mac", "linux", "chromeos"],
            _ if path_is(path, "web_accessible_resources[]") => &["resources", "matches", "extension_ids", "use_dynamic_url"],
            _ if path_is(path, "options_ui") => &["page", "open_in_tab"],
            _ if path_is(path, "side_panel") => &["default_path"],
            _ if path_is(path, "content_security_policy") => &["extension_pages", "sandbox"],
            _ => &[],
        },
        "package.json" => match () {
            _ if path_is(path, "") => PACKAGE_KEYS,
            _ if path_is(path, "scripts") => &["dev", "build", "start", "test", "lint", "format", "preview", "serve", "watch", "clean", "prepare", "postinstall"],
            _ if path_is(path, "engines") => &["node", "npm", "pnpm", "yarn"],
            _ if path_is(path, "repository") => &["type", "url", "directory"],
            _ => &[],
        },
        "tsconfig.json" | "jsconfig.json" => match () {
            _ if path_is(path, "") => &["compilerOptions", "include", "exclude", "files", "extends", "references"],
            _ if path_is(path, "compilerOptions") => TSCONFIG_OPTIONS,
            _ => &[],
        },
        "composer.json" => match () {
            _ if path_is(path, "") => &["name", "description", "type", "license", "require", "require-dev", "autoload", "autoload-dev", "scripts", "config", "extra", "minimum-stability", "prefer-stable", "keywords", "authors", "repositories"],
            _ if path_is(path, "autoload") || path_is(path, "autoload-dev") => &["psr-4", "classmap", "files"],
            _ => &[],
        },
        _ => &[],
    }
}

/// Values worth suggesting for `key` in the object at `path` (or the items of array `key`).
fn json_values(file: &str, path: &[String], key: &str) -> &'static [&'static str] {
    match (file, key) {
        ("manifest.json", "run_at") if path_is(path, "content_scripts[]") => &["document_idle", "document_start", "document_end"],
        ("manifest.json", "world") if path_is(path, "content_scripts[]") => &["ISOLATED", "MAIN"],
        ("manifest.json", "type") if path_is(path, "background") => &["module"],
        ("manifest.json", "permissions" | "optional_permissions") if path_is(path, "") => MANIFEST_PERMISSIONS,
        ("manifest.json", "host_permissions" | "matches") => &["<all_urls>", "https://*/*", "http://*/*", "*://*/*"],
        ("package.json", "type") if path_is(path, "") => &["module", "commonjs"],
        ("package.json", "license") if path_is(path, "") => &["MIT", "Apache-2.0", "GPL-3.0-or-later", "ISC", "BSD-3-Clause", "UNLICENSED"],
        ("tsconfig.json" | "jsconfig.json", "target") => &["ES2022", "ES2023", "ESNext", "ES2020", "ES2017", "ES5"],
        ("tsconfig.json" | "jsconfig.json", "module") => &["ESNext", "NodeNext", "CommonJS", "ES2022", "Preserve"],
        ("tsconfig.json" | "jsconfig.json", "moduleResolution") => &["Bundler", "NodeNext", "Node16", "Node10"],
        ("tsconfig.json" | "jsconfig.json", "jsx") => &["react-jsx", "preserve", "react", "react-jsxdev"],
        _ => &[],
    }
}

fn item(
    label: &str,
    insert: &str,
    start: lsp_types::Position,
    end: lsp_types::Position,
    kind: CompletionItemKind,
    doc: &str,
    sort: &str,
) -> CompletionItem {
    let range = lsp_types::Range::new(start, end);
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        sort_text: Some(format!("{sort}{label}")),
        detail: (!doc.is_empty()).then(|| doc.to_string()),
        documentation: (kind == CompletionItemKind::SNIPPET).then(|| Documentation::String(insert.to_string())),
        text_edit: Some(CompletionTextEdit::InsertAndReplace(InsertReplaceEdit { new_text: insert.to_string(), insert: range, replace: range })),
        ..Default::default()
    }
}

fn indent_of(line: &str) -> String {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

fn keywords(lang: &str) -> &'static [&'static str] {
    match lang {
        "javascript" | "typescript" | "tsx" => &[
            "async", "await", "break", "case", "catch", "class", "const", "constructor", "continue", "debugger", "default",
            "delete", "document", "else", "export", "extends", "false", "finally", "for", "function", "import", "instanceof",
            "interface", "let", "new", "null", "return", "static", "super", "switch", "this", "throw", "true", "try", "typeof",
            "undefined", "while", "yield", "console", "console.log", "JSON", "Promise", "Array", "Object", "String", "Number",
            "Boolean", "Map", "Set", "Math", "window", "addEventListener", "querySelector", "querySelectorAll",
            "getElementById", "setTimeout", "setInterval", "fetch", "require", "module", "exports", "type", "readonly",
        ],
        "python" => &[
            "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except",
            "False", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "None", "nonlocal", "not", "or",
            "pass", "raise", "return", "True", "try", "while", "with", "yield", "print", "range", "len", "input", "open",
            "enumerate", "isinstance", "self", "__init__", "__name__", "__main__", "dict", "list", "tuple", "set", "str", "int",
            "float", "bool", "super", "sorted", "zip", "map", "filter", "Exception", "ValueError", "TypeError", "KeyError",
        ],
        "rust" => &[
            "as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
            "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
            "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "String", "Vec",
            "Option", "Some", "None", "Result", "Ok", "Err", "Box", "Rc", "Arc", "println!", "format!", "vec!", "derive",
        ],
        "php" => &[
            "abstract", "array", "echo", "else", "elseif", "foreach", "function", "global", "include", "isset", "namespace",
            "private", "protected", "public", "require", "require_once", "return", "static", "class", "while", "$this",
            "count", "strlen", "explode", "implode", "json_encode", "json_decode", "array_map", "array_filter", "var_dump",
        ],
        "html" => &[
            "html", "head", "body", "title", "meta", "link", "script", "style", "div", "span", "section", "article", "header",
            "footer", "nav", "main", "aside", "button", "input", "form", "label", "select", "option", "textarea", "table",
            "thead", "tbody", "tr", "td", "th", "ul", "ol", "li", "img", "a", "p", "h1", "h2", "h3", "h4", "h5", "h6",
            "class", "id", "href", "src", "alt", "type", "value", "placeholder",
        ],
        "css" => &[
            "display", "flex", "grid", "position", "absolute", "relative", "fixed", "sticky", "margin", "padding", "width",
            "height", "color", "background", "background-color", "border", "border-radius", "font-size", "font-weight",
            "font-family", "justify-content", "align-items", "gap", "transition", "transform", "opacity", "overflow",
            "z-index", "box-shadow", "line-height", "text-align", "cursor", "none", "block", "inline-block", "center",
            "space-between", "max-width", "min-height", "important",
        ],
        "json" => &["true", "false", "null"],
        _ => &[],
    }
}

// ------------------------------------------------------------------ HTML Emmet

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const TAGS: &[&str] = &[
    "a", "abbr", "address", "article", "aside", "audio", "b", "blockquote", "body", "br", "button", "canvas", "caption",
    "code", "col", "dd", "details", "dialog", "div", "dl", "dt", "em", "embed", "fieldset", "figcaption", "figure", "footer",
    "form", "h1", "h2", "h3", "h4", "h5", "h6", "head", "header", "hr", "html", "i", "iframe", "img", "input", "label",
    "legend", "li", "link", "main", "mark", "meta", "nav", "ol", "option", "p", "pre", "section", "select", "small",
    "source", "span", "strong", "style", "sub", "summary", "sup", "svg", "table", "tbody", "td", "template", "textarea",
    "tfoot", "th", "thead", "title", "tr", "u", "ul", "video", "script",
];

fn child_of(parent: &str) -> &'static str {
    match parent {
        "ul" | "ol" => "li",
        "table" | "tbody" | "thead" | "tfoot" => "tr",
        "tr" => "td",
        "select" | "optgroup" | "datalist" => "option",
        "dl" => "dt",
        _ => "div",
    }
}

fn default_attrs(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        "a" => &[("href", "")],
        "img" => &[("src", ""), ("alt", "")],
        "link" => &[("rel", "stylesheet"), ("href", "")],
        "input" => &[("type", "text")],
        "label" => &[("for", "")],
        "form" => &[("action", "")],
        "iframe" => &[("src", "")],
        "textarea" => &[("name", ""), ("id", ""), ("cols", "30"), ("rows", "10")],
        "option" => &[("value", "")],
        _ => &[],
    }
}

#[derive(Default, Clone)]
struct El {
    tag: String,
    id: String,
    classes: Vec<String>,
    attrs: Vec<(String, String)>,
    text: String,
    count: usize,
    children: Vec<El>,
}

const BOILERPLATE: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\t<meta charset=\"UTF-8\">\n\t<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n\t<title>Document</title>\n</head>\n<body>\n\t\n</body>\n</html>";

fn html_emmet(abbr: &str, indent: String) -> Option<String> {
    if abbr == "!" || abbr == "html:5" {
        return Some(BOILERPLATE.replace('\n', &format!("\n{indent}")));
    }
    if abbr.is_empty() || abbr.starts_with(['<', '/', '>', '+', '*', '^']) || abbr.contains("..") {
        return None;
    }
    // A bare word must be a real tag, so normal words are not hijacked.
    let bare = abbr.chars().all(|c| c.is_ascii_alphanumeric());
    if bare && !TAGS.contains(&abbr) {
        return None;
    }
    let roots = parse(abbr)?;
    let mut out = String::new();
    for (i, el) in roots.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(&indent);
        }
        render(el, "", &indent, &mut out, true);
    }
    Some(out)
}

/// Parse `a>b+c^d`, `*n`, `#id`, `.cls`, `[attr=v]`, `{text}`. Groups `()` are not supported.
fn parse(abbr: &str) -> Option<Vec<El>> {
    let chars: Vec<char> = abbr.chars().collect();
    let mut i = 0;
    // Stack of sibling lists; each level's last element is the parent of the next level.
    let mut stack: Vec<Vec<El>> = vec![Vec::new()];
    let mut parent_tags: Vec<String> = vec![String::new()];
    loop {
        let mut el = El { count: 1, ..Default::default() };
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == ':' || chars[i] == '-') {
            i += 1;
        }
        el.tag = chars[start..i].iter().collect();
        loop {
            match chars.get(i) {
                Some('#') | Some('.') => {
                    let kind = chars[i];
                    i += 1;
                    let s = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || matches!(chars[i], '-' | '_' | '$')) {
                        i += 1;
                    }
                    let v: String = chars[s..i].iter().collect();
                    if v.is_empty() {
                        return None;
                    }
                    if kind == '#' { el.id = v } else { el.classes.push(v) }
                }
                Some('[') => {
                    let end = chars[i..].iter().position(|&c| c == ']')? + i;
                    let body: String = chars[i + 1..end].iter().collect();
                    for part in body.split_whitespace() {
                        let (k, v) = part.split_once('=').unwrap_or((part, ""));
                        el.attrs.push((k.to_string(), v.trim_matches(['"', '\'']).to_string()));
                    }
                    i = end + 1;
                }
                Some('{') => {
                    let end = chars[i..].iter().position(|&c| c == '}')? + i;
                    el.text = chars[i + 1..end].iter().collect();
                    i = end + 1;
                }
                Some('*') => {
                    i += 1;
                    let s = i;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    el.count = chars[s..i].iter().collect::<String>().parse().ok().filter(|n| (1..=100).contains(n))?;
                }
                _ => break,
            }
        }
        if el.tag.is_empty() {
            if el.id.is_empty() && el.classes.is_empty() && el.text.is_empty() {
                return None;
            }
            el.tag = child_of(parent_tags.last().map(String::as_str).unwrap_or("")).to_string();
        }
        if el.tag.contains(':') {
            let (t, ty) = el.tag.split_once(':').map(|(a, b)| (a.to_string(), b.to_string()))?;
            if t != "input" {
                return None;
            }
            el.tag = t;
            el.attrs.push(("type".into(), ty));
        }
        stack.last_mut()?.push(el);
        match chars.get(i) {
            None => break,
            Some('+') => i += 1,
            Some('>') => {
                i += 1;
                let tag = stack.last()?.last()?.tag.clone();
                parent_tags.push(tag);
                stack.push(Vec::new());
            }
            Some('^') => {
                while chars.get(i) == Some(&'^') {
                    i += 1;
                    if stack.len() > 1 {
                        let kids = stack.pop()?;
                        parent_tags.pop();
                        stack.last_mut()?.last_mut()?.children.extend(kids);
                    }
                }
            }
            _ => return None,
        }
    }
    while stack.len() > 1 {
        let kids = stack.pop()?;
        stack.last_mut()?.last_mut()?.children.extend(kids);
    }
    stack.pop()
}

fn render(el: &El, level: &str, base: &str, out: &mut String, first: bool) {
    for n in 1..=el.count {
        if !(first && n == 1) {
            out.push('\n');
            out.push_str(base);
            out.push_str(level);
        }
        let num = |s: &str| s.replace('$', &n.to_string());
        out.push('<');
        out.push_str(&el.tag);
        if !el.id.is_empty() {
            out.push_str(&format!(" id=\"{}\"", num(&el.id)));
        }
        if !el.classes.is_empty() {
            out.push_str(&format!(" class=\"{}\"", num(&el.classes.join(" "))));
        }
        let mut attrs: Vec<(String, String)> = default_attrs(&el.tag).iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        for (k, v) in &el.attrs {
            match attrs.iter_mut().find(|(ak, _)| ak == k) {
                Some(a) => a.1 = v.clone(),
                None => attrs.push((k.clone(), v.clone())),
            }
        }
        for (k, v) in attrs {
            out.push_str(&format!(" {k}=\"{}\"", num(&v)));
        }
        out.push('>');
        if VOID.contains(&el.tag.as_str()) {
            continue;
        }
        out.push_str(&num(&el.text));
        if !el.children.is_empty() {
            let inner = format!("{level}\t");
            for c in &el.children {
                render(c, &inner, base, out, false);
            }
            out.push('\n');
            out.push_str(base);
            out.push_str(level);
        }
        out.push_str(&format!("</{}>", el.tag));
    }
}

// ------------------------------------------------------------------ CSS Emmet

fn css_emmet(abbr: &str) -> Option<String> {
    const FIXED: &[(&str, &str)] = &[
        ("df", "display: flex;"), ("dg", "display: grid;"), ("db", "display: block;"), ("dn", "display: none;"),
        ("dib", "display: inline-block;"), ("di", "display: inline;"), ("posa", "position: absolute;"),
        ("posr", "position: relative;"), ("posf", "position: fixed;"), ("poss", "position: sticky;"),
        ("jcc", "justify-content: center;"), ("jcsb", "justify-content: space-between;"), ("aic", "align-items: center;"),
        ("fdc", "flex-direction: column;"), ("fww", "flex-wrap: wrap;"), ("tac", "text-align: center;"),
        ("fwb", "font-weight: bold;"), ("cp", "cursor: pointer;"), ("ovh", "overflow: hidden;"), ("bxsz", "box-sizing: border-box;"),
        ("bdn", "border: none;"), ("tdn", "text-decoration: none;"), ("lsn", "list-style: none;"),
    ];
    if let Some((_, v)) = FIXED.iter().find(|(k, _)| *k == abbr) {
        return Some(v.to_string());
    }
    const PROPS: &[(&str, &str, &str)] = &[
        ("bgc", "background-color", ""), ("bdrs", "border-radius", "px"), ("fz", "font-size", "px"), ("lh", "line-height", ""),
        ("zi", "z-index", ""), ("op", "opacity", ""), ("gap", "gap", "px"), ("mt", "margin-top", "px"), ("mr", "margin-right", "px"),
        ("mb", "margin-bottom", "px"), ("ml", "margin-left", "px"), ("pt", "padding-top", "px"), ("pr", "padding-right", "px"),
        ("pb", "padding-bottom", "px"), ("pl", "padding-left", "px"), ("m", "margin", "px"), ("p", "padding", "px"),
        ("w", "width", "px"), ("h", "height", "px"), ("t", "top", "px"), ("r", "right", "px"), ("b", "bottom", "px"),
        ("l", "left", "px"), ("c", "color", ""),
    ];
    for (k, prop, unit) in PROPS {
        if let Some(rest) = abbr.strip_prefix(k) {
            if rest.is_empty() {
                continue;
            }
            let value = if let Some(hex) = rest.strip_prefix('#') {
                format!("#{hex}")
            } else if rest.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                let u = if rest == "0" { "" } else { unit };
                format!("{rest}{u}")
            } else if let Some(n) = rest.strip_suffix('p') {
                format!("{n}%")
            } else {
                continue;
            };
            return Some(format!("{prop}: {value};"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_scan_window_bounds() {
        let small = "fn main() { let hello = 1; }";
        assert_eq!(word_scan_window(small, 10), (0, small.len()));
        let big = "x".repeat(200 * 1024);
        assert_eq!(
            word_scan_window(&big, 100 * 1024),
            (100 * 1024 - 64 * 1024, 100 * 1024 + 64 * 1024)
        );
        // A multi-byte character on the edge is never split: the window
        // moves to the nearest boundary (start back, end forward). "é" is
        // 2 bytes, so an odd edge lands inside a character.
        let wide = "é".repeat(200 * 1024);
        assert_eq!(
            word_scan_window(&wide, 200 * 1024 + 1),
            (
                200 * 1024 + 1 - 64 * 1024 - 1,
                200 * 1024 + 1 + 64 * 1024 + 1
            )
        );
    }

    #[test]
    fn emmet_nesting_and_multiply() {
        assert_eq!(
            html_emmet("ul>li.item$*2", String::new()).unwrap(),
            "<ul>\n\t<li class=\"item1\"></li>\n\t<li class=\"item2\"></li>\n</ul>"
        );
        assert_eq!(html_emmet("div#main.card", String::new()).unwrap(), "<div id=\"main\" class=\"card\"></div>");
        assert_eq!(html_emmet("input:email", String::new()).unwrap(), "<input type=\"email\">");
        assert_eq!(html_emmet("p{hi}+a", String::new()).unwrap(), "<p>hi</p>\n<a href=\"\"></a>");
        assert!(html_emmet("hello", String::new()).is_none());
    }

    #[test]
    fn css_abbreviations() {
        assert_eq!(css_emmet("m10").unwrap(), "margin: 10px;");
        assert_eq!(css_emmet("bgc#fff").unwrap(), "background-color: #fff;");
        assert_eq!(css_emmet("df").unwrap(), "display: flex;");
        assert_eq!(css_emmet("w50p").unwrap(), "width: 50%;");
    }

    fn key(path: &[&str], prefix: &str) -> Option<JsonPos> {
        Some(JsonPos::Key { path: path.iter().map(|s| s.to_string()).collect(), prefix: prefix.into() })
    }

    #[test]
    fn json_context() {
        assert_eq!(json_position("{\n  \"name\": \"x\",\n  \"back"), key(&[], "back"));
        assert_eq!(json_position("{ \"background\": { \"serv"), key(&["background"], "serv"));
        assert_eq!(json_position("{ \"content_scripts\": [ { \"js\": [\"a.js\"], \"run"), key(&["content_scripts[]"], "run"));
        assert_eq!(
            json_position("{ \"content_scripts\": [ { \"run_at\": \"doc"),
            Some(JsonPos::Value { path: vec!["content_scripts[]".into()], key: "run_at".into(), prefix: "doc".into() })
        );
        assert_eq!(
            json_position("{ \"permissions\": [\"storage\", \"ta"),
            Some(JsonPos::Value { path: vec![], key: "permissions".into(), prefix: "ta".into() })
        );
        // Outside a string, or a string with an escaped quote: no JSON suggestion / still the key.
        assert_eq!(json_position("{ \"name\": 1"), None);
        assert_eq!(json_position("{ \"a\\\"b"), key(&[], "a\"b"));
        assert!(json_keys("manifest.json", &[]).contains(&"background"));
        assert!(json_values("manifest.json", &["content_scripts[]".into()], "run_at").contains(&"document_idle"));
    }
}
