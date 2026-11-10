//! Usage: how much of the plan's limits and of the context window the agent has used,
//! in a panel of its own instead of typing `/usage`.
//!
//! Two sources, both from the agent itself:
//! - `usage_update` notifications (Claude Agent sends one per step): context tokens
//!   used and the window's size, the session's cost, and the 5-hour and 7-day limits
//!   with when each resets. These keep the meter under the message box live.
//! - The agent's own `/usage` and `/context` commands, run quietly when the panel
//!   opens (they cost no tokens): every limit (the Fable one too), the session's
//!   tokens, and what uses the limits most.
//!
//! Limits can't be reset from an app: each window resets by itself at the time shown.
//! What can be asked for is usage credits (`/usage-credits`), which the organization's
//! admin may have turned off; the panel runs it only after a confirmation and shows the
//! agent's answer.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Disableable, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::Value;
use std::time::Duration;

/// One limit window, as the live updates give it.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// `five_hour`, `seven_day`, ...
    pub key: String,
    /// 0.0 - 1.0
    pub used: f32,
    /// Unix seconds.
    pub resets: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    pub used: u64,
    pub size: u64,
    pub cost: Option<f64>,
    pub windows: Vec<Window>,
    pub model: String,
}

impl Live {
    /// Merge one `usage_update` (fields it leaves out keep their value).
    pub fn take(&mut self, u: &Value) {
        if let Some(n) = u.get("used").and_then(Value::as_u64) {
            self.used = n;
        }
        if let Some(n) = u.get("size").and_then(Value::as_u64) {
            self.size = n;
        }
        if let Some(a) = u.pointer("/cost/amount").and_then(Value::as_f64) {
            self.cost = Some(a);
        }
        if let Some(m) = u.pointer("/_meta/_claude~1model").and_then(Value::as_str) {
            self.model = m.to_string();
        }
        if let Some(w) = u.pointer("/_meta/_claude~1rateLimit/unifiedWindows").and_then(Value::as_object) {
            for (k, v) in w {
                let used = v.get("utilization").and_then(Value::as_f64).unwrap_or(0.) as f32;
                let resets = v.get("resetsAt").and_then(Value::as_i64).unwrap_or(0);
                match self.windows.iter_mut().find(|x| x.key == *k) {
                    Some(x) => {
                        x.used = used;
                        x.resets = resets;
                    }
                    None => self.windows.push(Window { key: k.clone(), used, resets }),
                }
            }
        }
    }
}

/// One limit from `/usage`: "5-hour limit", 16 (%), "Resets Nov 10, 10:40 PM GMT+7".
#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub name: String,
    pub pct: u32,
    pub resets: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// "Claude enterprise subscription usage".
    pub plan: String,
    pub limits: Vec<Limit>,
    /// "$0.00".
    pub cost: String,
    /// (Input, 0), (Output, 0), ...
    pub tokens: Vec<(String, String)>,
    /// What uses the limits most, last 24 hours: (share %, advice).
    pub tips: Vec<(u32, String)>,
}

fn strip_md(s: &str) -> String {
    s.replace("**", "").replace('`', "").replace("\\_", "_").trim().to_string()
}

