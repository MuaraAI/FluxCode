//! The agent panel's menus: which agent runs, and the model, mode and permissions under
//! the message box. All four share one look, drawn here rather than by a generic popup:
//!
//! - each opens inside the panel and as wide as it: the agent list under the title, the
//!   others right above the message box, so none leaves the panel or covers what you type;
//! - long lists have a search box, and Up, Down, Enter and Esc work in all of them;
//! - every row is a name over one dim line, with a check on the current choice, and the
//!   agent list says which agent is on and which are off.

use super::{agent_glyph, agent_id, model_groups, ConfigOpt, Status};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::Cell;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Menu {
    Agent,
    Model,
    Mode,
    Access,
}

pub struct MenuState {
    pub open: Option<Menu>,
    query: Entity<InputState>,
    focus: FocusHandle,
    sel: usize,
    scroll: ScrollHandle,
    /// Scroll the highlighted row into view on the next frame.
    reveal: Cell<bool>,
    /// A click outside just closed this menu: its own button then must not reopen it.
    closed: Option<(Menu, Instant)>,
    _sub: Subscription,
}

impl MenuState {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx));
        let _sub = cx.subscribe(&query, |this: &mut Workbench, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.agent.menu.sel = 0;
                this.agent.menu.reveal.set(true);
                cx.notify();
            }
        });
        Self { open: None, query, focus: cx.focus_handle(), sel: 0, scroll: ScrollHandle::new(), reveal: Cell::new(false), closed: None, _sub }
    }
}

#[derive(Clone)]
enum Act {
    Use(String),
    Install(String),
    Opt(String, String),
    Access(&'static str),
}

enum Lead {
    Agent(String),
    Icon(IconName, theme::Tok),
    None,
}

enum Trail {
    Download,
    Note(SharedString),
    /// A dot and a word: whether the agent is on.
    State(theme::Tok, SharedString),
    None,
}

struct Row {
    group: SharedString,
    lead: Lead,
    title: SharedString,
    sub: SharedString,
    /// The line under the name may take two lines (descriptions); else it is cut to one.
    wrap: bool,
    checked: bool,
    trail: Trail,
    act: Act,
}

impl Row {
    fn matches(&self, q: &str) -> bool {
        let hay = format!("{} {} {}", self.title, self.sub, self.group).to_lowercase();
        q.split_whitespace().all(|w| hay.contains(w))
    }
}

/// Lists this long get a search box.
const SEARCH_FROM: usize = 9;

fn searchable(m: Menu, rows: usize) -> bool {
    matches!(m, Menu::Agent | Menu::Model) || rows >= SEARCH_FROM
}

fn placeholder(m: Menu) -> &'static str {
    match m {
        Menu::Agent => "Search agents",
        Menu::Model => "Search models",
        _ => "Search",
    }
}

