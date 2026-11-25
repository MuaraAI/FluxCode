
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, text::TextView, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::time::{Duration, Instant};

pub type Times = std::collections::HashMap<String, (Instant, Option<Duration>)>;

pub fn verbs(kind: &str) -> (&'static str, &'static str, IconName) {
    match kind {
        "read" => ("Reading", "Read", IconName::FileText),
        "edit" => ("Editing", "Edited", IconName::Pencil),
        "delete" => ("Deleting", "Deleted", IconName::Delete),
        "move" => ("Moving", "Moved", IconName::ArrowRight),
        "search" => ("Searching", "Searched", IconName::Search),
        "execute" => ("Running", "Ran", IconName::SquareTerminal),
        "fetch" => ("Fetching", "Fetched", IconName::Globe),
        "think" => ("Thinking", "Thought", IconName::Brain),
        _ => ("Working", "Done", IconName::Settings2),
    }
}

pub fn web_kind<'a>(kind: &'a str, title: &str) -> Option<(&'a str, &'static str)> {
    if !matches!(kind, "other" | "") {
        return None;
    }
    match title.trim().to_ascii_lowercase().replace(['_', '-', ' '], "").as_str() {
        "websearch" => Some(("search", "the web")),
        "webfetch" => Some(("fetch", "a page")),
        _ => None,
    }
}

pub fn target(title: &str, kind: &str) -> (String, bool) {
    let t = title.trim();
    if let Some(cmd) = t.strip_prefix('`').and_then(|s| s.strip_suffix('`')) {
        return (cmd.to_string(), true);
    }
    for v in ["Read ", "Reading ", "Edit ", "Write ", "Writing ", "Search ", "Searching ", "Fetch ", "Delete ", "Move ", "Run ", "Running ", "Find ", "Grep ", "Glob "] {
        if let Some(rest) = t.strip_prefix(v) {
            return (rest.trim_matches(['"', '`']).to_string(), false);
        }
    }
    (t.to_string(), kind == "execute")
}

pub fn took(d: Duration) -> String {
    let s = d.as_secs_f32();
    if s < 10. {
        format!("{s:.1} s")
    } else if s < 60. {
        format!("{} s", s.round() as u64)
    } else {
        format!("{} min {} s", (s / 60.) as u64, (s % 60.) as u64)
    }
}

pub struct Group {
    pub first: usize,
    pub last: usize,
    pub thoughts: usize,
    pub tools: usize,
    pub running: bool,
    pub open: bool,
    pub total: Option<Duration>,
    pub joined_down: bool,
}

pub struct Step<'a> {
    pub key: String,
    pub kind: &'a str,
    pub title: &'a str,
    pub status: &'a str,
    pub joined_up: bool,
    pub pull_up: bool,
    pub joined_down: bool,
    pub live: bool,
    pub note: Option<String>,
    pub open: bool,
    pub detail: Option<String>,
}

fn circle_px() -> f32 {
    // Keep circle diameter an odd integer so the 1px vertical rail sits at exact integer center.
    let raw = f32::from(theme::u(18.)).round();
    if (raw as i32) % 2 == 0 { raw + 1. } else { raw }
}

fn build_rail(icon: IconName, running: bool, col: theme::Tok, joined_up: bool, joined_down: bool) -> AnyElement {
    let d = circle_px();
    let row_h = f32::from(theme::u(28.)).round();
    let top = ((row_h - d) / 2.).round().max(0.);
    let rail_x = (d - 1.) / 2.;
    let mid = top + rail_x;
    let icon_sz = f32::from(theme::u(10.)).round().min(d - 8.).max(9.);
    div()
        .relative()
        .flex_none()
        .w(px(d))
        .min_h(px(row_h))
        .h_full()
        .when(joined_up, |el| {
            el.child(
                div()
                    .absolute()
                    .left(px(rail_x))
                    .top_0()
                    .w(px(1.))
                    .h(px(mid))
                    .bg(c(theme::BORDER_STRONG)),
            )
        })
        .when(joined_down, |el| {
            el.child(
                div()
                    .absolute()
                    .left(px(rail_x))
                    .top(px(mid))
                    .bottom_0()
                    .w(px(1.))
                    .bg(c(theme::BORDER_STRONG)),
            )
        })
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(top))
                .size(px(d))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(c(if running { theme::ACCENT_TEXT } else { theme::BORDER }))
                .bg(c(theme::BG_ELEVATED))
                .child(Icon::new(icon).size(px(icon_sz)).text_color(c(col))),
        )
        .into_any_element()
}