/// Read the agent's `/usage` reply (Markdown).
pub fn parse_usage(md: &str) -> Report {
    let mut r = Report::default();
    let mut section = "";
    let mut in_day = false;
    let mut rows_seen = 0;
    for line in md.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("### ") {
            section = if h.contains("Limits") { "limits" } else if h.contains("session") { "session" } else if h.contains("using") { "using" } else { "" };
            continue;
        }
        if r.plan.is_empty() && section.is_empty() {
            if let Some(p) = t.strip_prefix("> ") {
                r.plan = strip_md(p);
                continue;
            }
        }
        match section {
            "limits" if t.starts_with("**") => {
                // **5-hour limit**, a dash, **16%** · Resets Nov 10, 10:40 PM GMT+7
                let parts: Vec<&str> = t.split("**").collect();
                let name = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
                let pct = parts.get(3).and_then(|p| p.trim().trim_end_matches('%').parse().ok()).unwrap_or(0);
                // After the share: " · Resets Nov 10, 10:40 PM GMT+7" (the name may hold a "·" too).
                let resets = parts.get(4..).map(|p| p.join("**")).unwrap_or_default().trim().trim_start_matches('·').trim().to_string();
                if !name.is_empty() {
                    r.limits.push(Limit { name, pct, resets });
                }
            }
            "session" if t.starts_with('|') => {
                let cells: Vec<String> = t.trim_matches('|').split('|').map(strip_md).collect();
                if cells.iter().all(|c| c.starts_with(':') || c.starts_with('-') || c.is_empty()) {
                    continue;
                }
                rows_seen += 1;
                if cells.first().is_some_and(|c| c.starts_with('$')) {
                    r.cost = cells[0].clone();
                } else if cells.len() == 2 && !["Breakdown", "Cost"].contains(&cells[0].as_str()) {
                    r.tokens.push((cells[0].clone(), cells[1].clone()));
                }
            }
            "using" => {
                if t.starts_with("**Last 24h**") {
                    in_day = true;
                } else if t.starts_with("**Last 7d**") {
                    in_day = false;
                } else if in_day {
                    if let Some(rest) = t.strip_prefix("- **") {
                        let (pct, text) = rest.split_once("**").unwrap_or(("", rest));
                        if let Ok(p) = pct.trim_end_matches('%').parse() {
                            r.tips.push((p, strip_md(text)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let _ = rows_seen;
    r
}

/// `/context`: "**Tokens:** 34.6k / 1m (3%)" → ("34.6k", "1m", 3).
pub fn parse_context(md: &str) -> Option<(String, String, u32)> {
    let line = md.lines().find(|l| l.contains("**Tokens:**"))?;
    let rest = strip_md(line.split("**Tokens:**").nth(1)?);
    let (used, more) = rest.split_once('/')?;
    let (total, pct) = more.trim().split_once('(').unwrap_or((more.trim(), ""));
    Some((used.trim().to_string(), total.trim().to_string(), pct.trim_end_matches([')', '%', ' ']).parse().unwrap_or(0)))
}

/// 59_125 → "59k", 1_000_000 → "1M".
pub fn tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", (n as f64 / 1000.).round() as u64),
        _ => {
            let m = n as f64 / 1_000_000.;
            if (m - m.round()).abs() < 0.05 { format!("{}M", m.round() as u64) } else { format!("{m:.1}M") }
        }
    }
}

/// Seconds until a Unix time, as "4 h 12 min" / "18 min" / "2 days 3 h".
pub fn until(resets: i64, now: i64) -> Option<String> {
    let s = resets - now;
    if s <= 0 {
        return None;
    }
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3600, (s % 3600) / 60);
    Some(if d > 0 {
        format!("{d} {} {h} h", if d == 1 { "day" } else { "days" })
    } else if h > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{} min", m.max(1))
    })
}

fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// What a quiet command is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    Usage,
    Context,
    Credits,
}

impl Probe {
    pub fn command(self) -> &'static str {
        match self {
            Probe::Usage => "/usage",
            Probe::Context => "/context",
            Probe::Credits => "/usage-credits",
        }
    }
}

#[derive(Default)]
pub struct UsageState {
    pub open: bool,
    /// Opened this many times (restarts the bars' fill animation).
    pub shown: u32,
    pub live: Option<Live>,
    pub report: Option<Report>,
    pub context: Option<(String, String, u32)>,
    /// The quiet command running now, and what it has said so far.
    pub probe: Option<(Probe, String)>,
    /// Quiet commands still to run.
    pub queue: Vec<Probe>,
    /// "Request usage credits" was clicked: asking before it runs.
    pub confirm_credits: bool,
    /// The agent's answer to `/usage-credits`.
    pub credits: Option<String>,
    pub error: Option<String>,
}

impl UsageState {
    /// Context used, 0.0 - 1.0, when known.
    pub fn context_share(&self) -> Option<f32> {
        let l = self.live.as_ref()?;
        (l.size > 0).then(|| (l.used as f32 / l.size as f32).clamp(0., 1.))
    }
}

/// The bar color for a share used: calm, then a warning, then the limit.
fn level(pct: f32) -> theme::Tok {
    if pct >= 90. {
        theme::DANGER
    } else if pct >= 75. {
        theme::WARN
    } else {
        theme::FG_MUTED
    }
}

/// A thin bar that fills to `pct` when the panel opens.
fn bar(key: SharedString, pct: f32) -> AnyElement {
    let frac = (pct / 100.).clamp(0., 1.);
    let col = level(pct);
    div()
        .relative()
        .flex_1()
        .min_w(theme::u(60.))
        .h(px(5.))
        .rounded_full()
        .bg(c(theme::BG_HOVER))
        .overflow_hidden()
        .child(
            div().absolute().left_0().top_0().bottom_0().rounded_full().bg(c(col)).with_animation(
                key,
                Animation::new(Duration::from_millis(520)).with_easing(gpui_kit::ease_out_quint()),
                move |el, t| el.w(relative(frac * t)),
            ),
        )
        .into_any_element()
}

impl Workbench {
    /// The agent can tell its usage (Claude Agent: a `/usage` command or live updates).
    pub(crate) fn agent_has_usage(&self) -> bool {
        self.agent.usage.live.is_some() || self.agent.commands.iter().any(|c| c.name == "usage")
    }

    pub(crate) fn agent_usage_open(&mut self, cx: &mut Context<Self>) {
        let u = &mut self.agent.usage;
        u.open = true;
        u.shown += 1;
        u.confirm_credits = false;
        u.error = None;
        if self.agent.commands.iter().any(|c| c.name == "usage") {
            self.agent_probe_queue(&[Probe::Usage, Probe::Context], cx);
        }
        cx.notify();
    }

    /// Queue quiet commands; they run when the agent is free.
    pub(crate) fn agent_probe_queue(&mut self, list: &[Probe], cx: &mut Context<Self>) {
        for p in list {
            if !self.agent.usage.queue.contains(p) && self.agent.usage.probe.as_ref().map(|x| x.0) != Some(*p) {
                self.agent.usage.queue.push(*p);
            }
        }
        self.agent_probe_next(cx);
    }

    /// Run the next quiet command if the agent has a session and isn't answering.
    pub(crate) fn agent_probe_next(&mut self, cx: &mut Context<Self>) {
        if self.agent.usage.probe.is_some() || self.agent.usage.queue.is_empty() {
            return;
        }
        if self.agent.status != super::Status::Ready || self.agent.session.is_none() {
            return;
        }
        let p = self.agent.usage.queue.remove(0);
        self.agent.usage.probe = Some((p, String::new()));
        self.agent_send_probe(p.command());
        cx.notify();
    }

    /// A quiet command finished: read what it said.
    pub(crate) fn agent_probe_done(&mut self, failed: Option<String>, cx: &mut Context<Self>) {
        let Some((p, text)) = self.agent.usage.probe.take() else { return };
        let u = &mut self.agent.usage;
        match (failed, p) {
            (Some(e), _) => u.error = Some(e),
            (None, Probe::Usage) => {
                let r = parse_usage(&text);
                if r.limits.is_empty() && r.cost.is_empty() {
                    u.error = Some(text.trim().chars().take(300).collect());
                } else {
                    u.report = Some(r);
                }
            }
            (None, Probe::Context) => {
                if let Some(ctx) = parse_context(&text) {
                    u.context = Some(ctx);
                }
            }
            (None, Probe::Credits) => u.credits = Some(text.trim().to_string()),
        }
        cx.notify();
    }

    /// The meter under the message box: how full the context is. Opens the usage panel.
    pub(crate) fn render_context_meter(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let u = &self.agent.usage;
        let l = u.live.as_ref()?;
        let share = u.context_share()?;
        let pct = (share * 100.).round();
        let tip: SharedString = crate::trf!("Context: {used} of {size} tokens. Click for usage and limits."; used => tokens(l.used), size => tokens(l.size)).into();
        // A small ring would be prettier but GPUI draws no arcs: a short bar reads the same.
        Some(
            h_flex()
                .id("ag-ctx-meter")
                .flex_none()
                .h(theme::u(22.))
                .px_1p5()
                .gap_1()
                .rounded(px(4.))
                .cursor_pointer()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG)))
                .child(
                    div()
                        .w(theme::u(22.))
                        .h(px(4.))
                        .rounded_full()
                        .bg(c(theme::BG_HOVER))
                        .overflow_hidden()
                        .child(div().h_full().rounded_full().bg(c(level(pct))).w(relative(share.max(0.04)))),
                )
                .child(format!("{pct}%"))
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.agent_usage_open(cx)
                }))
                .into_any_element(),
        )
    }

    /// The usage panel over the chat.
    pub(crate) fn render_usage_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let u = &self.agent.usage;
        if !u.open {
            return None;
        }
        let now = unix_now();
        let shown = u.shown;
        let busy = u.probe.is_some() || !u.queue.is_empty();
        let title_row = h_flex()
            .gap_2()
            .child(Icon::new(IconName::Gauge).size(theme::u(16.)).text_color(c(theme::FG_MUTED)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Usage")))
                    .when_some(u.report.as_ref().map(|r| r.plan.clone()).filter(|p| !p.is_empty()), |v, p| v.child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(p))),
            )
            .child(
                Button::new("ag-usage-refresh")
                    .icon(Icon::new(if busy { IconName::LoaderCircle } else { IconName::RefreshCcw }))
                    .ghost()
                    .xsmall()
                    .tooltip(tr("Read it again"))
                    .on_click(cx.listener(|this, _, _, cx| this.agent_probe_queue(&[Probe::Usage, Probe::Context], cx))),
            )
            .child(Button::new("ag-usage-close").icon(Icon::new(IconName::Close)).ghost().xsmall().tooltip(tr("Close (Esc)")).on_click(cx.listener(|this, _, _, cx| {
                this.agent.usage.open = false;
                cx.notify();
            })));

        // One limit: name and share on a line, the bar, then when it resets.
        let row = |i: usize, name: String, pct: f32, resets: Option<String>| {
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(name))
                        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(if pct >= 75. { level(pct) } else { theme::FG_STRONG })).child(format!("{}%", pct.round()))),
                )
                .child(h_flex().child(bar(SharedString::from(format!("ag-usage-bar-{shown}-{i}")), pct)))
                .when_some(resets, |v, r| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(r)))
        };
        let mut limits = v_flex().gap_3();
        let mut any = false;
        let window_of = |name: &str| -> Option<&Window> {
            let l = u.live.as_ref()?;
            let key = if name.starts_with("5-hour") { "five_hour" } else if name.starts_with("Weekly · all") || name == "Weekly" { "seven_day" } else { return None };
            l.windows.iter().find(|w| w.key == key)
        };
        if let Some(r) = &u.report {
            for (i, l) in r.limits.iter().enumerate() {
                any = true;
                // The live window gives the exact reset time; the report says it in words.
                let when = window_of(&l.name).and_then(|w| until(w.resets, now)).map(|t| crate::trf!("Resets in {t}"; t => t));
                let text = match (when, l.resets.is_empty()) {
                    (Some(w), false) => Some(format!("{w} · {}", l.resets.trim_start_matches("Resets ").trim())),
                    (Some(w), true) => Some(w),
                    (None, false) => Some(l.resets.clone()),
                    (None, true) => None,
                };
                limits = limits.child(row(i, l.name.clone(), l.pct as f32, text));
            }
        } else if let Some(l) = &u.live {
            for (i, w) in l.windows.iter().enumerate() {
                any = true;
                let name = match w.key.as_str() {
                    "five_hour" => tr("5-hour limit").to_string(),
                    "seven_day" => tr("Weekly limit").to_string(),
                    k => k.replace('_', " "),
                };
                limits = limits.child(row(i, name, w.used * 100., until(w.resets, now).map(|t| crate::trf!("Resets in {t}"; t => t))));
            }
        }
        if !any {
            limits = limits.child(div().text_xs().text_color(c(theme::FG_DIM)).child(if busy {
                tr("Reading your limits from the agent…")
            } else if self.agent.session.is_none() {
                tr("Turn the agent on to read your limits: it knows them, FluxCode doesn't.")
            } else {
                tr("The agent didn't report any limits.")
            }));
        }

        // The context window.
        let context = match (&u.context, &u.live) {
            (Some((used, total, pct)), _) => Some((format!("{used} / {total}"), *pct as f32)),
            (None, Some(l)) if l.size > 0 => Some((format!("{} / {}", tokens(l.used), tokens(l.size)), l.used as f32 * 100. / l.size as f32)),
            _ => None,
        };
        let session = u.report.as_ref().map(|r| {
            let mut s = if r.cost.is_empty() { String::new() } else { crate::trf!("Cost so far {cost}"; cost => r.cost) };
            let toks: Vec<String> = r.tokens.iter().filter(|(_, v)| v != "0").map(|(k, v)| format!("{} {}", v, k.to_lowercase())).collect();
            if !toks.is_empty() {
                s = format!("{s} · {}", toks.join(", "));
            }
            s
        });
        let tips: Vec<(u32, String)> = u.report.as_ref().map(|r| r.tips.iter().take(3).cloned().collect()).unwrap_or_default();

        let section = |t: &'static str| div().pt_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr(t));
        let mut body = v_flex()
            .id("ag-usage-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .pb_4()
            .gap_3()
            .child(section("Limits"))
            .child(limits);
        if let Some((label, pct)) = context {
            body = body.child(div().h(px(1.)).bg(c(theme::BORDER))).child(
                v_flex()
                    .gap_1()
                    .child(h_flex().gap_2().text_xs().child(div().flex_1().text_color(c(theme::FG)).child(tr("Context"))).child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(label)))
                    .child(h_flex().child(bar(SharedString::from(format!("ag-usage-ctx-{shown}")), pct)))
                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("What this chat holds now. A new chat (/new) starts it empty."))),
            );
        }
        if let Some(s) = session.filter(|s| !s.is_empty()) {
            body = body.child(div().text_xs().text_color(c(theme::FG_DIM)).child(s));
        }
        if !tips.is_empty() {
            body = body.child(div().h(px(1.)).bg(c(theme::BORDER))).child(section("What uses your limits, last 24 hours")).children(tips.into_iter().enumerate().map(|(i, (p, t))| {
                h_flex()
                    .id(("ag-usage-tip", i))
                    .gap_2()
                    .items_start()
                    .text_xs()
                    .child(div().flex_none().w(theme::u(34.)).text_color(c(theme::FG_STRONG)).child(format!("{p}%")))
                    .child(div().flex_1().min_w_0().text_color(c(theme::FG_MUTED)).child(t))
            }));
        }
        if let Some(e) = &u.error {
            body = body.child(div().text_xs().text_color(c(theme::DANGER)).child(e.clone()));
        }

        // Usage credits: the one way to get more before a limit resets.
        let can_credits = self.agent.commands.iter().any(|c| c.name == "usage-credits");
        let footer = if !can_credits {
            None
        } else if u.confirm_credits {
            Some(
                v_flex()
                    .gap_2()
                    .p_3()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG))
                    .bg(c(theme::BG_EDITOR))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Request usage credits?")))
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(tr("The agent asks your plan for extra usage, so you can keep working after a limit. Your admin may approve it, or it may be turned off for your organization.")))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(Button::new("ag-credits-no").label(tr("Cancel")).small().ghost().on_click(cx.listener(|this, _, _, cx| {
                                this.agent.usage.confirm_credits = false;
                                cx.notify();
                            })))
                            .child(Button::new("ag-credits-yes").label(tr("Request")).small().primary().on_click(cx.listener(|this, _, _, cx| {
                                this.agent.usage.confirm_credits = false;
                                this.agent.usage.credits = None;
                                this.agent_probe_queue(&[Probe::Credits], cx);
                            }))),
                    )
                    .into_any_element(),
            )
        } else {
            let waiting = u.probe.as_ref().map(|p| p.0) == Some(Probe::Credits) || u.queue.contains(&Probe::Credits);
            Some(
                v_flex()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(div().flex_1().min_w(theme::u(150.)).text_xs().text_color(c(theme::FG_DIM)).child(tr("Limits reset by themselves at the times above.")))
                            .child(
                                Button::new("ag-credits")
                                    .label(if waiting { tr("Requesting…") } else { tr("Request usage credits") })
                                    .small()
                                    .disabled(waiting || self.agent.session.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.agent.usage.confirm_credits = true;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when_some(u.credits.clone(), |v, t| {
                        v.child(
                            h_flex()
                                .gap_1p5()
                                .items_start()
                                .text_xs()
                                .text_color(c(theme::FG_MUTED))
                                .child(div().flex_none().pt(px(2.)).child(Icon::new(IconName::Info).size(theme::u(12.))))
                                .child(div().flex_1().min_w_0().child(t)),
                        )
                    })
                    .into_any_element(),
            )
        };

        let card = v_flex()
            .id("ag-usage")
            .key_context("AgentUsage")
            .occlude()
            .w_full()
            .max_w(theme::u(440.))
            .max_h(relative(0.92))
            .rounded(px(10.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(c(theme::BG_ELEVATED))
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if !this.agent.usage.confirm_credits {
                    this.agent.usage.open = false;
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    let u = &mut this.agent.usage;
                    if u.confirm_credits {
                        u.confirm_credits = false;
                    } else {
                        u.open = false;
                    }
                    cx.notify();
                }
            }))
            .child(div().px_4().pt_3().pb_2().child(title_row))
            .child(body)
            .when_some(footer, |v, f| v.child(div().px_4().pb_3().pt_1().border_t_1().border_color(c(theme::BORDER)).child(f)));
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(0.28))
                .flex()
                .justify_center()
                .items_start()
                .px_3()
                .pt(theme::u(44.))
                .pb_3()
                .child(card.with_animation(SharedString::from(format!("ag-usage-in-{shown}")), Animation::new(Duration::from_millis(180)).with_easing(gpui_kit::ease_out_quint()), |el, t| {
                    el.opacity(t).mt(px((1. - t) * 8.))
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_context, parse_usage, tokens, until, Limit, Live, Window};

    const USAGE: &str = "## Usage\n\n> Claude enterprise subscription usage\n\n### Limits\n\n**5-hour limit** \u{2014} **16%** · Resets Nov 10, 10:40 PM GMT+7\n\n`███░░`\n\n**Weekly · all models** \u{2014} **31%** · Resets Nov 16, 12:00 PM GMT+7\n\n**Weekly · Fable** \u{2014} **0%** · Resets Nov 16, 12:00 PM GMT+7\n\n---\n\n### This session\n\n| Cost | API time | Active |\n|:--|:--|:--|\n| $0.42 | 0s | 13s |\n\n| Breakdown | Tokens |\n|:--|--:|\n| Input | 120 |\n| Output | 0 |\n\n---\n\n### What\u{2019}s using your limits?\n\n**Last 24h** · 1678 requests · 10 sessions\n\n- **94%** Start fresh when you switch tasks · run /clear\n- **78%** Give simple subagents a cheaper model\n\n**Last 7d** · 14379 requests\n\n- **92%** Other\n";

    #[test]
    fn reads_the_usage_reply() {
        let r = parse_usage(USAGE);
        assert_eq!(r.plan, "Claude enterprise subscription usage");
        assert_eq!(r.limits.len(), 3, "{r:?}");
        assert_eq!(r.limits[0], Limit { name: "5-hour limit".into(), pct: 16, resets: "Resets Nov 10, 10:40 PM GMT+7".into() });
        assert_eq!(r.limits[2].name, "Weekly · Fable");
        assert_eq!(r.limits[1].resets, "Resets Nov 16, 12:00 PM GMT+7");
        assert_eq!(r.cost, "$0.42");
        assert_eq!(r.tokens[0], ("Input".into(), "120".into()));
        assert_eq!(r.tips.len(), 2);
        assert_eq!(r.tips[0].0, 94);
    }

    #[test]
    fn reads_context_and_live_updates() {
        assert_eq!(parse_context("**Model:** x  \n**Tokens:** 34.6k / 1m (3%)\n"), Some(("34.6k".into(), "1m".into(), 3)));
        let mut l = Live::default();
        l.take(&serde_json::json!({"used": 59125, "size": 1000000, "_meta": {"_claude/rateLimit": {"unifiedWindows": {"five_hour": {"utilization": 0.18, "resetsAt": 100}}}, "_claude/model": "claude-haiku-5-5"}}));
        l.take(&serde_json::json!({"used": 60000, "cost": {"amount": 0.02}}));
        assert_eq!((l.used, l.size, l.cost), (60000, 1000000, Some(0.02)));
        assert_eq!(l.windows, vec![Window { key: "five_hour".into(), used: 0.18, resets: 100 }]);
        assert_eq!(l.model, "claude-haiku-5-5");
        assert_eq!(tokens(59125), "59k");
        assert_eq!(tokens(1_000_000), "1M");
        assert_eq!(until(100 + 4 * 3600 + 12 * 60, 100).as_deref(), Some("4 h 12 min"));
        assert_eq!(until(100 + 2 * 86_400 + 3 * 3600, 100).as_deref(), Some("2 days 3 h"));
        assert_eq!(until(50, 100), None);
    }
}