impl Workbench {
    /// A menu's button was clicked: open it, or close it when it is open.
    pub(crate) fn agent_menu_toggle(&mut self, m: Menu, window: &mut Window, cx: &mut Context<Self>) {
        let st = &mut self.agent.menu;
        if let Some((was, at)) = st.closed.take() {
            if was == m && at.elapsed().as_millis() < 300 {
                return cx.notify();
            }
        }
        if st.open == Some(m) {
            return self.agent_menu_close(true, window, cx);
        }
        st.open = Some(m);
        let q = st.query.clone();
        q.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder(tr(placeholder(m)), window, cx);
        });
        let rows = self.agent_menu_rows(m, cx);
        let st = &mut self.agent.menu;
        st.sel = rows.iter().position(|r| r.checked).unwrap_or(0);
        // The handle still holds the last menu's rows: start at the top.
        st.scroll.set_offset(point(px(0.), px(0.)));
        if searchable(m, rows.len()) {
            q.focus_handle(cx).focus(window, cx);
        } else {
            st.focus.focus(window, cx);
        }
        // The rows' positions are known after the first frame: then scroll the
        // current choice into view.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(30)).await;
            _ = this.update(cx, |this, cx| {
                this.agent.menu.reveal.set(true);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Close the open menu. `quiet`: focus stays where the click went.
    fn agent_menu_close(&mut self, quiet: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = self.agent.menu.open.take() else { return };
        if quiet {
            self.agent.menu.closed = Some((m, Instant::now()));
        } else {
            let h = self.agent.input.focus_handle(cx);
            h.focus(window, cx);
        }
        cx.notify();
    }

    fn agent_menu_filtered(&self, cx: &App) -> Vec<Row> {
        let Some(m) = self.agent.menu.open else { return Vec::new() };
        let rows = self.agent_menu_rows(m, cx);
        let q = self.agent.menu.query.read(cx).value().trim().to_lowercase();
        if q.is_empty() || !searchable(m, rows.len()) {
            return rows;
        }
        rows.into_iter().filter(|r| r.matches(&q)).collect()
    }

    pub(crate) fn on_agent_menu_up(&mut self, _: &crate::workbench::AgentMenuUp, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.agent_menu_filtered(cx).len();
        let st = &mut self.agent.menu;
        if n > 0 {
            st.sel = if st.sel == 0 { n - 1 } else { st.sel.min(n) - 1 };
            st.reveal.set(true);
        }
        cx.notify();
    }

    pub(crate) fn on_agent_menu_down(&mut self, _: &crate::workbench::AgentMenuDown, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.agent_menu_filtered(cx).len();
        let st = &mut self.agent.menu;
        if n > 0 {
            st.sel = (st.sel + 1) % n;
            st.reveal.set(true);
        }
        cx.notify();
    }

    pub(crate) fn on_agent_menu_pick(&mut self, _: &crate::workbench::AgentMenuPick, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.agent_menu_filtered(cx);
        if let Some(r) = rows.get(self.agent.menu.sel.min(rows.len().saturating_sub(1))) {
            let act = r.act.clone();
            self.agent_menu_pick(act, window, cx);
        }
    }

    pub(crate) fn on_agent_menu_close(&mut self, _: &crate::workbench::AgentMenuClose, window: &mut Window, cx: &mut Context<Self>) {
        self.agent_menu_close(false, window, cx);
    }

    fn agent_menu_pick(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        self.agent.menu.open = None;
        match act {
            Act::Use(id) => self.agent_use(&id, window, cx),
            Act::Install(id) => self.agent_install(id, window, cx),
            Act::Opt(id, v) => self.agent_set_option(&id, v, true, cx),
            Act::Access(level) => self.agent_set_access(level, cx),
        }
        let h = self.agent.input.focus_handle(cx);
        h.focus(window, cx);
        cx.notify();
    }

    /// Every row of a menu, in order (the search filters them).
    fn agent_menu_rows(&self, m: Menu, _cx: &App) -> Vec<Row> {
        match m {
            Menu::Agent => self.agent_rows(),
            Menu::Model => {
                let view = self.agent_config_view();
                let Some(o) = view.iter().find(|o| o.category == "model") else { return Vec::new() };
                if let Some(rows) = claude_rows(o) {
                    return rows;
                }
                model_groups(&o.choices)
                    .into_iter()
                    .flat_map(|(group, models)| {
                        let title: SharedString = match group.as_str() {
                            "free" => tr("Free (OpenCode Zen)").into(),
                            g => g.to_string().into(),
                        };
                        let (id, cur) = (o.id.clone(), o.current.clone());
                        models.into_iter().map(move |(v, n)| Row {
                            group: title.clone(),
                            lead: Lead::None,
                            title: n.into(),
                            sub: SharedString::default(),
                            wrap: false,
                            checked: v == cur,
                            trail: Trail::None,
                            act: Act::Opt(id.clone(), v),
                        })
                    })
                    .collect()
            }
            Menu::Mode => {
                let view = self.agent_config_view();
                let Some(o) = view.iter().find(|o| o.category == "mode") else { return Vec::new() };
                o.choices
                    .iter()
                    .map(|(v, n, d)| Row {
                        group: SharedString::default(),
                        lead: Lead::None,
                        title: n.clone().into(),
                        sub: d.clone().into(),
                        wrap: true,
                        checked: *v == o.current,
                        trail: Trail::None,
                        act: Act::Opt(o.id.clone(), v.clone()),
                    })
                    .collect()
            }
            Menu::Access => {
                let level = self.settings.access().to_string();
                ACCESS
                    .iter()
                    .map(|&(id, icon, title, desc)| Row {
                        group: SharedString::default(),
                        lead: Lead::Icon(icon, if id == "full" { theme::WARN } else { theme::FG_MUTED }),
                        title: tr(title).into(),
                        sub: tr(desc).into(),
                        wrap: true,
                        checked: level == id,
                        trail: Trail::None,
                        act: Act::Access(id),
                    })
                    .collect()
            }
        }
    }

    /// Agents on this PC (with whether each is on), then the well-known ones to get,
    /// then the rest of the ACP registry.
    fn agent_rows(&self) -> Vec<Row> {
        let installed = crate::acp::installed();
        let reg = crate::acp::registry();
        let current = agent_id();
        let busy = self.agent.installing.as_ref().map(|(id, _, _)| id.clone());
        let by_of = |id: &str| -> SharedString {
            crate::acp::known(id).map(|k| k.by.to_string()).or_else(|| reg.iter().find(|e| e.id == id).map(|e| first_sentence(&e.description))).unwrap_or_default().into()
        };
        let mut rows = Vec::new();
        let here: SharedString = tr("On this PC").into();
        for id in &installed {
            let on = *id == current;
            let trail = if on {
                match &self.agent.status {
                    Status::Ready => Trail::State(theme::GREEN, tr("On").into()),
                    Status::Busy => Trail::State(theme::BLUE, tr("Working").into()),
                    Status::Starting => Trail::State(theme::WARN, tr("Starting").into()),
                    Status::Error(_) => Trail::State(theme::DANGER, tr("Error").into()),
                    Status::Off => Trail::State(theme::FG_DIM, tr("Off").into()),
                }
            } else {
                Trail::State(theme::FG_DIM, tr("Off").into())
            };
            rows.push(Row { group: here.clone(), lead: Lead::Agent(id.clone()), title: crate::acp::display_name(id).into(), sub: by_of(id), wrap: false, checked: on, trail, act: Act::Use(id.clone()) });
        }
        let get = |id: String, name: String, sub: SharedString, group: &SharedString| {
            let trail = if busy.as_deref() == Some(id.as_str()) { Trail::Note(tr("Installing…").into()) } else { Trail::Download };
            Row { group: group.clone(), lead: Lead::Agent(id.clone()), title: name.into(), sub, wrap: false, checked: false, trail, act: Act::Install(id) }
        };
        let avail: SharedString = tr("Available to download").into();
        for k in crate::acp::KNOWN.iter().filter(|k| !installed.iter().any(|i| i == k.id)) {
            rows.push(get(k.id.to_string(), k.name.to_string(), k.by.into(), &avail));
        }
        let more: SharedString = tr("More in the ACP registry").into();
        for e in reg.iter().filter(|e| crate::acp::known(&e.id).is_none() && !crate::acp::hidden(&e.id) && !installed.iter().any(|i| *i == e.id) && e.method().is_some()) {
            rows.push(get(e.id.clone(), e.name.clone(), first_sentence(&e.description).into(), &more));
        }
        rows
    }

    /// The open menu, when it belongs at `top` (the agent list) or above the message box.
    pub(crate) fn render_agent_menu(&self, top: bool, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = self.agent.menu.open?;
        if (m == Menu::Agent) != top {
            return None;
        }
        let st = &self.agent.menu;
        let total = self.agent_menu_rows(m, cx).len();
        let search = searchable(m, total);
        let rows = self.agent_menu_filtered(cx);
        let sel = st.sel.min(rows.len().saturating_sub(1));
        let vh = window.viewport_size().height / px(1.);
        let u = |v: f32| f32::from(theme::u(v));
        // Room left for the list: between the panel's top and the message box (both
        // measured as the panel was drawn), minus the menu's own title, search box and
        // effort slider. Without a measurement yet, the window height decides.
        let chrome = u(34.) + if search { u(38.) } else { 0. } + if m == Menu::Agent { u(38.) } else { 0. } + if m == Menu::Model && self.agent_effort().is_some() { u(74.) } else { 0. } + 12.;
        let (panel_top, composer_top) = self.agent.geom.get();
        let room = if composer_top > panel_top + 40. {
            if top { composer_top - panel_top - u(40.) } else { composer_top - panel_top - u(10.) }
        } else if top {
            vh - 150.
        } else {
            vh - 300.
        };
        let max_list = (room - chrome).clamp(90., if top { 460. } else { 520. });

        let mut list = v_flex().id("ag-menu-list").max_h(px(max_list)).overflow_y_scroll().track_scroll(&st.scroll).pb_1();
        let mut last: Option<SharedString> = None;
        let (mut child, mut sel_child) = (0usize, 0usize);
        for (i, r) in rows.iter().enumerate() {
            if !r.group.is_empty() && last.as_ref() != Some(&r.group) {
                list = list.child(div().flex_none().px_3().pt_2().pb_0p5().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_DIM)).child(r.group.clone()));
                child += 1;
            }
            last = Some(r.group.clone());
            if i == sel {
                sel_child = child;
            }
            list = list.child(self.render_menu_row(i, r, i == sel, cx));
            child += 1;
        }
        if rows.is_empty() {
            let q = st.query.read(cx).value().trim().to_string();
            list = list.child(div().flex_none().px_3().py_2().text_xs().text_color(c(theme::FG_DIM)).child(if total == 0 {
                tr("Nothing to choose yet: the agent lists these once it is on.").to_string()
            } else {
                crate::trf!("Nothing matches “{q}”"; q => q)
            }));
        }
        if st.reveal.replace(false) {
            st.scroll.scroll_to_item(sel_child);
        }

        let title: SharedString = match m {
            Menu::Agent => tr("Agents").into(),
            Menu::Model => tr("Model").into(),
            Menu::Mode => tr("Mode").into(),
            Menu::Access => tr("Permissions").into(),
        };
        let bg = gpui_kit::Hsla::from(rgb(theme::palette().swatches()[1]));
        let menu = v_flex()
            .id("ag-menu")
            .key_context("AgentMenu")
            .when(!search, |d| d.track_focus(&st.focus))
            .occlude()
            .w_full()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(bg)
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_down_out(cx.listener(|this, _, w, cx| this.agent_menu_close(true, w, cx)))
            .child(
                h_flex()
                    .h(theme::u(30.))
                    .px_3()
                    .gap_2()
                    .child(div().flex_1().min_w_0().truncate().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(title))
                    .child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(tr("↑↓ Enter · Esc"))),
            )
            .when(search, |v| {
                v.child(
                    div()
                        .px_2()
                        .pb_1()
                        .child(Input::new(&st.query).small().cleanable(true).prefix(Icon::new(IconName::Search).size(theme::u(13.)).text_color(c(theme::FG_DIM)))),
                )
            })
            .when(m == Menu::Model, |v| v.children(self.render_effort_strip(cx)))
            .child(div().h(px(1.)).bg(c(theme::BORDER)))
            .child(list)
            .when(m == Menu::Agent, |v| v.child(self.render_agent_menu_footer(cx)));
        Some(menu.into_any_element())
    }

    fn render_menu_row(&self, i: usize, r: &Row, sel: bool, cx: &mut Context<Self>) -> AnyElement {
        let lead: Option<AnyElement> = match &r.lead {
            Lead::Agent(id) => Some(div().flex_none().size(theme::u(18.)).flex().items_center().justify_center().child(agent_glyph(id, 16.)).into_any_element()),
            Lead::Icon(icon, tok) => Some(div().flex_none().pt(px(1.)).child(Icon::new(*icon).size(theme::u(14.)).text_color(c(*tok))).into_any_element()),
            Lead::None => None,
        };
        let trail: Option<AnyElement> = match &r.trail {
            Trail::Download => Some(Icon::new(IconName::Download).size(theme::u(14.)).text_color(c(theme::FG_MUTED)).into_any_element()),
            Trail::Note(t) => Some(div().text_xs().text_color(c(theme::FG_DIM)).child(t.clone()).into_any_element()),
            Trail::State(tok, t) => Some(
                h_flex()
                    .gap_1()
                    .child(div().size(px(6.)).rounded_full().bg(c(*tok)))
                    .child(div().text_xs().text_color(c(if *tok == theme::FG_DIM { theme::FG_DIM } else { theme::FG_MUTED })).child(t.clone()))
                    .into_any_element(),
            ),
            Trail::None => None,
        };
        let act = r.act.clone();
        h_flex()
            .id(("ag-menu-row", i))
            // A scrolling column would squeeze its rows to fit; they keep their height.
            .flex_none()
            .mx_1()
            .px_2()
            .py(px(if r.wrap { 5. } else { 3. }))
            .min_h(theme::u(28.))
            .gap_2p5()
            .when(r.wrap, |h| h.items_start())
            .rounded(px(5.))
            .cursor_pointer()
            .when(sel, |h| h.bg(c(theme::SELECT)))
            .when(!sel, |h| h.hover(|s| s.bg(c(theme::BG_HOVER))))
            .children(lead)
            .child({
                let name = div().text_sm().text_color(c(if r.checked { theme::FG_STRONG } else { theme::FG })).when(r.checked, |d| d.font_weight(FontWeight::MEDIUM)).child(r.title.clone());
                if r.wrap {
                    // Name over a description of up to two lines.
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(name.truncate())
                        .when(!r.sub.is_empty(), |v| v.child(div().text_xs().text_color(c(theme::FG_DIM)).line_clamp(2).child(r.sub.clone())))
                } else {
                    // One line: the name, then who makes it, cut to fit.
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .child(name.flex_shrink_0().max_w(relative(0.62)).truncate())
                        .when(!r.sub.is_empty(), |h| h.child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(r.sub.clone())))
                }
            })
            .children(trail.map(|t| div().flex_none().when(r.wrap, |d| d.pt(px(2.))).child(t)))
            .child(div().flex_none().w(theme::u(14.)).when(r.wrap, |d| d.pt(px(2.))).when(r.checked, |d| d.child(Icon::new(IconName::Check).size(theme::u(14.)).text_color(c(theme::ACCENT_TEXT)))))
            .on_click(cx.listener(move |this, _, w, cx| {
                cx.stop_propagation();
                this.agent_menu_pick(act.clone(), w, cx)
            }))
            .into_any_element()
    }

    /// Effort, as a slider above the models (it changes without closing the menu): the
    /// levels from least to most thinking, the current one named with what it costs.
    fn render_effort_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.agent_effort()?;
        let levels: Vec<(String, String)> = e.choices.iter().filter(|c| c.0 != "default").map(|c| (c.0.clone(), c.1.clone())).collect();
        if levels.len() < 2 {
            return None;
        }
        let n = levels.len();
        let at = levels.iter().position(|l| l.0 == e.current);
        let pos = |i: usize| i as f32 / (n - 1) as f32;
        let to = at.map(pos).unwrap_or(0.5);
        let from = self.agent.effort_from.get().unwrap_or(to);
        let name: SharedString = match at {
            Some(i) => levels[i].1.clone().into(),
            None => tr("Default").into(),
        };
        let note: SharedString = tr(effort_note(&e.current)).into();
        let key: SharedString = format!("ag-effort-{}", e.current).into();
        let track = div()
            .relative()
            .h(theme::u(18.))
            .mx(theme::u(7.))
            .flex()
            .items_center()
            // The rail, and the part up to the current level.
            .child(div().absolute().left_0().right_0().h(px(4.)).rounded_full().bg(c(theme::BG_HOVER)))
            // A tick at each level.
            .children((0..n).map(|i| {
                let on = at.is_some_and(|a| i <= a);
                div().absolute().left(relative(pos(i))).ml(px(-2.)).size(px(4.)).rounded_full().bg(c(if on { theme::ACCENT } else { theme::BORDER_STRONG }))
            }))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .child(div().h(px(4.)).rounded_full().bg(c(theme::ACCENT)).with_animation(SharedString::from(format!("{key}-fill")), Animation::new(std::time::Duration::from_millis(220)).with_easing(gpui_kit::ease_out_quint()), move |el, t| el.w(relative(from + (to - from) * t)))),
            )
            // The handle.
            .child(
                div()
                    .absolute()
                    .ml(theme::u(-7.))
                    .size(theme::u(14.))
                    .rounded_full()
                    .bg(c(theme::FG_STRONG))
                    .border_2()
                    .border_color(c(theme::ACCENT))
                    .shadow_sm()
                    .with_animation(SharedString::from(format!("{key}-knob")), Animation::new(std::time::Duration::from_millis(220)).with_easing(gpui_kit::ease_out_quint()), move |el, t| el.left(relative(from + (to - from) * t))),
            )
            // Each level is a click target as wide as its share of the rail.
            .child(h_flex().absolute().inset_0().mx(theme::u(-7.)).children(levels.iter().enumerate().map(|(i, (v, _))| {
                let (id, v, here) = (e.id.clone(), v.clone(), to);
                div().id(("ag-effort", i)).flex_1().h_full().cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.agent.effort_from.set(Some(here));
                    this.agent_set_option(&id, v.clone(), true, cx)
                }))
            })));
        let labels = h_flex().children(levels.iter().enumerate().map(|(i, (v, l))| {
            let on = at == Some(i);
            let (id, v, here) = (e.id.clone(), v.clone(), to);
            div()
                .id(("ag-effort-l", i))
                .flex_1()
                .min_w_0()
                .truncate()
                .text_center()
                .text_size(theme::u(11.))
                .cursor_pointer()
                .text_color(c(if on { theme::FG_STRONG } else { theme::FG_DIM }))
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                .hover(|s| s.text_color(c(theme::FG)))
                .child(l.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.agent.effort_from.set(Some(here));
                    this.agent_set_option(&id, v.clone(), true, cx)
                }))
        }));
        Some(
            v_flex()
                .px_3()
                .pt_0p5()
                .pb_2()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_none().text_xs().text_color(c(theme::FG_MUTED)).child(tr("Effort")))
                        .child(div().flex_none().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(name))
                        .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(note)),
                )
                .child(track)
                .child(labels)
                .into_any_element(),
        )
    }

    fn render_agent_menu_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = agent_id();
        let name = super::agent_name();
        let link = |id: &'static str, label: SharedString| {
            div().id(id).px_2().py_1().rounded(px(4.)).text_xs().text_color(c(theme::FG_MUTED)).cursor_pointer().hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))).child(label)
        };
        h_flex()
            .px_1()
            .py_1()
            .gap_1()
            .border_t_1()
            .border_color(c(theme::BORDER))
            .child(link("ag-menu-settings", tr("Agent settings").into()).on_click(cx.listener(|this, _, w, cx| {
                this.agent_menu_close(false, w, cx);
                this.agent_open_options(cx);
            })))
            .child(div().flex_1())
            .when(crate::acp::managed(&current), |h| {
                h.child(link("ag-menu-remove", crate::trf!("Remove {name}…"; name => name).into()).on_click(cx.listener(move |this, _, w, cx| {
                    this.agent_menu_close(false, w, cx);
                    this.agent_ask_uninstall(&current, w, cx);
                })))
            })
            .into_any_element()
    }

    fn agent_effort(&self) -> Option<ConfigOpt> {
        self.agent_config_view().into_iter().find(|o| o.category == "thought_level")
    }
}

