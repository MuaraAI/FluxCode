use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Disableable, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub key: String,
    pub used: f32,
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
        if let Some(w) = u
            .pointer("/_meta/_claude~1rateLimit/unifiedWindows")
            .and_then(Value::as_object)
        {
            for (k, v) in w {
                let used = v.get("utilization").and_then(Value::as_f64).unwrap_or(0.) as f32;
                let resets = v.get("resetsAt").and_then(Value::as_i64).unwrap_or(0);
                match self.windows.iter_mut().find(|x| x.key == *k) {
                    Some(x) => {
                        x.used = used;
                        x.resets = resets;
                    }
                    None => self.windows.push(Window {
                        key: k.clone(),
                        used,
                        resets,
                    }),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub name: String,
    pub pct: u32,
    pub resets: String,
    pub detail: String,
    pub remaining: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LimitRow {
    pub name: String,
    pub pct: f32,
    pub resets: Option<String>,
    pub detail: Option<String>,
    pub remaining: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub plan: String,
    pub limits: Vec<Limit>,
    pub cost: String,
    pub tokens: Vec<(String, String)>,
    pub tips: Vec<(u32, String)>,
}

fn strip_md(s: &str) -> String {
    s.replace("**", "")
        .replace('`', "")
        .replace("\\_", "_")
        .trim()
        .to_string()
}

fn parse_session_bullet(t: &str, tokens: &mut Vec<(String, String)>) {
    let raw = t.trim_start_matches(['-', '*']).trim();
    let (main_part, detail_part) = match raw.split_once('(') {
        Some((a, b)) => (a.trim(), Some(b.trim_end_matches(')').trim())),
        None => (raw, None),
    };
    let clean_main = strip_md(main_part);
    if let Some((k, v)) = clean_main.split_once(':') {
        let k = k.trim();
        let v = v.trim();
        if !k.is_empty() && !v.is_empty() {
            tokens.push((k.to_string(), v.to_string()));
        }
    }
    if let Some(sub) = detail_part {
        let mut parsed: Vec<(String, String)> = Vec::new();
        for piece in sub.split(',') {
            let cleaned = strip_md(piece);
            if let Some((k, v)) = cleaned.split_once(':') {
                let k = k.trim();
                let v = v.trim();
                if !k.is_empty() && !v.is_empty() {
                    let mut chars = k.chars();
                    let titled = match chars.next() {
                        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                        None => continue,
                    };
                    parsed.push((titled, v.to_string()));
                }
            } else if !cleaned.is_empty() && cleaned.bytes().all(|b| b.is_ascii_digit()) {
                if let Some((_, last_v)) = parsed.last_mut() {
                    last_v.push(',');
                    last_v.push_str(&cleaned);
                }
            }
        }
        tokens.extend(parsed);
    }
}

fn parse_limit_bullet(
    item: &str,
    cur_group: &mut String,
    cur_models: &mut String,
) -> Option<Limit> {
    let pct_pos = item.find('%')?;
    let before = &item[..pct_pos];
    let digit_start = before
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);
    let pct: u32 = before[digit_start..].parse().ok()?;
    let label_raw = match before[..digit_start].split_once(':') {
        Some((k, _)) => k,
        None => before[..digit_start].trim(),
    };
    let mut label = strip_md(label_raw).trim_end_matches(':').trim().to_string();
    let low_label = label.to_ascii_lowercase();
    if low_label.ends_with(" remaining") && label.len() > " remaining".len() {
        label = label[..label.len() - " remaining".len()].trim().to_string();
    }
    let item_low = item.to_ascii_lowercase();
    let remaining = item_low.contains("remaining") || item_low.contains("left");
    let after = item[pct_pos + 1..].trim();
    let resets = after
        .split_once('·')
        .map(|(_, r)| r.trim())
        .or_else(|| {
            let low = after.to_ascii_lowercase();
            low.find("reset")
                .map(|ix| after[ix..].trim_end_matches(')').trim())
        })
        .unwrap_or("")
        .to_string();
    let name = if cur_group.is_empty() {
        label
    } else if label.is_empty() {
        cur_group.clone()
    } else {
        format!("{cur_group} · {label}")
    };
    (!name.is_empty()).then(|| Limit {
        name,
        pct,
        resets,
        detail: cur_models.clone(),
        remaining,
    })
}

pub fn parse_usage(md: &str) -> Report {
    let mut r = Report::default();
    let mut section = "";
    let mut in_day = false;
    let mut cur_group = String::new();
    let mut cur_models = String::new();
    for line in md.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("### ") {
            let hl = h.to_ascii_lowercase();
            section = if hl.contains("using") {
                "using"
            } else if hl.contains("session") {
                "session"
            } else if hl.contains("limit") {
                "limits"
            } else {
                ""
            };
            cur_group.clear();
            cur_models.clear();
            continue;
        }
        if r.plan.is_empty() && section.is_empty() {
            if let Some(p) = t.strip_prefix("> ") {
                r.plan = strip_md(p);
                continue;
            }
        }
        match section {
            "limits" => {
                if t.starts_with("**") {
                    let parts: Vec<&str> = t.split("**").collect();
                    let name = parts
                        .get(1)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    let pct = parts
                        .get(3)
                        .and_then(|p| p.trim().trim_end_matches('%').parse().ok())
                        .unwrap_or(0);
                    let resets = parts
                        .get(4..)
                        .map(|p| p.join("**"))
                        .unwrap_or_default()
                        .trim()
                        .trim_start_matches('·')
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        r.limits.push(Limit {
                            name,
                            pct,
                            resets,
                            detail: String::new(),
                            remaining: false,
                        });
                    }
                } else if t.starts_with("- ") || t.starts_with("* ") {
                    let item = t[2..].trim();
                    if item.contains('%') {
                        if let Some(lim) = parse_limit_bullet(item, &mut cur_group, &mut cur_models)
                        {
                            r.limits.push(lim);
                        }
                    } else {
                        let g = strip_md(item).trim_end_matches(':').trim().to_string();
                        if !g.is_empty() {
                            cur_group = g;
                            cur_models.clear();
                        }
                    }
                } else {
                    let clean_note = t.trim_matches(['*', ' ']);
                    if clean_note.starts_with('(')
                        && clean_note.ends_with(')')
                        && clean_note.to_ascii_lowercase().contains("model")
                    {
                        let inner = clean_note
                            .trim_start_matches('(')
                            .trim_end_matches(')')
                            .trim();
                        let inner_low = inner.to_ascii_lowercase();
                        let stripped = if let Some(ix) = inner_low.find("group:") {
                            inner[ix + "group:".len()..].trim()
                        } else if let Some(ix) = inner_low.find("models:") {
                            inner[ix + "models:".len()..].trim()
                        } else {
                            inner
                        };
                        if !stripped.is_empty() {
                            cur_models = stripped.to_string();
                        }
                    }
                }
            }
            "session" => {
                if t.starts_with('|') {
                    let cells: Vec<String> = t.trim_matches('|').split('|').map(strip_md).collect();
                    if cells
                        .iter()
                        .all(|c| c.starts_with(':') || c.starts_with('-') || c.is_empty())
                    {
                        continue;
                    }
                    if cells.first().is_some_and(|c| c.starts_with('$')) {
                        r.cost = cells[0].clone();
                    } else if cells.len() == 2
                        && !["Breakdown", "Cost"].contains(&cells[0].as_str())
                    {
                        r.tokens.push((cells[0].clone(), cells[1].clone()));
                    }
                } else if t.starts_with("- ") || t.starts_with("* ") {
                    parse_session_bullet(t, &mut r.tokens);
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
    r
}

pub fn parse_context(md: &str) -> Option<(String, String, u32)> {
    let line = md.lines().find(|l| l.contains("**Tokens:**"))?;
    let rest = strip_md(line.split("**Tokens:**").nth(1)?);
    let (used, more) = rest.split_once('/')?;
    let (total, pct) = more.trim().split_once('(').unwrap_or((more.trim(), ""));
    Some((
        used.trim().to_string(),
        total.trim().to_string(),
        pct.trim_end_matches([')', '%', ' ']).parse().unwrap_or(0),
    ))
}

pub fn tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", (n as f64 / 1000.).round() as u64),
        _ => {
            let m = n as f64 / 1_000_000.;
            if (m - m.round()).abs() < 0.05 {
                format!("{}M", m.round() as u64)
            } else {
                format!("{m:.1}M")
            }
        }
    }
}

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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

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
    pub shown: u32,
    pub live: Option<Live>,
    pub report: Option<Report>,
    pub context: Option<(String, String, u32)>,
    pub probe: Option<(Probe, String)>,
    pub queue: Vec<Probe>,
    pub confirm_credits: bool,
    pub credits: Option<String>,
    pub error: Option<String>,
}

impl UsageState {
    pub fn limit_rows(&self, now: i64) -> Vec<LimitRow> {
        let window_of = |name: &str| -> Option<&Window> {
            let l = self.live.as_ref()?;
            let key = if name.starts_with("5-hour") {
                "five_hour"
            } else if name.starts_with("Weekly · all") || name == "Weekly" {
                "seven_day"
            } else {
                return None;
            };
            l.windows.iter().find(|w| w.key == key)
        };
        if let Some(r) = self.report.as_ref().filter(|r| !r.limits.is_empty()) {
            return r
                .limits
                .iter()
                .map(|l| {
                    let when = window_of(&l.name)
                        .and_then(|w| until(w.resets, now))
                        .map(|t| crate::trf!("Resets in {t}"; t => t));
                    let text = match (when, l.resets.is_empty()) {
                        (Some(w), false) => Some(format!(
                            "{w} · {}",
                            l.resets.trim_start_matches("Resets ").trim()
                        )),
                        (Some(w), true) => Some(w),
                        (None, false) => Some(l.resets.clone()),
                        (None, true) => None,
                    };
                    LimitRow {
                        name: l.name.clone(),
                        pct: l.pct as f32,
                        resets: text,
                        detail: (!l.detail.is_empty()).then(|| l.detail.clone()),
                        remaining: l.remaining,
                    }
                })
                .collect();
        }
        let Some(l) = &self.live else {
            return Vec::new();
        };
        l.windows
            .iter()
            .map(|w| {
                let name = match w.key.as_str() {
                    "five_hour" => tr("5-hour limit").to_string(),
                    "seven_day" => tr("Weekly limit").to_string(),
                    k => k.replace('_', " "),
                };
                LimitRow {
                    name,
                    pct: w.used * 100.,
                    resets: until(w.resets, now).map(|t| crate::trf!("Resets in {t}"; t => t)),
                    detail: None,
                    remaining: false,
                }
            })
            .collect()
    }

    pub fn context_share(&self) -> Option<f32> {
        let l = self.live.as_ref()?;
        (l.size > 0).then(|| (l.used as f32 / l.size as f32).clamp(0., 1.))
    }
}

fn level(pct: f32) -> theme::Tok {
    if pct >= 90. {
        theme::DANGER
    } else if pct >= 75. {
        theme::WARN
    } else {
        theme::ACCENT
    }
}

fn level_remaining(pct: f32) -> theme::Tok {
    if pct <= 10. {
        theme::DANGER
    } else if pct <= 25. {
        theme::WARN
    } else {
        theme::GREEN
    }
}

fn ring(share: f32, track: Hsla, fill: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let side = f32::from(b.size.width.min(b.size.height));
            let stroke = 2.;
            let r = (side - stroke) / 2.;
            let (x0, y0) = (
                f32::from(b.origin.x) + side / 2.,
                f32::from(b.origin.y) + side / 2.,
            );
            let at = |t: f32| {
                let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                point(px(x0 + r * a.cos()), px(y0 + r * a.sin()))
            };
            let radii = point(px(r), px(r));
            let arc = |t: f32| {
                let mut p = PathBuilder::stroke(px(stroke));
                p.move_to(at(0.));
                if t > 0.5 {
                    p.arc_to(radii, px(0.), false, true, at(0.5));
                }
                p.arc_to(radii, px(0.), false, true, at(t));
                p.build().ok()
            };
            if let Some(p) = arc(0.9999) {
                window.paint_path(p, track);
            }
            if let Some(p) = arc(share.clamp(0.03, 0.9999)) {
                window.paint_path(p, fill);
            }
        },
    )
    .flex_none()
    .size(theme::u(14.))
}