impl Workbench {
    pub(crate) fn render_step(&self, s: Step, on_click: Option<Box<dyn Fn(&mut Workbench, &mut Context<Workbench>) + 'static>>, cx: &mut Context<Self>) -> AnyElement {
        let running = s.live && matches!(s.status, "pending" | "in_progress" | "");
        let failed = s.status == "failed";
        let web = web_kind(s.kind, s.title);
        let kind = web.map_or(s.kind, |w| w.0);
        let (doing, done, icon) = verbs(kind);
        let label: SharedString = tr(if running { doing } else if failed && kind != "think" { "Failed" } else { done }).into();
        let (what, code) = match web {
            Some((_, what)) => (tr(what).to_string(), false),
            None if kind == "think" => (s.note.clone().unwrap_or_default(), false),
            None => target(s.title, kind),
        };
        let time = self.agent.times.get(&s.key).and_then(|(_, d)| *d).map(took);
        let col = if failed { theme::DANGER } else if running { theme::ACCENT_TEXT } else { theme::FG_DIM };

        let rail = build_rail(icon, running, col, s.joined_up, s.joined_down);

        let state_key: SharedString = format!("ag-step-{}-{}", s.key, if running { "run" } else { s.status }).into();
        let head = h_flex()
            .min_h(theme::u(28.))
            .gap_1p5()
            .text_xs()
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(if failed { theme::DANGER } else if running { theme::FG_STRONG } else { theme::FG_MUTED }))
                    .child(label)
                    .with_animation(state_key.clone(), Animation::new(Duration::from_millis(180)).with_easing(gpui_kit::ease_out_quint()), |el, t| el.opacity(0.35 + 0.65 * t)),
            )
            .when(!what.is_empty(), |h| {
                let w = div().flex_1().min_w_0().truncate();
                h.child(if code {
                    w.font_family("Cascadia Code").text_color(c(theme::FG)).child(what.clone())
                } else if s.kind == "think" {
                    w.text_color(c(theme::FG_DIM)).child(what.clone())
                } else {
                    w.text_color(c(theme::FG)).child(what.clone())
                })
            })
            .when(what.is_empty(), |h| h.child(div().flex_1()))
            .child(
                div()
                    .flex_none()
                    .min_w(theme::u(40.))
                    .flex()
                    .justify_end()
                    .text_color(c(theme::FG_DIM))
                    .when_some(time, |d, t| d.child(div().child(t).with_animation(SharedString::from(format!("{state_key}-t")), Animation::new(Duration::from_millis(220)), |el, t| el.opacity(t)))),
            )
            .child(
                div()
                    .flex_none()
                    .w(theme::u(12.))
                    .flex()
                    .justify_center()
                    .when(s.kind == "think", |d| d.child(Icon::new(if s.open { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(12.)).text_color(c(theme::FG_DIM)))),
            );
        let sweep = running.then(|| {
            div().relative().h(px(2.)).mr_2().rounded_full().bg(c(theme::BG_HOVER)).overflow_hidden().child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .w(relative(0.3))
                    .rounded_full()
                    .bg(c(theme::ACCENT_TEXT))
                    .with_animation(SharedString::from(format!("ag-step-sweep-{}", s.key)), Animation::new(Duration::from_millis(1100)).repeat().with_easing(gpui_kit::ease_in_out), |el, t| el.left(relative(-0.3 + t * 1.3))),
            )
        });
        let detail = s.detail.map(|t| {
            div()
                .pb_2()
                .pr_4()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(TextView::markdown(SharedString::from(format!("ag-thought-{}", s.key)), t).text_xs().selectable(true))
        });
        let body = v_flex().flex_1().min_w_0().justify_center().child(head).children(sweep).children(detail);
        let row = h_flex()
            .id(SharedString::from(format!("ag-step-{}", s.key)))
            .when(s.pull_up, |d| d.mt(px(-8.)))
            .gap_2()
            .items_stretch()
            .when(on_click.is_some(), |d| d.cursor_pointer())
            .child(rail)
            .child(body)
            .when_some(on_click, |d, f| d.on_click(cx.listener(move |this, _, _, cx| f(this, cx))));
        row.with_animation(SharedString::from(format!("ag-step-in-{}", s.key)), Animation::new(Duration::from_millis(160)).with_easing(gpui_kit::ease_out_quint()), |el, t| el.opacity(t).ml(px((1. - t) * 6.)))
            .into_any_element()
    }
}