/// The permission levels: id, icon, name, what it allows.
pub(super) const ACCESS: [(&str, IconName, &str, &str); 3] = [
    ("read", IconName::Eye, "Read only", "Reads files and answers. Changes nothing."),
    ("write", IconName::PencilLine, "Edit files", "Edits files. Asks before running commands."),
    ("full", IconName::ShieldAlert, "Full access", "Edits files and runs commands without asking."),
];

/// One line on what an effort level is for.
pub(super) fn effort_note(level: &str) -> &'static str {
    match level {
        "minimal" | "low" => "fast and cheap, for small edits",
        "medium" => "good results with fewer tokens",
        "high" => "more thinking for harder problems",
        "xhigh" => "deep work on big changes, many tokens",
        "max" => "as much thinking as it takes, costs most",
        _ => "the agent's own default",
    }
}

/// Claude's prices per million tokens (input, output), from Anthropic's model table
/// (checked 2026-11-10). Haiku 5.5's is for prompts up to 100K tokens.
pub(super) fn claude_price(name: &str) -> Option<(f32, f32)> {
    let n = name.to_lowercase();
    let table: [(&str, f32, f32); 15] = [
        ("fable 5.2", 8., 40.),
        ("fable 5.1", 10., 50.),
        ("fable 5", 10., 50.),
        ("opus 5.5", 4., 20.),
        ("opus 5", 5., 25.),
        ("opus 4.8", 5., 25.),
        ("opus 4.7", 5., 25.),
        ("opus 4.6", 5., 25.),
        ("opus 4.5", 5., 25.),
        ("sonnet 5.5", 2., 10.),
        ("sonnet 5", 2., 10.),
        ("sonnet 4.6", 3., 15.),
        ("sonnet 4.5", 3., 15.),
        ("haiku 5.5", 0.10, 0.50),
        ("haiku 4.5", 1., 5.),
    ];
    // Longest name first, so "opus 5" doesn't take "opus 5.5".
    table.iter().filter(|(k, _, _)| n == *k || n.starts_with(&format!("{k} ")) || n.ends_with(k)).max_by_key(|(k, _, _)| k.len()).map(|(_, i, o)| (*i, *o))
}

