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
        for e in reg.iter().filter(|e| crate::acp::known(&e.id).is_none() && !installed.iter().any(|i| *i == e.id) && e.method().is_some()) {
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
        // Room left for the list: the panel minus its title and the message box.
        let chrome = 34. + if search { 38. } else { 0. } + if m == Menu::Agent { 38. } else { 0. } + if m == Menu::Model && self.agent_effort().is_some() { 36. } else { 0. };
        let room = if top { vh - 150. } else { vh - 300. };
        let max_list = (room - chrome).clamp(120., if top { 460. } else { 380. });

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

    /// Effort, as a row of choices above the models: it can change without closing.
    fn render_effort_strip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.agent_effort()?;
        let mut choices = e.choices.clone();
        choices.sort_by_key(|c| if c.0 == "default" { 0 } else { 1 });
        Some(
            h_flex()
                .px_3()
                .pb_1p5()
                .gap_1()
                .child(div().flex_none().mr_1().text_xs().text_color(c(theme::FG_MUTED)).child(tr("Effort")))
                .child(
                    h_flex().flex_1().min_w_0().p_0p5().gap_0p5().rounded(px(5.)).border_1().border_color(c(theme::BORDER)).bg(c(theme::BG_EDITOR)).children(choices.into_iter().enumerate().map(|(i, (v, n, _))| {
                        let on = e.current == v;
                        let id = e.id.clone();
                        div()
                            .id(("ag-effort", i))
                            .flex_1()
                            .min_w_0()
                            .py_0p5()
                            .rounded(px(4.))
                            .text_center()
                            .truncate()
                            .text_xs()
                            .cursor_pointer()
                            .when(on, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                            .when(!on, |d| d.text_color(c(theme::FG_MUTED)).hover(|s| s.text_color(c(theme::FG))))
                            .child(n)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.agent_set_option(&id, v.clone(), true, cx)
                            }))
                    })),
                )
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
    fn short_lists_have_no_search_box() {
        assert!(searchable(Menu::Model, 2));
        assert!(searchable(Menu::Agent, 1));
        assert!(!searchable(Menu::Access, 3));
        assert!(!searchable(Menu::Mode, 4));
        assert!(searchable(Menu::Mode, 12));
        assert_eq!(first_sentence("An agent. It codes."), "An agent");
    }
}
