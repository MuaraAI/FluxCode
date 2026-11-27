
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::Value;
use std::time::Duration;

const FIRST: usize = 4;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Web {
    pub id: String,
    pub query: String,
    pub results: Vec<(String, String)>,
    pub status: String,
    pub fetch: bool,
}

pub fn tool_name(u: &Value) -> Option<&str> {
    u.pointer("/_meta/claudeCode/toolName").and_then(Value::as_str)
}

pub fn hidden_tool(u: &Value) -> bool {
    matches!(tool_name(u), Some("ToolSearch"))
}

pub fn is_web(u: &Value) -> bool {
    matches!(tool_name(u), Some("WebSearch" | "WebFetch"))
}

fn query_of_title(t: &str) -> Option<String> {
    let q = t.strip_prefix("Search ")?.trim().trim_matches(['"', '\u{201c}', '\u{201d}']);
    (!q.is_empty()).then(|| q.to_string())
}

impl Web {
    pub fn new(u: &Value) -> Self {
        let mut w = Web { id: u.get("toolCallId").and_then(Value::as_str).unwrap_or_default().to_string(), fetch: tool_name(u) == Some("WebFetch"), status: "pending".into(), ..Default::default() };
        w.take(u);
        w
    }

    pub fn take(&mut self, u: &Value) {
        if let Some(s) = u.get("status").and_then(Value::as_str) {
            self.status = s.to_string();
        }
        let input = u.get("rawInput");
        if let Some(q) = input.and_then(|i| i.get("query").or_else(|| i.get("url"))).and_then(Value::as_str) {
            self.query = q.to_string();
        } else if let Some(q) = u.get("title").and_then(Value::as_str).and_then(query_of_title) {
            self.query = q;
        }
        let found = u.pointer("/_meta/claudeCode/toolResponse/results").and_then(Value::as_array);
        if let Some(groups) = found {
            let mut out = Vec::new();
            for g in groups {
                for r in g.get("content").and_then(Value::as_array).into_iter().flatten() {
                    let (t, url) = (r.get("title").and_then(Value::as_str).unwrap_or(""), r.get("url").and_then(Value::as_str).unwrap_or(""));
                    if !url.is_empty() && !out.iter().any(|(_, u): &(String, String)| u == url) {
                        out.push((t.to_string(), url.to_string()));
                    }
                }
            }
            if !out.is_empty() {
                self.results = out;
            }
        }
    }

    pub fn running(&self) -> bool {
        !matches!(self.status.as_str(), "completed" | "failed")
    }
}