fn money(v: f32) -> String {
    if v < 1. { format!("${v:.2}") } else if v.fract() == 0. { format!("${v:.0}") } else { format!("${v:.1}") }
}

/// "Opus 5.5" → (5.5, rank of the family: Fable 0, Opus 1, Sonnet 2, Haiku 3).
fn claude_version(name: &str) -> Option<(f32, u8)> {
    let mut it = name.split_whitespace();
    let fam = it.next()?.to_lowercase();
    let rank = match fam.as_str() {
        "fable" | "mythos" => 0,
        "opus" => 1,
        "sonnet" => 2,
        "haiku" => 3,
        _ => return None,
    };
    let v: f32 = it.next()?.parse().ok()?;
    Some((v, rank))
}

/// Claude Agent's models by generation, newest first, each with its price: the newest
/// do the same work with fewer tokens, so the older a group, the less it is worth it.
fn claude_rows(o: &super::ConfigOpt) -> Option<Vec<Row>> {
    let named: Vec<(&String, &String, &String, Option<(f32, u8)>)> = o.choices.iter().map(|(v, n, d)| (v, n, d, claude_version(n))).collect();
    if named.iter().filter(|x| x.3.is_some()).count() < 3 {
        return None;
    }
    let price_line = |name: &str| claude_price(name).map(|(i, out)| crate::trf!("{i} / {o} per 1M"; i => money(i), o => money(out))).unwrap_or_default();
    let row = |group: SharedString, v: &String, title: &String, sub: String| Row {
        group,
        lead: Lead::None,
        title: title.clone().into(),
        sub: sub.into(),
        wrap: true,
        checked: *v == o.current,
        trail: Trail::None,
        act: Act::Opt(o.id.clone(), v.clone()),
    };
    let mut rows = Vec::new();
    // Models without a version (Default): first, saying what they stand for.
    for (v, n, d, ver) in &named {
        if ver.is_none() {
            let sub = if d.is_empty() { String::new() } else { format!("{} · {}", d, price_line(d)) };
            rows.push(row(SharedString::default(), v, n, sub.trim_end_matches(" · ").to_string()));
        }
    }
    let gens: [(&str, fn(f32) -> bool); 4] = [
        ("Claude 5.5 · newest, the most for each token", |v| v >= 5.5),
        ("Claude 5", |v| (5.0..5.5).contains(&v)),
        ("Claude 4 · older: more tokens for the same work", |v| (4.0..5.0).contains(&v)),
        ("Older", |v| v < 4.0),
    ];
    for (label, keep) in gens {
        let mut g: Vec<_> = named.iter().filter(|x| x.3.is_some_and(|(v, _)| keep(v))).collect();
        g.sort_by(|a, b| {
            let (va, ra) = a.3.unwrap();
            let (vb, rb) = b.3.unwrap();
            ra.cmp(&rb).then(vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal))
        });
        let group: SharedString = tr(label).into();
        for (v, n, d, _) in g {
            let price = price_line(n);
            let sub = match (price.is_empty(), d.is_empty()) {
                (false, false) => format!("{price} · {d}"),
                (false, true) => price,
                (true, _) => d.to_string(),
            };
            rows.push(row(group.clone(), v, n, sub));
        }
    }
    Some(rows)
}