impl Workbench {
    pub(crate) fn render_step_group(&self, g: Group, cx: &mut Context<Self>) -> AnyElement {
        let (first, last) = (g.first, g.last);
        let mut parts: Vec<String> = Vec::new();
        if g.thoughts > 0 {
            parts.push(if g.thoughts == 1 { tr("1 thought").to_string() } else { crate::trf!("{n} thoughts"; n => g.thoughts) });
        }
        if g.tools > 0 {
            parts.push(if g.tools == 1 { tr("1 tool").to_string() } else { crate::trf!("{n} tools"; n => g.tools) });
        }
        let col = if g.running { theme::ACCENT_TEXT } else { theme::FG_DIM };
        let rail = build_rail(IconName::ListCollapse, g.running, col, false, g.joined_down);
        h_flex()
            .id(("ag-group", first))
            .gap_2()
            .items_stretch()
            .cursor_pointer()
            .child(rail)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h(theme::u(28.))
                    .gap_1p5()
                    .text_xs()
                    .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(c(if g.running { theme::FG_STRONG } else { theme::FG_MUTED })).child(tr(if g.running { "Working" } else { "Worked" })))
                    .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_DIM)).child(parts.join(", ")))
                    .child(div().flex_none().min_w(theme::u(40.)).flex().justify_end().text_color(c(theme::FG_DIM)).when_some(g.total.map(took), |d, t| d.child(t)))
                    .child(div().flex_none().w(theme::u(12.)).flex().justify_center().child(Icon::new(if g.open { IconName::ChevronDown } else { IconName::ChevronRight }).size(theme::u(12.)).text_color(c(theme::FG_DIM)))),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.agent.open_groups.remove(&first) {
                    this.agent.open_groups.insert(first);
                }
                this.agent.remeasure.borrow_mut().push(first..last + 1);
                cx.notify();
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{target, took, verbs};
    use std::time::Duration;

    #[test]
    fn words_and_targets() {
        assert_eq!(verbs("read").0, "Reading");
        assert_eq!(verbs("execute").1, "Ran");
        assert_eq!(target("Read src/main.rs", "read"), ("src/main.rs".into(), false));
        assert_eq!(target("`cargo build --release`", "execute"), ("cargo build --release".into(), true));
        assert_eq!(target("Search \"fn main\"", "search"), ("fn main".into(), false));
        assert_eq!(took(Duration::from_millis(420)), "0.4 s");
        assert_eq!(took(Duration::from_secs(65)), "1 min 5 s");
        assert_eq!(super::web_kind("other", "websearch"), Some(("search", "the web")));
        assert_eq!(super::web_kind("other", "webfetch"), Some(("fetch", "a page")));
        assert_eq!(super::web_kind("read", "websearch"), None);
    }
}
