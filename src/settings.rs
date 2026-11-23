
use crate::i18n::tr;
use crate::theme::{self, c, UiSize, Visibility, PALETTES};
use crate::workbench::{ScrollTarget, Workbench};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    switch::Switch,
    v_flex, Disableable, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const REPO_URL: &str = "https://github.com/MuaraAI/FluxCode";
const ISSUES_URL: &str = "https://github.com/MuaraAI/FluxCode/issues/new/choose";
pub(crate) const CHANGELOG_URL: &str = "https://github.com/MuaraAI/FluxCode/blob/main/CHANGELOG.md";

pub(crate) fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("FluxCode-GPUI")
}

pub(crate) fn load_json<T: for<'a> Deserialize<'a> + Default>(name: &str) -> T {
    std::fs::read_to_string(config_dir().join(name))
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

pub(crate) fn save_json<T: Serialize>(name: &str, value: &T) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(s) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(dir.join(name), s);
    }
}

pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

const SETTINGS_REV: u32 = 1;

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub font_size: f32,
    pub font_family: SharedString,
    pub tab_size: usize,
    pub word_wrap: bool,
    pub show_whitespace: bool,
    pub auto_save: bool,
    pub agent_auto_approve: bool,
    pub theme: String,
    pub bg_image: Option<PathBuf>,
    pub bg_visibility: Visibility,
    pub ui_size: UiSize,
    pub agent_right: bool,
    pub right_views: Vec<String>,
    pub glass: bool,
    pub saver_packs: Vec<String>,
    pub saver_level: String,
    pub saver_compact: bool,
    pub saver_compact_at: u8,
    pub resume_at_reset: bool,
    pub bg_scene: String,
    pub bg_scene_before_sky: String,
    pub bg_motion: bool,
    pub agent_follow_edits: bool,
    pub agent_accounts: Vec<crate::agent::worktree::Account>,
    pub agent_account: std::collections::HashMap<String, String>,
    pub agent_cli: String,
    pub agent_idle_min: u32,
    pub language: String,
    pub agent_options: std::collections::HashMap<String, String>,
    pub agent_access: String,
    pub agent_access_off: String,
    pub agent_notify: bool,
    pub agent_browser: String,
    pub agent_browser_headless: bool,
    pub extensions: Vec<String>,
    pub agent_hint: u8,
    pub chat_view: String,
    pub full_left_w: f32,
    pub full_right_w: f32,
    pub chat_show_all: bool,
    pub chat_collapsed: Vec<String>,
    pub update_check: bool,
    pub wsl_helper: bool,
    #[cfg(feature = "embedded-preview")]
    #[serde(default = "default_true")]
    pub preview_webview_notice: bool,
    pub line_numbers: bool,
    pub folding: bool,
    pub auto_close: bool,
    pub smart_indent: bool,
    pub cursor_padding: u32,
    pub scroll_past_end: bool,
    #[serde(default = "default_true")]
    pub copilot: bool,
    pub last_build: String,
    #[serde(default)]
    pub rev: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 15.,
            font_family: "Cascadia Code".into(),
            tab_size: 4,
            word_wrap: false,
            show_whitespace: false,
            auto_save: true,
            agent_auto_approve: false,
            theme: PALETTES[0].id.into(),
            bg_image: None,
            bg_visibility: Visibility::default(),
            ui_size: UiSize::default(),
            agent_right: true,
            right_views: Vec::new(),
            glass: true,
            saver_packs: Vec::new(),
            saver_level: "full".into(),
            saver_compact: true,
            saver_compact_at: 70,
            resume_at_reset: false,
            bg_scene: "ocean".into(),
            bg_scene_before_sky: String::new(),
            bg_motion: false,
            agent_follow_edits: true,
            agent_accounts: Vec::new(),
            agent_account: Default::default(),
            agent_cli: "auto".into(),
            agent_idle_min: 10,
            language: "auto".into(),
            agent_options: Default::default(),
            agent_access: String::new(),
            agent_access_off: "write".into(),
            agent_notify: true,
            agent_browser: "auto".into(),
            agent_browser_headless: false,
            extensions: crate::ext::default_installed(),
            agent_hint: 0,
            chat_view: String::new(),
            full_left_w: 0.,
            full_right_w: 0.,
            chat_show_all: false,
            chat_collapsed: Vec::new(),
            update_check: true,
            wsl_helper: false,
            #[cfg(feature = "embedded-preview")]
            preview_webview_notice: true,
            line_numbers: true,
            folding: true,
            auto_close: true,
            smart_indent: true,
            cursor_padding: 0,
            scroll_past_end: true,
            copilot: true,
            last_build: String::new(),
            rev: SETTINGS_REV,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        EXISTED.get_or_init(|| config_dir().join("settings.json").is_file());
        let mut s: Self = load_json("settings.json");
        if s.rev < 1 {
            s.show_whitespace = false;
        }
        if s.agent_cli == "opencode" {
            s.agent_cli = "opencode-v2".into();
            s.agent_options = std::mem::take(&mut s.agent_options)
                .into_iter()
                .map(|(k, v)| match k.strip_prefix("opencode:") {
                    Some(rest) => (format!("opencode-v2:{rest}"), v),
                    None => (k, v),
                })
                .collect();
            s.save();
        }
        if s.rev < SETTINGS_REV {
            s.rev = SETTINGS_REV;
            s.save();
        }
        s
    }
    pub fn save(&self) {
        save_json("settings.json", self);
    }
    pub fn access(&self) -> &str {
        match self.agent_access.as_str() {
            "read" | "write" | "full" => &self.agent_access,
            _ if self.agent_auto_approve => "full",
            _ => "write",
        }
    }

    pub fn access_off(&self) -> &'static str {
        if self.agent_access_off == "read" { "read" } else { "write" }
    }

    pub fn background(&self) -> Option<&Path> {
        self.bg_image.as_deref().filter(|p| p.is_file())
    }

    pub fn scene(&self) -> Option<crate::scene::Scene> {
        if self.background().is_some() {
            return None;
        }
        crate::scene::Scene::from_id(&self.bg_scene)
    }

    pub fn backdrop_visibility(&self) -> Option<Visibility> {
        (self.background().is_some() || self.scene().is_some()).then_some(self.bg_visibility)
    }

    pub fn apply_backdrop(&self) {
        crate::theme::set(&self.theme, self.backdrop_visibility());
        crate::theme::set_glass(self.glass);
        crate::scene::set(self.scene(), self.bg_motion);
        crate::theme::set_scene_backdrop(self.scene().is_some());
    }

    pub fn reset_appearance(&mut self) {
        let def = Self::default();
        self.theme = def.theme;
        self.language = def.language;
        self.ui_size = def.ui_size;
        self.bg_image = def.bg_image;
        self.bg_visibility = def.bg_visibility;
        self.glass = def.glass;
        self.bg_scene = def.bg_scene;
        self.bg_scene_before_sky = def.bg_scene_before_sky;
        self.bg_motion = def.bg_motion;
    }

    pub fn reset_editor(&mut self) {
        let def = Self::default();
        self.font_size = def.font_size;
        self.font_family = def.font_family;
        self.tab_size = def.tab_size;
        self.word_wrap = def.word_wrap;
        self.show_whitespace = def.show_whitespace;
        self.auto_save = def.auto_save;
        self.line_numbers = def.line_numbers;
        self.folding = def.folding;
        self.auto_close = def.auto_close;
        self.smart_indent = def.smart_indent;
        self.cursor_padding = def.cursor_padding;
        self.scroll_past_end = def.scroll_past_end;
        self.copilot = def.copilot;
    }
}

static EXISTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

pub fn existed_before_start() -> bool {
    EXISTED.get().copied().unwrap_or(false)
}

#[derive(Clone, Copy)]
pub struct EditorOpts {
    pub line_numbers: bool,
    folding: bool,
    auto_close: bool,
    smart_indent: bool,
    cursor_padding: u32,
    scroll_past_end: bool,
}

impl EditorOpts {
    pub fn of(s: &Settings) -> Self {
        Self {
            line_numbers: s.line_numbers,
            folding: s.folding,
            auto_close: s.auto_close,
            smart_indent: s.smart_indent,
            cursor_padding: s.cursor_padding,
            scroll_past_end: s.scroll_past_end,
        }
    }