/// The first sentence of a description, for a row's one line (the row cuts it to fit).
fn first_sentence(s: &str) -> String {
    let s = s.trim();
    s.split(". ").next().unwrap_or(s).trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::{first_sentence, searchable, Act, Lead, Menu, Row, Trail};

    fn row(title: &str, sub: &str, group: &str) -> Row {
        Row { group: group.to_string().into(), lead: Lead::None, title: title.to_string().into(), sub: sub.to_string().into(), wrap: false, checked: false, trail: Trail::None, act: Act::Access("read") }
    }

    #[test]
    fn search_matches_every_word_anywhere() {
        let r = row("Claude Sonnet 5.5", "", "Anthropic");
        assert!(r.matches("sonnet"));
        assert!(r.matches("anthropic 5.5"));
        assert!(!r.matches("gpt"));
        assert!(row("Nova", "Nova by Compass AI", "More").matches("compass"));
    }

    #[test]
    fn claude_models_by_generation_with_prices() {
        use super::{claude_price, claude_rows};
        let o = crate::agent::ConfigOpt {
            id: "model".into(),
            name: "Model".into(),
            category: "model".into(),
            current: "haiku".into(),
            choices: vec![
                ("default".into(), "Default (recommended)".into(), "Opus 5.5".into()),
                ("opus".into(), "Opus 5.5".into(), "For complex work".into()),
                ("claude-opus-5".into(), "Opus 5".into(), "".into()),
                ("sonnet".into(), "Sonnet 5.5".into(), "".into()),
                ("claude-fable-5-1".into(), "Fable 5.1".into(), "".into()),
                ("haiku".into(), "Haiku 5.5".into(), "".into()),
                ("claude-opus-4-8".into(), "Opus 4.8".into(), "".into()),
                ("claude-sonnet-4-6".into(), "Sonnet 4.6".into(), "".into()),
            ],
        };
        let rows = claude_rows(&o).unwrap();
        let order: Vec<&str> = rows.iter().map(|r| r.title.as_ref()).collect();
        assert_eq!(order, ["Default (recommended)", "Opus 5.5", "Sonnet 5.5", "Haiku 5.5", "Fable 5.1", "Opus 5", "Opus 4.8", "Sonnet 4.6"]);
        assert!(rows[0].sub.contains("$4 / $20"), "{}", rows[0].sub);
        assert!(rows[3].checked && rows[3].sub.starts_with("$0.10 / $0.50 per 1M"), "{}", rows[3].sub);
        assert_eq!(claude_price("Opus 5"), Some((5., 25.)));
        assert_eq!(claude_price("Opus 5.5"), Some((4., 20.)));
        assert_eq!(claude_price("GPT-6"), None);
    }

    #[test]
    fn short_lists_have_no_search_box() {
        assert!(searchable(Menu::Model, 2));
        assert!(searchable(Menu::Agent, 1));
        assert!(!searchable(Menu::Access, 3));
        assert!(!searchable(Menu::Mode, 4));
        assert!(searchable(Menu::Mode, 12));
        assert_eq!(first_sentence("An agent. It codes."), "An agent");
    }
}
