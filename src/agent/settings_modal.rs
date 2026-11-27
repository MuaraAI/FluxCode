use super::{
    agent_glyph, agent_id, agent_name, menu::effort_note, model_groups, opencode_config_file,
    ConfigOpt, Status,
};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    #[default]
    Model,
    Permissions,
    Session,
    Browser,
    Agent,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Model,
        Tab::Permissions,
        Tab::Session,
        Tab::Browser,
        Tab::Agent,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Model => "Model",
            Tab::Permissions => "Permissions",
            Tab::Session => "Session",
            Tab::Browser => "Browser",
            Tab::Agent => "Agent",
        }
    }

    fn icon(self) -> Lucide {
        match self {
            Tab::Model => Lucide::Cpu,
            Tab::Permissions => Lucide::ShieldCheck,
            Tab::Session => Lucide::Timer,
            Tab::Browser => Lucide::Globe,
            Tab::Agent => Lucide::Bot,
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Tab::Model => "Which model answers, how hard it thinks, and how it works.",
            Tab::Permissions => "What the agent may do on this PC without asking you first.",
            Tab::Session => "When the agent stops on its own, and your saved chats.",
            Tab::Browser => "The browser agents open pages in.",
            Tab::Agent => "The program behind the chat.",
        }
    }
}

const COMPACT: f32 = 600.;

