//! Settings and session, stored as JSON in the user's config folder
//! (`%APPDATA%\FluxCode-GPUI` on Windows).

use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    v_flex, Sizable,
};
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("FluxCode-GPUI")
}

fn load_json<T: for<'a> Deserialize<'a> + Default>(name: &str) -> T {
    std::fs::read_to_string(config_dir().join(name)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_json<T: Serialize>(name: &str, value: &T) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(s) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(dir.join(name), s);
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub font_size: f32,
    pub font_family: SharedString,
    pub tab_size: usize,
    pub word_wrap: bool,
    pub auto_save: bool,
    pub agent_auto_approve: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 14.,
            font_family: "Cascadia Code".into(),
            tab_size: 4,
            word_wrap: false,
            auto_save: true,
            agent_auto_approve: false,
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
        let wrap = self.settings.word_wrap;
        for d in &self.docs {
            d.editor.update(cx, |s, cx| s.set_soft_wrap(wrap, window, cx));
        }
        cx.notify();
    }

    pub(crate) fn render_settings(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let close_btn = self.block_button("settings-close", "CLOSE", crate::ui::Kind::Plain, cx, |this, _, cx| {
            this.settings_open = false;
            cx.notify();
        });
        let s = &self.settings;
        let row = |title: &'static str, desc: &'static str, control: AnyElement| {
            h_flex()
                .w_full()
                .max_w(px(720.))
                .py_3()
                .gap_4()
                .border_b_2()
                .border_color(c(theme::INK))
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
        // A chunky block switch; the knob slides with a spring.
        let toggle = |id: &'static str, on: bool, f: fn(&mut Workbench)| {
            h_flex()
                .gap_2()
                .child(div().w(px(28.)).text_xs().font_weight(FontWeight::EXTRA_BOLD).text_color(c(if on { theme::ACCENT } else { theme::FG_DIM })).child(if on { "ON" } else { "OFF" }))
                .child(
                    div()
                        .id(id)
                        .w(px(46.))
                        .h(px(24.))
                        .relative()
                        .cursor_pointer()
                        .bg(c(if on { theme::ACCENT } else { theme::BG_ELEVATED }))
                        .border_2()
                        .border_color(c(theme::INK))
                        .shadow(crate::ui::hard_shadow(2., theme::INK))
                        .child(
                            div()
                                .absolute()
                                .top(px(2.))
                                .size(px(16.))
                                .bg(c(if on { theme::INK } else { theme::FG_MUTED }))
                                .with_spring(id, crate::ui::spring().to(if on { 1.0f32 } else { 0.0 }), |el, v| el.left(px(2. + 22. * v))),
                        )
                        .on_click(cx.listener(move |this, _, w, cx| {
                            f(this);
                            this.apply_settings(w, cx);
                        })),
                )
                .into_any_element()
        };
        v_flex()
            .id("settings")
            .size_full()
            .p_6()
            .gap_1()
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .child(
                h_flex()
                    .w_full()
                    .max_w(px(720.))
                    .pb_3()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .bg(c(theme::ACCENT))
                            .border_2()
                            .border_color(c(theme::INK))
                            .shadow(crate::ui::hard_shadow(4., theme::INK))
                            .text_color(c(theme::INK))
                            .text_xl()
                            .font_weight(FontWeight::BLACK)
                            .child("SETTINGS"),
                    )
                    .child(div().flex_1())
                    .child(close_btn),
            )
            .child(row("Editor: Font Size", "Font size of the editor and terminal, in pixels.", stepper("fs", format!("{}", s.font_size), |w, d| {
                w.settings.font_size = (w.settings.font_size + d as f32).clamp(8., 32.)
            })))
            .child(row("Editor: Tab Size", "Spaces per indentation level (applies to newly opened files).", stepper("ts", s.tab_size.to_string(), |w, d| {
                w.settings.tab_size = (w.settings.tab_size as i32 + d).clamp(1, 8) as usize
            })))
            .child(row("Editor: Word Wrap", "Wrap long lines at the edge of the editor.", toggle("ww", s.word_wrap, |w| w.settings.word_wrap = !w.settings.word_wrap)))
            .child(row("Files: Auto Save", "Save a file shortly after you stop typing.", toggle("as", s.auto_save, |w| w.settings.auto_save = !w.settings.auto_save)))
            .child(row(
                "OpenCode: Auto-approve",
                "Let the agent edit files and run commands without asking each time.",
                toggle("aa", s.agent_auto_approve, |w| w.settings.agent_auto_approve = !w.settings.agent_auto_approve),
            ))
            .child(row("GitHub", "Sign in so gh, git and OpenCode inside FluxCode can use your account (GH_TOKEN).", self.render_github(cx)))
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
