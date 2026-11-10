//! "What's new": the page that opens the first time a new build starts, from
//! `changelog.flux` (compiled in, so it works offline). It shows every release dated
//! after the build that ran before, in the interface language, and closes with the
//! button or Escape. Help > What's New opens it with every release.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{button::{Button, ButtonVariants}, h_flex, text::TextView, v_flex, Sizable};
use gpui_kit::{prelude::FluentBuilder, *};

pub const SOURCE: &str = include_str!("../changelog.flux");

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Release {
    /// YYYY-MM-DD, as in the build names.
    pub date: String,
    pub title_en: String,
    pub title_id: String,
    pub body_en: String,
    pub body_id: String,
}

impl Release {
    pub fn title(&self, id: bool) -> &str {
        if id && !self.title_id.is_empty() { &self.title_id } else { &self.title_en }
    }
    pub fn body(&self, id: bool) -> &str {
        if id && !self.body_id.trim().is_empty() { &self.body_id } else { &self.body_en }
    }
}

/// Parse `changelog.flux` (see its header for the format).
pub fn parse(src: &str) -> Vec<Release> {
    let mut out: Vec<Release> = Vec::new();
    let mut lang = "en";
    for line in src.lines() {
        let t = line.trim_end();
        if let Some(d) = t.strip_prefix("@release ") {
            out.push(Release { date: d.trim().to_string(), ..Default::default() });
            lang = "en";
            continue;
        }
        let Some(r) = out.last_mut() else { continue };
        if let Some(rest) = t.strip_prefix("@title ") {
            if let Some(v) = rest.strip_prefix("en:") {
                r.title_en = v.trim().to_string();
            } else if let Some(v) = rest.strip_prefix("id:") {
                r.title_id = v.trim().to_string();
            }
        } else if t == "@en" || t == "@id" {
            lang = &t[1..];
        } else {
            let body = if lang == "id" { &mut r.body_id } else { &mut r.body_en };
            body.push_str(line);
            body.push('\n');
        }
    }
    for r in &mut out {
        r.body_en = r.body_en.trim().to_string();
        r.body_id = r.body_id.trim().to_string();
    }
    out
}

/// "BuildDev-2026-11-10-08-00-00" -> "2026-11-10".
pub fn build_date(build: &str) -> Option<&str> {
    let d = build.strip_prefix("BuildDev-")?.get(..10)?;
    (d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-').then_some(d)
}

/// Indices of the releases to show after updating from `last` (a build name): those
/// dated after it, or the newest one when none is (an update on the same day).
pub fn since(releases: &[Release], last: Option<&str>) -> Vec<usize> {
    let after: Vec<usize> = match last.and_then(build_date) {
        Some(d) => releases.iter().enumerate().filter(|(_, r)| r.date.as_str() > d).map(|(i, _)| i).collect(),
        None => (0..releases.len()).collect(),
    };
    if after.is_empty() && !releases.is_empty() { vec![0] } else { after }
}

/// The page's state: the releases it shows and the build it came from.
pub struct Page {
    pub releases: Vec<Release>,
    pub from: Option<String>,
}

impl Page {
    pub fn after_update(from: &str) -> Self {
        let all = parse(SOURCE);
        let pick = since(&all, Some(from));
        Self { releases: pick.into_iter().map(|i| all[i].clone()).collect(), from: Some(from.to_string()) }
    }
    pub fn all() -> Self {
        Self { releases: parse(SOURCE), from: None }
    }
}

impl Workbench {
    pub(crate) fn render_whats_new(&self, page: &Page, cx: &mut Context<Self>) -> AnyElement {
        let id = crate::i18n::is_id();
        let close = Button::new("wn-close").label(tr("Close")).small().on_click(cx.listener(|this, _, _, cx| {
            this.whats_new = None;
            cx.notify();
        }));
        let now = crate::update::BUILD;
        let sub = match &page.from {
            Some(f) => crate::trf!("You are on {now}. Before this you ran {from}."; now => now, from => f),
            None => crate::trf!("You are on {now}."; now => now),
        };
        let header = h_flex()
            .flex_none()
            .w_full()
            .max_w(theme::u(760.))
            .gap_4()
            .items_center()
            .child(crate::ui::meander(44., c(theme::FG_STRONG), c(theme::ACCENT_TEXT)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("What's new in FluxCode")))
                    .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(sub)),
            )
            .child(close);
        let cards = page.releases.iter().enumerate().map(|(i, r)| {
            v_flex()
                .flex_none()
                .w_full()
                .max_w(theme::u(760.))
                .mt_6()
                .rounded(px(8.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_SIDE))
                .overflow_hidden()
                .child(
                    h_flex()
                        .px_5()
                        .py_3()
                        .gap_3()
                        .border_b_1()
                        .border_color(c(theme::BORDER))
                        .child(
                            div()
                                .flex_none()
                                .px_2()
                                .rounded(px(4.))
                                .border_1()
                                .border_color(c(theme::BORDER_STRONG))
                                .text_xs()
                                .text_color(c(theme::ACCENT_TEXT))
                                .child(r.date.clone()),
                        )
                        .child(div().flex_1().min_w_0().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(r.title(id).to_string())),
                )
                .child(div().px_5().py_4().text_sm().text_color(c(theme::FG)).child(TextView::markdown(("wn-md", i), r.body(id).to_string())))
        });
        v_flex()
            .id("whats-new")
            .key_context("WhatsNew")
            .size_full()
            .px_8()
            .py_6()
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.whats_new = None;
                    cx.notify();
                }
            }))
            .child(header)
            .children(cards)
            .child(
                h_flex().w_full().max_w(theme::u(760.)).pt_6().gap_2().child(
                    Button::new("wn-all")
                        .label(tr("Full changelog on GitHub"))
                        .xsmall()
                        .ghost()
                        .on_click(|_, _, cx| cx.open_url(crate::settings::CHANGELOG_URL)),
                ),
            )
            .when(page.releases.is_empty(), |v| v.child(div().pt_6().text_sm().text_color(c(theme::FG_DIM)).child(tr("No notes for this build."))))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{build_date, parse, since, SOURCE};

    #[test]
    fn the_shipped_changelog_parses_in_both_languages() {
        let all = parse(SOURCE);
        assert!(all.len() >= 2);
        for r in &all {
            assert_eq!(r.date.len(), 10, "{}", r.date);
            assert!(!r.title_en.is_empty() && !r.title_id.is_empty(), "{}", r.date);
            assert!(!r.body_en.is_empty() && !r.body_id.is_empty(), "{}", r.date);
            assert!(!r.body_en.contains("@id") && !r.body_en.contains('\u{2014}'), "{}", r.date);
        }
        // Newest first.
        assert!(all.windows(2).all(|w| w[0].date > w[1].date));
    }

    #[test]
    fn shows_what_is_newer_than_the_last_build() {
        let src = "@release 2026-11-12\n@title en: C\n@en\nc\n@release 2026-11-10\n@title en: B\n@en\nb\n@release 2026-11-01\n@title en: A\n@en\na\n";
        let all = parse(src);
        assert_eq!(since(&all, Some("BuildDev-2026-11-09-10-00-00")), vec![0, 1]);
        assert_eq!(since(&all, Some("BuildDev-2026-11-12-23-00-00")), vec![0]);
        assert_eq!(since(&all, None), vec![0, 1, 2]);
        assert_eq!(build_date("BuildDev-2026-11-10-08-00-00"), Some("2026-11-10"));
        assert_eq!(build_date("0.4.2-dev"), None);
    }
}