impl Workbench {
    pub(crate) fn render_agent_settings(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.agent.show_options {
            return None;
        }
        let vp = window.viewport_size();
        let card_w = (f32::from(vp.width) - 32.).clamp(300., 780.);
        let card_h = (f32::from(vp.height) - 72.).clamp(320., 580.);
        let compact = card_w < COMPACT;
        let tab = self.agent.settings_tab;

        let nav = Tab::ALL.into_iter().map(|t| {
            let on = t == tab;
            h_flex()
                .id(SharedString::from(format!("ag-set-tab-{t:?}")))
                .flex_none()
                .h(theme::u(30.))
                .px_2p5()
                .gap_2()
                .rounded(px(6.))
                .cursor_pointer()
                .text_sm()
                .when(on, |d| {
                    d.bg(c(theme::SELECT)).text_color(c(theme::FG_STRONG))
                })
                .when(!on, |d| {
                    d.text_color(c(theme::FG_MUTED))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG)))
                })
                .child(
                    Icon::new(t.icon())
                        .size(theme::u(15.))
                        .flex_none()
                        .text_color(c(if on {
                            theme::ACCENT_TEXT
                        } else {
                            theme::FG_DIM
                        })),
                )
                .child(div().whitespace_nowrap().child(tr(t.label())))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.agent.settings_tab = t;
                    cx.notify();
                }))
        });
        let nav = if compact {
            h_flex()
                .id("ag-set-nav")
                .flex_none()
                .px_3()
                .py_2()
                .gap_1()
                .overflow_x_scroll()
                .border_b_1()
                .border_color(c(theme::BORDER))
                .children(nav)
                .into_any_element()
        } else {
            v_flex()
                .flex_none()
                .w(theme::u(184.))
                .p_2()
                .gap_0p5()
                .border_r_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_EDITOR))
                .children(nav)
                .into_any_element()
        };

        let body = match tab {
            Tab::Model => self.settings_model(compact, cx),
            Tab::Permissions => self.settings_permissions(compact, cx),
            Tab::Session => self.settings_session(compact, cx),
            Tab::Browser => v_flex()
                .child(row(
                    tr("Browser for agents").into(),
                    None,
                    self.render_browser_setting(cx),
                    true,
                ))
                .into_any_element(),
            Tab::Agent => self.settings_agent(compact, window, cx),
        };
        let content = v_flex()
            .id("ag-set-content")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .px_5()
            .pt_4()
            .pb_5()
            .gap_1()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_STRONG))
                    .child(tr(tab.label())),
            )
            .child(
                div()
                    .pb_2()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(tr(tab.blurb())),
            )
            .child(body);

        let name = agent_name();
        let (dot, state) = match &self.agent.status {
            Status::Ready => (theme::GREEN, tr("On")),
            Status::Busy => (theme::BLUE, tr("Working")),
            Status::Starting => (theme::WARN, tr("Starting")),
            Status::Error(_) => (theme::DANGER, tr("Error")),
            Status::Off => (theme::FG_DIM, tr("Off")),
        };
        let header = h_flex()
            .flex_none()
            .h(theme::u(48.))
            .px_4()
            .gap_2p5()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(agent_glyph(&agent_id(), 20.))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_STRONG))
                    .child(crate::trf!("{name} settings"; name => name)),
            )
            .child(
                h_flex()
                    .flex_none()
                    .px_2()
                    .h(theme::u(20.))
                    .gap_1p5()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(div().size(px(6.)).rounded_full().bg(c(dot)))
                    .child(state),
            )
            .when(crate::acp::locate(&agent_id()).is_some(), |h| {
                h.child(super::menu::optimized_badge())
            })
            .child(div().flex_1())
            .when(!compact, |h| {
                h.child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(tr("Esc closes")),
                )
            })
            .child(
                Button::new("ag-set-close")
                    .icon(IconName::Close)
                    .ghost()
                    .small()
                    .tooltip(tr("Close (Esc)"))
                    .on_click(cx.listener(|this, _, w, cx| this.agent_close_options(w, cx))),
            );

        let card = v_flex()
            .id("ag-settings")
            .key_context("AgentSettings")
            .track_focus(&self.agent.settings_focus)
            .occlude()
            .w(px(card_w))
            .h(px(card_h))
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(c(theme::BG_SIDE))
            .shadow_lg()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.agent_close_options(w, cx);
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, w, cx| this.agent_close_options(w, cx)))
            .child(header)
            .child(if compact {
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(nav)
                    .child(content)
                    .into_any_element()
            } else {
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(nav)
                    .child(content)
                    .into_any_element()
            });

        let modal_card: AnyElement = if cx.reduce_motion() {
            card.into_any_element()
        } else {
            card.with_animation(
                "ag-settings-in",
                Animation::new(std::time::Duration::from_millis(160))
                    .with_easing(gpui_kit::ease_out_quint()),
                |el, t| el.opacity(t).mt(px((1. - t) * 6.)),
            )
            .into_any_element()
        };

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(0.36))
                .flex()
                .items_center()
                .justify_center()
                .child(modal_card)
                .into_any_element(),
        )
    }

    fn settings_model(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let view = self.agent_config_view();
        let name = agent_name();
        if view.is_empty() {
            let on = matches!(
                self.agent.status,
                Status::Starting | Status::Ready | Status::Busy
            );
            return v_flex()
                .gap_2()
                .pt_1()
                .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(if on {
                    crate::trf!("{name} is starting. Its models and settings show here in a moment."; name => name)
                } else {
                    crate::trf!("{name} lists its models and settings once it is on."; name => name)
                }))
                .when(!on, |v| v.child(h_flex().child(Button::new("ag-set-on").label(crate::trf!("Turn {name} on"; name => name)).small().primary().on_click(cx.listener(|this, _, _, cx| this.agent_set_on(true, cx))))))
                .into_any_element();
        }
        let mut col = v_flex();
        if let Some(m) = view.iter().find(|o| o.category == "model") {
            col = col.child(row(tr("Model").into(), Some(crate::trf!("{n} models. Free ones first, then by provider."; n => m.choices.len()).into()), self.settings_model_list(m, cx), true));
        }
        if let Some(e) = view.iter().find(|o| o.category == "thought_level") {
            let note: SharedString = tr(effort_note(&e.current)).into();
            col = col.child(divider()).child(row(
                tr("Effort").into(),
                Some(note),
                segmented(e, cx),
                compact,
            ));
        }
        if let Some(m) = view
            .iter()
            .find(|o| o.category == "mode")
            .filter(|o| o.choices.len() > 1)
        {
            let desc: SharedString = m
                .choices
                .iter()
                .find(|c| c.0 == m.current)
                .map(|c| c.2.clone())
                .unwrap_or_default()
                .into();
            col = col.child(divider()).child(row(
                tr("Mode").into(),
                (!desc.is_empty()).then_some(desc),
                segmented(m, cx),
                compact,
            ));
        }
        for o in view
            .iter()
            .filter(|o| !matches!(o.category.as_str(), "model" | "thought_level" | "mode"))
        {
            col = col.child(divider()).child(row(
                o.name.clone().into(),
                None,
                segmented(o, cx),
                compact,
            ));
        }
        col.into_any_element()
    }

    fn settings_model_list(&self, m: &ConfigOpt, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex()
            .id("ag-set-models")
            .max_h(theme::u(248.))
            .overflow_y_scroll()
            .py_1()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_EDITOR));
        for (gi, (group, models)) in model_groups(&m.choices).into_iter().enumerate() {
            let title = match group.as_str() {
                "free" => tr("Free (OpenCode Zen)").to_string(),
                "" => tr("Models").to_string(),
                g => g.to_string(),
            };
            list = list.child(
                h_flex()
                    .px_3()
                    .pt(px(if gi == 0 { 4. } else { 10. }))
                    .pb_1()
                    .gap_2()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(52.))
                            .text_right()
                            .child(tr("Context")),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme::u(48.))
                            .text_right()
                            .child(tr("Output")),
                    )
                    .child(div().flex_none().w(theme::u(16.))),
            );
            for (i, (v, n)) in models.into_iter().enumerate() {
                let on = v == m.current;
                let meta = super::catalog::get(&v);
                let ctx = meta
                    .map(|x| super::catalog::tokens(x.context))
                    .unwrap_or_default();
                let out = meta
                    .filter(|x| x.output > 0)
                    .map(|x| super::catalog::tokens(x.output))
                    .unwrap_or_default();
                let (id, val) = (m.id.clone(), v.clone());
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("ag-set-model-{gi}-{i}")))
                        .mx_1()
                        .px_2()
                        .h(theme::u(28.))
                        .gap_2()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .text_sm()
                        .when(on, |d| d.bg(c(theme::SELECT)))
                        .when(!on, |d| d.hover(|s| s.bg(c(theme::BG_HOVER))))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(c(if on { theme::FG_STRONG } else { theme::FG }))
                                .when(on, |d| d.font_weight(FontWeight::MEDIUM))
                                .child(n),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(theme::u(52.))
                                .text_right()
                                .text_xs()
                                .text_color(c(theme::FG_MUTED))
                                .child(ctx),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(theme::u(48.))
                                .text_right()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(out),
                        )
                        .child(div().flex_none().w(theme::u(16.)).when(on, |d| {
                            d.child(
                                Icon::new(IconName::Check)
                                    .size(theme::u(14.))
                                    .text_color(c(theme::ACCENT_TEXT)),
                            )
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.agent_set_option(&id, val.clone(), true, cx)
                        })),
                );
            }
        }
        list.into_any_element()
    }

    fn settings_permissions(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.settings;
        let full = s.access() == "full";
        let off = s.access_off();
        let weak = cx.entity().downgrade();
        let full_switch = crate::ui::McToggle::new("ag-set-full")
            .checked(full)
            .on_click({
                let weak = weak.clone();
                move |on: &bool, _, cx| {
                    let on = *on;
                    _ = weak.update(cx, |this, cx| this.agent_toggle_full(on, cx));
                }
            });
        let full_desc: SharedString = if full {
            tr("On: whenever the agent asks to run a command or use a file anywhere on this PC, the answer is yes. Nothing waits for you.").into()
        } else {
            tr("Off: when the agent asks to run a command, change a file or open a folder outside the project, the request shows in the chat and it waits for your answer.").into()
        };
        let levels = h_flex().gap_1().flex_wrap().children(
            [
                ("write", tr("Edit files, ask for commands")),
                ("read", tr("Read only")),
            ]
            .into_iter()
            .map(|(id, label)| {
                Button::new(SharedString::from(format!("ag-set-off-{id}")))
                    .label(label)
                    .small()
                    .when(off == id, |b| b.primary())
                    .when(off != id, |b| b.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.settings.access() == "full" {
                            this.settings.agent_access_off = id.into();
                            this.settings.save();
                            cx.notify();
                        } else {
                            this.agent_set_access(id, cx);
                        }
                    }))
            }),
        );
        let notify = crate::ui::McToggle::new("ag-set-notify")
            .checked(s.agent_notify)
            .on_click(move |on: &bool, _, cx| {
                let on = *on;
                _ = weak.update(cx, |this, cx| {
                    this.settings.agent_notify = on;
                    this.settings.save();
                    cx.notify();
                });
            });
        let warn = full.then(|| {
            h_flex()
                .mt_1()
                .px_3()
                .py_2()
                .gap_2()
                .items_start()
                .rounded(px(6.))
                .bg(c(theme::WARN).opacity(0.10))
                .text_xs()
                .text_color(c(theme::FG))
                .child(div().flex_none().pt(px(1.)).child(Icon::new(Lucide::TriangleAlert).size(theme::u(13.)).text_color(c(theme::WARN))))
                .child(div().flex_1().min_w_0().child(tr("Full access is on. Turn it off before you let the agent work in a folder you don't trust.")))
        });
        v_flex()
            .child(row(tr("Full access to this PC").into(), Some(full_desc), full_switch.into_any_element(), false))
            .children(warn)
            .child(divider())
            .child(
                row(
                    tr("Without full access").into(),
                    Some(if full { tr("Used again when you switch full access off.").into() } else { tr("Reads always go through. Choose whether edits ask too.").into() }),
                    levels.into_any_element(),
                    compact,
                )
                .when(full, |d| d.opacity(0.6)),
            )
            .child(divider())
            .child(row(tr("Tell me when it waits").into(), Some(tr("The request shows in the chat and the taskbar button flashes while FluxCode is in the background.").into()), notify.into_any_element(), false))
            .child(div().pt_3().text_xs().text_color(c(theme::FG_DIM)).child(tr("The agent's own rules still come first: what its config denies stays denied, even with full access, and what it allows never reaches you.")))
            .into_any_element()
    }

    fn settings_session(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.settings;
        let weak = cx.entity().downgrade();
        let idle = crate::ui::McToggle::new("ag-set-idle")
            .checked(s.agent_idle_min > 0)
            .on_click(move |on: &bool, _, cx| {
                let on = *on;
                _ = weak.update(cx, |this, cx| {
                    this.settings.agent_idle_min = if on { 10 } else { 0 };
                    this.settings.save();
                    cx.notify();
                });
            });
        let chats = self.agent.history.len();
        v_flex()
            .child(row(tr("Stop when idle").into(), Some(tr("FluxCode already trims idle RAM and CPU automatically while keeping the agent live. Turn this on only if you want it fully stopped after 10 quiet minutes.").into()), idle.into_any_element(), false))
            .child(divider())
            .child(row(
                tr("Saved chats").into(),
                Some(if chats == 1 { tr("1 chat saved on this PC.").into() } else { crate::trf!("{n} chats saved on this PC."; n => chats).into() }),
                h_flex()
                    .gap_1()
                    .child(Button::new("ag-set-history").label(tr("Open")).small().on_click(cx.listener(|this, _, w, cx| {
                        this.agent.show_history = true;
                        this.agent_load_remote_list(cx);
                        this.agent_close_options(w, cx);
                    })))
                    .child(Button::new("ag-set-new").label(tr("New chat")).small().ghost().on_click(cx.listener(|this, _, w, cx| {
                        this.agent_new_chat(cx);
                        this.agent_close_options(w, cx);
                    })))
                    .into_any_element(),
                compact,
            ))
            .into_any_element()
    }

    fn settings_agent(
        &self,
        compact: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = agent_id();
        let name = agent_name();
        let by: SharedString = crate::acp::known(&id)
            .map(|k| k.by.to_string())
            .unwrap_or_default()
            .into();
        let path: SharedString = crate::acp::locate(&id)
            .map(|l| l.cmd.display().to_string())
            .unwrap_or_else(|| tr("Not installed").to_string())
            .into();
        let mut col = v_flex()
            .child(row(
                name.to_string().into(),
                (!by.is_empty()).then_some(by),
                Button::new("ag-set-switch")
                    .label(tr("Switch agent"))
                    .small()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.agent_close_options(w, cx);
                        this.agent_menu_toggle(super::Menu::Agent, w, cx);
                    }))
                    .into_any_element(),
                compact,
            ))
            .child(divider())
            .child(row(
                tr("Program").into(),
                None,
                div()
                    .text_xs()
                    .font_family("Cascadia Code")
                    .text_color(c(theme::FG_MUTED))
                    .child(path)
                    .into_any_element(),
                true,
            ));
        if crate::acp::locate(&id).is_some() {
            let mut stats = Vec::new();
            if self.agent.thermal.last_filtered_ws_mb > 0.1 {
                stats.push(crate::trf!(
                    "RAM {ws} MB / target {eq} MB";
                    ws => self.agent.thermal.last_filtered_ws_mb.round() as u64,
                    eq => self.agent.thermal.eq_mb.round() as u64
                ));
            }
            if self.agent.thermal.eco_active {
                stats.push(tr("EcoQoS idle").to_string());
            }
            let delta_hits = self.agent.ctx_cache.hits_unchanged + self.agent.ctx_cache.hits_delta;
            if delta_hits > 0 && self.agent.ctx_cache.bytes_saved > 0 {
                stats.push(crate::trf!(
                    "Delta saved {kb} KB";
                    kb => (self.agent.ctx_cache.bytes_saved / 1024).max(1)
                ));
            }
            let opt_ctrl = v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .items_center()
                        .child(super::menu::optimized_badge())
                        .child(
                            div().text_xs().text_color(c(theme::FG_MUTED)).child(tr(self
                                .agent
                                .thermal
                                .profile
                                .label())),
                        ),
                )
                .when(!stats.is_empty(), |v| {
                    v.child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(stats.join(" · ")),
                    )
                });
            col = col.child(divider()).child(row(
                tr("Optimization").into(),
                Some(tr("Live Kalman-Lyapunov memory control, EcoQoS idle cooling, lossless multi-hunk delta context, and frame-paced streaming.").into()),
                opt_ctrl.into_any_element(),
                true,
            ));
        }
        if crate::acp::is_opencode(&id) {
            if let Some(cfg) = opencode_config_file() {
                col = col.child(divider()).child(row(
                    tr("OpenCode's config").into(),
                    Some(tr("Providers, models and agents of your own.").into()),
                    Button::new("ag-set-cfg")
                        .label(tr("Open file"))
                        .small()
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.agent_close_options(w, cx);
                            this.open_file(cfg.clone(), w, cx);
                        }))
                        .into_any_element(),
                    compact,
                ));
            }
        }
        let (fam, accounts) = self.agent_accounts();
        if fam.is_some() {
            let current = self.agent_account().map(|a| a.name);
            let waiting = self.agent.account_wait.as_ref().map(|a| a.name.clone());
            let mut acc_row = h_flex().flex_wrap().gap_1p5().child(
                Button::new("ag-set-acc-def")
                    .label(tr("Default account"))
                    .xsmall()
                    .when(current.is_none(), |b| b.primary())
                    .when(current.is_some(), |b| b.ghost())
                    .on_click(cx.listener(|this, _, _, cx| this.agent_use_account(None, cx))),
            );
            for a in &accounts {
                let on = current.as_deref() == Some(a.name.as_str());
                let name = a.name.clone();
                let signout_name = a.name.clone();
                let tag = match &a.flow_profile {
                    Some(fp) => format!("{} ({fp})", a.name),
                    None => a.name.clone(),
                };
                acc_row = acc_row.child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new(SharedString::from(format!("ag-set-acc-{name}")))
                                .label(tag)
                                .xsmall()
                                .when(on, |b| b.primary())
                                .when(!on, |b| b.ghost())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.agent_use_account(Some(name.clone()), cx);
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!(
                                "ag-set-acc-out-{signout_name}"
                            )))
                            .icon(IconName::Close)
                            .xsmall()
                            .ghost()
                            .tooltip(crate::trf!("Sign out of {name}"; name => a.name))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.agent_sign_out(signout_name.clone(), cx);
                                },
                            )),
                        ),
                );
            }
            acc_row = acc_row.child(match waiting {
                Some(w_name) => Button::new("ag-set-acc-wait")
                    .label(crate::trf!("Waiting for {name} to sign in… (cancel)"; name => w_name))
                    .xsmall()
                    .on_click(cx.listener(|this, _, _, cx| this.agent_cancel_account(cx))),
                None => Button::new("ag-set-acc-add")
                    .label(tr("Add account and sign in…"))
                    .xsmall()
                    .on_click(cx.listener(|this, _, _, cx| this.agent_add_account(cx))),
            });
            col = col.child(divider()).child(row(
                tr("Account").into(),
                Some(tr("Isolated Windows sessions saved for this agent without touching global logins.").into()),
                acc_row.into_any_element(),
                true,
            ));
        }
        col = col.child(divider()).child(row(
            tr("Antigravity & Google Flow").into(),
            Some(tr("Pure-Rust multi-account router, Chrome session manager, credit check, and Flow MCP bridge.").into()),
            self.render_gflow_panel(cx),
            true,
        ));
        if crate::acp::managed(&id) {
            let id2 = id.clone();
            col = col.child(divider()).child(row(
                crate::trf!("Remove {name}"; name => name).into(),
                Some(tr("Deletes the program FluxCode downloaded. Your chats stay.").into()),
                Button::new("ag-set-remove")
                    .label(tr("Remove…"))
                    .small()
                    .danger()
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.agent_close_options(w, cx);
                        this.agent_ask_uninstall(&id2, w, cx);
                    }))
                    .into_any_element(),
                compact,
            ));
        }
        col.into_any_element()
    }

    pub(crate) fn render_gflow_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = crate::gflow::load_state();
        let profiles = crate::gflow::discover_profiles();
        let chrome_info = crate::gflow::chrome_binary();
        let has_google_chrome = chrome_info.as_ref().is_some_and(|(k, _)| *k == "chrome");
        let weak = cx.entity().downgrade();
        let weak2 = weak.clone();

        let mcp_toggle = h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                crate::ui::McToggle::new("ag-gf-mcp")
                    .checked(self.settings.agent_gflow_mcp)
                    .on_click(move |on: &bool, _, cx| {
                        let on = *on;
                        _ = weak.update(cx, |this, cx| {
                            this.settings.agent_gflow_mcp = on;
                            this.settings.save();
                            cx.notify();
                        });
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(c(theme::FG))
                    .child(tr("Connect Flow MCP to agents")),
            );

        let sync_toggle = h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                crate::ui::McToggle::new("ag-gf-sync")
                    .checked(self.settings.agent_gflow_sync)
                    .on_click(move |on: &bool, _, cx| {
                        let on = *on;
                        _ = weak2.update(cx, |this, cx| {
                            this.settings.agent_gflow_sync = on;
                            this.settings.save();
                            cx.notify();
                        });
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(c(theme::FG))
                    .child(tr("Sync with Antigravity account")),
            );

        let mode_bar = crate::ui::SegmentedControl::new(
            "ag-gf-mode",
            crate::gflow::RoutingMode::ALL.into_iter().map(|m| {
                let on = state.mode == m;
                crate::ui::SegItem::new(
                    SharedString::from(format!("ag-gf-mode-{}", m.as_str())),
                    tr(m.label()),
                    on,
                    cx.listener(move |this, _, _, cx| {
                        let _ = crate::gflow::set_mode(m, None, None, None);
                        this.agent.gflow_note = Some((tr(m.blurb()).to_string(), false));
                        cx.notify();
                    }),
                )
            }),
        );

        let next_name = crate::gflow::next_profile_name(&profiles);
        let out_dir = crate::gflow::output_dir();
        let actions = h_flex()
            .flex_wrap()
            .gap_1p5()
            .child(
                Button::new("ag-gf-add")
                    .label(tr("Add Flow account"))
                    .xsmall()
                    .primary()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let name = next_name.clone();
                        this.agent.gflow_note = Some((
                            crate::trf!("Opened browser for Flow profile {name}."; name => name),
                            false,
                        ));
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let res = cx
                                .background_spawn(async move {
                                    crate::gflow::launch_browser_login(&name)
                                })
                                .await;
                            _ = this.update(cx, |this, cx| {
                                if let Err(e) = res {
                                    this.agent.gflow_note = Some((e, true));
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                Button::new("ag-gf-credits")
                    .label(tr("Check credits"))
                    .xsmall()
                    .loading(self.agent.gflow_busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.agent.gflow_busy {
                            return;
                        }
                        this.agent.gflow_busy = true;
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let res = cx
                                .background_spawn(async move { crate::gflow::check_credits(false) })
                                .await;
                            _ = this.update(cx, |this, cx| {
                                this.agent.gflow_busy = false;
                                this.agent.gflow_note = Some(match res {
                                    Ok(rep) => {
                                        let mut msg = crate::trf!(
                                            "Total Flow credits: {total}";
                                            total => rep.total_credits
                                        );
                                        if let Some(note) = rep.note {
                                            msg = format!("{msg} · {note}");
                                        }
                                        (msg, false)
                                    }
                                    Err(e) => (e, true),
                                });
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                Button::new("ag-gf-refresh")
                    .label(tr("Refresh"))
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.gflow_note = None;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ag-gf-out")
                    .label(tr("Open folder"))
                    .xsmall()
                    .ghost()
                    .on_click(move |_, _, cx| {
                        let _ = std::fs::create_dir_all(&out_dir);
                        cx.reveal_path(&out_dir);
                    }),
            );

        let mut col = v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_4()
                    .child(mcp_toggle)
                    .child(sync_toggle),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(tr("Flow routing mode")),
                            )
                            .child(mode_bar),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(tr(state.mode.blurb())),
                    ),
            )
            .when(!has_google_chrome, |v| {
                v.child(
                    div()
                        .p_2()
                        .rounded(px(6.))
                        .bg(c(theme::WARN).opacity(0.10))
                        .text_xs()
                        .text_color(c(theme::FG))
                        .child(tr(
                            "Google Chrome not detected; install Google Chrome for full Flow login compatibility (fallback uses Edge/Brave).",
                        )),
                )
            })
            .child(actions);

        if profiles.is_empty() {
            col = col.child(
                div()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(tr("No saved Flow profiles yet. Click Add Flow account to sign in with an isolated Chrome window.")),
            );
        } else {
            for p in profiles {
                let p_pin = p.name.clone();
                let p_login = p.name.clone();
                let p_rm = p.name.clone();
                let is_pinned = state.sticky_profile.as_deref() == Some(p.name.as_str())
                    || (state.sticky_profile.is_none() && p.is_default);
                let email = p.email.unwrap_or_else(|| tr("Not signed in").to_string());
                let strat_label: SharedString = if p.chrome_strategy {
                    tr("Chrome").into()
                } else {
                    p.strategy.clone().into()
                };
                col = col.child(
                    v_flex()
                        .w_full()
                        .p_2()
                        .gap_1p5()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .bg(c(theme::BG_ELEVATED))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c(theme::FG_STRONG))
                                        .child(p.name.clone()),
                                )
                                .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(email))
                                .child(
                                    div()
                                        .px_1p5()
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(
                                            c(if p.chrome_strategy {
                                                theme::GREEN
                                            } else {
                                                theme::BORDER
                                            })
                                            .opacity(if p.chrome_strategy { 0.45 } else { 1.0 }),
                                        )
                                        .bg(c(theme::BG_ELEVATED))
                                        .text_size(theme::u(10.5))
                                        .text_color(c(if p.chrome_strategy {
                                            theme::GREEN
                                        } else {
                                            theme::FG_MUTED
                                        }))
                                        .child(strat_label),
                                )
                                .when_some(p.cached_credits, |h, cred| {
                                    h.child(
                                        div()
                                            .px_1p5()
                                            .rounded(px(4.))
                                            .border_1()
                                            .border_color(c(theme::BORDER))
                                            .text_size(theme::u(10.5))
                                            .text_color(c(theme::FG))
                                            .child(crate::trf!("{n} credits"; n => cred)),
                                    )
                                })
                                .when(p.in_cooldown, |h| {
                                    h.child(
                                        div()
                                            .px_1p5()
                                            .rounded(px(4.))
                                            .bg(c(theme::WARN).opacity(0.14))
                                            .text_size(theme::u(10.5))
                                            .text_color(c(theme::WARN))
                                            .child(tr("Cooldown")),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_1()
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "ag-gf-pin-{}",
                                        p.name
                                    )))
                                    .label(tr(if is_pinned { "Pinned" } else { "Pin" }))
                                    .xsmall()
                                    .when(is_pinned, |b| b.primary())
                                    .when(!is_pinned, |b| b.ghost())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let _ = crate::gflow::set_mode(
                                            crate::gflow::RoutingMode::Sticky,
                                            Some(p_pin.clone()),
                                            None,
                                            None,
                                        );
                                        this.agent.gflow_note = None;
                                        cx.notify();
                                    })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("ag-gf-in-{}", p.name)))
                                        .label(tr("Open login"))
                                        .xsmall()
                                        .ghost()
                                        .on_click(cx.listener(move |_this, _, _, cx| {
                                            let name = p_login.clone();
                                            cx.spawn(async move |this, cx| {
                                                let res = cx
                                                    .background_spawn(async move {
                                                        crate::gflow::launch_browser_login(&name)
                                                    })
                                                    .await;
                                                _ = this.update(cx, |this, cx| {
                                                    if let Err(e) = res {
                                                        this.agent.gflow_note = Some((e, true));
                                                    }
                                                    cx.notify();
                                                });
                                            })
                                            .detach();
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("ag-gf-rm-{}", p.name)))
                                        .label(tr("Remove"))
                                        .xsmall()
                                        .ghost()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Err(e) = crate::gflow::remove_profile(&p_rm) {
                                                this.agent.gflow_note = Some((e, true));
                                            }
                                            cx.notify();
                                        })),
                                ),
                        ),
                );
            }
        }

        if let Some((note, is_err)) = &self.agent.gflow_note {
            col = col.child(
                div()
                    .text_xs()
                    .text_color(c(if *is_err {
                        theme::DANGER
                    } else {
                        theme::FG_MUTED
                    }))
                    .child(note.clone()),
            );
        }

        col.into_any_element()
    }
}

