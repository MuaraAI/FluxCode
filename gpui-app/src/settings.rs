//! Settings and session, stored as JSON in the user's config folder
//! (`%APPDATA%\FluxCode-GPUI` on Windows).

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 15.,
            font_family: "Cascadia Code".into(),
            tab_size: 4,
            word_wrap: false,
            show_whitespace: true,
            auto_save: true,
            agent_auto_approve: false,
            theme: PALETTES[0].id.into(),
            bg_image: None,
            bg_visibility: Visibility::default(),
            ui_size: UiSize::default(),
            agent_right: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        load_json("settings.json")
    }
    pub fn save(&self) {
        save_json("settings.json", self);
    }
    /// The background image, if one is set and the file is still there.
    pub fn background(&self) -> Option<&Path> {
        self.bg_image.as_deref().filter(|p| p.is_file())
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub root: Option<PathBuf>,
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
    fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.save();
        if self.settings.agent_right && self.view == crate::workbench::SideView::Agent {
            self.view = crate::workbench::SideView::Explorer;
            self.agent_open = true;
        }
        let (wrap, dots) = (self.settings.word_wrap, self.settings.show_whitespace);
        for d in &self.docs {
            d.editor.update(cx, |s, cx| {
                s.set_soft_wrap(wrap, window, cx);
                s.set_show_whitespaces(dots, window, cx);
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
        self.status = format!("Color theme: {name}").into();
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
                    this.status = format!("{} is not an image FluxCode can show (PNG, JPEG, WebP, GIF or BMP).", path.display()).into();
                    cx.notify();
                    return;
                }
                this.settings.bg_image = Some(path);
                this.apply_appearance(window, cx);
                this.status = "Background image set".into();
            })
            .ok()
        })
        .detach();
    }

    pub(crate) fn render_settings(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let close_btn = self.action_button("settings-close", "Close", crate::ui::Kind::Plain, cx, |this, _, cx| {
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
                    v_flex()
                        .flex_1()
                        .gap_1()
                        .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
                        .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc)),
                )
                .child(control)
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
                .label(if on { "On" } else { "Off" })
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
                .hover(|s| s.border_color(c(theme::BORDER_STRONG)))
                .when(on, |d| d.hover(|s| s.border_color(c(theme::ACCENT))))
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
                                .when(on, |h| h.child(div().text_xs().text_color(c(theme::ACCENT_TEXT)).child("Active"))),
                        )
                        .child(div().text_xs().truncate().text_color(c(theme::FG_MUTED)).child(p.note)),
                )
        }));

        // Background image: choose / remove, and how strongly it shows.
        let bg_state: (SharedString, bool) = match (&s.bg_image, s.background()) {
            (None, _) => ("No image. Panels use the theme's solid colors.".into(), false),
            (Some(p), None) => (format!("{} was not found. Choose another image or remove it.", p.display()).into(), true),
            (Some(p), Some(_)) => (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(), false),
        };
        let bg_controls = h_flex()
            .gap_2()
            .child(Button::new("bg-choose").label(if s.bg_image.is_some() { "Change image…" } else { "Choose image…" }).xsmall().on_click(cx.listener(|this, _, w, cx| this.choose_background(w, cx))))
            .when(s.bg_image.is_some(), |h| {
                h.child(Button::new("bg-remove").label("Remove").xsmall().on_click(cx.listener(|this, _, w, cx| {
                    this.settings.bg_image = None;
                    this.apply_appearance(w, cx);
                    this.status = "Background image removed".into();
                })))
            })
            .into_any_element();
        let vis = s.bg_visibility;
        let vis_controls = h_flex()
            .gap_1()
            .children(Visibility::ALL.iter().map(|v| {
                let v = *v;
                Button::new(SharedString::from(format!("bg-vis-{}", v.label())))
                    .label(v.label())
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
                    .label(z.label())
                    .xsmall()
                    .when(z == size, |b| b.primary())
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.settings.ui_size = z;
                        this.apply_appearance(w, cx);
                    }))
            }))
            .into_any_element();
        let vis_desc: SharedString = match vis {
            Visibility::Strong => "Strong: the picture shows clearly. Over very bright pictures, dim text (line numbers, hints) can drop below comfortable contrast.".into(),
            _ => "How much of the picture shows through the panels. The code side of the editor always gets less of it than the right edge.".into(),
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
                    .child(div().flex_1().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child("Settings"))
                    .child(close_btn),
            )
            .child(section("Appearance"))
            .child(
                div()
                    .w_full()
                    .max_w(theme::u(760.))
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child("Color theme. Ctrl+K Ctrl+T switches to the next one from anywhere."),
            )
            .child(cards)
            .child(row(
                "Interface size",
                "Menus, tabs, the explorer, panels and icons. The editor font has its own size below.".into(),
                size_controls,
            ))
            .child(row(
                "Background image",
                if bg_state.1 { bg_state.0.clone() } else { format!("A picture of your own behind the panels. Stays on your PC. Current: {}", bg_state.0).into() },
                bg_controls,
            ))
            .when(s.background().is_some(), |v| v.child(row("Background visibility", vis_desc, vis_controls)))
            .child(section("Editor"))
            .child(row("Font size", "Font size of the editor and terminal, in pixels.".into(), stepper("fs", format!("{}", s.font_size), |w, d| {
                w.settings.font_size = (w.settings.font_size + d as f32).clamp(8., 32.)
            })))
            .child(row("Tab size", "Spaces per indentation level (applies to newly opened files).".into(), stepper("ts", s.tab_size.to_string(), |w, d| {
                w.settings.tab_size = (w.settings.tab_size as i32 + d).clamp(1, 8) as usize
            })))
            .child(row("Word wrap", "Wrap long lines at the edge of the editor.".into(), toggle("ww", s.word_wrap, |w| w.settings.word_wrap = !w.settings.word_wrap)))
            .child(row(
                "Whitespace dots",
                "Show each space as a faint dot, next to the guide lines that mark how far each block reaches.".into(),
                toggle("wsd", s.show_whitespace, |w| w.settings.show_whitespace = !w.settings.show_whitespace),
            ))
            .child(row("Auto save", "Save a file shortly after you stop typing.".into(), toggle("as", s.auto_save, |w| w.settings.auto_save = !w.settings.auto_save)))
            .child(section("OpenCode and GitHub"))
            .child(row(
                "OpenCode: auto-approve",
                "Let the agent edit files and run commands without asking each time.".into(),
                toggle("aa", s.agent_auto_approve, |w| w.settings.agent_auto_approve = !w.settings.agent_auto_approve),
            ))
            .child(row(
                "OpenCode on the right",
                "Dock the OpenCode chat beside the editor instead of in the side bar. The arrow button in its header does the same.".into(),
                toggle("ar", s.agent_right, |w| w.settings.agent_right = !w.settings.agent_right),
            ))
            .child(row("GitHub", "Sign in so gh, git and OpenCode inside FluxCode can use your account (GH_TOKEN).".into(), self.render_github(cx)))
            .child(
                div()
                    .pt_4()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(format!("Stored in {}", config_dir().join("settings.json").display())),
            )
            .into_any_element()
    }
}

impl Workbench {
    fn github_result(&mut self, r: Result<crate::github::Account, String>, cx: &mut Context<Self>) {
        match r {
            Ok(a) => {
                self.github = Some(a);
                self.status = "Signed in to GitHub".into();
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
                .child(Button::new("gh-out").label("Sign out").xsmall().on_click(cx.listener(|this, _, _, cx| {
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
                    .child(Button::new("gh-token").label("Sign in with token").xsmall().primary().on_click(cx.listener(|this, _, _, cx| {
                        let t = this.github_input.read(cx).value().to_string();
                        cx.spawn(async move |this, cx| {
                            let r = cx.background_spawn(async move { crate::github::login_token(&t) }).await;
                            _ = this.update(cx, |this, cx| this.github_result(r, cx));
                        })
                        .detach();
                    })))
                    .child(Button::new("gh-cli").label("Use GitHub CLI login").xsmall().on_click(cx.listener(|_, _, _, cx| {
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