    pub fn apply(self, e: &mut gpui_kit::component::input::EditorState, window: &mut Window, cx: &mut Context<gpui_kit::component::input::EditorState>) {
        e.set_line_number(self.line_numbers, window, cx);
        e.set_folding(self.folding, window, cx);
        e.set_auto_close(self.auto_close, window, cx);
        e.set_smart_indent(self.smart_indent, window, cx);
        e.set_cursor_surrounding_lines((self.cursor_padding > 0).then_some(self.cursor_padding as usize), window, cx);
        e.set_scroll_beyond_last_line(if self.scroll_past_end { None } else { Some(0) }, window, cx);
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub root: Option<PathBuf>,
    pub extra: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub active: Option<usize>,
}

impl Session {
    pub fn load() -> Self {
        load_json("session.json")
    }
    pub fn save(&self) {
        save_json("session.json", self);
    }
}

const SETTINGS_SCROLL_KEY: u64 = 0x5E77_1A65;

fn theme_grid_cols(content_w: f32) -> u16 {
    if content_w >= 660. {
        3
    } else if content_w >= 430. {
        2
    } else {
        1
    }
}

fn scene_grid_cols(content_w: f32) -> u16 {
    if content_w >= 640. {
        4
    } else if content_w >= 440. {
        3
    } else {
        2
    }
}

fn matches_query(query: &str, haystack: &[&str]) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    q.split_whitespace().all(|tok| haystack.iter().any(|h| h.to_lowercase().contains(tok)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavGroup {
    Workspace,
    Ai,
    System,
    App,
}

impl NavGroup {
    const ALL: [Self; 4] = [Self::Workspace, Self::Ai, Self::System, Self::App];

    fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::Ai => "AI & accounts",
            Self::System => "System",
            Self::App => "Application",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    Appearance,
    Editor,
    Wsl,
    Windows,
    Agents,
    SoulFiles,
    Shortcuts,
    Updates,
    About,
}

impl SettingsSection {
    pub(crate) const ALL: [SettingsSection; 9] = [
        SettingsSection::Appearance,
        SettingsSection::Editor,
        SettingsSection::Agents,
        SettingsSection::SoulFiles,
        SettingsSection::Wsl,
        SettingsSection::Windows,
        SettingsSection::Shortcuts,
        SettingsSection::Updates,
        SettingsSection::About,
    ];

    fn group(self) -> NavGroup {
        match self {
            Self::Appearance | Self::Editor => NavGroup::Workspace,
            Self::Agents | Self::SoulFiles => NavGroup::Ai,
            Self::Wsl | Self::Windows => NavGroup::System,
            Self::Shortcuts | Self::Updates | Self::About => NavGroup::App,
        }
    }

    fn icon(self) -> Icon {
        use gpui_kit::{assets::IconName as A, component::IconName as C};
        match self {
            Self::Appearance => Icon::new(C::Eye),
            Self::Editor => Icon::new(C::FileText),
            Self::Agents => Icon::new(C::Bot),
            Self::SoulFiles => Icon::new(A::ScrollText),
            Self::Wsl => Icon::new(C::SquareTerminal),
            Self::Windows => Icon::new(C::WindowMaximize),
            Self::Shortcuts => Icon::new(A::TextCursorInput),
            Self::Updates => Icon::new(C::RefreshCw),
            Self::About => Icon::new(C::Info),
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            SettingsSection::Appearance => "Appearance",
            SettingsSection::Editor => "Editor",
            SettingsSection::Wsl => "WSL",
            SettingsSection::Windows => "Windows",
            SettingsSection::Agents => "AI agents and GitHub",
            SettingsSection::SoulFiles => "Agent instructions (soul files)",
            SettingsSection::Shortcuts => "Keyboard shortcuts",
            SettingsSection::Updates => "Updates",
            SettingsSection::About => "About FluxCode",
        }
    }

    pub(crate) fn nav_label(self) -> &'static str {
        match self {
            SettingsSection::SoulFiles => "Agent instructions",
            s => s.title(),
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            SettingsSection::Appearance => "Theme, language, interface size, and the picture behind the panels.",
            SettingsSection::Editor => "Font, wrapping, line numbers, and the Copilot suggestions the editor shows as you type.",
            SettingsSection::Wsl => "Read folders inside a WSL distro through a helper, instead of the slow Windows share.",
            SettingsSection::Windows => "How FluxCode shows up in Windows Explorer.",
            SettingsSection::Agents => "What the agent may do, how it docks, and the GitHub and Copilot sign-ins.",
            SettingsSection::SoulFiles => "The files each installed agent reads for its standing instructions and character. Open one to read or edit it; a missing one can be created empty.",
            SettingsSection::Shortcuts => "VS Code's keys. Ctrl+Shift+P (or F1) opens every command with its keys; type to filter.",
            SettingsSection::Updates => "Check for new development builds and whether FluxCode looks for them.",
            SettingsSection::About => "What FluxCode is, where its code lives, and how to report a problem.",
        }
    }

    pub(crate) fn modified(self, w: &Workbench) -> bool {
        let s = &w.settings;
        let def = Settings::default();
        match self {
            SettingsSection::Appearance => {
                s.theme != def.theme
                    || s.language != def.language
                    || s.ui_size != def.ui_size
                    || s.bg_image.is_some()
                    || s.bg_visibility != def.bg_visibility
                    || s.glass != def.glass
                    || s.bg_scene != def.bg_scene
                    || s.bg_motion != def.bg_motion
            }
            SettingsSection::Editor => {
                s.font_size != def.font_size
                    || s.tab_size != def.tab_size
                    || s.word_wrap != def.word_wrap
                    || s.show_whitespace != def.show_whitespace
                    || s.auto_save != def.auto_save
                    || s.font_family != def.font_family
                    || s.line_numbers != def.line_numbers
                    || s.folding != def.folding
                    || s.auto_close != def.auto_close
                    || s.smart_indent != def.smart_indent
                    || s.cursor_padding != def.cursor_padding
                    || s.scroll_past_end != def.scroll_past_end
                    || s.copilot != def.copilot
            }
            SettingsSection::Wsl => s.wsl_helper,
            SettingsSection::Agents => {
                !s.agent_access.is_empty()
                    || s.agent_auto_approve
                    || s.agent_access_off != "write"
                    || s.agent_idle_min != 10
                    || !s.agent_right
                    || !s.agent_follow_edits
                    || w.github.is_some()
            }
            SettingsSection::Windows | SettingsSection::SoulFiles | SettingsSection::Shortcuts | SettingsSection::About => false,
            SettingsSection::Updates => !s.update_check,
        }
    }
}

struct SettingEntry {
    group: &'static str,
    title: &'static str,
    desc: SharedString,
    keywords: &'static str,
    stacked: bool,
    control: AnyElement,
}

impl SettingEntry {
    fn inline(group: &'static str, title: &'static str, desc: SharedString, keywords: &'static str, control: AnyElement) -> Self {
        Self { group, title, desc, keywords, stacked: false, control }
    }

    fn stacked(group: &'static str, title: &'static str, desc: SharedString, keywords: &'static str, control: AnyElement) -> Self {
        Self { group, title, desc, keywords, stacked: true, control }
    }

    fn render(self, compact: bool) -> AnyElement {
        if self.stacked || compact {
            settings_row_stacked(tr(self.title), self.desc, self.control)
        } else {
            settings_row_inline(tr(self.title), self.desc, self.control)
        }
    }
}

fn settings_row_inline(title: &'static str, desc: SharedString, control: AnyElement) -> AnyElement {
    h_flex()
        .w_full()
        .py_3()
        .px_4()
        .gap_4()
        .items_center()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(title))
                .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc)),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

fn settings_row_stacked(title: &'static str, desc: SharedString, control: AnyElement) -> AnyElement {
    v_flex()
        .w_full()
        .py_3()
        .px_4()
        .gap_2p5()
        .child(
            v_flex()
                .w_full()
                .gap_0p5()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(title))
                .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc)),
        )
        .child(div().w_full().child(control))
        .into_any_element()
}

