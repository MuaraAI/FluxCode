use super::{Item, Status};
use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex, v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;

#[derive(Default)]
pub struct LimitState {
    pub hit: Option<(String, Option<i64>)>,
    pub last: Option<(String, Vec<PathBuf>)>,
    pub gen: u64,
}

pub fn is_limit_text(text: &str) -> bool {
    let low = text.to_lowercase();
    let short = low.len() < 600;
    short
        && (low.contains("hit your") && low.contains("limit")
            || low.contains("usage limit")
            || low.contains("rate limit")
            || low.contains("limit reached")
            || low.contains("quota") && low.contains("exceed")
            || low.contains("429"))
}

pub fn reset_clock(text: &str) -> Option<(u32, u32)> {
    let low = text.to_lowercase();
    let rest = &low[low.find("reset")?..];
    let mut it = rest
        .char_indices()
        .skip_while(|(_, ch)| !ch.is_ascii_digit());
    let (start, _) = it.next()?;
    let tail = &rest[start..];
    let num: String = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == ':' || *ch == '.')
        .collect();
    let after = tail[num.len()..].trim_start();
    let (h, m) = match num.split_once([':', '.']) {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (num.parse::<u32>().ok()?, 0),
    };
    let h = match (after.starts_with("am"), after.starts_with("pm")) {
        (true, _) if h == 12 => 0,
        (_, true) if h < 12 => h + 12,
        _ => h,
    };
    (h < 24 && m < 60).then_some((h, m))
}

fn next_local(h: u32, m: u32) -> Option<i64> {
    use chrono::{Local, TimeZone};
    let now = Local::now();
    let today = now.date_naive().and_hms_opt(h, m, 0)?;
    let at = Local.from_local_datetime(&today).earliest()?;
    let at = if at <= now {
        at + chrono::Duration::days(1)
    } else {
        at
    };
    Some(at.timestamp())
}

impl Workbench {
    pub(crate) fn agent_check_limit(&mut self, text: &str, cx: &mut Context<Self>) {
        if !is_limit_text(text) {
            return;
        }
        let now = chrono::Utc::now().timestamp();
        let full = self
            .agent
            .usage
            .live
            .iter()
            .flat_map(|l| &l.windows)
            .filter(|w| w.used >= 0.99 && w.resets > now)
            .map(|w| w.resets)
            .max();
        let resets = full.or_else(|| reset_clock(text).and_then(|(h, m)| next_local(h, m)));
        let line = text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or(text)
            .trim()
            .to_string();
        self.agent.limit.hit = Some((line, resets));
        self.agent.limit.gen += 1;
        self.agent_schedule_resume(cx);
        cx.notify();
    }

    fn agent_schedule_resume(&mut self, cx: &mut Context<Self>) {
        let Some((_, Some(resets))) = self.agent.limit.hit else {
            return;
        };
        if !self.settings.resume_at_reset {
            return;
        }
        let gen = self.agent.limit.gen;
        let key = self.agent.key;
        let wait = (resets - chrono::Utc::now().timestamp()).max(0) as u64 + 20;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(wait))
                .await;
            _ = this.update(cx, |this, cx| {
                this.agent_with_run(key, cx, |this, cx| {
                    if this.agent.limit.gen == gen && this.agent.limit.hit.is_some() {
                        this.agent_continue_after_limit(cx);
                    }
                });
            });
        })
        .detach();
    }

    pub(crate) fn agent_continue_after_limit(&mut self, cx: &mut Context<Self>) {
        self.agent.limit.hit = None;
        self.agent.limit.gen += 1;
        let Some((text, files)) = self.agent.limit.last.clone() else {
            return cx.notify();
        };
        if self.agent.status != Status::Ready || self.agent.session.is_none() {
            return cx.notify();
        }
        self.agent.items.push(Item::Note(
            tr("Continuing after the usage limit.").into(),
            false,
        ));
        self.agent_prompt(format!("(Your previous answer stopped at a usage limit. Continue from where you stopped.)\n\n{text}"), text, files, cx);
        cx.notify();
    }

    pub(crate) fn render_limit_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, resets) = self.agent.limit.hit.as_ref()?;
        let now = chrono::Utc::now().timestamp();
        let when = resets.map(|r| {
            let local = chrono::DateTime::from_timestamp(r, 0)
                .map(|t| t.with_timezone(&chrono::Local).format("%H.%M").to_string())
                .unwrap_or_default();
            match super::usage::until(r, now) {
                Some(left) => crate::trf!("Resets {0} (in {1})"; 0 => local, 1 => left),
                None => crate::trf!("Resets {0}"; 0 => local),
            }
        });
        let resume = self.settings.resume_at_reset;
        let notice = h_flex()
            .flex_1()
            .min_w(theme::u(180.))
            .gap_2()
            .items_start()
            .child(
                Icon::new(IconName::TriangleAlert)
                    .flex_none()
                    .mt(px(2.))
                    .size(theme::u(15.))
                    .text_color(c(theme::WARN)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_color(c(theme::FG_STRONG))
                            .font_weight(FontWeight::MEDIUM)
                            .child(tr("Usage limit reached")),
                    )
                    .children(
                        when.map(|w| div().text_xs().text_color(c(theme::FG_MUTED)).child(w)),
                    ),
            );
        let controls = h_flex()
            .flex_none()
            .ml_auto()
            .gap_2()
            .when(resets.is_some(), |h| {
                h.child(
                    Checkbox::new("ag-resume-reset")
                        .label(tr("Resume at reset"))
                        .checked(resume)
                        .small()
                        .on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.settings.resume_at_reset = *on;
                            this.settings.save();
                            this.agent.limit.gen += 1;
                            this.agent_schedule_resume(cx);
                            cx.notify();
                        })),
                )
            })
            .child(
                Button::new("ag-limit-continue")
                    .label(tr("Continue now"))
                    .small()
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.agent_continue_after_limit(cx))),
            )
            .child(
                Button::new("ag-limit-close")
                    .icon(IconName::Close)
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.limit.hit = None;
                        this.agent.limit.gen += 1;
                        cx.notify();
                    })),
            );
        Some(
            h_flex()
                .flex_wrap()
                .mx_3()
                .mb_2()
                .px_3()
                .py_2()
                .gap_x_3()
                .gap_y_2()
                .glass()
                .border_color(c(theme::WARN).opacity(0.45))
                .bg(c(theme::WARN).opacity(0.12))
                .text_sm()
                .child(notice)
                .child(controls)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{is_limit_text, reset_clock};

    #[test]
    fn limit_notices_are_told_apart_from_answers() {
        assert!(is_limit_text(
            "You've hit your weekly limit · resets 12pm (Asia/Jakarta)"
        ));
        assert!(is_limit_text("Error: rate limit exceeded (429)"));
        assert!(!is_limit_text(
            "I set the loop limit to 10 and the tests pass."
        ));
    }

    #[test]
    fn reset_times_are_read_in_24_hours() {
        assert_eq!(
            reset_clock("You've hit your weekly limit · resets 12pm (Asia/Jakarta)"),
            Some((12, 0))
        );
        assert_eq!(reset_clock("resets 12am"), Some((0, 0)));
        assert_eq!(reset_clock("Resets 10:40 PM GMT+7"), Some((22, 40)));
        assert_eq!(reset_clock("Resets at 3am"), Some((3, 0)));
        assert_eq!(reset_clock("Resets 12.00"), Some((12, 0)));
        assert_eq!(reset_clock("no time here"), None);
    }
}
