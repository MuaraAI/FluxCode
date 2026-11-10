//! Settings and session, stored as JSON in the user's config folder
//! (`%APPDATA%\FluxCode-GPUI` on Windows).

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
    // A file saved by Notepad or PowerShell can start with a UTF-8 BOM, which serde_json rejects.
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

/// Image formats GPUI can decode for the background.
pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

const SETTINGS_REV: u32 = 1;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub font_size: f32,
    pub font_family: SharedString,
    pub tab_size: usize,
    pub word_wrap: bool,
    /// Draw each space as a faint dot, so indentation reads as a level, not a gap.
    pub show_whitespace: bool,
    pub auto_save: bool,
    pub agent_auto_approve: bool,
    /// Palette id, see `theme::PALETTES`.
    pub theme: String,
    /// A picture of the user's choice behind the panels. Only the path is stored;
    /// the file stays where it is and is never copied.
    pub bg_image: Option<PathBuf>,
    pub bg_visibility: Visibility,
    /// Size of the menus, bars, lists and icons (not the editor font).
    pub ui_size: UiSize,
    /// OpenCode docks on the right of the editor (like a chat beside the code)
    /// instead of in the left side bar.
    pub agent_right: bool,
    /// Which agent harness the panel runs: "auto" (Mura when installed), "opencode" or "mura".
    pub agent_cli: String,
    /// Turn the agent process off after this many idle minutes (0 = never). It
    /// resumes the same session on the next message.
    pub agent_idle_min: u32,
    /// Interface language: "auto" (Indonesian when Windows is), "en" or "id".
    pub language: String,
    /// Agent settings picked in the panel (ACP config options such as model,
    /// effort and mode), as "harness:option" → value. Applied to each new
    /// session when the harness still offers that value.
    pub agent_options: std::collections::HashMap<String, String>,
    /// What the agent may do without asking: "read" (read only), "write" (read and
    /// edit files; commands ask) or "full". Empty: from the old auto-approve switch.
    pub agent_access: String,
    /// Agent memory is off: nothing is recalled, saved or asked of the agent.
    pub memory_off: bool,
    /// The browser the agents drive (`browser.rs`): "auto" (Chrome, else Brave, Edge or
    /// Chromium), a browser id, or "off".
    pub agent_browser: String,
    /// Run that browser without a window.
    pub agent_browser_headless: bool,
    /// Launches that showed the Ctrl+L hint in the status bar (100: shortcut used).
    pub agent_hint: u8,
    /// Look for new development builds on GitHub at start and every 6 hours.
    pub update_check: bool,
    /// The editor's line numbers in the gutter.
    pub line_numbers: bool,
    /// Fold arrows in the gutter for blocks.
    pub folding: bool,
    /// Typing an opening bracket or quote adds the closing one.
    pub auto_close: bool,
    /// A new line keeps (and after `{` / `:` adds) indentation.
    pub smart_indent: bool,
    /// Lines kept visible above and below the cursor when it moves (VS Code's
    /// cursorSurroundingLines). 0: the editor's default.
    pub cursor_padding: u32,
    /// The editor scrolls past its last line.
    pub scroll_past_end: bool,
    /// The build that ran last, so a new one can open its release notes once.
    pub last_build: String,
    /// Settings revision, for one-time changes of defaults on existing installs.
    /// Missing in an older file means 0 (not the struct default, which is current).
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
            agent_cli: "auto".into(),
            agent_idle_min: 10,
            language: "auto".into(),
            agent_options: Default::default(),
            agent_access: String::new(),
            memory_off: false,
            agent_browser: "auto".into(),
            agent_browser_headless: false,
            agent_hint: 0,
            update_check: true,
            line_numbers: true,
            folding: true,
            auto_close: true,
            smart_indent: true,
            cursor_padding: 0,
            scroll_past_end: true,
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
            // 0.9: whitespace dots are off by default (they cluttered long lines);
            // Settings → Whitespace dots turns them back on.
            s.show_whitespace = false;
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
    /// The agent's access level: "read", "write" or "full".
    pub fn access(&self) -> &str {
        match self.agent_access.as_str() {
            "read" | "write" | "full" => &self.agent_access,
            _ if self.agent_auto_approve => "full",
            _ => "write",
        }
    }

    /// The background image, if one is set and the file is still there.
    pub fn background(&self) -> Option<&Path> {
        self.bg_image.as_deref().filter(|p| p.is_file())
    }
}

static EXISTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// settings.json was there when FluxCode started (so this is not a first run).
pub fn existed_before_start() -> bool {
    EXISTED.get().copied().unwrap_or(false)
}

/// The editor options from Settings, applied to each open editor and every new one.
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
    /// Folders added next to `root` (a multi-folder workspace).
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