fn row(title: SharedString, desc: Option<SharedString>, control: AnyElement, stacked: bool) -> Div {
    let text = v_flex()
        .flex_1()
        .min_w_0()
        .gap_0p5()
        .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
        .when_some(desc, |v, d| {
            v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(d))
        });
    if stacked {
        v_flex().py_3().gap_2().child(text).child(control)
    } else {
        h_flex()
            .py_3()
            .gap_4()
            .items_start()
            .child(text)
            .child(div().flex_none().pt(px(2.)).child(control))
    }
}

fn divider() -> Div {
    div().h(px(1.)).bg(c(theme::BORDER))
}

fn segmented(o: &ConfigOpt, cx: &mut Context<Workbench>) -> AnyElement {
    let mut choices = o.choices.clone();
    if o.category == "thought_level" {
        choices.sort_by_key(|c| if c.0 == "default" { 0 } else { 1 });
    }
    let group_id = SharedString::from(format!("ag-set-seg-{}", o.id));
    crate::ui::SegmentedControl::new(
        group_id,
        choices.into_iter().map(|(v, n, _)| {
            let id = o.id.clone();
            let on = o.current == v;
            crate::ui::SegItem::new(
                SharedString::from(format!("ag-set-{id}-{v}")),
                n,
                on,
                cx.listener(move |this, _, _, cx| this.agent_set_option(&id, v.clone(), true, cx)),
            )
        }),
    )
    .into_any_element()
}