fn bar(key: SharedString, pct: f32, col: theme::Tok) -> AnyElement {
    let frac = (pct / 100.).clamp(0., 1.);
    div()
        .relative()
        .w_full()
        .min_w(theme::u(60.))
        .h(px(6.))
        .rounded_full()
        .bg(c(theme::BORDER_STRONG).opacity(0.45))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .when(frac > 0., |d| d.min_w(px(6.)))
                .rounded_full()
                .bg(c(col))
                .with_animation(
                    key,
                    Animation::new(Duration::from_millis(520))
                        .with_easing(gpui_kit::ease_out_quint()),
                    move |el, t| el.w(relative(frac * t)),
                ),
        )
        .into_any_element()
}

impl Workbench {
    pub(crate) fn agent_usage_probes(&self) -> Vec<Probe> {
        let mut out = Vec::new();
        if self.agent.commands.iter().any(|c| c.name == "usage") {
            out.push(Probe::Usage);
        }
        if self.agent.commands.iter().any(|c| c.name == "context") {
            out.push(Probe::Context);
        }
        out
    }
    pub(crate) fn agent_model_free(&self) -> Option<String> {
        let view = self.agent_config_view();
        let m = view.iter().find(|o| o.category == "model")?;
        let name = m
            .current_name()
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string();
        let free = m.current.ends_with("-free")
            || name.to_lowercase().ends_with(" free")
            || super::catalog::get(&m.current).is_some_and(|x| x.free);
        free.then(|| {
            name.strip_suffix(" Free")
                .or_else(|| name.strip_suffix(" free"))
                .unwrap_or(&name)
                .to_string()
        })
    }