impl Workbench {
    pub(crate) fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        if self.settings.agent_right && self.view == crate::workbench::SideView::Agent {
            self.view = crate::workbench::SideView::Explorer;
            self.agent_open = true;
        }
        let (wrap, dots) = (self.settings.word_wrap, self.settings.show_whitespace);
        let opts = EditorOpts::of(&self.settings);
        for d in &self.docs {
            d.editor.update(cx, |s, cx| {
                s.set_soft_wrap(wrap, window, cx);
                s.set_show_whitespaces(dots, window, cx);
                opts.apply(s, window, cx);
            });
        }
        cx.notify();
    }

    /// Re-apply the palette and image state everywhere (after a theme or background change).
    pub(crate) fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        theme::set_ui_size(self.settings.ui_size);
        theme::set(&self.settings.theme, self.settings.background().map(|_| self.settings.bg_visibility));
        theme::apply(cx);
        window.refresh();
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.theme = id.into();
        self.apply_appearance(window, cx);
        let name = PALETTES.iter().find(|p| p.id == id).map_or("", |p| p.name);
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

    pub(crate) fn render_settings(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let close_btn = self.action_button("settings-close", tr("Close"), crate::ui::Kind::Plain, cx, |this, _, cx| {
            this.settings_open = false;
            cx.notify();
        });
        let s = &self.settings;
        let section = |title: &'static str| {
            div()
                .w_full()
                .max_w(theme::u(760.))
                .pt_6()
                .pb_1()
                .text_color(c(theme::FG_STRONG))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title)
        };
        let row = |title: &'static str, desc: SharedString, control: AnyElement| {
            h_flex()
                .w_full()
                .max_w(theme::u(760.))
                .py_3()
                .gap_4()
                .border_b_1()
                .border_color(c(theme::BORDER))
                .child(
                    // min_w_0: lets the description wrap instead of pushing the control off screen.
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
                        .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc)),
                )
                .child(div().flex_none().child(control))
        };
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

        // Theme cards: a strip of the palette's own colors, its name and one line.
        let active_theme = theme::palette().id;
        let cards = h_flex().w_full().max_w(theme::u(760.)).flex_wrap().gap_3().py_3().children(PALETTES.iter().map(|p| {
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
                // One hover style (gpui allows only one): the active card keeps its accent.
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
        }));

        // Background image: choose / remove, and how strongly it shows.
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
        // Language names stay in their own language, so they can be found in either.
        let lang = s.language.clone();
        let lang_controls = h_flex()
            .gap_1()
            .children([("auto", tr("Automatic")), ("en", "English"), ("id", "Bahasa Indonesia")].into_iter().map(|(id, name)| {
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
        let update_controls = h_flex()
            .gap_2()
            .child(Button::new("upd-now").label(tr("Check now")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| this.check_updates_now(cx))))
            .child(toggle("upd", s.update_check, |w| w.settings.update_check = !w.settings.update_check))
            .into_any_element();
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
        let shortcuts = v_flex().w_full().max_w(theme::u(760.)).children(crate::commands::commands().into_iter().filter(|k| !k.keys.is_empty()).map(|k| {
            h_flex()
                .w_full()
                .py_1()
                .gap_3()
                .border_b_1()
                .border_color(c(theme::BORDER))
                .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(theme::FG)).child(tr(k.label)))
                .child(crate::quick::keys_chips(k.keys))
        }));
        let vis_desc: SharedString = match vis {
            Visibility::Strong => tr("Strong: the picture shows clearly. Over very bright pictures, dim text (line numbers, hints) can drop below comfortable contrast.").into(),
            _ => tr("How much of the picture shows through the panels. The code side of the editor always gets less of it than the right edge.").into(),
        };

        v_flex()
            .id("settings")
            .size_full()
            .px_8()
            .py_6()
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .child(
                h_flex()
                    .w_full()
                    .max_w(theme::u(760.))
                    .child(div().flex_1().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Settings")))
                    .child(close_btn),
            )
            .child(section(tr("Appearance")))
            .child(
                div()
                    .w_full()
                    .max_w(theme::u(760.))
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(tr("Color theme. Ctrl+K Ctrl+T switches to the next one from anywhere.")),
            )
            .child(cards)
            .child(row(tr("Language"), tr("Menus, panels and messages. Automatic follows Windows.").into(), lang_controls))
            .child(row(
                tr("Interface size"),
                tr("Menus, tabs, the explorer, panels and icons. The editor font has its own size below.").into(),
                size_controls,
            ))
            .child(row(
                tr("Background image"),
                if bg_state.1 { bg_state.0.clone() } else { crate::trf!("A picture of your own behind the panels. Stays on your PC. Current: {0}"; 0 => bg_state.0).into() },
                bg_controls,
            ))
            .when(s.background().is_some(), |v| v.child(row(tr("Background visibility"), vis_desc, vis_controls)))
            .child(section(tr("Editor")))
            .child(row(tr("Font size"), tr("Font size of the editor and terminal, in pixels.").into(), stepper("fs", format!("{}", s.font_size), |w, d| {
                w.settings.font_size = (w.settings.font_size + d as f32).clamp(8., 32.)
            })))
            .child(row(tr("Tab size"), tr("Spaces per indentation level (applies to newly opened files).").into(), stepper("ts", s.tab_size.to_string(), |w, d| {
                w.settings.tab_size = (w.settings.tab_size as i32 + d).clamp(1, 8) as usize
            })))
            .child(row(tr("Word wrap"), tr("Wrap long lines at the edge of the editor.").into(), toggle("ww", s.word_wrap, |w| w.settings.word_wrap = !w.settings.word_wrap)))
            .child(row(
                tr("Whitespace dots"),
                tr("Show each space as a faint dot, next to the guide lines that mark how far each block reaches.").into(),
                toggle("wsd", s.show_whitespace, |w| w.settings.show_whitespace = !w.settings.show_whitespace),
            ))
            .child(row(tr("Auto save"), tr("Save a file shortly after you stop typing.").into(), toggle("as", s.auto_save, |w| w.settings.auto_save = !w.settings.auto_save)))
            .child(row(tr("Font"), tr("The editor and terminal font. Fonts that are not installed fall back to the system's monospace font.").into(), font_controls))
            .child(row(tr("Line numbers"), tr("Numbers in the gutter left of the code.").into(), toggle("ln", s.line_numbers, |w| w.settings.line_numbers = !w.settings.line_numbers)))
            .child(row(tr("Code folding"), tr("Arrows in the gutter that fold a block away.").into(), toggle("fold", s.folding, |w| w.settings.folding = !w.settings.folding)))
            .child(row(tr("Close brackets and quotes"), tr("Typing ( [ { or a quote adds its closing pair.").into(), toggle("acl", s.auto_close, |w| w.settings.auto_close = !w.settings.auto_close)))
            .child(row(tr("Smart indent"), tr("A new line keeps the indentation, and indents one more level after { or :.").into(), toggle("si", s.smart_indent, |w| w.settings.smart_indent = !w.settings.smart_indent)))
            .child(row(tr("Cursor padding"), tr("Lines kept visible above and below the cursor as it moves (0: the editor's default).").into(), stepper("cp", s.cursor_padding.to_string(), |w, d| {
                w.settings.cursor_padding = (w.settings.cursor_padding as i32 + d).clamp(0, 12) as u32
            })))
            .child(row(tr("Scroll past the last line"), tr("Leave room below the end of a file, so its last lines can sit mid-screen.").into(), toggle("spe", s.scroll_past_end, |w| w.settings.scroll_past_end = !w.settings.scroll_past_end)))
            .child(section(tr("AI agents and GitHub")))
            .child(row(
                tr("AI agent: access"),
                tr("Read only: it reads and explains, edits and commands are refused. Read & write: it edits files (Undo is under the chat), commands ask first. Full access: nothing asks.").into(),
                access_controls,
            ))
            .child(row(
                tr("AI agent: turn off when idle"),
                tr("After 10 idle minutes the agent process stops and frees its memory (OpenCode uses several hundred MB). Your next message starts it again and continues the same chat.").into(),
                toggle("idle", s.agent_idle_min > 0, |w| w.settings.agent_idle_min = if w.settings.agent_idle_min == 0 { 10 } else { 0 }),
            ))
            .child(row(
                tr("Agent panel on the right"),
                tr("Dock the agent chat beside the editor instead of in the side bar. The arrow button in its header does the same.").into(),
                toggle("ar", s.agent_right, |w| w.settings.agent_right = !w.settings.agent_right),
            ))
            .child(row(tr("GitHub"), tr("Sign in so gh, git and OpenCode inside FluxCode can use your account (GH_TOKEN).").into(), self.render_github(cx)))
            .child(section(tr("Agent instructions (soul files)")))
            .child(
                div()
                    .w_full()
                    .max_w(theme::u(760.))
                    .pb_2()
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(tr("The files each installed agent reads for its standing instructions and character. Open one to read or edit it; a missing one can be created empty.")),
            )
            .child(self.render_soul_files(cx))
            .child(section(tr("Keyboard shortcuts")))
            .child(
                div()
                    .w_full()
                    .max_w(theme::u(760.))
                    .pb_2()
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(tr("VS Code's keys. Ctrl+Shift+P (or F1) opens every command with its keys; type to filter.")),
            )
            .child(shortcuts)
            .child(section(tr("Updates")))
            .child(row(
                tr("Check for new builds"),
                crate::trf!("FluxCode asks github.com for its list of releases at start and every 6 hours. Nothing about you, this PC or your files is sent. This is {0}."; 0 => crate::update::BUILD).into(),
                update_controls,
            ))
            .child(section(tr("About FluxCode")))
            .child(
                v_flex()
                    .w_full()
                    .max_w(theme::u(760.))
                    .py_3()
                    .gap_2()
                    .text_xs()
                    .child(div().text_color(c(theme::FG)).child(tr("FluxCode is open source under the GPL-3.0 license and still in development, so expect rough edges and frequent builds. The code, every build and the issue tracker are on GitHub.")))
                    .child(div().text_color(c(theme::FG_MUTED)).child(tr("A MuaraAI project, built by a first-semester university student in Indonesia.")))
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
            .child(
                div()
                    .w_full()
                    .max_w(theme::u(760.))
                    .pt_4()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(crate::trf!("Stored in {0}"; 0 => config_dir().join("settings.json").display())),
            )
            .into_any_element()
    }
}

impl Workbench {
    /// Every installed agent with its instruction files: where each one is, and Open
    /// (or Create, when it isn't there yet).
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