pub fn domain(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

fn shown_title(title: &str, url: &str, repeated: bool) -> String {
    let t = title.trim();
    let weak = repeated || t.is_empty() || t.eq_ignore_ascii_case(&domain(url)) || t == "Expand description";
    if !weak {
        return t.to_string();
    }
    let rest = url.split("://").nth(1).unwrap_or(url);
    let path = rest.split_once('/').map(|(_, p)| p.trim_end_matches('/')).unwrap_or("");
    if path.is_empty() { domain(url) } else { format!("{} / {}", domain(url), path.replace('/', " / ")) }
}

impl Workbench {
    pub(crate) fn render_web_item(&self, i: usize, w: &Web, cx: &mut Context<Self>) -> AnyElement {
        let running = w.running();
        let failed = w.status == "failed";
        let open = self.agent.web_open.contains(&w.id);
        let n = w.results.len();
        let title: SharedString = match (w.fetch, running, failed) {
            (true, true, _) => tr("Reading a page").into(),
            (true, false, false) => tr("Read a page").into(),
            (false, true, _) => tr("Searching the web").into(),
            (false, false, false) if n > 0 => crate::trf!("Searched the web · {n} sources"; n => n).into(),
            (false, false, false) => tr("Searched the web").into(),
            (_, _, true) => tr("The search failed").into(),
        };
        let head = h_flex()
            .gap_2()
            .child(Icon::new(IconName::Globe).size(theme::u(14.)).text_color(c(if failed { theme::DANGER } else if running { theme::ACCENT_TEXT } else { theme::FG_MUTED })))
            .child(div().flex_none().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG)).child(title))
            .when(!w.query.is_empty(), |h| {
                let q = if w.fetch { domain(&w.query) } else { w.query.clone() };
                h.child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(if w.fetch { q } else { format!("\u{201c}{q}\u{201d}") }))
            })
            .when(w.query.is_empty(), |h| h.child(div().flex_1()));
        let sweep = running.then(|| {
            div()
                .relative()
                .h(px(2.))
                .mx_2()
                .rounded_full()
                .bg(c(theme::BG_HOVER))
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .w(relative(0.28))
                        .rounded_full()
                        .bg(c(theme::ACCENT_TEXT))
                        .with_animation(SharedString::from(format!("ag-web-sweep-{}", w.id)), Animation::new(Duration::from_millis(1100)).repeat().with_easing(gpui_kit::ease_in_out), |el, t| el.left(relative(-0.28 + t * 1.28))),
                )
        });
        let shown: Vec<(usize, &(String, String))> = w.results.iter().enumerate().take(if open { n } else { FIRST }).collect();
        let repeated = |t: &str| w.results.iter().filter(|(x, _)| x.trim() == t.trim()).count() > 1;
        let total = shown.len();
        let rows = (!running && n > 0).then(|| {
            v_flex().gap_px().children(shown.into_iter().map(|(k, (t, url))| {
                let u = url.clone();
                let parsed = crate::smart_link::parse_link(url);
                let row = h_flex()
                    .id(SharedString::from(format!("ag-web-{i}-{k}")))
                    .h(theme::u(26.))
                    .px_2()
                    .gap_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(
                        div()
                            .flex_none()
                            .size(theme::u(16.))
                            .rounded(px(4.))
                            .bg(c(theme::BG_HOVER))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(parsed.icon())
                                    .size(theme::u(11.))
                                    .text_color(c(parsed.icon_color())),
                            ),
                    )
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG)).child(shown_title(t, url, repeated(t))))
                    .child(div().flex_none().max_w(relative(0.4)).truncate().text_xs().text_color(c(theme::FG_DIM)).child(parsed.provider))
                    .tooltip({
                        let u = url.clone();
                        move |w, cx| gpui_kit::component::tooltip::Tooltip::new(u.clone()).build(w, cx)
                    })
                    .on_click(move |_, _, cx| cx.open_url(&u));
                let delay = k as f32 * 40.;
                let span = 160. + delay;
                row.with_animation(SharedString::from(format!("ag-web-in-{}-{k}-{total}", w.id)), Animation::new(Duration::from_millis(span as u64)).with_easing(gpui_kit::ease_out_quint()), move |el, t| {
                    let local = ((t * span - delay) / 160.).clamp(0., 1.);
                    el.opacity(local).ml(px((1. - local) * 6.))
                })
                .into_any_element()
            }))
        });
        let more = (!running && n > FIRST).then(|| {
            let id = w.id.clone();
            div()
                .id(SharedString::from(format!("ag-web-more-{i}")))
                .px_2()
                .py_0p5()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .cursor_pointer()
                .hover(|s| s.text_color(c(theme::FG_STRONG)))
                .child(if open { tr("Show fewer").to_string() } else { crate::trf!("Show all {n}"; n => n) })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.agent.web_open.remove(&id) {
                        this.agent.web_open.insert(id.clone());
                    }
                    cx.notify();
                }))
        });
        v_flex()
            .py_1p5()
            .gap_1()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .child(div().px_2().child(head))
            .children(sweep)
            .children(rows.map(|r| div().px_1().child(r)))
            .children(more)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{domain, hidden_tool, is_web, shown_title, Web};

    #[test]
    fn a_search_from_its_updates() {
        let start = serde_json::json!({"_meta":{"claudeCode":{"toolName":"WebSearch"}},"toolCallId":"t1","sessionUpdate":"tool_call","status":"pending","title":"Web search","kind":"fetch"});
        assert!(is_web(&start) && !hidden_tool(&start));
        let mut w = Web::new(&start);
        w.take(&serde_json::json!({"toolCallId":"t1","rawInput":{"query":"GPUI Rust"},"title":"Search \"GPUI Rust\""}));
        assert_eq!(w.query, "GPUI Rust");
        assert!(w.running());
        w.take(&serde_json::json!({"_meta":{"claudeCode":{"toolResponse":{"results":[{"content":[{"title":"gpui component","url":"https://github.com/longbridge/gpui-component"},{"title":"Expand description","url":"https://docs.rs/gpui-pre/"}]}]}}}}));
        w.take(&serde_json::json!({"status":"completed"}));
        assert!(!w.running());
        assert_eq!(w.results.len(), 2);
        assert_eq!(domain("https://www.github.com/x"), "github.com");
        assert_eq!(shown_title("Expand description", "https://docs.rs/gpui-pre/", false), "docs.rs / gpui-pre");
        assert_eq!(shown_title("overview summary", "https://jd.papermc.io/paper/1.21/", true), "jd.papermc.io / paper / 1.21");
        assert!(hidden_tool(&serde_json::json!({"_meta":{"claudeCode":{"toolName":"ToolSearch"}}})));
    }
}
