
use crate::i18n::tr;
use crate::theme::{self, c, UiSize, Visibility, PALETTES};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    v_flex, Sizable,
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
        SettingsSection::Wsl,
        SettingsSection::Windows,
        SettingsSection::Agents,
        SettingsSection::SoulFiles,
        SettingsSection::Shortcuts,
        SettingsSection::Updates,
        SettingsSection::About,
    ];

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
        match self {
            SettingsSection::Appearance => s.theme != PALETTES[0].id || s.language != "auto" || s.ui_size != UiSize::default() || s.bg_image.is_some(),
            SettingsSection::Editor => {
                s.font_size != 15.
                    || s.tab_size != 4
                    || s.word_wrap
                    || s.show_whitespace
                    || !s.auto_save
                    || &*s.font_family != "Cascadia Code"
                    || !s.line_numbers
                    || !s.folding
                    || !s.auto_close
                    || !s.smart_indent
                    || s.cursor_padding != 0
                    || !s.scroll_past_end
                    || !s.copilot
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

fn settings_row(title: &'static str, desc: SharedString, control: AnyElement) -> AnyElement {
    h_flex()
        .w_full()
        .py_3()
        .px_4()
        .gap_4()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
                .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc)),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

fn settings_card(rows: Vec<AnyElement>) -> AnyElement {
    let n = rows.len();
    v_flex()
        .w_full()
        .max_w(theme::u(760.))
        .rounded(theme::u(8.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .children(rows.into_iter().enumerate().map(|(i, row)| {
            div()
                .when(i + 1 < n, |d| d.border_b_1().border_color(c(theme::BORDER)))
                .child(row)
        }))
        .into_any_element()
}

fn scene_cards(current: &str, cx: &mut Context<Workbench>) -> Div {
    h_flex().w_full().flex_wrap().gap_2().children(crate::scene::Scene::ALL.into_iter().map(|sc| {
        let on = current == sc.id();
        v_flex()
            .id(SharedString::from(format!("sc-{}", sc.id())))
            .w(theme::u(128.))
            .rounded(px(6.))
            .overflow_hidden()
            .cursor_pointer()
            .border_2()
            .border_color(if on { c(theme::ACCENT) } else { c(theme::BORDER) })
            .hover(move |st| st.border_color(c(if on { theme::ACCENT } else { theme::BORDER_STRONG })))
            .on_click(cx.listener(move |this, _, w, cx| {
                this.settings.bg_scene = sc.id().into();
                this.apply_appearance(w, cx);
            }))
            .child(div().h(theme::u(72.)).w_full().overflow_hidden().child(crate::scene::preview(sc, 12.)))
            .child(
                h_flex()
                    .px_2()
                    .py_1()
                    .gap_1()
                    .bg(c(theme::BG_ELEVATED))
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(if on { theme::FG_STRONG } else { theme::FG_MUTED })).child(tr(sc.label())))
                    .when(on, |h| h.child(gpui_kit::component::Icon::new(gpui_kit::assets::IconName::Check).size(theme::u(12.)).text_color(c(theme::ACCENT_TEXT)))),
            )
    }))
}

fn theme_cards(cx: &mut Context<Workbench>) -> Div {
    let active_theme = theme::palette().id;
    h_flex().w_full().max_w(theme::u(760.)).flex_wrap().gap_3().py_3().children(theme::all().into_iter().map(|p| {
        let on = p.id == active_theme;
        let id = p.id;
        let [surface, side, accent, text] = p.swatches();
        v_flex()
            .id(SharedString::from(format!("theme-{id}")))
            .w(theme::u(176.))
            .rounded(px(6.))
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
                            .p_2()
                            .gap_1()
                            .bg(c(surface))
                            .child(div().w(px(70.)).h(px(4.)).rounded(px(2.)).bg(c(text)))
                            .child(div().w(px(46.)).h(px(4.)).rounded(px(2.)).bg(c(accent)))
                            .child(div().w(px(92.)).h(px(4.)).rounded(px(2.)).bg(c(text)).opacity(0.6)),
                    ),
            )
            .child(
                v_flex()
                    .px_2()
                    .py_2()
                    .gap(px(2.))
                    .bg(c(theme::BG_ELEVATED))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(p.name))
                            .when(on, |h| h.child(div().text_xs().text_color(c(theme::ACCENT_TEXT)).child(tr("Active")))),
                    )
                    .child(div().text_xs().truncate().text_color(c(theme::FG_MUTED)).child(p.note)),
            )
    }))
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
            let wrap = d.wraps(wrap);
            d.editor.update(cx, |s, cx| {
                s.set_soft_wrap(wrap, window, cx);
                s.set_show_whitespaces(dots, window, cx);
                opts.apply(s, window, cx);
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
        if old.id != id {
            theme::fade_from(old);
            cx.spawn_in(window, async move |this, cx| loop {
                cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
                let more = theme::fading();
                let ok = this.update_in(cx, |_, window, cx| {
                    theme::apply(cx);
                    window.refresh();
                    cx.notify();
                });
                if ok.is_err() || !more {
                    break;
                }
            })
            .detach();
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

    fn settings_in_view(&self, sections: &[SettingsSection]) -> SettingsSection {
        let h = &self.settings_scroll;
        if h.bounds_for_item(0).is_none() {
            return self.settings_section;
        }
        let top = h.bounds().top() - h.offset().y + theme::u(96.);
        let mut ix = 0;
        while ix + 1 < sections.len() && h.bounds_for_item(ix).is_some_and(|b| b.bottom() <= top) {
            ix += 1;
        }
        let at_end = h.offset().y <= -h.max_offset().y + px(2.);
        let picked = sections.iter().position(|s| *s == self.settings_section).unwrap_or(0);
        if at_end && picked > ix {
            ix = picked;
        }
        sections[ix]
    }

    pub(crate) fn render_settings(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let sections = Self::settings_sections();
        let active = self.settings_in_view(&sections);
        let blocks = sections.iter().map(|sec| {
            v_flex()
                .w_full()
                .max_w(theme::u(860.))
                .pb(theme::u(40.))
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr(sec.title())))
                .child(div().pt(px(2.)).text_xs().text_color(c(theme::FG_MUTED)).child(tr(sec.description())))
                .child(div().h(theme::u(14.)))
                .child(self.settings_section_content(*sec, cx))
                .into_any_element()
        }).collect::<Vec<_>>();
        h_flex()
            .id("settings")
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .child(self.render_settings_nav(&sections, active, cx))
            .child(
                v_flex()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.settings_scroll)
                    .px_10()
                    .pt_8()
                    .children(blocks)
                    .child(div().flex_none().h(theme::u(360.))),
            )
            .into_any_element()
    }

    fn render_settings_nav(&self, sections: &[SettingsSection], active: SettingsSection, cx: &mut Context<Self>) -> AnyElement {
        let close_btn = self.action_button("settings-close", tr("Close"), crate::ui::Kind::Plain, cx, |this, _, cx| {
            this.settings_open = false;
            cx.notify();
        });
        v_flex()
            .w(theme::u(220.))
            .h_full()
            .flex_shrink_0()
            .bg(c(theme::BG_SIDE))
            .border_r_1()
            .border_color(c(theme::BORDER))
            .py_3()
            .px_2()
            .gap(px(3.))
            .child(
                h_flex()
                    .pl_3()
                    .pb_2()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Settings")))
                    .child(close_btn),
            )
            .children(sections.iter().enumerate().map(|(ix, sec)| {
                let sec = *sec;
                let on = sec == active;
                let modified = sec.modified(self);
                div()
                    .relative()
                    .id(SharedString::from(format!("settings-nav-{ix}")))
                    .h(theme::u(30.))
                    .px_3()
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
                    .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(if on { theme::FG_STRONG } else { theme::FG })).child(tr(sec.nav_label())))
                    .when(modified, |d| d.child(div().size(px(6.)).rounded_full().bg(c(theme::WARN))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_section = sec;
                        this.settings_scroll.scroll_to_top_of_item(ix);
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    fn settings_section_content(&self, section: SettingsSection, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.settings;
        let stepper = |id: &'static str, value: String, f: fn(&mut Workbench, i32)| {
            h_flex()
                .gap_1()
                .child(Button::new((id, 0usize)).label("−").xsmall().on_click(cx.listener(move |this, _, w, cx| {
                    f(this, -1);
                    this.apply_settings(w, cx);
                })))
                .child(div().w(px(48.)).text_center().text_sm().child(value))
                .child(Button::new((id, 1usize)).label("+").xsmall().on_click(cx.listener(move |this, _, w, cx| {
                    f(this, 1);
                    this.apply_settings(w, cx);
                })))
                .into_any_element()
        };
        let toggle = |id: &'static str, on: bool, f: fn(&mut Workbench)| {
            Button::new(id)
                .label(if on { tr("On") } else { tr("Off") })
                .xsmall()
                .when(on, |b| b.primary())
                .on_click(cx.listener(move |this, _, w, cx| {
                    f(this);
                    this.apply_settings(w, cx);
                }))
                .into_any_element()
        };
        match section {
            SettingsSection::Appearance => {
                let lang = s.language.clone();
                let lang_controls = h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children([("auto", tr("Automatic")), ("en", "English"), ("id", "Bahasa Indonesia"), ("fr", "Français"), ("ms", "Bahasa Melayu")].into_iter().map(|(id, name)| {
                        Button::new(SharedString::from(format!("lang-{id}")))
                            .label(name)
                            .xsmall()
                            .when(lang == id, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.language = id.into();
                                crate::i18n::set(id);
                                this.apply_language(w, cx);
                                this.apply_settings(w, cx);
                            }))
                    }))
                    .into_any_element();
                let size = s.ui_size;
                let size_controls = h_flex()
                    .gap_1()
                    .children(UiSize::ALL.iter().map(|z| {
                        let z = *z;
                        Button::new(SharedString::from(format!("ui-size-{}", z.label())))
                            .label(tr(z.label()))
                            .xsmall()
                            .when(z == size, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.ui_size = z;
                                this.apply_appearance(w, cx);
                            }))
                    }))
                    .into_any_element();
                let bg_state: (SharedString, bool) = match (&s.bg_image, s.background()) {
                    (None, _) => ("No image. Panels use the theme's solid colors.".into(), false),
                    (Some(p), None) => (crate::trf!("{0} was not found. Choose another image or remove it."; 0 => p.display()).into(), true),
                    (Some(p), Some(_)) => (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(), false),
                };
                let bg_controls = h_flex()
                    .gap_2()
                    .child(Button::new("bg-choose").label(if s.bg_image.is_some() { tr("Change image…") } else { tr("Choose image…") }).xsmall().on_click(cx.listener(|this, _, w, cx| this.choose_background(w, cx))))
                    .when(s.bg_image.is_some(), |h| {
                        h.child(Button::new("bg-remove").label(tr("Remove")).xsmall().on_click(cx.listener(|this, _, w, cx| {
                            this.settings.bg_image = None;
                            this.apply_appearance(w, cx);
                            this.status = tr("Background image removed").into();
                        })))
                    })
                    .into_any_element();
                let vis = s.bg_visibility;
                let vis_controls = h_flex()
                    .gap_1()
                    .children(Visibility::ALL.iter().map(|v| {
                        let v = *v;
                        Button::new(SharedString::from(format!("bg-vis-{}", v.label())))
                            .label(tr(v.label()))
                            .xsmall()
                            .when(v == vis, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.bg_visibility = v;
                                this.apply_appearance(w, cx);
                            }))
                    }))
                    .into_any_element();
                let vis_desc: SharedString = match vis {
                    Visibility::Strong => tr("Strong: the picture shows clearly. Over very bright pictures, dim text (line numbers, hints) can drop below comfortable contrast.").into(),
                    _ => tr("How much of the picture shows through the panels. The code side of the editor always gets less of it than the right edge.").into(),
                };
                let mut rows = vec![
                    div().p_4().child(theme_cards(cx)).into_any_element(),
                    settings_row(tr("Language"), tr("Menus, panels and messages. Automatic follows Windows.").into(), lang_controls),
                    settings_row(
                        tr("Interface size"),
                        tr("Menus, tabs, the explorer, panels and icons. The editor font has its own size below.").into(),
                        size_controls,
                    ),
                    settings_row(
                        tr("Background image"),
                        if bg_state.1 { bg_state.0.clone() } else { crate::trf!("A picture of your own behind the panels. Stays on your PC. Current: {0}"; 0 => bg_state.0).into() },
                        bg_controls,
                    ),
                    settings_row(
                        tr("Liquid glass"),
                        tr("Translucent panels over a blurred window, with floating cards. Off gives solid panels (a little lighter on the GPU).").into(),
                        Button::new("glass")
                            .label(if s.glass { tr("On") } else { tr("Off") })
                            .xsmall()
                            .when(s.glass, |b| b.primary())
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.settings.glass = !this.settings.glass;
                                this.apply_settings(w, cx);
                            }))
                            .into_any_element(),
                    ),
                ];
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
                rows.push(settings_row(tr("Animated background"), scene_desc, none_btn));
                rows.push(div().px_4().pb_4().child(scene_cards(&s.bg_scene, cx)).into_any_element());
                rows.push(settings_row(
                    tr("Background animation"),
                    tr("Off by default: the scene stands still and costs nothing while you work. On, it moves 24 times a second, which takes about 10 to 15% of one CPU core while FluxCode is in front (much less behind other windows). Turning it on checks what your PC can take.").into(),
                    Button::new("sc-anim")
                        .label(if s.bg_motion { tr("On") } else { tr("Off") })
                        .xsmall()
                        .when(s.bg_motion, |b| b.primary())
                        .on_click(cx.listener(|this, _, w, cx| this.toggle_bg_motion(w, cx)))
                        .into_any_element(),
                ));
                if let Some(check) = &self.motion_check {
                    use crate::motion_check::{Check, Level};
                    let (tok, text): (theme::Tok, SharedString) = match check {
                        Check::Running => (theme::FG_MUTED, tr("Checking what your PC can take: watching FluxCode's CPU use for a few seconds…").into()),
                        Check::Done(Level::Light, t) => (theme::GREEN, t.clone().into()),
                        Check::Done(Level::Fine, t) => (theme::WARN, t.clone().into()),
                        Check::Done(Level::Heavy, t) => (theme::DANGER, t.clone().into()),
                    };
                    let heavy = matches!(check, Check::Done(Level::Heavy, _));
                    rows.push(
                        h_flex()
                            .px_4()
                            .pb_3()
                            .gap_3()
                            .items_start()
                            .child(div().flex_none().mt(px(5.)).size(px(7.)).rounded_full().bg(c(tok)))
                            .child(div().flex_1().min_w_0().text_xs().text_color(c(tok)).child(text))
                            .when(heavy, |h| {
                                h.child(Button::new("sc-anim-off").label(tr("Turn it off")).xsmall().on_click(cx.listener(|this, _, w, cx| {
                                    if this.settings.bg_motion {
                                        this.toggle_bg_motion(w, cx);
                                    }
                                })))
                            })
                            .into_any_element(),
                    );
                }
                if s.backdrop_visibility().is_some() {
                    rows.push(settings_row(tr("Background visibility"), vis_desc, vis_controls));
                }
                settings_card(rows)
            }
            SettingsSection::Editor => {
                let fonts: &[&'static str] = if cfg!(windows) {
                    &["Cascadia Code", "Cascadia Mono", "Consolas", "Courier New"]
                } else {
                    &["DejaVu Sans Mono", "Liberation Mono", "Noto Sans Mono", "Ubuntu Mono"]
                };
                let current_font = s.font_family.to_string();
                let font_controls = h_flex()
                    .gap_1()
                    .flex_wrap()
                    .justify_end()
                    .max_w(theme::u(360.))
                    .children(fonts.iter().map(|f| {
                        let f = *f;
                        Button::new(SharedString::from(format!("font-{f}")))
                            .label(f)
                            .xsmall()
                            .when(current_font == f, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.font_family = f.into();
                                this.apply_settings(w, cx);
                            }))
                    }))
                    .into_any_element();
                settings_card(vec![
                    settings_row(tr("Font size"), tr("Font size of the editor and terminal, in pixels.").into(), stepper("fs", format!("{}", s.font_size), |w, d| {
                        w.settings.font_size = (w.settings.font_size + d as f32).clamp(8., 32.)
                    })),
                    settings_row(tr("Tab size"), tr("Spaces per indentation level (applies to newly opened files).").into(), stepper("ts", s.tab_size.to_string(), |w, d| {
                        w.settings.tab_size = (w.settings.tab_size as i32 + d).clamp(1, 8) as usize
                    })),
                    settings_row(tr("Word wrap"), tr("Wrap long lines at the edge of the editor.").into(), toggle("ww", s.word_wrap, |w| w.settings.word_wrap = !w.settings.word_wrap)),
                    settings_row(
                        tr("Whitespace dots"),
                        tr("Show each space as a faint dot, next to the guide lines that mark how far each block reaches.").into(),
                        toggle("wsd", s.show_whitespace, |w| w.settings.show_whitespace = !w.settings.show_whitespace),
                    ),
                    settings_row(tr("Auto save"), tr("Save a file shortly after you stop typing.").into(), toggle("as", s.auto_save, |w| w.settings.auto_save = !w.settings.auto_save)),
                    settings_row(tr("Font"), tr("The editor and terminal font. Fonts that are not installed fall back to the system's monospace font.").into(), font_controls),
                    settings_row(tr("Line numbers"), tr("Numbers in the gutter left of the code.").into(), toggle("ln", s.line_numbers, |w| w.settings.line_numbers = !w.settings.line_numbers)),
                    settings_row(tr("Code folding"), tr("Arrows in the gutter that fold a block away.").into(), toggle("fold", s.folding, |w| w.settings.folding = !w.settings.folding)),
                    settings_row(tr("Close brackets and quotes"), tr("Typing ( [ { or a quote adds its closing pair.").into(), toggle("acl", s.auto_close, |w| w.settings.auto_close = !w.settings.auto_close)),
                    settings_row(tr("Smart indent"), tr("A new line keeps the indentation, and indents one more level after { or :.").into(), toggle("si", s.smart_indent, |w| w.settings.smart_indent = !w.settings.smart_indent)),
                    settings_row(tr("Cursor padding"), tr("Lines kept visible above and below the cursor as it moves (0: the editor's default).").into(), stepper("cp", s.cursor_padding.to_string(), |w, d| {
                        w.settings.cursor_padding = (w.settings.cursor_padding as i32 + d).clamp(0, 12) as u32
                    })),
                    settings_row(tr("Scroll past the last line"), tr("Leave room below the end of a file, so its last lines can sit mid-screen.").into(), toggle("spe", s.scroll_past_end, |w| w.settings.scroll_past_end = !w.settings.scroll_past_end)),
                    settings_row(
                        tr("Copilot suggestions"),
                        tr("Ghost-text completions from GitHub Copilot, a moment after you stop typing. Tab accepts, Esc dismisses. The Copilot language server does the work in its own process, so the editor stays light.").into(),
                        toggle("cpl", s.copilot, |w| w.settings.copilot = !w.settings.copilot),
                    ),
                ])
            }
            SettingsSection::Wsl => settings_card(vec![settings_row(
                tr("Read WSL folders through a helper in the distro"),
                tr("Faster: the helper reads the distro's own disk instead of Windows' 9p share. It is FluxCode itself, built for Linux, taken from this build's release and installed once in ~/.fluxcode/helper. Off: folders are read through \\\\wsl.localhost, which works but is slower.").into(),
                toggle("wslh", s.wsl_helper, |w| w.settings.wsl_helper = !w.settings.wsl_helper),
            )]),
            SettingsSection::Windows => {
                let on = crate::shell::is_registered();
                let controls = h_flex()
                    .gap_2()
                    .when(on, |h| {
                        h.child(
                            Button::new("shell-defaults")
                                .label(tr("Default apps…"))
                                .xsmall()
                                .ghost()
                                .on_click(|_, _, cx| cx.open_url("ms-settings:defaultapps")),
                        )
                    })
                    .child(
                        Button::new("shell-open-with")
                            .label(if on { tr("On") } else { tr("Off") })
                            .xsmall()
                            .when(on, |b| b.primary())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let r = if on { crate::shell::unregister() } else { crate::shell::register() };
                                this.status = match (r, on) {
                                    (Ok(()), false) => tr("FluxCode is in Explorer's right-click menu and in Open with").into(),
                                    (Ok(()), true) => tr("Removed FluxCode from Explorer's right-click menu and Open with").into(),
                                    (Err(e), _) => crate::trf!("Could not change the Explorer entries: {0}"; 0 => e).into(),
                                };
                                cx.notify();
                            })),
                    )
                    .into_any_element();
                settings_card(vec![settings_row(
                    tr("Open with FluxCode"),
                    tr("Adds \"Open with FluxCode\" to the right-click menu of files and folders, and FluxCode to Open with for code and text files (.js, .ts, .json, .html, .css, .py and more). Types no app opens yet open in FluxCode on a double-click. For the rest, Windows asks you: pick FluxCode once in Open with, tick Always, or set it in Default apps. A file opened this way joins the FluxCode window that is already open.").into(),
                    controls,
                )])
            }
            SettingsSection::Agents => {
                let access = s.access().to_string();
                let access_controls = h_flex()
                    .gap_1()
                    .children([("read", tr("Read only")), ("write", tr("Read & write")), ("full", tr("Full access"))].into_iter().map(|(id, name)| {
                        Button::new(SharedString::from(format!("access-{id}")))
                            .label(name)
                            .xsmall()
                            .when(access == id, |b| b.primary())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.settings.agent_access = id.into();
                                this.apply_settings(w, cx);
                            }))
                    }))
                    .into_any_element();
                settings_card(vec![
                    settings_row(
                        tr("AI agent: access"),
                        tr("Read only: it reads and explains, edits and commands are refused. Read & write: it edits files (Undo is under the chat), commands ask first. Full access: nothing asks.").into(),
                        access_controls,
                    ),
                    settings_row(
                        tr("AI agent: turn off when idle"),
                        tr("After 10 idle minutes the agent process stops and frees its memory (OpenCode uses several hundred MB). Your next message starts it again and continues the same chat.").into(),
                        toggle("idle", s.agent_idle_min > 0, |w| w.settings.agent_idle_min = if w.settings.agent_idle_min == 0 { 10 } else { 0 }),
                    ),
                    settings_row(
                        tr("Agent panel on the right"),
                        tr("Dock the agent chat beside the editor instead of in the side bar. The arrow button in its header does the same.").into(),
                        toggle("ar", s.agent_right, |w| w.settings.agent_right = !w.settings.agent_right),
                    ),
                    settings_row(
                        tr("Follow the agent's edits"),
                        tr("When the agent changes a file, it opens in the editor and the change is typed in line by line while the view follows it. Takes under a second and never slows the agent.").into(),
                        toggle("follow", s.agent_follow_edits, |w| w.settings.agent_follow_edits = !w.settings.agent_follow_edits),
                    ),
                    settings_row(tr("GitHub"), tr("Sign in so gh, git and OpenCode inside FluxCode can use your account (GH_TOKEN).").into(), self.render_github(cx)),
                    settings_row(
                        tr("GitHub Copilot"),
                        tr("Sign in so the editor can suggest the next lines of code. Needs the Copilot language server installed once, and a Copilot plan (a free plan exists).").into(),
                        self.render_copilot(cx),
                    ),
                ])
            }
            SettingsSection::SoulFiles => settings_card(vec![div().p_4().child(self.render_soul_files(cx)).into_any_element()]),
            SettingsSection::Shortcuts => {
                let shortcuts = v_flex().w_full().children(crate::commands::commands().into_iter().filter(|k| !k.keys.is_empty()).map(|k| {
                    h_flex()
                        .w_full()
                        .py_1()
                        .gap_3()
                        .border_b_1()
                        .border_color(c(theme::BORDER))
                        .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG)).child(tr(k.label)))
                        .child(crate::quick::keys_chips(k.keys))
                }));
                settings_card(vec![div().p_4().child(shortcuts).into_any_element()])
            }
            SettingsSection::Updates => {
                let update_controls = h_flex()
                    .gap_2()
                    .child(Button::new("upd-now").label(tr("Check now")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| this.check_updates_now(cx))))
                    .child(toggle("upd", s.update_check, |w| w.settings.update_check = !w.settings.update_check))
                    .into_any_element();
                settings_card(vec![settings_row(
                    tr("Check for new builds"),
                    crate::trf!("FluxCode asks github.com for its list of releases at start and every 6 hours. Nothing about you, this PC or your files is sent. This is {0}."; 0 => crate::update::BUILD).into(),
                    update_controls,
                )])
            }
            SettingsSection::About => {
                v_flex()
                    .w_full()
                    .max_w(theme::u(760.))
                    .gap_4()
                    .child(settings_card(vec![div()
                        .p_4()
                        .child(
                            v_flex()
                                .w_full()
                                .gap_2()
                                .text_xs()
                                .child(div().text_color(c(theme::FG)).child(tr("FluxCode is open source under the GPL-3.0 license and still in development, so expect rough edges and frequent builds. The code, every build and the issue tracker are on GitHub.")))
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
                        .into_any_element()]))
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(crate::trf!("Stored in {0}"; 0 => config_dir().join("settings.json").display())),
                    )
                    .into_any_element()
            }
        }
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
                col = col.child(install_progress());
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
            .w(px(300.))
            .gap_1()
            .child(Input::new(&self.github_input).small())
            .child(
                h_flex()
                    .gap_1()
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

fn install_progress() -> AnyElement {
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
                .child(
                    div().absolute().top_0().h_full().w(relative(0.35)).rounded_full().bg(c(theme::ACCENT)).with_animation(
                        "cpl-install-bar",
                        Animation::new(Duration::from_millis(1300)).repeat().with_easing(gpui_kit::ease_in_out),
                        |d, t| d.left(relative(-0.35 + 1.35 * t)),
                    ),
                ),
        )
        .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("npm is downloading @github/copilot-language-server; this usually takes a minute or two. You can keep working.")))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    #[cfg(feature = "embedded-preview")]
    fn webview_notice_is_on_by_default() {
        assert!(Settings::default().preview_webview_notice);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(old.preview_webview_notice);
        let off: Settings = serde_json::from_str("{\"preview_webview_notice\":false}").unwrap();
        assert!(!off.preview_webview_notice);
    }
}
