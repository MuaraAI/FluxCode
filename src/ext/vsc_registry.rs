use super::market;
use gpui_kit::SharedString;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub const VSC_ICON_PREFIX: &str = "vsc-ext-icon/";
const MAX_SVG_BYTES: u64 = 256 * 1024;
const MAX_RASTER_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtIcon {
    SvgMonochrome(SharedString),
    SvgColor(SharedString),
    Raster(PathBuf),
    Codicon(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VscSvgIcon(pub SharedString);

impl gpui_kit::assets::IconNamed for VscSvgIcon {
    fn path(self) -> SharedString {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewContainerLocation {
    ActivityBar,
    Panel,
    SecondarySidebar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewContainerDesc {
    pub id: String,
    pub title: String,
    pub icon: ExtIcon,
    pub location: ViewContainerLocation,
    pub ext_id: String,
    pub ext_display: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewType {
    Tree,
    Webview,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewDesc {
    pub id: String,
    pub name: String,
    pub container_id: String,
    pub kind: ViewType,
    pub when: WhenExpr,
    pub icon: Option<ExtIcon>,
    pub contextual_title: Option<String>,
    pub ext_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewWelcomeLink {
    pub label: String,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewWelcomeDesc {
    pub view_id: String,
    pub text_lines: Vec<String>,
    pub links: Vec<ViewWelcomeLink>,
    pub when: WhenExpr,
    pub ext_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandDesc {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub icon: Option<ExtIcon>,
    pub enablement: WhenExpr,
    pub palette_when: WhenExpr,
    pub ext_id: String,
    pub ext_display: String,
}

impl CommandDesc {
    pub fn display_label(&self) -> String {
        match self.category.as_deref().filter(|c| !c.is_empty()) {
            Some(cat) => format!("{cat}: {}", self.title),
            None => self.title.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItemDesc {
    pub command_or_submenu: String,
    pub is_submenu: bool,
    pub when: WhenExpr,
    pub group: String,
    pub order: i32,
    pub alt: Option<String>,
    pub ext_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmenuDesc {
    pub id: String,
    pub label: String,
    pub icon: Option<ExtIcon>,
    pub ext_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingDesc {
    pub command: String,
    pub key: String,
    pub when: WhenExpr,
    pub ext_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomEditorDesc {
    pub view_type: String,
    pub display_name: String,
    pub patterns: Vec<String>,
    pub ext_id: String,
}

#[derive(Clone, Debug)]
pub struct ExtRuntimeDesc {
    pub ext_id: String,
    pub display: String,
    pub version: String,
    pub folder: PathBuf,
    pub main: Option<PathBuf>,
    #[allow(dead_code)]
    pub activation_events: Vec<String>,
    pub default_config: HashMap<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtSummary {
    pub activity_views: Vec<(String, String)>,
    pub views: usize,
    pub commands: usize,
    pub keybindings: usize,
    pub menus: usize,
    pub custom_editors: usize,
    pub has_main: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenOp {
    PushTrue,
    PushFalse,
    PushKey(String),
    Eq(String, String),
    Neq(String, String),
    Regex(String, String),
    Not,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenExpr(Vec<WhenOp>);

impl Default for WhenExpr {
    fn default() -> Self {
        Self::always_true()
    }
}

pub struct WhenContext<'a> {
    pub view: Option<&'a str>,
    pub focused_view: Option<&'a str>,
    pub editor_open: bool,
    pub editor_focus: bool,
    pub editor_has_selection: bool,
    pub editor_readonly: bool,
    pub terminal_focus: bool,
    pub resource_scheme: Option<&'a str>,
    pub resource_ext: Option<&'a str>,
    pub dynamic: &'a HashMap<String, Value>,
    pub config: &'a HashMap<String, Value>,
}

impl<'a> WhenContext<'a> {
    fn key_value(&self, key: &str) -> Option<String> {
        match key {
            "true" => return Some("true".into()),
            "false" => return Some("false".into()),
            "isWindows" => return Some(if cfg!(windows) { "true" } else { "false" }.into()),
            "isMac" => {
                return Some(
                    if cfg!(target_os = "macos") {
                        "true"
                    } else {
                        "false"
                    }
                    .into(),
                )
            }
            "isLinux" => {
                return Some(
                    if cfg!(target_os = "linux") {
                        "true"
                    } else {
                        "false"
                    }
                    .into(),
                )
            }
            "isWeb" => return Some("false".into()),
            "view" => return self.view.map(str::to_string),
            "focusedView" => return self.focused_view.map(str::to_string),
            "editorIsOpen" => return Some(bool_str(self.editor_open).into()),
            "editorTextFocus" => return Some(bool_str(self.editor_focus).into()),
            "editorHasSelection" => return Some(bool_str(self.editor_has_selection).into()),
            "editorReadonly" => return Some(bool_str(self.editor_readonly).into()),
            "terminalFocus" => return Some(bool_str(self.terminal_focus).into()),
            "resourceScheme" => return self.resource_scheme.map(str::to_string),
            "resourceExtname" => return self.resource_ext.map(str::to_string),
            _ => {}
        }
        if let Some(v) = self.dynamic.get(key) {
            return value_to_str(v);
        }
        if let Some(cfg_key) = key.strip_prefix("config.") {
            if let Some(v) = self.config.get(cfg_key) {
                return value_to_str(v);
            }
        }
        None
    }

    fn key_truthy(&self, key: &str) -> bool {
        match key {
            "true" => true,
            "false" => false,
            "isWindows" => cfg!(windows),
            "isMac" => cfg!(target_os = "macos"),
            "isLinux" => cfg!(target_os = "linux"),
            "isWeb" => false,
            "view" => self.view.is_some_and(|v| !v.is_empty()),
            "focusedView" => self.focused_view.is_some_and(|v| !v.is_empty()),
            "editorIsOpen" => self.editor_open,
            "editorTextFocus" => self.editor_focus,
            "editorHasSelection" => self.editor_has_selection,
            "editorReadonly" => self.editor_readonly,
            "terminalFocus" => self.terminal_focus,
            "resourceScheme" => self.resource_scheme.is_some_and(|s| !s.is_empty()),
            "resourceExtname" => self.resource_ext.is_some_and(|s| !s.is_empty()),
            _ => {
                if let Some(v) = self.dynamic.get(key) {
                    return value_truthy(v);
                }
                if let Some(cfg_key) = key.strip_prefix("config.") {
                    if let Some(v) = self.config.get(cfg_key) {
                        return value_truthy(v);
                    }
                }
                false
            }
        }
    }
}

fn bool_str(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

fn value_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty() && s != "false",
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn value_to_str(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Bool(b) => Some(bool_str(*b).into()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        Value::Array(_) | Value::Object(_) => Some(v.to_string()),
    }
}

impl WhenExpr {
    pub fn always_true() -> Self {
        Self(vec![WhenOp::PushTrue])
    }

    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self::always_true();
        }
        let tokens = tokenize_when(trimmed);
        if tokens.is_empty() {
            return Self::always_true();
        }
        let mut p = WhenParser {
            tokens: &tokens,
            pos: 0,
            ops: Vec::with_capacity(tokens.len()),
        };
        p.parse_or();
        if p.ops.is_empty() {
            Self::always_true()
        } else {
            Self(p.ops)
        }
    }

    pub fn eval(&self, ctx: &WhenContext<'_>) -> bool {
        let mut stack = [false; 64];
        let mut sp = 0usize;
        for op in &self.0 {
            match op {
                WhenOp::PushTrue => {
                    if sp < 64 {
                        stack[sp] = true;
                        sp += 1;
                    }
                }
                WhenOp::PushFalse => {
                    if sp < 64 {
                        stack[sp] = false;
                        sp += 1;
                    }
                }
                WhenOp::PushKey(k) => {
                    if sp < 64 {
                        stack[sp] = ctx.key_truthy(k);
                        sp += 1;
                    }
                }
                WhenOp::Eq(k, v) => {
                    if sp < 64 {
                        stack[sp] = ctx.key_value(k).as_deref() == Some(v.as_str());
                        sp += 1;
                    }
                }
                WhenOp::Neq(k, v) => {
                    if sp < 64 {
                        stack[sp] = ctx.key_value(k).as_deref() != Some(v.as_str());
                        sp += 1;
                    }
                }
                WhenOp::Regex(k, pat) => {
                    if sp < 64 {
                        stack[sp] = ctx
                            .key_value(k)
                            .is_some_and(|val| match_simple_regex(&val, pat));
                        sp += 1;
                    }
                }
                WhenOp::Not => {
                    if sp > 0 {
                        stack[sp - 1] = !stack[sp - 1];
                    }
                }
                WhenOp::And => {
                    if sp >= 2 {
                        sp -= 1;
                        stack[sp - 1] = stack[sp - 1] && stack[sp];
                    }
                }
                WhenOp::Or => {
                    if sp >= 2 {
                        sp -= 1;
                        stack[sp - 1] = stack[sp - 1] || stack[sp];
                    }
                }
            }
        }
        sp > 0 && stack[sp - 1]
    }
}

fn match_simple_regex(val: &str, pat: &str) -> bool {
    let mut body = pat.trim();
    if let Some(rest) = body.strip_prefix('/') {
        if let Some(end) = rest.rfind('/') {
            body = &rest[..end];
        }
    }
    let anch_start = body.starts_with('^');
    let anch_end = body.ends_with('$');
    let core = body.trim_start_matches('^').trim_end_matches('$');
    if core.is_empty() {
        return true;
    }
    for alt in core.split('|') {
        let hit = match (anch_start, anch_end) {
            (true, true) => val == alt,
            (true, false) => val.starts_with(alt),
            (false, true) => val.ends_with(alt),
            (false, false) => val.contains(alt),
        };
        if hit {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WhenTok {
    Or,
    And,
    Not,
    LParen,
    RParen,
    Eq,
    Neq,
    RegexMatch,
    Ident(String),
}

fn tokenize_when(s: &str) -> Vec<WhenTok> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"||") {
            out.push(WhenTok::Or);
            i += 2;
        } else if bytes[i..].starts_with(b"&&") {
            out.push(WhenTok::And);
            i += 2;
        } else if bytes[i..].starts_with(b"===") || bytes[i..].starts_with(b"==") {
            out.push(WhenTok::Eq);
            i += if bytes[i..].starts_with(b"===") { 3 } else { 2 };
        } else if bytes[i..].starts_with(b"!==") || bytes[i..].starts_with(b"!=") {
            out.push(WhenTok::Neq);
            i += if bytes[i..].starts_with(b"!==") { 3 } else { 2 };
        } else if bytes[i..].starts_with(b"=~") {
            out.push(WhenTok::RegexMatch);
            i += 2;
        } else if b == b'!' {
            out.push(WhenTok::Not);
            i += 1;
        } else if b == b'(' {
            out.push(WhenTok::LParen);
            i += 1;
        } else if b == b')' {
            out.push(WhenTok::RParen);
            i += 1;
        } else if b == b'\'' || b == b'"' {
            let q = b;
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != q {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let text = s[start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
            out.push(WhenTok::Ident(text));
        } else {
            let start = i;
            while i < bytes.len() {
                let c = bytes[i];
                if c.is_ascii_whitespace() || matches!(c, b'(' | b')' | b'&' | b'|' | b'!' | b'=') {
                    break;
                }
                i += 1;
            }
            out.push(WhenTok::Ident(s[start..i].to_string()));
        }
    }
    out
}

struct WhenParser<'a> {
    tokens: &'a [WhenTok],
    pos: usize,
    ops: Vec<WhenOp>,
}

impl<'a> WhenParser<'a> {
    fn peek(&self) -> Option<&WhenTok> {
        self.tokens.get(self.pos)
    }

    fn parse_or(&mut self) {
        self.parse_and();
        while matches!(self.peek(), Some(WhenTok::Or)) {
            self.pos += 1;
            self.parse_and();
            self.ops.push(WhenOp::Or);
        }
    }

    fn parse_and(&mut self) {
        self.parse_unary();
        while matches!(self.peek(), Some(WhenTok::And)) {
            self.pos += 1;
            self.parse_unary();
            self.ops.push(WhenOp::And);
        }
    }

    fn parse_unary(&mut self) {
        if matches!(self.peek(), Some(WhenTok::Not)) {
            self.pos += 1;
            self.parse_unary();
            self.ops.push(WhenOp::Not);
            return;
        }
        self.parse_primary();
    }

    fn parse_primary(&mut self) {
        match self.peek().cloned() {
            Some(WhenTok::LParen) => {
                self.pos += 1;
                self.parse_or();
                if matches!(self.peek(), Some(WhenTok::RParen)) {
                    self.pos += 1;
                }
            }
            Some(WhenTok::Ident(key)) => {
                self.pos += 1;
                match self.peek().cloned() {
                    Some(WhenTok::Eq) => {
                        self.pos += 1;
                        let val = match self.peek().cloned() {
                            Some(WhenTok::Ident(v)) => {
                                self.pos += 1;
                                v
                            }
                            _ => String::new(),
                        };
                        self.ops.push(WhenOp::Eq(key, val));
                    }
                    Some(WhenTok::Neq) => {
                        self.pos += 1;
                        let val = match self.peek().cloned() {
                            Some(WhenTok::Ident(v)) => {
                                self.pos += 1;
                                v
                            }
                            _ => String::new(),
                        };
                        self.ops.push(WhenOp::Neq(key, val));
                    }
                    Some(WhenTok::RegexMatch) => {
                        self.pos += 1;
                        let pat = match self.peek().cloned() {
                            Some(WhenTok::Ident(v)) => {
                                self.pos += 1;
                                v
                            }
                            _ => String::new(),
                        };
                        self.ops.push(WhenOp::Regex(key, pat));
                    }
                    _ => match key.as_str() {
                        "true" => self.ops.push(WhenOp::PushTrue),
                        "false" => self.ops.push(WhenOp::PushFalse),
                        _ => self.ops.push(WhenOp::PushKey(key)),
                    },
                }
            }
            Some(_) => {
                self.pos += 1;
            }
            None => {}
        }
    }
}

#[derive(Default)]
struct Registry {
    containers: Vec<ViewContainerDesc>,
    views: Vec<ViewDesc>,
    welcomes: Vec<ViewWelcomeDesc>,
    commands: Vec<CommandDesc>,
    view_title_menus: HashMap<String, Vec<MenuItemDesc>>,
    submenus: HashMap<String, SubmenuDesc>,
    submenu_items: HashMap<String, Vec<MenuItemDesc>>,
    keybindings: Vec<KeybindingDesc>,
    custom_editors: Vec<CustomEditorDesc>,
    runtimes: Vec<ExtRuntimeDesc>,
    view_to_ext: HashMap<String, String>,
    cmd_to_ext: HashMap<String, String>,
    summaries: HashMap<String, ExtSummary>,
}

static REGISTRY: RwLock<Option<Registry>> = RwLock::new(None);
static ICON_ASSETS: RwLock<Option<HashMap<String, Vec<u8>>>> = RwLock::new(None);

pub fn load_icon_asset(path: &str) -> Option<Vec<u8>> {
    if !path.starts_with(VSC_ICON_PREFIX) {
        return None;
    }
    ICON_ASSETS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(path).cloned())
}

pub fn load(installed: &[market::Installed]) {
    let (reg, assets) = build_registry(installed);
    *ICON_ASSETS.write().unwrap_or_else(|e| e.into_inner()) = Some(assets);
    *REGISTRY.write().unwrap_or_else(|e| e.into_inner()) = Some(reg);
}

fn build_registry(installed: &[market::Installed]) -> (Registry, HashMap<String, Vec<u8>>) {
    let mut reg = Registry::default();
    let mut assets: HashMap<String, Vec<u8>> = HashMap::new();
    let mut icon_seq = 0usize;

    for ext in installed {
        if ext.bundled || ext.folder.as_os_str().is_empty() {
            continue;
        }
        let Ok(root) = ext.folder.canonicalize() else {
            continue;
        };
        let Ok(pkg_str) = std::fs::read_to_string(root.join("package.json")) else {
            continue;
        };
        let Ok(pkg) = serde_json::from_str::<Value>(&pkg_str) else {
            continue;
        };
        let mut nls = None;
        let mut loc = |s: &str| -> String { market::localized(&root, s, &mut nls) };
        let mut summary = ExtSummary::default();
        let mut implicit_events: Vec<String> = Vec::new();

        let main = pkg
            .get("main")
            .and_then(Value::as_str)
            .and_then(|rel| resolve_safe_path(&root, rel))
            .filter(|p| p.is_file());
        summary.has_main = main.is_some();

        let contribs = pkg.get("contributes").unwrap_or(&Value::Null);

        let palette_rules = collect_palette_rules(contribs);
        let mut cmd_count = 0usize;
        if let Some(cmds) = contribs.get("commands").and_then(Value::as_array) {
            for c in cmds {
                let Some(id) = c.get("command").and_then(Value::as_str).map(str::trim) else {
                    continue;
                };
                if id.is_empty() {
                    continue;
                }
                let title = clean_loc(
                    &loc(c.get("title").and_then(Value::as_str).unwrap_or(id)),
                    id,
                );
                let category = c
                    .get("category")
                    .and_then(Value::as_str)
                    .map(&mut loc)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && !s.starts_with('%'));
                let icon = c
                    .get("icon")
                    .and_then(|v| resolve_ext_icon(&root, &ext.id, v, &mut icon_seq, &mut assets));
                let enablement = c
                    .get("enablement")
                    .and_then(Value::as_str)
                    .map(WhenExpr::parse)
                    .unwrap_or_default();
                let palette_when = palette_rules
                    .get(id)
                    .cloned()
                    .unwrap_or_else(WhenExpr::always_true);
                implicit_events.push(format!("onCommand:{id}"));
                reg.cmd_to_ext.insert(id.to_string(), ext.id.clone());
                reg.commands.push(CommandDesc {
                    id: id.to_string(),
                    title,
                    category,
                    icon,
                    enablement,
                    palette_when,
                    ext_id: ext.id.clone(),
                    ext_display: ext.display.clone(),
                });
                cmd_count += 1;
            }
        }
        summary.commands = cmd_count;

        let mut ext_container_ids: HashSet<String> = HashSet::new();
        if let Some(vc) = contribs.get("viewsContainers").and_then(Value::as_object) {
            for (loc_key, loc_kind) in [
                ("activitybar", ViewContainerLocation::ActivityBar),
                ("panel", ViewContainerLocation::Panel),
                ("secondarySidebar", ViewContainerLocation::SecondarySidebar),
            ] {
                if let Some(arr) = vc.get(loc_key).and_then(Value::as_array) {
                    for item in arr {
                        let Some(id) = item.get("id").and_then(Value::as_str).map(str::trim) else {
                            continue;
                        };
                        if id.is_empty() {
                            continue;
                        }
                        let title = clean_loc(
                            &loc(item.get("title").and_then(Value::as_str).unwrap_or(id)),
                            id,
                        );
                        let icon = item
                            .get("icon")
                            .and_then(|v| {
                                resolve_ext_icon(&root, &ext.id, v, &mut icon_seq, &mut assets)
                            })
                            .unwrap_or_else(|| ExtIcon::Codicon("extensions".into()));
                        if loc_kind == ViewContainerLocation::ActivityBar {
                            summary.activity_views.push((id.to_string(), title.clone()));
                        }
                        ext_container_ids.insert(id.to_string());
                        reg.containers.push(ViewContainerDesc {
                            id: id.to_string(),
                            title,
                            icon,
                            location: loc_kind,
                            ext_id: ext.id.clone(),
                            ext_display: ext.display.clone(),
                        });
                    }
                }
            }
        }

        let mut view_count = 0usize;
        if let Some(views_obj) = contribs.get("views").and_then(Value::as_object) {
            for (container_id, list_val) in views_obj {
                let Some(arr) = list_val.as_array() else {
                    continue;
                };
                for v in arr {
                    let Some(id) = v.get("id").and_then(Value::as_str).map(str::trim) else {
                        continue;
                    };
                    if id.is_empty() {
                        continue;
                    }
                    let name = clean_loc(
                        &loc(v.get("name").and_then(Value::as_str).unwrap_or(id)),
                        id,
                    );
                    let kind = match v.get("type").and_then(Value::as_str) {
                        Some("webview") => ViewType::Webview,
                        _ => ViewType::Tree,
                    };
                    let when = v
                        .get("when")
                        .and_then(Value::as_str)
                        .map(WhenExpr::parse)
                        .unwrap_or_default();
                    let icon = v.get("icon").and_then(|ic| {
                        resolve_ext_icon(&root, &ext.id, ic, &mut icon_seq, &mut assets)
                    });
                    let contextual_title = v
                        .get("contextualTitle")
                        .and_then(Value::as_str)
                        .map(&mut loc)
                        .filter(|s| !s.is_empty() && !s.starts_with('%'));
                    implicit_events.push(format!("onView:{id}"));
                    reg.view_to_ext.insert(id.to_string(), ext.id.clone());
                    reg.views.push(ViewDesc {
                        id: id.to_string(),
                        name,
                        container_id: container_id.clone(),
                        kind,
                        when,
                        icon,
                        contextual_title,
                        ext_id: ext.id.clone(),
                    });
                    view_count += 1;
                }
            }
        }
        summary.views = view_count;

        if let Some(welcomes) = contribs.get("viewsWelcome").and_then(Value::as_array) {
            for w in welcomes {
                let Some(view_id) = w.get("view").and_then(Value::as_str).map(str::trim) else {
                    continue;
                };
                let contents = loc(w.get("contents").and_then(Value::as_str).unwrap_or(""));
                let (text_lines, links) = parse_welcome_contents(&contents);
                let when = w
                    .get("when")
                    .and_then(Value::as_str)
                    .map(WhenExpr::parse)
                    .unwrap_or_default();
                reg.welcomes.push(ViewWelcomeDesc {
                    view_id: view_id.to_string(),
                    text_lines,
                    links,
                    when,
                    ext_id: ext.id.clone(),
                });
            }
        }

        if let Some(submenus) = contribs.get("submenus").and_then(Value::as_array) {
            for s in submenus {
                let Some(id) = s.get("id").and_then(Value::as_str).map(str::trim) else {
                    continue;
                };
                if id.is_empty() {
                    continue;
                }
                let label = clean_loc(
                    &loc(s.get("label").and_then(Value::as_str).unwrap_or(id)),
                    id,
                );
                let icon = s
                    .get("icon")
                    .and_then(|v| resolve_ext_icon(&root, &ext.id, v, &mut icon_seq, &mut assets));
                reg.submenus.insert(
                    id.to_string(),
                    SubmenuDesc {
                        id: id.to_string(),
                        label,
                        icon,
                        ext_id: ext.id.clone(),
                    },
                );
            }
        }

        let mut menu_count = 0usize;
        if let Some(menus_obj) = contribs.get("menus").and_then(Value::as_object) {
            for (loc_key, items_val) in menus_obj {
                let Some(arr) = items_val.as_array() else {
                    continue;
                };
                for item in arr {
                    menu_count += 1;
                    let (target, is_submenu) =
                        if let Some(cmd) = item.get("command").and_then(Value::as_str) {
                            (cmd.trim().to_string(), false)
                        } else if let Some(sub) = item.get("submenu").and_then(Value::as_str) {
                            (sub.trim().to_string(), true)
                        } else {
                            continue;
                        };
                    if target.is_empty() {
                        continue;
                    }
                    let when = item
                        .get("when")
                        .and_then(Value::as_str)
                        .map(WhenExpr::parse)
                        .unwrap_or_default();
                    let (group, order) =
                        parse_menu_group(item.get("group").and_then(Value::as_str).unwrap_or(""));
                    let alt = item
                        .get("alt")
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let desc = MenuItemDesc {
                        command_or_submenu: target,
                        is_submenu,
                        when,
                        group,
                        order,
                        alt,
                        ext_id: ext.id.clone(),
                    };
                    if loc_key == "view/title" {
                        let view_targets = extract_view_targets(
                            item.get("when").and_then(Value::as_str).unwrap_or(""),
                        );
                        if view_targets.is_empty() {
                            reg.view_title_menus
                                .entry("*".into())
                                .or_default()
                                .push(desc);
                        } else {
                            for vid in view_targets {
                                reg.view_title_menus
                                    .entry(vid)
                                    .or_default()
                                    .push(desc.clone());
                            }
                        }
                    } else if reg.submenus.contains_key(loc_key)
                        || !matches!(
                            loc_key.as_str(),
                            "commandPalette"
                                | "editor/context"
                                | "editor/title"
                                | "editor/title/context"
                                | "explorer/context"
                                | "terminal/context"
                                | "scm/title"
                                | "scm/resourceState/context"
                                | "scm/resourceGroup/context"
                        )
                    {
                        reg.submenu_items
                            .entry(loc_key.clone())
                            .or_default()
                            .push(desc);
                    }
                }
            }
        }
        summary.menus = menu_count;

        let mut kb_count = 0usize;
        if let Some(kbs) = contribs.get("keybindings").and_then(Value::as_array) {
            for kb in kbs {
                let Some(command) = kb.get("command").and_then(Value::as_str).map(str::trim) else {
                    continue;
                };
                let os_key = if cfg!(windows) {
                    kb.get("win")
                } else if cfg!(target_os = "macos") {
                    kb.get("mac")
                } else {
                    kb.get("linux")
                };
                let key = os_key
                    .or_else(|| kb.get("key"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if command.is_empty() || key.is_empty() {
                    continue;
                }
                let when = kb
                    .get("when")
                    .and_then(Value::as_str)
                    .map(WhenExpr::parse)
                    .unwrap_or_default();
                reg.keybindings.push(KeybindingDesc {
                    command: command.to_string(),
                    key,
                    when,
                    ext_id: ext.id.clone(),
                });
                kb_count += 1;
            }
        }
        summary.keybindings = kb_count;

        let mut ce_count = 0usize;
        if let Some(ces) = contribs.get("customEditors").and_then(Value::as_array) {
            for ce in ces {
                let Some(view_type) = ce.get("viewType").and_then(Value::as_str).map(str::trim)
                else {
                    continue;
                };
                if view_type.is_empty() {
                    continue;
                }
                let display_name = clean_loc(
                    &loc(ce
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(view_type)),
                    view_type,
                );
                let patterns = ce
                    .get("selector")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.get("filenamePattern").and_then(Value::as_str))
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                implicit_events.push(format!("onCustomEditor:{view_type}"));
                reg.custom_editors.push(CustomEditorDesc {
                    view_type: view_type.to_string(),
                    display_name,
                    patterns,
                    ext_id: ext.id.clone(),
                });
                ce_count += 1;
            }
        }
        summary.custom_editors = ce_count;

        if let Some(langs) = contribs.get("languages").and_then(Value::as_array) {
            for l in langs {
                if let Some(lid) = l.get("id").and_then(Value::as_str).map(str::trim) {
                    if !lid.is_empty() {
                        implicit_events.push(format!("onLanguage:{lid}"));
                    }
                }
            }
        }

        let mut activation_events: Vec<String> = pkg
            .get("activationEvents")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        for ev in implicit_events {
            if !activation_events.contains(&ev) {
                activation_events.push(ev);
            }
        }

        let default_config = collect_default_config(contribs.get("configuration"));
        reg.runtimes.push(ExtRuntimeDesc {
            ext_id: ext.id.clone(),
            display: ext.display.clone(),
            version: ext.version.clone(),
            folder: root,
            main,
            activation_events,
            default_config,
        });
        reg.summaries.insert(ext.id.clone(), summary);
    }

    for items in reg.view_title_menus.values_mut() {
        items.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.order.cmp(&b.order)));
    }
    for items in reg.submenu_items.values_mut() {
        items.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.order.cmp(&b.order)));
    }

    (reg, assets)
}

fn clean_loc(s: &str, fallback: &str) -> String {
    let t = s.trim();
    if t.is_empty() || (t.starts_with('%') && t.ends_with('%')) {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

fn collect_palette_rules(contribs: &Value) -> HashMap<String, WhenExpr> {
    let mut map = HashMap::new();
    let Some(arr) = contribs
        .get("menus")
        .and_then(|m| m.get("commandPalette"))
        .and_then(Value::as_array)
    else {
        return map;
    };
    for item in arr {
        if let Some(cmd) = item.get("command").and_then(Value::as_str).map(str::trim) {
            if !cmd.is_empty() {
                let when = item
                    .get("when")
                    .and_then(Value::as_str)
                    .map(WhenExpr::parse)
                    .unwrap_or_default();
                map.insert(cmd.to_string(), when);
            }
        }
    }
    map
}

fn parse_menu_group(raw: &str) -> (String, i32) {
    let t = raw.trim();
    if t.is_empty() {
        return ("zzz".into(), 0);
    }
    match t.rsplit_once('@') {
        Some((g, n)) => (g.trim().to_string(), n.trim().parse::<i32>().unwrap_or(0)),
        None => (t.to_string(), 0),
    }
}

fn extract_view_targets(when: &str) -> Vec<String> {
    let mut out = Vec::new();
    let tokens = tokenize_when(when);
    let mut i = 0;
    while i + 2 < tokens.len() {
        if matches!(&tokens[i], WhenTok::Ident(k) if k == "view")
            && matches!(tokens[i + 1], WhenTok::Eq)
        {
            if let WhenTok::Ident(v) = &tokens[i + 2] {
                if !out.contains(v) {
                    out.push(v.clone());
                }
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn parse_welcome_contents(raw: &str) -> (Vec<String>, Vec<ViewWelcomeLink>) {
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for line in raw.lines() {
        let mut clean = String::new();
        let mut rest = line.trim();
        while let Some(open_b) = rest.find('[') {
            if let Some(mid) = rest[open_b + 1..].find("](") {
                let label_end = open_b + 1 + mid;
                let url_start = label_end + 2;
                if let Some(close_p) = rest[url_start..].find(')') {
                    let url_end = url_start + close_p;
                    let label = rest[open_b + 1..label_end].trim();
                    let target = rest[url_start..url_end].trim();
                    if let Some(cmd) = target.strip_prefix("command:") {
                        let cmd = cmd.split('?').next().unwrap_or(cmd).trim();
                        if !label.is_empty() && !cmd.is_empty() {
                            links.push(ViewWelcomeLink {
                                label: label.to_string(),
                                command: cmd.to_string(),
                            });
                            clean.push_str(&rest[..open_b]);
                            rest = &rest[url_end + 1..];
                            continue;
                        }
                    }
                }
            }
            clean.push_str(&rest[..open_b + 1]);
            rest = &rest[open_b + 1..];
        }
        clean.push_str(rest);
        let t = clean.trim();
        if !t.is_empty() {
            lines.push(t.to_string());
        }
    }
    (lines, links)
}

fn collect_default_config(cfg: Option<&Value>) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    let Some(val) = cfg else { return out };
    let mut collect_obj = |obj: &Value| {
        if let Some(props) = obj.get("properties").and_then(Value::as_object) {
            for (k, schema) in props {
                if let Some(def) = schema.get("default") {
                    out.insert(k.clone(), def.clone());
                }
            }
        }
    };
    match val {
        Value::Array(arr) => {
            for item in arr {
                collect_obj(item);
            }
        }
        Value::Object(_) => collect_obj(val),
        _ => {}
    }
    out
}

fn resolve_safe_path(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim();
    if rel.is_empty() || rel.contains("..") || Path::new(rel).is_absolute() {
        return None;
    }
    let clean = rel.trim_start_matches("./").trim_start_matches(".\\");
    let joined = root.join(clean);
    let canon = joined.canonicalize().ok()?;
    if canon.starts_with(root) {
        Some(canon)
    } else {
        None
    }
}

fn resolve_ext_icon(
    root: &Path,
    ext_id: &str,
    val: &Value,
    seq: &mut usize,
    assets: &mut HashMap<String, Vec<u8>>,
) -> Option<ExtIcon> {
    let rel = match val {
        Value::String(s) => s.trim(),
        Value::Object(o) => o
            .get("dark")
            .or_else(|| o.get("light"))
            .and_then(Value::as_str)?
            .trim(),
        _ => return None,
    };
    if rel.is_empty() {
        return None;
    }
    if let Some(inner) = rel.strip_prefix("$(").and_then(|s| s.strip_suffix(')')) {
        let name = inner.split('~').next().unwrap_or(inner).trim();
        return Some(ExtIcon::Codicon(name.to_string()));
    }
    let path = resolve_safe_path(root, rel)?;
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() {
        return None;
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        if meta.len() > MAX_SVG_BYTES {
            return None;
        }
        let bytes = std::fs::read(&path).ok()?;
        let mono = is_monochrome_svg(&bytes);
        *seq += 1;
        let asset_key = format!("{VSC_ICON_PREFIX}{ext_id}/{seq}.svg");
        assets.insert(asset_key.clone(), bytes);
        return Some(if mono {
            ExtIcon::SvgMonochrome(asset_key.into())
        } else {
            ExtIcon::SvgColor(asset_key.into())
        });
    }
    if meta.len() <= MAX_RASTER_BYTES {
        Some(ExtIcon::Raster(path))
    } else {
        None
    }
}

pub fn is_monochrome_svg(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    if lower.contains("currentcolor") {
        return true;
    }
    for attr in ["fill=", "stroke=", "stop-color="] {
        let mut pos = 0;
        while let Some(rel) = lower[pos..].find(attr) {
            let start = pos + rel + attr.len();
            let rest = lower[start..].trim_start();
            let val = if let Some(q) = rest.strip_prefix('"') {
                q.split('"').next().unwrap_or("")
            } else if let Some(q) = rest.strip_prefix('\'') {
                q.split('\'').next().unwrap_or("")
            } else {
                ""
            }
            .trim();
            if !val.is_empty()
                && !matches!(
                    val,
                    "none"
                        | "currentcolor"
                        | "inherit"
                        | "transparent"
                        | "#000"
                        | "#000000"
                        | "#fff"
                        | "#ffffff"
                        | "black"
                        | "white"
                )
            {
                return false;
            }
            pos = start;
        }
    }
    true
}

pub fn codicon_to_icon(name: &str) -> gpui_kit::assets::IconName {
    use gpui_kit::assets::IconName as I;
    match name.trim() {
        "comment-discussion" | "chat" | "sparkle" | "copilot" => I::MessagesSquare,
        "robot" | "hubot" | "wand" => I::Bot,
        "plus" | "add" | "new-file" => I::Plus,
        "refresh" | "sync" => I::RefreshCw,
        "gear" | "settings-gear" | "settings" => I::Settings,
        "feedback" | "comment" | "mail" => I::MessageSquareCode,
        "ellipsis" | "more" => I::Ellipsis,
        "trash" | "clear-all" => I::Trash,
        "close" | "x" => I::Close,
        "check" | "pass" => I::Check,
        "search" => I::Search,
        "play" | "debug-start" | "run" => I::Play,
        "debug-stop" | "stop" => I::Square,
        "history" | "clock" => I::Clock,
        "account" | "person" => I::User,
        "globe" | "browser" => I::Globe,
        "book" | "notebook" | "preview" => I::FileText,
        "code" | "file-code" | "braces" => I::Braces,
        "terminal" | "console" => I::SquareTerminal,
        "beaker" | "testing" => I::FlaskConical,
        "bug" | "wrench" | "tools" => I::Wrench,
        "extensions" | "blocks" => I::Blocks,
        "folder" | "folder-opened" => I::FolderOpen,
        "file" => I::File,
        "arrow-left" => I::ArrowLeft,
        "arrow-right" => I::ArrowRight,
        "info" => I::Info,
        "warning" => I::TriangleAlert,
        "error" => I::CircleX,
        "chevron-right" => I::ChevronRight,
        "chevron-down" => I::ChevronDown,
        "copy" => I::Copy,
        "link-external" => I::ExternalLink,
        "cloud-download" | "download" => I::Download,
        "package" => I::Package,
        _ => I::Sparkles,
    }
}

pub fn activitybar_containers() -> Vec<ViewContainerDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.containers
                .iter()
                .filter(|c| c.location == ViewContainerLocation::ActivityBar)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn container_by_id(id: &str) -> Option<ViewContainerDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.containers.iter().find(|c| c.id == id).cloned())
}

pub fn views_for_container(container_id: &str) -> Vec<ViewDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.views
                .iter()
                .filter(|v| v.container_id == container_id)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn welcome_for_view(view_id: &str, ctx: &WhenContext<'_>) -> Vec<ViewWelcomeDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.welcomes
                .iter()
                .filter(|w| w.view_id == view_id && w.when.eval(ctx))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn view_title_menus(view_id: &str, ctx: &WhenContext<'_>) -> Vec<MenuItemDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.view_title_menus
                .get(view_id)
                .into_iter()
                .flatten()
                .filter(|m| m.when.eval(ctx))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn submenu(id: &str) -> Option<SubmenuDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.submenus.get(id).cloned())
}

pub fn submenu_items(submenu_id: &str, ctx: &WhenContext<'_>) -> Vec<MenuItemDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.submenu_items
                .get(submenu_id)
                .into_iter()
                .flatten()
                .filter(|m| m.when.eval(ctx))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn palette_commands(ctx: &WhenContext<'_>) -> Vec<CommandDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.commands
                .iter()
                .filter(|c| c.palette_when.eval(ctx) && c.enablement.eval(ctx))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn commands_for_ext(ext_id: &str, ctx: &WhenContext<'_>) -> Vec<CommandDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| {
            r.commands
                .iter()
                .filter(|c| c.ext_id == ext_id && c.palette_when.eval(ctx))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn command_by_id(id: &str) -> Option<CommandDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.commands.iter().find(|c| c.id == id).cloned())
}

pub fn keybinding_for_command(cmd_id: &str) -> Option<String> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| {
            r.keybindings
                .iter()
                .find(|k| k.command == cmd_id)
                .map(|k| k.key.clone())
        })
}

pub fn runtime_for_ext(ext_id: &str) -> Option<ExtRuntimeDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.runtimes.iter().find(|rt| rt.ext_id == ext_id).cloned())
}

pub fn all_runtimes() -> Vec<ExtRuntimeDesc> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|r| r.runtimes.clone())
        .unwrap_or_default()
}

#[allow(dead_code)]
pub fn ext_for_view(view_id: &str) -> Option<String> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.view_to_ext.get(view_id).cloned())
}

pub fn ext_for_command(cmd_id: &str) -> Option<String> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.cmd_to_ext.get(cmd_id).cloned())
}

pub fn summary_for_ext(ext_id: &str) -> ExtSummary {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|r| r.summaries.get(ext_id).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx<'a>(
        dyn_map: &'a HashMap<String, Value>,
        cfg_map: &'a HashMap<String, Value>,
    ) -> WhenContext<'a> {
        WhenContext {
            view: Some("antigravity.panel"),
            focused_view: Some("antigravity.panel"),
            editor_open: true,
            editor_focus: true,
            editor_has_selection: false,
            editor_readonly: false,
            terminal_focus: false,
            resource_scheme: Some("file"),
            resource_ext: Some(".rs"),
            dynamic: dyn_map,
            config: cfg_map,
        }
    }

    #[test]
    fn when_expr_parses_and_evaluates_correctly() {
        let mut dyn_map = HashMap::new();
        dyn_map.insert("antigravity.ready".into(), Value::Bool(true));
        dyn_map.insert("mode".into(), Value::String("chat".into()));
        let mut cfg_map = HashMap::new();
        cfg_map.insert("antigravity.enabled".into(), Value::Bool(true));

        let ctx = test_ctx(&dyn_map, &cfg_map);
        assert!(WhenExpr::parse("").eval(&ctx));
        assert!(!WhenExpr::parse("false").eval(&ctx));
        assert!(WhenExpr::parse("view == antigravity.panel && editorTextFocus").eval(&ctx));
        assert!(!WhenExpr::parse("editorHasSelection || !antigravity.ready").eval(&ctx));
        assert!(WhenExpr::parse(
            "(mode == 'chat' || mode == 'edit') && config.antigravity.enabled"
        )
        .eval(&ctx));
        assert!(WhenExpr::parse("resourceScheme != untitled && !editorReadonly").eval(&ctx));
        assert_eq!(WhenExpr::parse("isWindows").eval(&ctx), cfg!(windows));
    }

    #[test]
    fn parses_antigravity_opencode_and_cline_contributions() {
        let root = std::env::temp_dir().join(format!("fx-vsc-reg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        // 1. Google Antigravity fixture
        let ag = root.join("google.google-antigravity-1.13.0");
        std::fs::create_dir_all(ag.join("img")).unwrap();
        std::fs::write(
            ag.join("img/logo.svg"),
            r#"<svg viewBox="0 0 24 24"><path fill="currentColor" d="M0 0h24v24z"/></svg>"#,
        )
        .unwrap();
        std::fs::write(ag.join("extension.js"), "exports.activate = () => {};").unwrap();
        std::fs::write(
            ag.join("package.json"),
            r#"{
                "publisher": "google", "name": "google-antigravity", "version": "1.13.0",
                "displayName": "Google Antigravity", "main": "./extension.js",
                "contributes": {
                    "viewsContainers": { "activitybar": [{ "id": "antigravity-sidebar", "title": "Antigravity", "icon": "img/logo.svg" }] },
                    "views": { "antigravity-sidebar": [{ "type": "webview", "id": "antigravity.panel", "name": "Antigravity", "icon": "img/logo.svg" }] },
                    "commands": [
                        { "command": "antigravity.open", "title": "Open Antigravity", "category": "Antigravity" },
                        { "command": "antigravity.internal", "title": "Internal" },
                        { "command": "antigravity.startNewConversation", "title": "New Chat", "icon": "$(plus)" }
                    ],
                    "submenus": [{ "id": "antigravity.panelMoreSubmenu", "label": "More...", "icon": "$(ellipsis)" }],
                    "menus": {
                        "commandPalette": [{ "command": "antigravity.internal", "when": "false" }],
                        "view/title": [
                            { "command": "antigravity.startNewConversation", "when": "view == antigravity.panel", "group": "navigation@1" },
                            { "submenu": "antigravity.panelMoreSubmenu", "when": "view == antigravity.panel", "group": "navigation@9" }
                        ],
                        "antigravity.panelMoreSubmenu": [{ "command": "antigravity.open", "group": "1_main@1" }]
                    },
                    "keybindings": [{ "command": "antigravity.open", "key": "ctrl+shift+i", "win": "ctrl+alt+a" }],
                    "customEditors": [{ "viewType": "antigravity.artifactEditor", "displayName": "Artifact", "selector": [{ "filenamePattern": "*.artifact" }] }]
                }
            }"#,
        )
        .unwrap();

        // 2. OpenCode fixture + path-traversal icon check
        let oc = root.join("tanishqkancharla.opencode-vscode-0.1.0");
        std::fs::create_dir_all(oc.join("media")).unwrap();
        std::fs::write(
            oc.join("media/icon.svg"),
            r##"<svg viewBox="0 0 24 24"><rect fill="#ff5500" width="24" height="24"/></svg>"##,
        )
        .unwrap();
        std::fs::write(oc.join("dist-ext.js"), "").unwrap();
        std::fs::write(
            oc.join("package.json"),
            r#"{
                "publisher": "TanishqKancharla", "name": "opencode-vscode", "version": "0.1.0",
                "displayName": "OpenCode", "main": "./dist-ext.js",
                "contributes": {
                    "viewsContainers": { "activitybar": [{ "id": "opencode", "title": "OpenCode", "icon": "media/icon.svg" }] },
                    "views": { "opencode": [{ "type": "webview", "id": "opencode.chatView", "name": "OpenCode" }] },
                    "viewsWelcome": [{ "view": "opencode.chatView", "contents": "Start OpenCode.\n[Launch](command:opencode.open)" }],
                    "commands": [{ "command": "opencode.open", "title": "Open", "category": "OpenCode" }],
                    "configuration": { "properties": { "opencode.port": { "default": 4096 } } }
                }
            }"#,
        )
        .unwrap();

        // 3. Path-traversal fixture
        let bad = root.join("bad.evil-1.0.0");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(
            bad.join("package.json"),
            r#"{
                "publisher": "bad", "name": "evil", "version": "1.0.0",
                "main": "../escape.js",
                "contributes": { "viewsContainers": { "activitybar": [{ "id": "evil", "title": "Evil", "icon": "../secret.svg" }] } }
            }"#,
        )
        .unwrap();

        let installed = vec![
            market::Installed {
                id: "google.google-antigravity".into(),
                folder: ag,
                display: "Google Antigravity".into(),
                version: "1.13.0".into(),
                ..Default::default()
            },
            market::Installed {
                id: "tanishqkancharla.opencode-vscode".into(),
                folder: oc,
                display: "OpenCode".into(),
                version: "0.1.0".into(),
                ..Default::default()
            },
            market::Installed {
                id: "bad.evil".into(),
                folder: bad,
                display: "Evil".into(),
                version: "1.0.0".into(),
                ..Default::default()
            },
        ];

        let (reg, assets) = build_registry(&installed);
        let dyn_map = HashMap::new();
        let cfg_map = HashMap::new();
        let ctx = test_ctx(&dyn_map, &cfg_map);

        let ab: Vec<&ViewContainerDesc> = reg
            .containers
            .iter()
            .filter(|c| c.location == ViewContainerLocation::ActivityBar)
            .collect();
        assert_eq!(ab.len(), 3);
        assert!(
            matches!(ab[0].icon, ExtIcon::SvgMonochrome(_)),
            "Antigravity logo must be monochrome SVG"
        );
        assert!(
            matches!(ab[1].icon, ExtIcon::SvgColor(_)),
            "Colored SVG must preserve colors"
        );
        assert!(
            matches!(ab[2].icon, ExtIcon::Codicon(_)),
            "Escaped icon path must fall back safely"
        );
        assert!(!assets.is_empty());

        let visible_cmds: Vec<&str> = reg
            .commands
            .iter()
            .filter(|c| c.palette_when.eval(&ctx))
            .map(|c| c.id.as_str())
            .collect();
        assert!(visible_cmds.contains(&"antigravity.open"));
        assert!(!visible_cmds.contains(&"antigravity.internal"));

        let vt = reg.view_title_menus.get("antigravity.panel").unwrap();
        assert_eq!(vt.len(), 2);
        assert_eq!(vt[0].command_or_submenu, "antigravity.startNewConversation");
        assert!(vt[1].is_submenu);

        let rt = reg
            .runtimes
            .iter()
            .find(|r| r.ext_id == "google.google-antigravity")
            .unwrap();
        assert!(rt
            .activation_events
            .contains(&"onView:antigravity.panel".to_string()));
        assert!(rt
            .activation_events
            .contains(&"onCustomEditor:antigravity.artifactEditor".to_string()));
        assert!(reg
            .runtimes
            .iter()
            .find(|r| r.ext_id == "bad.evil")
            .unwrap()
            .main
            .is_none());

        let oc_rt = reg
            .runtimes
            .iter()
            .find(|r| r.ext_id == "tanishqkancharla.opencode-vscode")
            .unwrap();
        assert_eq!(
            oc_rt.default_config.get("opencode.port"),
            Some(&serde_json::json!(4096))
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn real_installed_antigravity_parses_when_present() {
        let dir = market::dir();
        if !dir.is_dir() {
            return;
        }
        let installed = market::installed_list();
        let ag = installed
            .into_iter()
            .filter(|i| i.id == "google.google-antigravity")
            .collect::<Vec<_>>();
        if ag.is_empty() {
            return;
        }
        let (reg, _) = build_registry(&ag);
        assert!(reg.containers.iter().any(|c| c.id == "antigravity-sidebar"));
        assert!(reg
            .views
            .iter()
            .any(|v| v.id == "antigravity.panel" && v.kind == ViewType::Webview));
        let rt = reg.runtimes.first().unwrap();
        assert!(rt.main.as_ref().is_some_and(|p| p.is_file()));
    }
}