fn settings_card(rows: Vec<AnyElement>) -> AnyElement {
    let n = rows.len();
    v_flex()
        .w_full()
        .rounded(theme::u(8.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .overflow_hidden()
        .children(rows.into_iter().enumerate().map(|(i, row)| {
            div()
                .when(i + 1 < n, |d| d.border_b_1().border_color(c(theme::BORDER)))
                .child(row)
        }))
        .into_any_element()
}

fn settings_group(label: &'static str, rows: Vec<AnyElement>) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(div().px_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr(label)))
        .child(settings_card(rows))
        .into_any_element()
}

fn settings_segmented(items: impl IntoIterator<Item = AnyElement>) -> AnyElement {
    h_flex()
        .p(px(2.))
        .gap(px(2.))
        .flex_wrap()
        .rounded(theme::u(7.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_SIDE))
        .children(items)
        .into_any_element()
}

fn settings_pill(id: SharedString, label: impl Into<SharedString>, active: bool, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> AnyElement {
    let btn = Button::new(id).label(label.into()).xsmall();
    if active { btn.primary() } else { btn.ghost() }.on_click(on_click).into_any_element()
}

fn settings_switch(
    id: &'static str,
    label: &'static str,
    on: bool,
    cx: &mut Context<Workbench>,
    toggle: impl Fn(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
) -> AnyElement {
    Switch::new(id)
        .small()
        .checked(on)
        .accessibility_label(tr(label))
        .on_click(cx.listener(move |this, _: &bool, w, cx| toggle(this, w, cx)))
        .into_any_element()
}

fn settings_toggle(id: &'static str, label: &'static str, on: bool, cx: &mut Context<Workbench>, flip: fn(&mut Workbench)) -> AnyElement {
    Switch::new(id)
        .small()
        .checked(on)
        .accessibility_label(tr(label))
        .on_click(cx.listener(move |this, _: &bool, w, cx| {
            flip(this);
            this.apply_settings(w, cx);
        }))
        .into_any_element()
}

fn settings_stepper(id: &'static str, value: String, can_dec: bool, can_inc: bool, cx: &mut Context<Workbench>, step: fn(&mut Workbench, i32)) -> AnyElement {
    h_flex()
        .items_center()
        .p(px(2.))
        .gap(px(2.))
        .rounded(theme::u(7.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_SIDE))
        .child(
            Button::new((id, 0usize))
                .label("−")
                .xsmall()
                .ghost()
                .disabled(!can_dec)
                .on_click(cx.listener(move |this, _, w, cx| {
                    step(this, -1);
                    this.apply_settings(w, cx);
                })),
        )
        .child(div().min_w(px(52.)).px_1p5().text_center().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(value))
        .child(
            Button::new((id, 1usize))
                .label("+")
                .xsmall()
                .ghost()
                .disabled(!can_inc)
                .on_click(cx.listener(move |this, _, w, cx| {
                    step(this, 1);
                    this.apply_settings(w, cx);
                })),
        )
        .into_any_element()
}

fn scene_cards(cols: u16, current: &str, cx: &mut Context<Workbench>) -> AnyElement {
    div()
        .grid()
        .grid_cols(cols)
        .w_full()
        .gap_2p5()
        .children(crate::scene::Scene::ALL.into_iter().map(|sc| {
            let on = current == sc.id();
            v_flex()
                .id(SharedString::from(format!("sc-{}", sc.id())))
                .w_full()
                .min_w_0()
                .rounded(px(7.))
                .overflow_hidden()
                .cursor_pointer()
                .border_2()
                .border_color(if on { c(theme::ACCENT) } else { c(theme::BORDER) })
                .hover(move |st| st.border_color(c(if on { theme::ACCENT } else { theme::BORDER_STRONG })))
                .on_click(cx.listener(move |this, _, w, cx| {
                    this.settings.bg_scene = sc.id().into();
                    this.apply_appearance(w, cx);
                }))
                .child(div().h(theme::u(74.)).w_full().overflow_hidden().child(crate::scene::preview(sc, 12.)))
                .child(
                    h_flex()
                        .px_2p5()
                        .py_1p5()
                        .gap_1()
                        .bg(c(theme::BG_ELEVATED))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                .text_color(c(if on { theme::FG_STRONG } else { theme::FG_MUTED }))
                                .child(tr(sc.label())),
                        )
                        .when(on, |h| h.child(Icon::new(gpui_kit::assets::IconName::Check).size(theme::u(12.)).text_color(c(theme::ACCENT_TEXT)))),
                )
        }))
        .into_any_element()
}

fn theme_cards(cols: u16, cx: &mut Context<Workbench>) -> AnyElement {
    let active_theme = theme::palette().id;
    div()
        .grid()
        .grid_cols(cols)
        .w_full()
        .gap_3()
        .children(theme::all().into_iter().map(|p| {
            let on = p.id == active_theme;
            let id = p.id;
            let [surface, side, accent, text] = p.swatches();
            v_flex()
                .id(SharedString::from(format!("theme-{id}")))
                .w_full()
                .min_w_0()
                .rounded(px(8.))
                .overflow_hidden()
                .cursor_pointer()
                .border_2()
                .border_color(if on { c(theme::ACCENT) } else { c(theme::BORDER) })
                .hover(move |s| s.border_color(c(if on { theme::ACCENT } else { theme::BORDER_STRONG })))
                .on_click(cx.listener(move |this, _, w, cx| this.set_theme(id, w, cx)))
                .child(
                    h_flex()
                        .h(theme::u(56.))
                        .child(div().w(px(36.)).h_full().bg(c(side)))
                        .child(
                            v_flex()
                                .flex_1()
                                .h_full()
                                .p_2p5()
                                .gap_1()
                                .bg(c(surface))
                                .child(div().w(px(72.)).h(px(4.)).rounded(px(2.)).bg(c(text)))
                                .child(div().w(px(48.)).h(px(4.)).rounded(px(2.)).bg(c(accent)))
                                .child(div().w(px(96.)).h(px(4.)).rounded(px(2.)).bg(c(text)).opacity(0.6)),
                        ),
                )
                .child(
                    v_flex()
                        .px_3()
                        .py_2()
                        .gap(px(2.))
                        .bg(c(theme::BG_ELEVATED))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(p.name))
                                .when(on, |h| {
                                    h.child(
                                        h_flex()
                                            .gap_1()
                                            .items_center()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(c(theme::ACCENT_TEXT))
                                            .child(Icon::new(gpui_kit::assets::IconName::Check).size(theme::u(11.)))
                                            .child(tr("Active")),
                                    )
                                }),
                        )
                        .child(div().text_xs().truncate().text_color(c(theme::FG_MUTED)).child(tr(p.note))),
                )
        }))
        .into_any_element()
}

fn step_theme_fade(weak: WeakEntity<Workbench>, window: &mut Window, _cx: &mut App) {
    window.on_next_frame(move |window, cx| {
        if weak.upgrade().is_none() {
            return;
        }
        let more = theme::fading();
        theme::apply(cx);
        window.refresh();
        _ = weak.update(cx, |_, cx| cx.notify());
        if more {
            step_theme_fade(weak, window, cx);
        }
    });
}

impl Workbench {
    pub(crate) fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        self.copilot.set_enabled(self.settings.copilot);
        self.kick_copilot(cx);
        crate::remote::set_helper_enabled(self.settings.wsl_helper);
        if self.settings.agent_right && self.view == crate::workbench::SideView::Agent {
            self.view = crate::workbench::SideView::Explorer;
            self.agent_open = true;
            self.right_view = crate::workbench::SideView::Agent;
        }
        let (wrap, dots) = (self.settings.word_wrap, self.settings.show_whitespace);
        let opts = EditorOpts::of(&self.settings);
        for d in &self.docs {
            let Some(ed) = &d.editor else { continue };
            let wrap = if d.wrap_override.is_none() && d.complex { false } else { d.wraps(wrap) };
            let complex_or_ro = d.complex || d.read_only;
            ed.update(cx, |s, cx| {
                s.set_soft_wrap(wrap, window, cx);
                s.set_show_whitespaces(dots && !d.read_only, window, cx);
                opts.apply(s, window, cx);
                if complex_or_ro {
                    s.set_folding(false, window, cx);
                }
            });
        }
        cx.notify();
    }

    pub(crate) fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        theme::set_ui_size(self.settings.ui_size);
        self.settings.apply_backdrop();
        theme::apply(cx);
        window.refresh();
        cx.notify();
    }

    pub(crate) fn toggle_bg_motion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::motion_check::{self, Check};
        self.settings.bg_motion = !self.settings.bg_motion;
        self.apply_appearance(window, cx);
        if !self.settings.bg_motion {
            self.motion_check = None;
            return cx.notify();
        }
        self.motion_check = Some(Check::Running);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(motion_check::SETTLE).await;
            let start = motion_check::cpu_seconds();
            cx.background_executor().timer(motion_check::MEASURE).await;
            let pct = match (start, motion_check::cpu_seconds()) {
                (Some(a), Some(b)) => Some(((b - a) / motion_check::MEASURE.as_secs_f64() * 100.) as f32),
                _ => None,
            };
            let specs = cx.background_spawn(async { motion_check::specs() }).await;
            _ = this.update(cx, |this, cx| {
                if this.settings.bg_motion && matches!(this.motion_check, Some(Check::Running)) {
                    let (level, text) = motion_check::verdict(pct, specs);
                    this.motion_check = Some(Check::Done(level, text));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn set_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let old = theme::palette();
        self.settings.theme = id.into();
        let sky = crate::scene::Scene::Sky.id();
        if id == "flux-sky" && old.id != id && self.settings.bg_image.is_none() && self.settings.bg_scene != sky {
            self.settings.bg_scene_before_sky = std::mem::replace(&mut self.settings.bg_scene, sky.into());
        } else if id != "flux-sky" && old.id == "flux-sky" && self.settings.bg_scene == sky && !self.settings.bg_scene_before_sky.is_empty() {
            self.settings.bg_scene = std::mem::take(&mut self.settings.bg_scene_before_sky);
        }
        if old.id != id && !cx.reduce_motion() {
            theme::fade_from(old);
            step_theme_fade(cx.entity().downgrade(), window, cx);
        }
        self.apply_appearance(window, cx);
        let name = theme::find(id).map_or("", |p| p.name);
        self.status = crate::trf!("Color theme: {name}"; name => name).into();
    }

    pub(crate) fn choose_background(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose Background Image".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = paths.await.ok()?.ok()??.into_iter().next()?;
            this.update_in(cx, |this, window, cx| {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                    this.status = crate::trf!("{0} is not an image FluxCode can show (PNG, JPEG, WebP, GIF or BMP)."; 0 => path.display()).into();
                    cx.notify();
                    return;
                }
                this.settings.bg_image = Some(path);
                this.apply_appearance(window, cx);
                this.status = tr("Background image set").into();
            })
            .ok()
        })
        .detach();
    }

    fn settings_sections() -> Vec<SettingsSection> {
        SettingsSection::ALL.iter().copied().filter(|s| cfg!(windows) || *s != SettingsSection::Windows).collect()
    }

    pub(crate) fn render_settings(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let vp_w = f32::from(window.viewport_size().width);
        let narrow_nav = vp_w < 820.;
        let nav_w = if narrow_nav { 188. } else { 240. } * theme::scale();
        let pad_x = if vp_w < 920. { 40. } else { 64. };
        let content_w = (vp_w - 44. * theme::scale() - nav_w - pad_x).clamp(300., 780. * theme::scale());
        let compact = content_w < 540.;

        let sections = Self::settings_sections();
        let active = if sections.contains(&self.settings_section) { self.settings_section } else { SettingsSection::Appearance };
        let query = self.settings_search.read(cx).value().trim().to_string();
        let searching = !query.is_empty();

        let (prev_sec, mut sec_gen) = self.settings_sec_anim.get();
        if prev_sec != active {
            sec_gen += 1;
            self.settings_sec_anim.set((active, sec_gen));
        }

        let body: AnyElement = if searching {
            self.render_settings_search(&sections, &query, compact, cx)
        } else {
            self.render_settings_category(active, content_w, compact, sec_gen, cx)
        };

        let scroll_area = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                v_flex()
                    .id("settings-content")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.settings_scroll)
                    .when(vp_w < 920., |d| d.px_5())
                    .when(vp_w >= 920., |d| d.px_8())
                    .pt_7()
                    .pb(theme::u(120.))
                    .child(v_flex().w_full().max_w(theme::u(780.)).mx_auto().child(body)),
            )
            .child(self.smooth_scroll_layer(ScrollTarget::Handle(SETTINGS_SCROLL_KEY, self.settings_scroll.clone()), cx));

        h_flex()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.settings_focus)
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    if !this.settings_search.read(cx).value().trim().is_empty() {
                        this.settings_search.update(cx, |s, cx| s.set_value("", w, cx));
                        this.settings_focus.focus(w, cx);
                    } else {
                        this.settings_open = false;
                        this.focus.focus(w, cx);
                    }
                    cx.notify();
                }
            }))
            .child(self.render_settings_nav(&sections, active, searching, narrow_nav, cx))
            .child(scroll_area)
            .into_any_element()
    }

    fn render_settings_category(
        &self,
        active: SettingsSection,
        content_w: f32,
        compact: bool,
        sec_gen: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let modified = active.modified(self);
        let reset_btn = match active {
            SettingsSection::Appearance if modified => Some(
                Button::new("settings-reset-appearance")
                    .label(tr("Reset defaults"))
                    .icon(gpui_kit::assets::IconName::Undo2)
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.settings.reset_appearance();
                        this.motion_check = None;
                        crate::i18n::set(&this.settings.language);
                        this.apply_language(w, cx);
                        this.apply_appearance(w, cx);
                        this.status = tr("Appearance settings reset to defaults").into();
                    }))
                    .into_any_element(),
            ),
            SettingsSection::Editor if modified => Some(
                Button::new("settings-reset-editor")
                    .label(tr("Reset defaults"))
                    .icon(gpui_kit::assets::IconName::Undo2)
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.settings.reset_editor();
                        this.apply_settings(w, cx);
                        this.status = tr("Editor settings reset to defaults").into();
                    }))
                    .into_any_element(),
            ),
            _ => None,
        };
        let header = h_flex()
            .w_full()
            .gap_3()
            .items_start()
            .pb_4()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr(active.title())))
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(tr(active.description()))),
            )
            .children(reset_btn);
        let col = v_flex()
            .w_full()
            .gap_5()
            .child(header)
            .child(self.settings_section_content(active, content_w, compact, cx));
        if sec_gen > 0 && !cx.reduce_motion() {
            col.with_animation(
                SharedString::from(format!("settings-sec-{sec_gen}")),
                Animation::new(Duration::from_millis(150)).with_easing(gpui_kit::ease_out_quint()),
                |el, t| el.opacity(0.25 + 0.75 * t).mt(px((1. - t) * 6.)),
            )
            .into_any_element()
        } else {
            col.into_any_element()
        }
    }

    fn render_settings_nav(
        &self,
        sections: &[SettingsSection],
        active: SettingsSection,
        searching: bool,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let close_btn = Button::new("settings-close")
            .icon(gpui_kit::component::IconName::Close)
            .xsmall()
            .ghost()
            .tooltip(tr("Close"))
            .on_click(cx.listener(|this, _, w, cx| {
                this.settings_open = false;
                this.focus.focus(w, cx);
                cx.notify();
            }));
        let mut nav = v_flex()
            .w(theme::u(if narrow { 188. } else { 240. }))
            .h_full()
            .flex_shrink_0()
            .bg(c(theme::BG_SIDE))
            .border_r_1()
            .border_color(c(theme::BORDER))
            .py_3()
            .px_2p5()
            .gap_1()
            .child(
                h_flex()
                    .pl_2()
                    .pr_1()
                    .pb_1()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Settings")))
                    .child(close_btn),
            )
            .child(
                div().px_1().pb_2().child(
                    Input::new(&self.settings_search)
                        .small()
                        .cleanable(true)
                        .prefix(Icon::new(gpui_kit::component::IconName::Search).xsmall().text_color(c(theme::FG_MUTED))),
                ),
            );
        let mut list = v_flex().id("settings-nav-list").flex_1().min_h_0().overflow_y_scroll().gap(px(2.));
        for (gi, grp) in NavGroup::ALL.into_iter().enumerate() {
            let group_secs: Vec<SettingsSection> = sections.iter().copied().filter(|s| s.group() == grp).collect();
            if group_secs.is_empty() {
                continue;
            }
            list = list.child(
                div()
                    .px_2p5()
                    .when(gi > 0, |d| d.pt_3())
                    .pb_1()
                    .text_size(theme::u(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_DIM))
                    .child(tr(grp.label())),
            );
            for sec in group_secs {
                let on = !searching && sec == active;
                let modified = sec.modified(self);
                list = list.child(
                    div()
                        .relative()
                        .id(SharedString::from(format!("settings-nav-{}", sec.title())))
                        .h(theme::u(32.))
                        .px_2p5()
                        .rounded(theme::u(6.))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(on, |d| d.bg(c(theme::SELECT)))
                        .when(!on, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                        .when(on, |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .left(px(0.))
                                    .top(theme::u(7.))
                                    .bottom(theme::u(7.))
                                    .w(px(2.))
                                    .rounded_full()
                                    .bg(c(theme::ACCENT)),
                            )
                        })
                        .child(sec.icon().size(theme::u(14.)).text_color(c(if on { theme::ACCENT_TEXT } else { theme::FG_MUTED })))
                        .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(if on { theme::FG_STRONG } else { theme::FG })).child(tr(sec.nav_label())))
                        .when(modified, |d| d.child(div().size(px(6.)).rounded_full().bg(c(theme::WARN))))
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.settings_section = sec;
                            if !this.settings_search.read(cx).value().is_empty() {
                                this.settings_search.update(cx, |s, cx| s.set_value("", w, cx));
                            }
                            this.settings_scroll.set_offset(point(px(0.), px(0.)));
                            cx.notify();
                        })),
                );
            }
        }
        nav = nav.child(list);
        nav.into_any_element()
    }

    fn settings_entries(&self, section: SettingsSection, cx: &mut Context<Self>) -> Vec<SettingEntry> {
        let s = &self.settings;
        match section {
            SettingsSection::Appearance => self.appearance_entries(s, cx),
            SettingsSection::Editor => self.editor_entries(s, cx),
            SettingsSection::Agents => self.agent_entries(s, cx),
            SettingsSection::Wsl => vec![SettingEntry::inline(
                "WSL",
                "Read WSL folders through a helper in the distro",
                tr("Faster: the helper reads the distro's own disk instead of Windows' 9p share. It is FluxCode itself, built for Linux, taken from this build's release and installed once in ~/.fluxcode/helper. Off: folders are read through \\\\wsl.localhost, which works but is slower.").into(),
                "wsl linux distro helper 9p share",
                settings_toggle("wslh", "Read WSL folders through a helper in the distro", s.wsl_helper, cx, |w| w.settings.wsl_helper = !w.settings.wsl_helper),
            )],
            SettingsSection::Windows => {
                let on = crate::shell::is_registered();
                let controls = h_flex()
                    .gap_2()
                    .items_center()
                    .when(on, |h| {
                        h.child(Button::new("shell-defaults").label(tr("Default apps…")).xsmall().ghost().on_click(|_, _, cx| cx.open_url("ms-settings:defaultapps")))
                    })
                    .child(settings_switch("shell-open-with", "Open with FluxCode", on, cx, move |this, _, cx| {
                        let r = if on { crate::shell::unregister() } else { crate::shell::register() };
                        this.status = match (r, on) {
                            (Ok(()), false) => tr("FluxCode is in Explorer's right-click menu and in Open with").into(),
                            (Ok(()), true) => tr("Removed FluxCode from Explorer's right-click menu and Open with").into(),
                            (Err(e), _) => crate::trf!("Could not change the Explorer entries: {0}"; 0 => e).into(),
                        };
                        cx.notify();
                    }))
                    .into_any_element();
                vec![SettingEntry::inline(
                    "Windows",
                    "Open with FluxCode",
                    tr("Adds \"Open with FluxCode\" to the right-click menu of files and folders, and FluxCode to Open with for code and text files (.js, .ts, .json, .html, .css, .py and more). Types no app opens yet open in FluxCode on a double-click. For the rest, Windows asks you: pick FluxCode once in Open with, tick Always, or set it in Default apps. A file opened this way joins the FluxCode window that is already open.").into(),
                    "windows explorer context menu open with default apps",
                    controls,
                )]
            }
            SettingsSection::Updates => {
                let update_controls = h_flex()
                    .gap_2()
                    .items_center()
                    .child(Button::new("upd-now").label(tr("Check now")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| this.check_updates_now(cx))))
                    .child(settings_toggle("upd", "Check for new builds", s.update_check, cx, |w| w.settings.update_check = !w.settings.update_check))
                    .into_any_element();
                vec![SettingEntry::inline(
                    "Updates",
                    "Check for new builds",
                    crate::trf!("FluxCode asks github.com for its list of releases at start and every 6 hours. Nothing about you, this PC or your files is sent. This is {0}."; 0 => crate::update::BUILD).into(),
                    "updates releases build check github",
                    update_controls,
                )]
            }
            _ => Vec::new(),
        }
    }

    fn appearance_entries(&self, s: &Settings, cx: &mut Context<Self>) -> Vec<SettingEntry> {
        let lang = s.language.clone();
        let lang_controls = settings_segmented([("auto", tr("Automatic")), ("en", "English"), ("id", "Bahasa Indonesia"), ("fr", "Français"), ("ms", "Bahasa Melayu")].into_iter().map(|(id, name)| {
            settings_pill(SharedString::from(format!("lang-{id}")), name, lang == id, cx.listener(move |this, _, w, cx| {
                this.settings.language = id.into();
                crate::i18n::set(id);
                this.apply_language(w, cx);
                this.apply_settings(w, cx);
            }))
        }));
        let size = s.ui_size;
        let size_controls = settings_segmented(UiSize::ALL.iter().map(|z| {
            let z = *z;
            settings_pill(SharedString::from(format!("ui-size-{}", z.label())), tr(z.label()), z == size, cx.listener(move |this, _, w, cx| {
                this.settings.ui_size = z;
                this.apply_appearance(w, cx);
            }))
        }));
        let bg_state: (SharedString, bool) = match (&s.bg_image, s.background()) {
            (None, _) => (tr("No image. Panels use the theme's solid colors.").into(), false),
            (Some(p), None) => (crate::trf!("{0} was not found. Choose another image or remove it."; 0 => p.display()).into(), true),
            (Some(p), Some(_)) => (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(), false),
        };
        let bg_controls = h_flex()
            .gap_2()
            .items_center()
            .child(Button::new("bg-choose").label(if s.bg_image.is_some() { tr("Change image…") } else { tr("Choose image…") }).xsmall().on_click(cx.listener(|this, _, w, cx| this.choose_background(w, cx))))
            .when(s.bg_image.is_some(), |h| {
                h.child(Button::new("bg-remove").label(tr("Remove")).xsmall().ghost().on_click(cx.listener(|this, _, w, cx| {
                    this.settings.bg_image = None;
                    this.apply_appearance(w, cx);
                    this.status = tr("Background image removed").into();
                })))
            })
            .into_any_element();
        let vis = s.bg_visibility;
        let vis_controls = settings_segmented(Visibility::ALL.iter().map(|v| {
            let v = *v;
            settings_pill(SharedString::from(format!("bg-vis-{}", v.label())), tr(v.label()), v == vis, cx.listener(move |this, _, w, cx| {
                this.settings.bg_visibility = v;
                this.apply_appearance(w, cx);
            }))
        }));
        let vis_desc: SharedString = match vis {
            Visibility::Strong => tr("Strong: the picture shows clearly. Over very bright pictures, dim text (line numbers, hints) can drop below comfortable contrast.").into(),
            _ => tr("How much of the picture shows through the panels. The code side of the editor always gets less of it than the right edge.").into(),
        };
        let none_btn = Button::new("sc-none")
            .label(tr("None"))
            .xsmall()
            .when(s.bg_scene == "none", |b| b.primary())
            .on_click(cx.listener(|this, _, w, cx| {
                this.settings.bg_scene = "none".into();
                this.apply_appearance(w, cx);
            }))
            .into_any_element();
        let scene_desc: SharedString = if s.background().is_some() {
            tr("Your background image shows instead. Remove it to see the scene.").into()
        } else {
            tr("Twelve scenes drawn live in your theme's colors: change the theme and the scene changes with it. Nothing to download, and they fit any window size.").into()
        };
        let mut out = vec![
            SettingEntry::stacked("Display & language", "Language", tr("Menus, panels and messages. Automatic follows Windows.").into(), "language locale indonesia english french malay", lang_controls),
            SettingEntry::inline("Display & language", "Interface size", tr("Menus, tabs, the explorer, panels and icons. The editor font has its own size below.").into(), "ui scale zoom size compact comfortable", size_controls),
            SettingEntry::inline(
                "Display & language",
                "Liquid glass",
                tr("Translucent panels over a blurred window, with floating cards. Off gives solid panels (a little lighter on the GPU).").into(),
                "glass acrylic blur transparency",
                settings_switch("glass", "Liquid glass", s.glass, cx, |this, w, cx| {
                    this.settings.glass = !this.settings.glass;
                    this.apply_appearance(w, cx);
                }),
            ),
            SettingEntry::inline(
                "Backdrop & scenes",
                "Background image",
                if bg_state.1 { bg_state.0.clone() } else { crate::trf!("A picture of your own behind the panels. Stays on your PC. Current: {0}"; 0 => bg_state.0).into() },
                "wallpaper photo image background",
                bg_controls,
            ),
            SettingEntry::inline(
                "Backdrop & scenes",
                "Animated background",
                scene_desc,
                "scene clouds ocean mountains aurora forest lake beach dunes city rain snow galaxy",
                none_btn,
            ),
            SettingEntry::inline(
                "Backdrop & scenes",
                "Background animation",
                tr("Off by default: the scene stands still and costs nothing while you work. On, it moves smoothly at up to 60 frames per second (pauses in the background and respects Windows Reduce Motion). Turning it on checks what your PC can take.").into(),
                "motion 60fps animation cpu reduce motion",
                settings_switch("sc-anim", "Background animation", s.bg_motion, cx, |this, w, cx| this.toggle_bg_motion(w, cx)),
            ),
        ];
        if s.backdrop_visibility().is_some() {
            out.push(SettingEntry::inline("Backdrop & scenes", "Background visibility", vis_desc, "opacity visibility soft balanced strong", vis_controls));
        }
        out
    }

    fn editor_entries(&self, s: &Settings, cx: &mut Context<Self>) -> Vec<SettingEntry> {
        let fonts: &[&'static str] = if cfg!(windows) {
            &["Cascadia Code", "Cascadia Mono", "Consolas", "Courier New"]
        } else {
            &["DejaVu Sans Mono", "Liberation Mono", "Noto Sans Mono", "Ubuntu Mono"]
        };
        let current_font = s.font_family.to_string();
        let font_controls = settings_segmented(fonts.iter().map(|f| {
            let f = *f;
            settings_pill(SharedString::from(format!("font-{f}")), f, current_font == f, cx.listener(move |this, _, w, cx| {
                this.settings.font_family = f.into();
                this.apply_settings(w, cx);
            }))
        }));
        vec![
            SettingEntry::stacked("Typography & layout", "Font", tr("The editor and terminal font. Fonts that are not installed fall back to the system's monospace font.").into(), "font family typeface monospace", font_controls),
            SettingEntry::inline("Typography & layout", "Font size", tr("Font size of the editor and terminal, in pixels.").into(), "font size zoom px", settings_stepper("fs", format!("{} px", s.font_size), s.font_size > 8., s.font_size < 32., cx, |w, d| {
                w.settings.font_size = (w.settings.font_size + d as f32).clamp(8., 32.)
            })),
            SettingEntry::inline("Typography & layout", "Tab size", tr("Spaces per indentation level (applies to newly opened files).").into(), "tab indent spaces", settings_stepper("ts", s.tab_size.to_string(), s.tab_size > 1, s.tab_size < 8, cx, |w, d| {
                w.settings.tab_size = (w.settings.tab_size as i32 + d).clamp(1, 8) as usize
            })),
            SettingEntry::inline("Typography & layout", "Word wrap", tr("Wrap long lines at the edge of the editor.").into(), "wrap soft line", settings_toggle("ww", "Word wrap", s.word_wrap, cx, |w| w.settings.word_wrap = !w.settings.word_wrap)),
            SettingEntry::inline("Typography & layout", "Line numbers", tr("Numbers in the gutter left of the code.").into(), "gutter line numbers", settings_toggle("ln", "Line numbers", s.line_numbers, cx, |w| w.settings.line_numbers = !w.settings.line_numbers)),
            SettingEntry::inline("Typography & layout", "Code folding", tr("Arrows in the gutter that fold a block away.").into(), "fold collapse gutter", settings_toggle("fold", "Code folding", s.folding, cx, |w| w.settings.folding = !w.settings.folding)),
            SettingEntry::inline("Typography & layout", "Whitespace dots", tr("Show each space as a faint dot, next to the guide lines that mark how far each block reaches.").into(), "spaces dots indent guides", settings_toggle("wsd", "Whitespace dots", s.show_whitespace, cx, |w| w.settings.show_whitespace = !w.settings.show_whitespace)),
            SettingEntry::inline("Editing & scrolling", "Auto save", tr("Save a file shortly after you stop typing.").into(), "save autosave", settings_toggle("as", "Auto save", s.auto_save, cx, |w| w.settings.auto_save = !w.settings.auto_save)),
            SettingEntry::inline("Editing & scrolling", "Close brackets and quotes", tr("Typing ( [ { or a quote adds its closing pair.").into(), "brackets quotes autopair", settings_toggle("acl", "Close brackets and quotes", s.auto_close, cx, |w| w.settings.auto_close = !w.settings.auto_close)),
            SettingEntry::inline("Editing & scrolling", "Smart indent", tr("A new line keeps the indentation, and indents one more level after { or :.").into(), "indent smart auto", settings_toggle("si", "Smart indent", s.smart_indent, cx, |w| w.settings.smart_indent = !w.settings.smart_indent)),
            SettingEntry::inline("Editing & scrolling", "Cursor padding", tr("Lines kept visible above and below the cursor as it moves (0: the editor's default).").into(), "scrolloff cursor padding", settings_stepper("cp", s.cursor_padding.to_string(), s.cursor_padding > 0, s.cursor_padding < 12, cx, |w, d| {
                w.settings.cursor_padding = (w.settings.cursor_padding as i32 + d).clamp(0, 12) as u32
            })),
            SettingEntry::inline("Editing & scrolling", "Scroll past the last line", tr("Leave room below the end of a file, so its last lines can sit mid-screen.").into(), "scroll beyond last line", settings_toggle("spe", "Scroll past the last line", s.scroll_past_end, cx, |w| w.settings.scroll_past_end = !w.settings.scroll_past_end)),
            SettingEntry::inline("Completions", "Copilot suggestions", tr("Ghost-text completions from GitHub Copilot, a moment after you stop typing. Tab accepts, Esc dismisses. The Copilot language server does the work in its own process, so the editor stays light.").into(), "copilot ai autocomplete ghost", settings_toggle("cpl", "Copilot suggestions", s.copilot, cx, |w| w.settings.copilot = !w.settings.copilot)),
        ]
    }

    fn agent_entries(&self, s: &Settings, cx: &mut Context<Self>) -> Vec<SettingEntry> {
        let access = s.access().to_string();
        let access_controls = settings_segmented([("read", tr("Read only")), ("write", tr("Read & write")), ("full", tr("Full access"))].into_iter().map(|(id, name)| {
            settings_pill(SharedString::from(format!("access-{id}")), name, access == id, cx.listener(move |this, _, w, cx| {
                this.settings.agent_access = id.into();
                this.apply_settings(w, cx);
            }))
        }));
        vec![
            SettingEntry::inline("Permissions & behavior", "AI agent: access", tr("Read only: it reads and explains, edits and commands are refused. Read & write: it edits files (Undo is under the chat), commands ask first. Full access: nothing asks.").into(), "agent access permissions", access_controls),
            SettingEntry::inline("Permissions & behavior", "AI agent: turn off when idle", tr("After 10 idle minutes the agent process stops and frees its memory (OpenCode uses several hundred MB). Your next message starts it again and continues the same chat.").into(), "idle memory agent timeout", settings_toggle("idle", "AI agent: turn off when idle", s.agent_idle_min > 0, cx, |w| w.settings.agent_idle_min = if w.settings.agent_idle_min == 0 { 10 } else { 0 })),
            SettingEntry::inline("Permissions & behavior", "Agent panel on the right", tr("Dock the agent chat beside the editor instead of in the side bar. The arrow button in its header does the same.").into(), "agent dock right panel", settings_toggle("ar", "Agent panel on the right", s.agent_right, cx, |w| w.settings.agent_right = !w.settings.agent_right)),
            SettingEntry::inline("Permissions & behavior", "Follow the agent's edits", tr("When the agent changes a file, it opens in the editor and the change is typed in line by line while the view follows it. Takes under a second and never slows the agent.").into(), "agent follow edits typing", settings_toggle("follow", "Follow the agent's edits", s.agent_follow_edits, cx, |w| w.settings.agent_follow_edits = !w.settings.agent_follow_edits)),
            SettingEntry::stacked("Accounts", "GitHub", tr("Sign in so gh, git and OpenCode inside FluxCode can use your account (GH_TOKEN).").into(), "github token cli account login", self.render_github(cx)),
            SettingEntry::stacked("Accounts", "GitHub Copilot", tr("Sign in so the editor can suggest the next lines of code. Needs the Copilot language server installed once, and a Copilot plan (a free plan exists).").into(), "copilot github login server", self.render_copilot(cx)),
        ]
    }

    fn settings_section_content(&self, section: SettingsSection, content_w: f32, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.settings;
        match section {
            SettingsSection::Appearance => {
                let mut entries = self.appearance_entries(s, cx);
                let idx_scene = entries.iter().position(|e| e.title == "Animated background").unwrap_or(entries.len());
                let display_rows: Vec<AnyElement> = entries.drain(..idx_scene).map(|e| e.render(compact)).collect();
                let mut scene_rows: Vec<AnyElement> = Vec::new();
                for e in entries {
                    let is_scene = e.title == "Animated background";
                    let is_motion = e.title == "Background animation";
                    scene_rows.push(e.render(compact));
                    if is_scene {
                        scene_rows.push(div().px_4().pb_4().child(scene_cards(scene_grid_cols(content_w), &s.bg_scene, cx)).into_any_element());
                    }
                    if is_motion {
                        if let Some(banner) = self.render_motion_banner(cx) {
                            scene_rows.push(banner);
                        }
                    }
                }
                v_flex()
                    .w_full()
                    .gap_5()
                    .child(settings_group("Theme", vec![div().p_4().child(theme_cards(theme_grid_cols(content_w), cx)).into_any_element()]))
                    .child(settings_group("Display & language", display_rows))
                    .child(settings_group("Backdrop & scenes", scene_rows))
                    .into_any_element()
            }
            SettingsSection::Editor | SettingsSection::Agents => self.grouped_entries(self.settings_entries(section, cx), compact),
            SettingsSection::Wsl | SettingsSection::Windows | SettingsSection::Updates => {
                let rows = self.settings_entries(section, cx).into_iter().map(|e| e.render(compact)).collect();
                settings_card(rows)
            }
            SettingsSection::SoulFiles => settings_card(vec![div().p_4().child(self.render_soul_files(cx)).into_any_element()]),
            SettingsSection::Shortcuts => self.render_shortcuts_card(None),
            SettingsSection::About => self.render_about_section(),
        }
    }

    fn grouped_entries(&self, entries: Vec<SettingEntry>, compact: bool) -> AnyElement {
        let mut groups: Vec<(&'static str, Vec<AnyElement>)> = Vec::new();
        for e in entries {
            let grp = e.group;
            let row = e.render(compact);
            if let Some(last) = groups.last_mut().filter(|g| g.0 == grp) {
                last.1.push(row);
            } else {
                groups.push((grp, vec![row]));
            }
        }
        v_flex().w_full().gap_5().children(groups.into_iter().map(|(g, rows)| settings_group(g, rows))).into_any_element()
    }

    fn render_motion_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let check = self.motion_check.as_ref()?;
        use crate::motion_check::{Check, Level};
        let (tok, text): (theme::Tok, SharedString) = match check {
            Check::Running => (theme::FG_MUTED, tr("Checking what your PC can take: watching FluxCode's CPU use for a few seconds…").into()),
            Check::Done(Level::Light, t) => (theme::GREEN, t.clone().into()),
            Check::Done(Level::Fine, t) => (theme::WARN, t.clone().into()),
            Check::Done(Level::Heavy, t) => (theme::DANGER, t.clone().into()),
        };
        let heavy = matches!(check, Check::Done(Level::Heavy, _));
        Some(
            h_flex()
                .px_4()
                .pb_3()
                .gap_3()
                .items_center()
                .child(div().flex_none().size(px(7.)).rounded_full().bg(c(tok)))
                .child(div().flex_1().min_w_0().text_xs().text_color(c(tok)).child(text))
                .when(heavy, |h| {
                    h.child(Button::new("sc-anim-off").label(tr("Turn it off")).xsmall().on_click(cx.listener(|this, _, w, cx| {
                        if this.settings.bg_motion {
                            this.toggle_bg_motion(w, cx);
                        }
                    })))
                })
                .into_any_element(),
        )
    }

    fn render_shortcuts_card(&self, filter: Option<&str>) -> AnyElement {
        let list: Vec<_> = crate::commands::commands()
            .into_iter()
            .filter(|k| !k.keys.is_empty())
            .filter(|k| filter.is_none_or(|q| matches_query(q, &[k.label, tr(k.label), k.keys])))
            .collect();
        if list.is_empty() {
            return div().into_any_element();
        }
        let rows = list
            .into_iter()
            .enumerate()
            .map(|(i, k)| {
                h_flex()
                    .w_full()
                    .px_4()
                    .py_2p5()
                    .gap_3()
                    .when(i % 2 == 1, |h| h.bg(c(theme::BG_SIDE).opacity(0.4)))
                    .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG)).child(tr(k.label)))
                    .child(crate::quick::keys_chips(k.keys))
                    .into_any_element()
            })
            .collect();
        settings_card(rows)
    }

    fn render_about_section(&self) -> AnyElement {
        let engine = if crate::accel::zig_enabled() { tr("Rust + Zig SIMD") } else { tr("Pure Rust") };
        v_flex()
            .w_full()
            .gap_4()
            .child(settings_card(vec![
                div()
                    .p_4()
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2p5()
                            .text_xs()
                            .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(tr("FluxCode is open source under the GPL-3.0 license and still in development, so expect rough edges and frequent builds. The code, every build and the issue tracker are on GitHub.")))
                            .child(div().text_color(c(theme::FG_MUTED)).child(tr("A MuaraAI project. Copyright (C) 2026 MuaraAI and contributors.")))
                            .child(
                                h_flex()
                                    .pt_1()
                                    .gap_2()
                                    .flex_wrap()
                                    .child(Button::new("about-repo").label(tr("Open the repository")).xsmall().on_click(|_, _, cx| cx.open_url(REPO_URL)))
                                    .child(Button::new("about-issue").label(tr("Report a problem")).xsmall().ghost().on_click(|_, _, cx| cx.open_url(ISSUES_URL)))
                                    .child(Button::new("about-changes").label(tr("What changed")).xsmall().ghost().on_click(|_, w, cx| w.dispatch_action(Box::new(crate::workbench::ShowWhatsNew), cx))),
                            )
                            .child(div().text_color(c(theme::FG_DIM)).child(format!("github.com/MuaraAI/FluxCode · {}", crate::update::BUILD))),
                    )
                    .into_any_element(),
                settings_row_inline(
                    tr("Engine"),
                    tr("Native GPU UI with SIMD acceleration for large files and search.").into(),
                    div().px_2p5().py_1().rounded(theme::u(6.)).border_1().border_color(c(theme::BORDER)).bg(c(theme::BG_SIDE)).text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(engine).into_any_element(),
                ),
            ]))
            .child(div().px_1().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("Stored in {0}"; 0 => config_dir().join("settings.json").display())))
            .into_any_element()
    }

    fn render_settings_search(&self, sections: &[SettingsSection], query: &str, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut results = v_flex().w_full().gap_5();
        let mut total = 0usize;
        let open_btn = |sec: SettingsSection, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("sr-open-{}", sec.title())))
                .label(tr("Open section"))
                .xsmall()
                .ghost()
                .on_click(cx.listener(move |this, _, w, cx| {
                    this.settings_section = sec;
                    this.settings_search.update(cx, |s, cx| s.set_value("", w, cx));
                    this.settings_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }))
        };
        for &sec in sections {
            let mut rows: Vec<AnyElement> = Vec::new();
            for entry in self.settings_entries(sec, cx) {
                if matches_query(query, &[sec.title(), tr(sec.title()), entry.group, tr(entry.group), entry.title, tr(entry.title), &entry.desc, entry.keywords]) {
                    rows.push(entry.render(compact));
                }
            }
            if sec == SettingsSection::Appearance && matches_query(query, &["theme color palette dark light FluxDark FluxCream FluxMono FluxRed FluxMuara FluxSky", tr("Theme")]) {
                rows.insert(
                    0,
                    settings_row_inline(
                        tr("Theme"),
                        tr("Color scheme and surface palette across panels and editor.").into(),
                        open_btn(SettingsSection::Appearance, cx).into_any_element(),
                    ),
                );
            }
            if sec == SettingsSection::SoulFiles && matches_query(query, &[sec.title(), tr(sec.title()), sec.description(), tr(sec.description()), "claude codex mura opencode gemini soul"]) {
                rows.push(div().p_4().child(self.render_soul_files(cx)).into_any_element());
            }
            if sec == SettingsSection::About && matches_query(query, &[sec.title(), tr(sec.title()), "version license github engine rust zig simd", crate::update::BUILD]) {
                rows.push(settings_row_inline(tr("About FluxCode"), tr(sec.description()).into(), open_btn(SettingsSection::About, cx).into_any_element()));
            }
            if !rows.is_empty() {
                total += rows.len();
                results = results.child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            h_flex()
                                .px_1()
                                .gap_2()
                                .items_center()
                                .child(sec.icon().size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
                                .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr(sec.title())))
                                .child(open_btn(sec, cx)),
                        )
                        .child(settings_card(rows)),
                );
            }
            if sec == SettingsSection::Shortcuts {
                let has_keys = crate::commands::commands().into_iter().any(|k| !k.keys.is_empty() && matches_query(query, &[k.label, tr(k.label), k.keys]));
                if has_keys {
                    total += 1;
                    results = results.child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                h_flex()
                                    .px_1()
                                    .gap_2()
                                    .items_center()
                                    .child(sec.icon().size(theme::u(13.)).text_color(c(theme::FG_MUTED)))
                                    .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr(sec.title())))
                                    .child(open_btn(sec, cx)),
                            )
                            .child(self.render_shortcuts_card(Some(query))),
                    );
                }
            }
        }
        if total == 0 {
            return settings_card(vec![v_flex()
                .w_full()
                .py_8()
                .px_4()
                .gap_3()
                .items_center()
                .child(Icon::new(gpui_kit::component::IconName::Search).size(theme::u(20.)).text_color(c(theme::FG_DIM)))
                .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(crate::trf!("No settings match \"{q}\""; q => query)))
                .child(Button::new("sr-clear").label(tr("Clear search")).xsmall().on_click(cx.listener(|this, _, w, cx| {
                    this.settings_search.update(cx, |s, cx| s.set_value("", w, cx));
                    this.settings_focus.focus(w, cx);
                    cx.notify();
                })))
                .into_any_element()]);
        }
        v_flex()
            .w_full()
            .gap_5()
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .pb_3()
                    .border_b_1()
                    .border_color(c(theme::BORDER))
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Search results")))
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(crate::trf!("Showing matches for \"{q}\""; q => query))),
            )
            .child(results)
            .into_any_element()
    }
}

