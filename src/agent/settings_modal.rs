//! The agent's settings, as a dialog over the window instead of a page that replaces
//! the chat: open it from the gear in the panel's title row, change things, and the
//! chat is still where it was when it closes (Esc, the close button, or a click outside).
//!
//! Sections sit in a rail on the left (a row of tabs when the window is narrow). Each
//! section is a column of rows: the setting's name and one line on what it does, the
//! control on the right (under it when there is no room).

use super::{agent_glyph, agent_id, agent_name, menu::effort_note, model_groups, opencode_config_file, ConfigOpt, Status};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    switch::Switch,
    v_flex, Icon, IconName, Sizable,
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
    const ALL: [Tab; 5] = [Tab::Model, Tab::Permissions, Tab::Session, Tab::Browser, Tab::Agent];

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

    /// One line under the section's title.
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

/// Below this card width the section rail becomes a row of tabs and rows stack.
const COMPACT: f32 = 600.;

impl Workbench {
    /// The dialog, when it is open (drawn over the whole window).
    pub(crate) fn render_agent_settings(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                .when(on, |d| d.bg(c(theme::SELECT)).text_color(c(theme::FG_STRONG)))
                .when(!on, |d| d.text_color(c(theme::FG_MUTED)).hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG))))
                .child(Icon::new(t.icon()).size(theme::u(15.)).flex_none().text_color(c(if on { theme::ACCENT_TEXT } else { theme::FG_DIM })))
                .child(div().whitespace_nowrap().child(tr(t.label())))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.agent.settings_tab = t;
                    cx.notify();
                }))
        });
        let nav = if compact {
            h_flex().id("ag-set-nav").flex_none().px_3().py_2().gap_1().overflow_x_scroll().border_b_1().border_color(c(theme::BORDER)).children(nav).into_any_element()
        } else {
            v_flex().flex_none().w(theme::u(184.)).p_2().gap_0p5().border_r_1().border_color(c(theme::BORDER)).bg(c(theme::BG_EDITOR)).children(nav).into_any_element()
        };

        let body = match tab {
            Tab::Model => self.settings_model(compact, cx),
            Tab::Permissions => self.settings_permissions(compact, cx),
            Tab::Session => self.settings_session(compact, cx),
            Tab::Browser => v_flex().child(row(tr("Browser for agents").into(), None, self.render_browser_setting(cx), true)).into_any_element(),
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
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr(tab.label())))
            .child(div().pb_2().text_xs().text_color(c(theme::FG_DIM)).child(tr(tab.blurb())))
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
            .child(div().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(crate::trf!("{name} settings"; name => name)))
            .child(
                h_flex()
                    .flex_none()
                    .px_2()
                    .h(theme::u(20.))
                    .gap_1p5()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(div().size(px(6.)).rounded_full().bg(c(dot)))
                    .child(state),
            )
            .child(div().flex_1())
            .when(!compact, |h| h.child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(tr("Esc closes"))))
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
            .rounded(px(10.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(c(theme::BG_SIDE))
            // Elevation: the one thing above the window while it is open.
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
            .child(if compact { v_flex().flex_1().min_h_0().child(nav).child(content).into_any_element() } else { h_flex().flex_1().min_h_0().items_stretch().child(nav).child(content).into_any_element() });

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(0.36))
                .flex()
                .items_center()
                .justify_center()
                .child(card.with_animation("ag-settings-in", Animation::new(std::time::Duration::from_millis(160)).with_easing(gpui_kit::ease_out_quint()), |el, t| el.opacity(t).mt(px((1. - t) * 6.))))
                .into_any_element(),
        )
    }

    fn settings_model(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let view = self.agent_config_view();
        let name = agent_name();
        if view.is_empty() {
            let on = matches!(self.agent.status, Status::Starting | Status::Ready | Status::Busy);
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
            col = col.child(divider()).child(row(tr("Effort").into(), Some(note), segmented(e, cx), compact));
        }
        if let Some(m) = view.iter().find(|o| o.category == "mode").filter(|o| o.choices.len() > 1) {
            let desc: SharedString = m.choices.iter().find(|c| c.0 == m.current).map(|c| c.2.clone()).unwrap_or_default().into();
            col = col.child(divider()).child(row(tr("Mode").into(), (!desc.is_empty()).then_some(desc), segmented(m, cx), compact));
        }
        for o in view.iter().filter(|o| !matches!(o.category.as_str(), "model" | "thought_level" | "mode")) {
            col = col.child(divider()).child(row(o.name.clone().into(), None, segmented(o, cx), compact));
        }
        col.into_any_element()
    }

    /// The models in one scrolling list, laid out like the composer's menu: the name,
    /// then the context and output limits in columns that line up.
    fn settings_model_list(&self, m: &ConfigOpt, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex().id("ag-set-models").max_h(theme::u(248.)).overflow_y_scroll().py_1().rounded(px(6.)).border_1().border_color(c(theme::BORDER)).bg(c(theme::BG_EDITOR));
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
                    .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().flex_none().w(theme::u(52.)).text_right().child(tr("Context")))
                    .child(div().flex_none().w(theme::u(48.)).text_right().child(tr("Output")))
                    .child(div().flex_none().w(theme::u(16.))),
            );
            for (i, (v, n)) in models.into_iter().enumerate() {
                let on = v == m.current;
                let meta = super::catalog::get(&v);
                let ctx = meta.map(|x| super::catalog::tokens(x.context)).unwrap_or_default();
                let out = meta.filter(|x| x.output > 0).map(|x| super::catalog::tokens(x.output)).unwrap_or_default();
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
                        .child(div().flex_1().min_w_0().truncate().text_color(c(if on { theme::FG_STRONG } else { theme::FG })).when(on, |d| d.font_weight(FontWeight::MEDIUM)).child(n))
                        .child(div().flex_none().w(theme::u(52.)).text_right().text_xs().text_color(c(theme::FG_MUTED)).child(ctx))
                        .child(div().flex_none().w(theme::u(48.)).text_right().text_xs().text_color(c(theme::FG_DIM)).child(out))
                        .child(div().flex_none().w(theme::u(16.)).when(on, |d| d.child(Icon::new(IconName::Check).size(theme::u(14.)).text_color(c(theme::ACCENT_TEXT)))))
                        .on_click(cx.listener(move |this, _, _, cx| this.agent_set_option(&id, val.clone(), true, cx))),
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
        let full_switch = Switch::new("ag-set-full").checked(full).on_click({
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
        let levels = h_flex().gap_1().flex_wrap().children([("write", tr("Edit files, ask for commands")), ("read", tr("Read only"))].into_iter().map(|(id, label)| {
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
        }));
        let notify = Switch::new("ag-set-notify").checked(s.agent_notify).on_click(move |on: &bool, _, cx| {
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
        let idle = Switch::new("ag-set-idle").checked(s.agent_idle_min > 0).on_click(move |on: &bool, _, cx| {
            let on = *on;
            _ = weak.update(cx, |this, cx| {
                this.settings.agent_idle_min = if on { 10 } else { 0 };
                this.settings.save();
                cx.notify();
            });
        });
        let chats = self.agent.history.len();
        v_flex()
            .child(row(tr("Stop when idle").into(), Some(tr("Turns the agent off after 10 quiet minutes to free memory. Your next message starts it again in the same chat.").into()), idle.into_any_element(), false))
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

    fn settings_agent(&self, compact: bool, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let id = agent_id();
        let name = agent_name();
        let by: SharedString = crate::acp::known(&id).map(|k| k.by.to_string()).unwrap_or_default().into();
        let path: SharedString = crate::acp::locate(&id).map(|l| l.cmd.display().to_string()).unwrap_or_else(|| tr("Not installed").to_string()).into();
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
            .child(row(tr("Program").into(), None, div().text_xs().font_family("Cascadia Code").text_color(c(theme::FG_MUTED)).child(path).into_any_element(), true));
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
}

/// One setting: its name and what it does on the left, the control on the right, or
/// under them (`stacked`) for wide controls and narrow windows.
fn row(title: SharedString, desc: Option<SharedString>, control: AnyElement, stacked: bool) -> Div {
    let text = v_flex()
        .flex_1()
        .min_w_0()
        .gap_0p5()
        .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
        .when_some(desc, |v, d| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(d)));
    if stacked {
        v_flex().py_3().gap_2().child(text).child(control)
    } else {
        h_flex().py_3().gap_4().items_start().child(text).child(div().flex_none().pt(px(2.)).child(control))
    }
}

fn divider() -> Div {
    div().h(px(1.)).bg(c(theme::BORDER))
}

/// A few choices as a row of buttons, the current one filled.
fn segmented(o: &ConfigOpt, cx: &mut Context<Workbench>) -> AnyElement {
    let mut choices = o.choices.clone();
    if o.category == "thought_level" {
        choices.sort_by_key(|c| if c.0 == "default" { 0 } else { 1 });
    }
    h_flex()
        .gap_1()
        .flex_wrap()
        .children(choices.into_iter().map(|(v, n, _)| {
            let id = o.id.clone();
            let on = o.current == v;
            Button::new(SharedString::from(format!("ag-set-{id}-{v}")))
                .label(n)
                .small()
                .when(on, |b| b.primary())
                .when(!on, |b| b.ghost())
                .on_click(cx.listener(move |this, _, _, cx| this.agent_set_option(&id, v.clone(), true, cx)))
        }))
        .into_any_element()
}
