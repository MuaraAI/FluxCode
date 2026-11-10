//! What the agent is doing, as one connected timeline: thinking, reading, editing,
//! running a command, searching. Every step is a row on the same rail, so a turn reads
//! as one sequence instead of a pile of boxes.
//!
//! Motion has one job each, and nothing else moves:
//! - a step that is running has its icon in the accent and a thin line sweeping under
//!   it (the same sweep as a web search);
//! - when it finishes, the label changes from "Reading" to "Read" with a short
//!   cross-fade, the sweep is gone, and the time it took appears;
//! - a new step slides in once, 6 px, in 160 ms.
//! Animations run only while something changes, so an idle chat costs nothing.
//!
//! References: AI Elements' Reasoning ("Thought for N seconds", shimmer while
//! streaming) and Chain of Thought, assistant-ui's grouped tool calls, HeroUI's streaming
//! steps, Zed's tool cards, GAIA's collapsible tool timeline, Hightouch's agent activity
//! log, Warp's command blocks, Perplexity's Searching / Reading / Writing steps.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use std::time::{Duration, Instant};

/// When a step started, and how long it took once it ended.
pub type Times = std::collections::HashMap<String, (Instant, Option<Duration>)>;

/// The words for a kind of step: while it runs, and once it is done.
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

/// Agents that send web tools as kind `other` with the tool's bare name (OpenCode's
/// `websearch`, `webfetch`): the kind they mean, and what the row says they worked on.
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

/// What a step works on, from its title: "Read src/main.rs" → "src/main.rs" (the
/// verb is said by the row), "`cargo build`" → "cargo build" (shown as code).
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

/// "0.4 s", "12 s", "1 min 5 s".
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

/// One step on the rail.
pub struct Step<'a> {
    pub key: String,
    pub kind: &'a str,
    pub title: &'a str,
    /// pending, in_progress, completed, failed.
    pub status: &'a str,
    /// The step above and the one below are steps too (the rail connects them).
    pub joined_up: bool,
    pub joined_down: bool,
    /// The agent is still on this turn (a pending step left when it stopped is not running).
    pub live: bool,
    /// A thought: its last line, shown dim after the label.
    pub note: Option<String>,
    pub open: bool,
    /// An opened thought's full text: it sits inside the step, so the rail runs down
    /// beside it to the next step.
    pub detail: Option<String>,
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

        // The rail: a line from the step above, the icon, a line to the step below.
        let rail = v_flex()
            .flex_none()
            .w(theme::u(18.))
            .h_full()
            .items_center()
            .child(div().w(px(1.)).h(theme::u(5.)).when(s.joined_up, |d| d.bg(c(theme::BORDER_STRONG))))
            .child(
                div()
                    .flex_none()
                    .size(theme::u(18.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(c(if running { theme::ACCENT_TEXT } else { theme::BORDER }))
                    .bg(c(theme::BG_ELEVATED))
                    .child(Icon::new(icon).size(theme::u(10.)).text_color(c(col))),
            )
            .child(div().w(px(1.)).flex_1().min_h(theme::u(5.)).when(s.joined_down, |d| d.bg(c(theme::BORDER_STRONG))));

        // The label changes with a cross-fade when the step's state does.
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
            // Time and chevron sit in fixed columns, so every row's time lines up on the
            // right whether the row opens or not.
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
        // While it runs: the sweep under the label (the one thing that keeps moving).
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
        let tip: SharedString = s.title.to_string().into();
        let detail = s.detail.map(|t| div().pb_2().pr_4().text_xs().line_height(relative(1.55)).text_color(c(theme::FG_DIM)).child(t));
        let body = v_flex().flex_1().min_w_0().justify_center().child(head).children(sweep).children(detail);
        let row = h_flex()
            .id(SharedString::from(format!("ag-step-{}", s.key)))
            // The chat puts 8 px between items: a step joined to the one above eats it,
            // so the rail runs on without a gap.
            .when(s.joined_up, |d| d.mt(px(-8.)))
            .gap_2()
            .items_stretch()
            .when(on_click.is_some(), |d| d.cursor_pointer())
            .child(rail)
            .child(body)
            .when(s.kind != "think" && !s.title.is_empty(), |d| d.tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)))
            .when_some(on_click, |d, f| d.on_click(cx.listener(move |this, _, _, cx| f(this, cx))));
        // A new step slides in once.
        row.with_animation(SharedString::from(format!("ag-step-in-{}", s.key)), Animation::new(Duration::from_millis(160)).with_easing(gpui_kit::ease_out_quint()), |el, t| el.opacity(t).ml(px((1. - t) * 6.)))
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