impl Workbench {
    fn render_soul_files(&self, cx: &mut Context<Self>) -> AnyElement {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
        let installed = crate::acp::installed();
        if installed.is_empty() {
            return div().w_full().max_w(theme::u(760.)).py_2().text_xs().text_color(c(theme::FG_DIM)).child(tr("No agent installed yet. The agent panel (Ctrl+L) lists agents to download.")).into_any_element();
        }
        let mut list = v_flex().w_full().max_w(theme::u(760.));
        for id in installed {
            let mut files = v_flex().gap_1();
            if id == "mura" {
                files = files.child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("Mura's character is built into the program. It follows these project files:")));
            }
            for (k, place) in crate::acp::instruction_files(&id).iter().enumerate() {
                let (label, path) = match *place {
                    crate::acp::Place::Home(rel) => (format!("~/{rel}"), home.as_ref().map(|h| rel.split('/').fold(h.clone(), |p, s| p.join(s)))),
                    crate::acp::Place::Project(rel) => (format!("{}/{rel}", tr("project")), self.root.as_ref().map(|r| rel.split('/').fold(r.clone(), |p, s| p.join(s)))),
                };
                let exists = path.as_ref().is_some_and(|p| p.is_file());
                let key = SharedString::from(format!("soul-{id}-{k}"));
                files = files.child(
                    h_flex()
                        .gap_2()
                        .child(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::ScrollText).size(theme::u(13.)).text_color(c(if exists { theme::FG_MUTED } else { theme::FG_DIM })))
                        .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(if exists { theme::FG } else { theme::FG_DIM })).child(label))
                        .when_some(path, |h, p| {
                            h.child(
                                Button::new(key)
                                    .label(if exists { tr("Open") } else { tr("Create") })
                                    .xsmall()
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        if !p.is_file() {
                                            if let Some(d) = p.parent() {
                                                let _ = std::fs::create_dir_all(d);
                                            }
                                            let _ = std::fs::write(&p, "");
                                        }
                                        this.settings_open = false;
                                        this.open_file(p.clone(), w, cx);
                                    })),
                            )
                        }),
                );
            }
            list = list.child(
                h_flex()
                    .w_full()
                    .py_2()
                    .gap_4()
                    .items_start()
                    .border_b_1()
                    .border_color(c(theme::BORDER))
                    .child(div().w(theme::u(150.)).flex_none().text_sm().text_color(c(theme::FG_STRONG)).child(crate::acp::display_name(&id)))
                    .child(div().flex_1().min_w_0().child(files)),
            );
        }
        list.into_any_element()
    }

    fn github_result(&mut self, r: Result<crate::github::Account, String>, cx: &mut Context<Self>) {
        match r {
            Ok(a) => {
                self.github = Some(a);
                self.status = tr("Signed in to GitHub").into();
            }
            Err(e) => self.status = e.into(),
        }
        cx.notify();
    }

    fn copilot_signin_result(&mut self, r: Result<String, String>, cx: &mut Context<Self>) {
        match r {
            Ok(code) if code.is_empty() => self.status = tr("Signed in to Copilot").into(),
            Ok(code) => {
                cx.write_to_clipboard(ClipboardItem::new_string(code));
                self.status = tr("Sign-in code copied: paste it on the GitHub page that opened, then click Authorize.").into();
            }
            Err(e) => self.status = e.into(),
        }
        cx.notify();
    }

    #[allow(clippy::too_many_lines)]
    fn render_copilot(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::copilot::Status;
        let (text, show_signin, show_signout): (SharedString, bool, bool) = match self.copilot.status() {
            Status::Off => (tr("Off. Turn Copilot suggestions on in the Editor section.").into(), false, false),
            Status::Starting => (tr("Checking Copilot…").into(), false, false),
            Status::NotInstalled => (
                tr("Not installed. Install it once with: npm install -g @github/copilot-language-server (Node.js 20.8 or later).").into(),
                false,
                false,
            ),
            Status::Ready => (
                match self.copilot.user() {
                    Some(u) => crate::trf!("Ready, signed in as {user}. Suggestions appear as ghost text a moment after you stop typing; Tab accepts."; user => u).into(),
                    None => tr("Ready. Suggestions appear as ghost text a moment after you stop typing.").into(),
                },
                false,
                true,
            ),
            Status::NotSignedIn(m) => (
                if m.is_empty() { tr("Not signed in to Copilot yet.").into() } else { m.into() },
                true,
                false,
            ),
            Status::Warning(m) => (
                if m.is_empty() { tr("Copilot has a temporary problem.").into() } else { m.into() },
                false,
                false,
            ),
            Status::Inactive => (tr("Copilot ignores this file.").into(), false, false),
            Status::Error(e) => (e.into(), true, false),
        };
        let code = self.copilot.user_code();
        let mut col = v_flex()
            .w(px(300.))
            .gap_1()
            .text_sm()
            .child(div().child(text));
        if matches!(self.copilot.status(), Status::NotInstalled) || self.copilot_installing {
            if self.copilot_installing {
                col = col.child(install_progress(!cx.reduce_motion()));
            } else if crate::copilot::npm_present() {
                col = col.child(
                    Button::new("cpl-install")
                        .label(tr("Install server"))
                        .xsmall()
                        .loading(self.copilot_installing)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.copilot_installing {
                                return;
                            }
                            this.copilot_installing = true;
                            cx.notify();
                            cx.spawn(async move |this, cx| {
                                let r = cx
                                    .background_spawn(async move {
                                        #[cfg(windows)]
                                        let out = crate::proc::shell_command("npm install -g @github/copilot-language-server").output();
                                        #[cfg(not(windows))]
                                        let out = std::process::Command::new("npm")
                                            .args(["install", "-g", "@github/copilot-language-server"])
                                            .output();
                                        match out {
                                            Ok(o) if o.status.success() => Ok(()),
                                            Ok(o) => Err(crate::copilot::npm_install_error(
                                                &String::from_utf8_lossy(&o.stderr),
                                                &o.status.to_string(),
                                            )),
                                            Err(e) => Err(e.to_string()),
                                        }
                                    })
                                    .await;
                                let installed = this
                                    .update(cx, |this, cx| {
                                        this.copilot_installing = false;
                                        cx.notify();
                                        match r {
                                            Ok(()) => {
                                                this.copilot.retry();
                                                this.status = tr("Copilot server installed.").into();
                                                Some(this.copilot.clone())
                                            }
                                            Err(e) => {
                                                this.status = crate::trf!("Install failed: {e}"; e => e).into();
                                                None
                                            }
                                        }
                                    })
                                    .ok()
                                    .flatten();
                                if let Some(copilot) = installed {
                                    let started = cx.background_spawn(async move { copilot.start() }).await;
                                    _ = this.update(cx, |this, cx| {
                                        if let Err(e) = started {
                                            this.status = e.into();
                                        }
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        })),
                );
            } else {
                col = col.child(
                    Button::new("cpl-node")
                        .label(tr("Get Node.js"))
                        .xsmall()
                        .on_click(|_, _, cx| cx.open_url("https://nodejs.org")),
                );
            }
        }
        if let Some(notice) = self.copilot.notice() {
            col = col.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(notice));
        }
        if let Some(code) = code {
            let copy = code.clone();
            col = col.child(
                v_flex()
                    .gap_1()
                    .child(div().child(crate::trf!("Your sign-in code: {code}"; code => code)))
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(tr("Paste it on github.com/login/device and click Authorize; this page updates by itself.")))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("cpl-copy")
                                    .label(tr("Copy code"))
                                    .xsmall()
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                                    }),
                            )
                            .child(Button::new("cpl-open").label(tr("Open GitHub")).xsmall().on_click(|_, _, cx| cx.open_url("https://github.com/login/device"))),
                    ),
            );
        }
        col = col.child(
            h_flex()
                .gap_1()
                .when(show_signin, |row| {
                    row.child(
                        Button::new("cpl-in")
                            .label(tr("Sign in"))
                            .xsmall()
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let copilot = this.copilot.clone();
                                cx.spawn(async move |this, cx| {
                                    let r =
                                        cx.background_spawn(async move { copilot.sign_in() }).await;
                                    _ = this
                                        .update(cx, |this, cx| this.copilot_signin_result(r, cx));
                                })
                                .detach();
                            })),
                    )
                })
                .when(show_signout, |row| {
                    row.child(
                        Button::new("cpl-out")
                            .label(tr("Sign out"))
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let copilot = this.copilot.clone();
                                cx.spawn(async move |this, cx| {
                                    cx.background_spawn(async move { copilot.sign_out() }).await;
                                    _ = this.update(cx, |this, cx| {
                                        this.status = tr("Signed out of Copilot").into();
                                        cx.notify();
                                    });
                                })
                                .detach();
                            })),
                    )
                }),
        );
        col.into_any_element()
    }

    fn render_github(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(a) = &self.github {
            return h_flex()
                .gap_2()
                .text_sm()
                .child(format!("{} ({})", a.login, if a.source == "gh" { "GitHub CLI" } else { "token" }))
                .child(Button::new("gh-out").label(tr("Sign out")).xsmall().on_click(cx.listener(|this, _, _, cx| {
                    let _ = crate::github::logout();
                    this.github = None;
                    cx.notify();
                })))
                .into_any_element();
        }
        v_flex()
            .w_full()
            .max_w(px(360.))
            .gap_1p5()
            .child(Input::new(&self.github_input).small())
            .child(
                h_flex()
                    .gap_1p5()
                    .flex_wrap()
                    .child(Button::new("gh-token").label(tr("Sign in with token")).xsmall().primary().on_click(cx.listener(|this, _, _, cx| {
                        let t = this.github_input.read(cx).value().to_string();
                        cx.spawn(async move |this, cx| {
                            let r = cx.background_spawn(async move { crate::github::login_token(&t) }).await;
                            _ = this.update(cx, |this, cx| this.github_result(r, cx));
                        })
                        .detach();
                    })))
                    .child(Button::new("gh-cli").label(tr("Use GitHub CLI login")).xsmall().on_click(cx.listener(|_, _, _, cx| {
                        cx.spawn(async move |this, cx| {
                            let r = cx.background_spawn(async move { crate::github::login_cli() }).await;
                            _ = this.update(cx, |this, cx| this.github_result(r, cx));
                        })
                        .detach();
                    }))),
            )
            .into_any_element()
    }
}

