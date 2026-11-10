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

// RELEASE_BUILDS: the build each release came in, worked out from git by build.rs.
include!(concat!(env!("OUT_DIR"), "/release_builds.rs"));

/// The build a release came in, when git could tell (by its English title).
pub fn build_of(title_en: &str) -> Option<&'static str> {
    RELEASE_BUILDS.iter().find(|(t, _)| *t == title_en).map(|(_, b)| *b)
}

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
    /// A newer build on offer (the update card's "What's new"): its tag, release page,
    /// and every build between this one and it.
    pub update: Option<crate::update::Release>,
    /// Fetching the new build's changelog.flux.
    pub loading: bool,
    pub error: Option<String>,
}

/// The new build's notes, from its changelog.flux at its tag on GitHub (one small text
/// file, fetched when asked). Blocking.
pub fn fetch(tag: &str) -> Result<String, String> {
    let url = format!("https://raw.githubusercontent.com/MuaraAI/FluxCode/{tag}/changelog.flux");
    let out = crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-s", "-S", "-f", "-L", "--max-time", "15", "-H", "User-Agent: FluxCode", &url])
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        return Err(crate::acp::curl_error(&String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Releases in `newer` that this build doesn't carry (by date and title), newest first.
pub fn not_in(newer: Vec<Release>, mine: &[Release]) -> Vec<Release> {
    newer.into_iter().filter(|r| !mine.iter().any(|m| m.date == r.date && m.title_en == r.title_en)).collect()
}

impl Page {
    pub fn after_update(from: &str) -> Self {
        let all = parse(SOURCE);
        let pick = since(&all, Some(from));
        // An empty name means the build before didn't record itself: say nothing about it.
        let from = (!from.is_empty()).then(|| from.to_string());
        Self { releases: pick.into_iter().map(|i| all[i].clone()).collect(), from, update: None, loading: false, error: None }
    }
    pub fn all() -> Self {
        Self { releases: parse(SOURCE), from: None, update: None, loading: false, error: None }
    }
}

impl Workbench {
    /// The update card's "What's new": the new build's notes, compared with this one's.
    pub(crate) fn show_update_notes(&mut self, rel: crate::update::Release, cx: &mut Context<Self>) {
        let tag = rel.tag.clone();
        self.settings_open = false;
        // The page says the same and more: the card steps aside for this build.
        self.update.hidden = Some(tag.clone());
        self.whats_new = Some(Page { releases: Vec::new(), from: Some(crate::update::BUILD.to_string()), update: Some(rel), loading: true, error: None });
        cx.notify();
        let task = cx.background_spawn(async move { fetch(&tag) });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            _ = this.update(cx, |this, cx| {
                if let Some(p) = this.whats_new.as_mut().filter(|p| p.update.is_some()) {
                    p.loading = false;
                    match res {
                        Ok(text) => p.releases = not_in(parse(&text), &parse(SOURCE)),
                        Err(e) => p.error = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn render_whats_new(&self, page: &Page, cx: &mut Context<Self>) -> AnyElement {
        let id = crate::i18n::is_id();
        let close = Button::new("wn-close").label(tr("Close")).small().on_click(cx.listener(|this, _, _, cx| {
            this.whats_new = None;
            cx.notify();
        }));
        let now = crate::update::BUILD;
        let title = match &page.update {
            Some(_) => tr("What the new build changes"),
            None => tr("What's new in FluxCode"),
        };
        // Label and build name, one pair per line; the names in a fixed-width font so
        // the dates and times line up and read at a glance.
        let pair = |label: SharedString, build: String, strong: bool| {
            h_flex()
                .flex_wrap()
                .gap_x_2()
                .text_xs()
                .child(div().flex_none().w(theme::u(64.)).text_color(c(theme::FG_DIM)).child(label))
                .child(div().font_family("Cascadia Code").text_color(c(if strong { theme::FG_STRONG } else { theme::FG_MUTED })).child(build))
        };
        let mut builds = v_flex().gap_0p5().pt_1();
        builds = builds.child(pair(tr("Installed").into(), now.to_string(), true));
        match (&page.update, &page.from) {
            (Some(u), _) => {
                builds = builds.child(pair(tr("Ready").into(), u.tag.clone(), false));
                if u.between.len() > 1 {
                    builds = builds.child(div().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("{n} builds since yours"; n => u.between.len())));
                }
            }
            (None, Some(f)) => builds = builds.child(pair(tr("Before").into(), f.clone(), false)),
            (None, None) => {}
        }
        // The logo, title and Close on one line; the build names get the full width below.
        let header = v_flex()
            .flex_none()
            .w_full()
            .max_w(theme::u(760.))
            .gap_2()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().flex_none().child(crate::ui::meander(32., c(theme::FG_STRONG), c(theme::ACCENT_TEXT))))
                    .child(div().flex_1().min_w_0().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
                    .child(div().flex_none().child(close)),
            )
            .child(builds);
        let newer = page.update.is_some();
        let cards = page.releases.iter().enumerate().map(|(i, r)| {
            let build = build_of(&r.title_en);
            let here = build == Some(now);
            // The build name when git knew it, else the date.
            let chip: SharedString = build.map(str::to_string).unwrap_or_else(|| r.date.clone()).into();
            let note: Option<SharedString> = if here {
                Some(tr("installed now").into())
            } else if newer {
                Some(tr("in the new build").into())
            } else {
                None
            };
            v_flex()
                .flex_none()
                .w_full()
                .max_w(theme::u(760.))
                .mt_5()
                .rounded(px(8.))
                .border_1()
                .border_color(c(if here { theme::BORDER_STRONG } else { theme::BORDER }))
                .bg(c(theme::BG_SIDE))
                .overflow_hidden()
                .child(
                    v_flex()
                        .px_4()
                        .py_3()
                        .gap_1p5()
                        .border_b_1()
                        .border_color(c(theme::BORDER))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_x_2()
                                .gap_y_1()
                                .items_center()
                                .text_xs()
                                .child({
                                    let tip = chip.clone();
                                    div()
                                        .id(("wn-build", i))
                                        .flex_shrink(1.)
                                        .min_w_0()
                                        .truncate()
                                        .px_1p5()
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(c(theme::BORDER_STRONG))
                                        .text_color(c(theme::ACCENT_TEXT))
                                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                                        .child(chip)
                                })
                                .children(note.map(|n| div().text_color(c(theme::FG_DIM)).child(n))),
                        )
                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(r.title(id).to_string())),
                )
                .child(div().px_4().py_3().text_sm().text_color(c(theme::FG)).child(TextView::markdown(("wn-md", i), r.body(id).to_string())))
        });
        v_flex()
            .id("whats-new")
            .key_context("WhatsNew")
            .size_full()
            // The column sits in the middle of wide windows and fills narrow ones.
            .items_center()
            .px(theme::u(20.))
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
            .when_some(page.update.as_ref(), |v, u| v.child(self.render_update_builds(u, page, cx)))
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
            .when(page.releases.is_empty() && !page.loading && page.error.is_none(), |v| {
                v.child(div().w_full().max_w(theme::u(760.)).pt_6().text_sm().text_color(c(theme::FG_DIM)).child(if page.update.is_some() { tr("The new build has no notes beyond the list above.") } else { tr("No notes for this build.") }))
            })
            .into_any_element()
    }
}

impl Workbench {
    /// The builds between this one and the new one, and the update button.
    fn render_update_builds(&self, u: &crate::update::Release, page: &Page, cx: &mut Context<Self>) -> AnyElement {
        let builds: Vec<(String, String)> = if u.between.is_empty() { vec![(u.tag.clone(), String::new())] } else { u.between.clone() };
        let page_url = u.page.clone();
        v_flex()
            .flex_none()
            .w_full()
            .max_w(theme::u(760.))
            .mt_6()
            .gap_3()
            .child(
                v_flex()
                    .w_full()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .overflow_hidden()
                    .child(div().px_4().py_2().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).border_b_1().border_color(c(theme::BORDER)).child(tr("Builds since yours")))
                    .children(builds.into_iter().enumerate().map(|(i, (tag, first))| {
                        h_flex()
                            .px_4()
                            .py_2()
                            .flex_wrap()
                            .gap_x_3()
                            .gap_y_0p5()
                            .items_start()
                            .when(i > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                            .child(div().flex_none().w(theme::u(230.)).font_family("Cascadia Code").text_xs().text_color(c(if i == 0 { theme::ACCENT_TEXT } else { theme::FG })).child(tag))
                            .child(div().flex_1().min_w(theme::u(180.)).text_xs().text_color(c(theme::FG_MUTED)).child(first))
                    }))
                    .child(
                        h_flex()
                            .px_4()
                            .py_2()
                            .flex_wrap()
                            .gap_x_3()
                            .border_t_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_SIDE))
                            .child(div().flex_none().w(theme::u(230.)).font_family("Cascadia Code").text_xs().text_color(c(theme::FG_DIM)).child(crate::update::BUILD))
                            .child(div().flex_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("Yours, installed now"))),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("wn-update").label(tr("Update and restart")).small().primary().on_click(cx.listener(|this, _, _, cx| {
                        this.whats_new = None;
                        this.update_start(cx);
                    })))
                    .child(Button::new("wn-release").label(tr("Release page")).small().ghost().on_click(move |_, _, cx| cx.open_url(&page_url))),
            )
            .when(page.loading, |v| v.child(div().text_sm().text_color(c(theme::FG_DIM)).child(tr("Reading the new build's notes…"))))
            .when_some(page.error.clone(), |v, e| v.child(div().text_xs().text_color(c(theme::DANGER)).child(crate::trf!("Could not read the new build's notes. {why}"; why => e))))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{build_date, build_of, not_in, parse, since, Page, SOURCE};

    #[test]
    fn no_previous_build_means_no_before_line() {
        assert_eq!(Page::after_update("").from, None);
        assert_eq!(Page::after_update("BuildDev-2026-11-09-10-00-00").from.as_deref(), Some("BuildDev-2026-11-09-10-00-00"));
    }

    #[test]
    fn release_builds_are_build_names() {
        // Empty without git tags (a shallow checkout); when known, each is a build name.
        for r in parse(SOURCE) {
            if let Some(b) = build_of(&r.title_en) {
                assert!(build_date(b).is_some(), "{b}");
            }
        }
    }

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
        // Newest first (several releases can share a day).
        assert!(all.windows(2).all(|w| w[0].date >= w[1].date));
    }

    #[test]
    fn the_new_builds_notes_minus_ours() {
        let mine = parse("@release 2026-11-10\n@title en: A\n@en\na\n");
        let theirs = parse("@release 2026-11-10\n@title en: B\n@en\nb\n@release 2026-11-10\n@title en: A\n@en\na\n");
        let new = not_in(theirs, &mine);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].title_en, "B");
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