    pub(crate) fn agent_has_usage(&self) -> bool {
        self.agent.usage.live.is_some() || self.agent.commands.iter().any(|c| c.name == "usage")
    }

    pub(crate) fn agent_usage_toggle(&mut self, cx: &mut Context<Self>) {
        let may = self.pop_may_open("usage");
        if self.agent.usage.open {
            self.agent.usage.open = false;
            cx.notify();
        } else if may {
            self.agent_usage_open(cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn agent_usage_open(&mut self, cx: &mut Context<Self>) {
        let probes = self.agent_usage_probes();
        let u = &mut self.agent.usage;
        u.open = true;
        u.shown += 1;
        u.confirm_credits = false;
        u.error = None;
        if !probes.is_empty() {
            self.agent_probe_queue(&probes, cx);
        }
        cx.notify();
    }

    pub(crate) fn agent_probe_queue(&mut self, list: &[Probe], cx: &mut Context<Self>) {
        for p in list {
            if !self.agent.usage.queue.contains(p)
                && self.agent.usage.probe.as_ref().map(|x| x.0) != Some(*p)
            {
                self.agent.usage.queue.push(*p);
            }
        }
        self.agent_probe_next(cx);
    }

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

    pub(crate) fn agent_probe_done(&mut self, failed: Option<String>, cx: &mut Context<Self>) {
        let Some((p, text)) = self.agent.usage.probe.take() else {
            return;
        };
        let u = &mut self.agent.usage;
        match (failed, p) {
            (Some(e), _) => u.error = Some(e),
            (None, Probe::Usage) => {
                let r = parse_usage(&text);
                if r.limits.is_empty() && r.cost.is_empty() && r.tokens.is_empty() {
                    u.error = Some(text.trim().chars().take(300).collect());
                } else {
                    u.error = None;
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

    pub(crate) fn render_context_meter(
        &self,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let u = &self.agent.usage;
        let l = u.live.as_ref()?;
        let share = u.context_share()?;
        let pct = (share * 100.).round();
        let tip: SharedString = crate::trf!("Context: {used} of {size} tokens ({pct}%). Click for usage."; used => tokens(l.used), size => tokens(l.size), pct => pct).into();
        Some(
            h_flex()
                .id("ag-ctx-meter")
                .flex_none()
                .h(theme::u(24.))
                .px_1()
                .gap_1()
                .rounded(px(5.))
                .cursor_pointer()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG)))
                .when(!narrow, |h| {
                    h.child(ring(share, c(theme::BORDER_STRONG), c(level(pct))))
                })
                .child(div().whitespace_nowrap().child(format!("{pct}%")))
                .tooltip(move |w, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.agent_usage_toggle(cx)
                }))
                .into_any_element(),
        )
    }

    pub(crate) fn render_limits_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let u = &self.agent.usage;
        let now = unix_now();
        let rows = u.limit_rows(now);
        let busy = u.probe.is_some() || !u.queue.is_empty();
        let can_read = self.agent.commands.iter().any(|c| c.name == "usage");
        let mut body = v_flex().gap_2();
        for r in &rows {
            let col = if r.remaining {
                level_remaining(r.pct)
            } else {
                level(r.pct)
            };
            let pct_text = if r.remaining {
                format!("{}% {}", r.pct.round(), tr("remaining"))
            } else {
                format!("{}%", r.pct.round())
            };
            body = body.child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(c(theme::FG))
                                    .child(r.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(col))
                                    .child(pct_text),
                            ),
                    )
                    .child(h_flex().child(bar(
                        SharedString::from(format!("agf-lim-{}", r.name)),
                        r.pct,
                        col,
                    )))
                    .when_some(r.detail.clone(), |v, d| {
                        v.child(
                            div()
                                .text_size(theme::u(10.5))
                                .text_color(c(theme::FG_DIM))
                                .child(d),
                        )
                    })
                    .when_some(r.resets.clone(), |v, res| {
                        v.child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(res),
                        )
                    }),
            );
        }
        if rows.is_empty() {
            let note = if self.agent_model_free().is_some() {
                tr("Free model: no plan limits to count.")
            } else if busy {
                tr("Reading your limits from the agent…")
            } else if self.agent.session.is_none() {
                tr("Turn the agent on to read your limits.")
            } else {
                tr("Limits show here once the agent reports them.")
            };
            body = body.child(div().text_xs().text_color(c(theme::FG_DIM)).child(note));
        }
        let context = match (&u.context, &u.live) {
            (Some((used, total, pct)), _) => Some((format!("{used} / {total}"), *pct as f32)),
            (None, Some(l)) if l.size > 0 => Some((
                format!("{} / {}", tokens(l.used), tokens(l.size)),
                l.used as f32 * 100. / l.size as f32,
            )),
            _ => None,
        };
        if let Some((text, pct)) = context {
            body = body.child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(c(theme::FG))
                                    .child(tr("Context")),
                            )
                            .child(div().flex_none().text_color(c(theme::FG_MUTED)).child(text)),
                    )
                    .child(h_flex().child(bar("agf-lim-ctx".into(), pct, level(pct)))),
            );
        }
        v_flex()
            .id("agf-limits")
            .flex_none()
            .max_h(theme::u(240.))
            .overflow_y_scroll()
            .px_3()
            .py_2p5()
            .gap_2()
            .border_t_1()
            .border_color(theme::glass_edge())
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Gauge)
                            .size(theme::u(13.))
                            .text_color(c(theme::FG_MUTED)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(theme::FG_MUTED))
                            .child(tr("Limits")),
                    )
                    .when(can_read, |h| {
                        h.child(
                            Button::new("agf-lim-read")
                                .icon(Icon::new(if busy {
                                    IconName::LoaderCircle
                                } else {
                                    IconName::RefreshCcw
                                }))
                                .ghost()
                                .xsmall()
                                .tooltip(tr("Read it again"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let probes = this.agent_usage_probes();
                                    this.agent_probe_queue(&probes, cx);
                                })),
                        )
                    })
                    .when(self.agent_has_usage(), |h| {
                        h.child(
                            Button::new("agf-lim-more")
                                .icon(Icon::new(IconName::Maximize2))
                                .ghost()
                                .xsmall()
                                .tooltip(tr("Usage: limits, context and cost"))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.agent_usage_toggle(cx)),
                                ),
                        )
                    }),
            )
            .child(body)
            .into_any_element()
    }

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
            .child(
                Icon::new(IconName::Gauge)
                    .size(theme::u(16.))
                    .text_color(c(theme::FG_MUTED)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(theme::FG_STRONG))
                            .child(tr("Usage")),
                    )
                    .when_some(
                        u.report
                            .as_ref()
                            .map(|r| r.plan.clone())
                            .filter(|p| !p.is_empty()),
                        |v, p| {
                            v.child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(p),
                            )
                        },
                    ),
            )
            .child(
                Button::new("ag-usage-refresh")
                    .icon(Icon::new(if busy {
                        IconName::LoaderCircle
                    } else {
                        IconName::RefreshCcw
                    }))
                    .ghost()
                    .xsmall()
                    .tooltip(tr("Read it again"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let probes = this.agent_usage_probes();
                        this.agent_probe_queue(&probes, cx);
                    })),
            )
            .child(
                Button::new("ag-usage-close")
                    .icon(Icon::new(IconName::Close))
                    .ghost()
                    .xsmall()
                    .tooltip(tr("Close (Esc)"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.usage.open = false;
                        cx.notify();
                    })),
            );

        let row = |i: usize, r: LimitRow| {
            let col = if r.remaining {
                level_remaining(r.pct)
            } else {
                level(r.pct)
            };
            let pct_text = if r.remaining {
                format!("{}% {}", r.pct.round(), tr("remaining"))
            } else {
                format!("{}%", r.pct.round())
            };
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(c(theme::FG_STRONG))
                                .child(r.name),
                        )
                        .child(
                            div()
                                .flex_none()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(col))
                                .child(pct_text),
                        ),
                )
                .child(h_flex().child(bar(
                    SharedString::from(format!("ag-usage-bar-{shown}-{i}")),
                    r.pct,
                    col,
                )))
                .when_some(r.detail, |v, d| {
                    v.child(
                        div()
                            .text_size(theme::u(11.))
                            .text_color(c(theme::FG_DIM))
                            .child(d),
                    )
                })
                .when_some(r.resets, |v, res| {
                    v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(res))
                })
        };
        let mut limits = v_flex().gap_3();
        let mut any = false;
        for (i, r) in u.limit_rows(now).into_iter().enumerate() {
            any = true;
            limits = limits.child(row(i, r));
        }
        let free = self.agent_model_free();
        if !any && free.is_some() {
            let name = free.unwrap_or_default();
            limits = limits.child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(div().flex_none().pt(px(1.)).child(Icon::new(IconName::BadgeCheck).size(theme::u(14.)).text_color(c(theme::GREEN))))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(crate::trf!("{name} is a free model"; name => name)))
                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("It runs on OpenCode Zen's free tier, so there is no plan, no usage limit and no cost to show here. The context below still fills up as the chat grows."))),
                    ),
            );
        } else if !any {
            limits = limits.child(div().text_xs().text_color(c(theme::FG_DIM)).child(if busy {
                tr("Reading your limits from the agent…")
            } else if self.agent.session.is_none() {
                tr("Turn the agent on to read your limits: it knows them, FluxCode doesn't.")
            } else {
                tr("The agent didn't report any limits.")
            }));
        }

        let context = match (&u.context, &u.live) {
            (Some((used, total, pct)), _) => Some((format!("{used} / {total}"), *pct as f32)),
            (None, Some(l)) if l.size > 0 => Some((
                format!("{} / {}", tokens(l.used), tokens(l.size)),
                l.used as f32 * 100. / l.size as f32,
            )),
            _ => None,
        };
        let session_cost = u
            .report
            .as_ref()
            .filter(|r| !r.cost.is_empty())
            .map(|r| crate::trf!("Cost so far {cost}"; cost => r.cost));
        let session_tokens: Vec<(String, String)> = u
            .report
            .as_ref()
            .map(|r| {
                r.tokens
                    .iter()
                    .filter(|(_, v)| !v.is_empty() && v != "0")
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let tips: Vec<(u32, String)> = u
            .report
            .as_ref()
            .map(|r| r.tips.iter().take(3).cloned().collect())
            .unwrap_or_default();

        let section = |t: &'static str| {
            div()
                .pt_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_MUTED))
                .child(tr(t))
        };
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
                    .child(
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .child(div().flex_1().text_color(c(theme::FG)).child(tr("Context")))
                            .child(
                                div()
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(label),
                            ),
                    )
                    .child(h_flex().child(bar(
                        SharedString::from(format!("ag-usage-ctx-{shown}")),
                        pct,
                        level(pct),
                    )))
                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr(
                        "What this chat holds now. A new chat (/new) starts it empty.",
                    ))),
            );
        }
        if session_cost.is_some() || !session_tokens.is_empty() {
            let mut sess_box = v_flex().gap_1p5().child(section("Session"));
            if let Some(cost) = session_cost {
                sess_box =
                    sess_box.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(cost));
            }
            if !session_tokens.is_empty() {
                sess_box = sess_box.child(h_flex().flex_wrap().gap_1p5().children(
                    session_tokens.into_iter().map(|(k, v)| {
                        h_flex()
                            .px_2()
                            .py_1()
                            .gap_1()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_EDITOR).opacity(0.65))
                            .text_xs()
                            .child(div().text_color(c(theme::FG_DIM)).child(format!("{k}:")))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(v),
                            )
                    }),
                ));
            }
            body = body
                .child(div().h(px(1.)).bg(c(theme::BORDER)))
                .child(sess_box);
        }
        if !tips.is_empty() {
            body = body
                .child(div().h(px(1.)).bg(c(theme::BORDER)))
                .child(section("What uses your limits, last 24 hours"))
                .children(tips.into_iter().enumerate().map(|(i, (p, t))| {
                    h_flex()
                        .id(("ag-usage-tip", i))
                        .gap_2()
                        .items_start()
                        .text_xs()
                        .child(
                            div()
                                .flex_none()
                                .w(theme::u(34.))
                                .text_color(c(theme::FG_STRONG))
                                .child(format!("{p}%")),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(c(theme::FG_MUTED))
                                .child(t),
                        )
                }));
        }
        if let Some(e) = &u.error {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(c(theme::DANGER))
                    .child(e.clone()),
            );
        }

        let can_credits = self
            .agent
            .commands
            .iter()
            .any(|c| c.name == "usage-credits");
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
            let waiting = u.probe.as_ref().map(|p| p.0) == Some(Probe::Credits)
                || u.queue.contains(&Probe::Credits);
            Some(
                v_flex()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(theme::u(150.))
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(tr("Limits reset by themselves at the times above.")),
                            )
                            .child(
                                Button::new("ag-credits")
                                    .label(if waiting {
                                        tr("Requesting…")
                                    } else {
                                        tr("Request usage credits")
                                    })
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
                                .child(
                                    div()
                                        .flex_none()
                                        .pt(px(2.))
                                        .child(Icon::new(IconName::Info).size(theme::u(12.))),
                                )
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
            .glass_pop()
            .bg(c(if theme::glass() {
                theme::BG_SIDE
            } else {
                theme::BG_ELEVATED
            })
            .opacity(if theme::glass() { 0.96 } else { 1.0 }))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if !this.agent.usage.confirm_credits {
                    this.pop_closed_outside("usage");
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
            .when_some(footer, |v, f| {
                v.child(
                    div()
                        .px_4()
                        .pb_3()
                        .pt_1()
                        .border_t_1()
                        .border_color(c(theme::BORDER))
                        .child(f),
                )
            });
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(0.42))
                .flex()
                .justify_center()
                .items_start()
                .px_3()
                .pt(theme::u(44.))
                .pb_3()
                .child(
                    card.with_animation(
                        SharedString::from(format!("ag-usage-in-{shown}")),
                        Animation::new(Duration::from_millis(180))
                            .with_easing(gpui_kit::ease_out_quint()),
                        |el, t| el.opacity(t).mt(px((1. - t) * 8.)),
                    ),
                )
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
        assert_eq!(
            r.limits[0],
            Limit {
                name: "5-hour limit".into(),
                pct: 16,
                resets: "Resets Nov 10, 10:40 PM GMT+7".into(),
                detail: String::new(),
                remaining: false,
            }
        );
        assert_eq!(r.limits[2].name, "Weekly · Fable");
        assert_eq!(r.limits[1].resets, "Resets Nov 16, 12:00 PM GMT+7");
        assert_eq!(r.cost, "$0.42");
        assert_eq!(r.tokens[0], ("Input".into(), "120".into()));
        assert_eq!(r.tips.len(), 2);
        assert_eq!(r.tips[0].0, 94);
    }

    #[test]
    fn reads_context_and_live_updates() {
        assert_eq!(
            parse_context("**Model:** x  \n**Tokens:** 34.6k / 1m (3%)\n"),
            Some(("34.6k".into(), "1m".into(), 3))
        );
        let mut l = Live::default();
        l.take(&serde_json::json!({"used": 59125, "size": 1000000, "_meta": {"_claude/rateLimit": {"unifiedWindows": {"five_hour": {"utilization": 0.18, "resetsAt": 100}}}, "_claude/model": "claude-haiku-5-5"}}));
        l.take(&serde_json::json!({"used": 60000, "cost": {"amount": 0.02}}));
        assert_eq!((l.used, l.size, l.cost), (60000, 1000000, Some(0.02)));
        assert_eq!(
            l.windows,
            vec![Window {
                key: "five_hour".into(),
                used: 0.18,
                resets: 100
            }]
        );
        assert_eq!(l.model, "claude-haiku-5-5");
        assert_eq!(tokens(59125), "59k");
        assert_eq!(tokens(1_000_000), "1M");
        assert_eq!(
            until(100 + 4 * 3600 + 12 * 60, 100).as_deref(),
            Some("4 h 12 min")
        );
        assert_eq!(
            until(100 + 2 * 86_400 + 3 * 3600, 100).as_deref(),
            Some("2 days 3 h")
        );
        assert_eq!(until(50, 100), None);
    }

    #[test]
    fn reads_antigravity_usage_reply() {
        let md = "### Session Usage\n- **Session total:** 18,640 tokens (prompt: 18,427, completion: 137, thinking: 76)\n\n### Quota Limits\n- **Gemini Models**\n  *(Models within this group: Gemini 4 Neon, Gemini 4 Carbon, Gemini 4 Argon, Gemini 4 Hydrogen, Gemini 3.8 Flash)*\n  - **Weekly Limit Remaining:** 60% remaining\n";
        let r = parse_usage(md);
        assert_eq!(r.limits.len(), 1);
        assert_eq!(r.limits[0].name, "Gemini Models · Weekly Limit");
        assert_eq!(r.limits[0].pct, 60);
        assert!(r.limits[0].remaining);
        assert_eq!(
            r.limits[0].detail,
            "Gemini 4 Neon, Gemini 4 Carbon, Gemini 4 Argon, Gemini 4 Hydrogen, Gemini 3.8 Flash"
        );
        assert_eq!(
            r.tokens,
            vec![
                ("Session total".into(), "18,640 tokens".into()),
                ("Prompt".into(), "18,427".into()),
                ("Completion".into(), "137".into()),
                ("Thinking".into(), "76".into()),
            ]
        );
    }
}