fn install_progress(animate: bool) -> AnyElement {
    let bar = div().absolute().top_0().h_full().w(relative(0.35)).rounded_full().bg(c(theme::ACCENT));
    let bar_el = if animate {
        bar.with_animation(
            "cpl-install-bar",
            Animation::new(Duration::from_millis(1300)).repeat().with_easing(gpui_kit::ease_in_out),
            |d, t| d.left(relative(-0.35 + 1.35 * t)),
        )
        .into_any_element()
    } else {
        bar.left(relative(0.32)).into_any_element()
    };
    v_flex()
        .w_full()
        .gap_1p5()
        .child(
            h_flex()
                .gap_2()
                .text_sm()
                .text_color(c(theme::FG_STRONG))
                .child(gpui_kit::component::spinner::Spinner::new().icon(gpui_kit::assets::IconName::LoaderCircle).small().color(c(theme::ACCENT_TEXT)))
                .child(tr("Installing the Copilot server…")),
        )
        .child(
            div()
                .relative()
                .w_full()
                .h(px(3.))
                .rounded_full()
                .overflow_hidden()
                .bg(c(theme::BG_HOVER))
                .child(bar_el),
        )
        .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("npm is downloading @github/copilot-language-server; this usually takes a minute or two. You can keep working.")))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{matches_query, scene_grid_cols, theme_grid_cols, Settings};

    #[test]
    #[cfg(feature = "embedded-preview")]
    fn webview_notice_is_on_by_default() {
        assert!(Settings::default().preview_webview_notice);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(old.preview_webview_notice);
        let off: Settings = serde_json::from_str("{\"preview_webview_notice\":false}").unwrap();
        assert!(!off.preview_webview_notice);
    }

    #[test]
    fn grid_columns_avoid_scene_orphans() {
        for w in [320., 460., 600., 780., 1024.] {
            let cols = scene_grid_cols(w);
            assert_eq!(12 % cols, 0, "12 scenes should divide evenly for width {w} (got {cols})");
        }
        assert_eq!(theme_grid_cols(780.), 3);
        assert_eq!(theme_grid_cols(560.), 2);
        assert_eq!(theme_grid_cols(360.), 1);
    }

    #[test]
    fn search_matches_multi_word_queries() {
        assert!(matches_query("", &["anything"]));
        assert!(matches_query("font size", &["Font size", "Pixels in editor"]));
        assert!(matches_query("liquid glass", &["Appearance", "Translucent Liquid Glass panel"]));
        assert!(!matches_query("font missingword", &["Font size", "Pixels in editor"]));
    }

    #[test]
    fn reset_defaults_restores_section_fields() {
        let mut s = Settings::default();
        s.theme = "flux-sky".into();
        s.glass = false;
        s.bg_motion = true;
        s.font_size = 20.;
        s.copilot = false;

        s.reset_appearance();
        assert_eq!(s.theme, Settings::default().theme);
        assert!(s.glass);
        assert!(!s.bg_motion);
        assert_eq!(s.font_size, 20.);

        s.reset_editor();
        assert_eq!(s.font_size, Settings::default().font_size);
        assert!(s.copilot);
    }
}
