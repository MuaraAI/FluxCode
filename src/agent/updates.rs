
use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::time::Duration;

const FIRST: Duration = Duration::from_secs(15);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

pub(crate) enum Step {
    Ready,
    Running(crate::acp::Tracker),
    Done,
    Failed(String),
}

pub(crate) struct Pending {
    pub id: String,
    pub from: String,
    pub to: String,
    pub step: Step,
}

pub(crate) fn newer(a: &str, b: &str) -> bool {
    let nums = |v: &str| -> Vec<u64> { v.trim_start_matches('v').split(|ch: char| !ch.is_ascii_digit()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect() };
    let (x, y) = (nums(a), nums(b));
    if x.is_empty() || y.is_empty() {
        return a != b;
    }
    x > y
}

pub(crate) fn available(reg: &[crate::acp::Entry]) -> Vec<(String, String, String)> {
    crate::acp::installed()
        .into_iter()
        .filter(|id| crate::acp::managed(id))
        .filter_map(|id| {
            let have = crate::acp::locate(&id)?.version;
            let e = reg.iter().find(|e| e.id == id)?;
            (!have.is_empty() && e.method().is_some() && newer(&e.version, &have)).then(|| (id, have, e.version.clone()))
        })
        .collect()
}

fn v(s: &str) -> String {
    if s.starts_with('v') { s.to_string() } else { format!("v{s}") }
}

impl Workbench {
    pub(crate) fn start_agent_update_checks(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut wait = FIRST;
            loop {
                cx.background_executor().timer(wait).await;
                wait = EVERY;
                let found = cx
                    .background_spawn(async move {
                        if !crate::acp::installed().iter().any(|id| crate::acp::managed(id)) {
                            return Vec::new();
                        }
                        if crate::acp::registry_stale() {
                            let _ = crate::acp::refresh_registry();
                        }
                        crate::acp::fetch_icons();
                        available(&crate::acp::registry())
                    })
                    .await;
                if this.update(cx, |this, cx| this.agent_updates_found(found, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn agent_updates_found(&mut self, found: Vec<(String, String, String)>, cx: &mut Context<Self>) {
        let list = &mut self.agent.updates;
        let mut new = false;
        for (id, from, to) in found {
            match list.iter_mut().find(|p| p.id == id) {
                Some(p) if matches!(p.step, Step::Running(_) | Step::Done) => {}
                Some(p) => {
                    new |= p.to != to;
                    (p.from, p.to, p.step) = (from, to, Step::Ready);
                }
                None => {
                    new = true;
                    list.push(Pending { id, from, to, step: Step::Ready });
                }
            }
        }
        if new {
            self.agent.updates_hidden = false;
        }
        cx.notify();
    }

    pub(crate) fn agent_update(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(p) = self.agent.updates.iter_mut().find(|p| p.id == id) else { return };
        if matches!(p.step, Step::Running(_)) {
            return;
        }
        let tracker = crate::acp::Tracker::default();
        p.step = Step::Running(tracker.clone());
        cx.notify();
        let id2 = id.clone();
        let task = cx.background_spawn(async move {
            let entry = crate::acp::registry().iter().find(|e| e.id == id2).cloned().ok_or_else(|| tr("This agent is no longer in the ACP registry.").to_string())?;
            let res = crate::acp::install(&entry, &tracker);
            tracker.phase(if res.is_ok() { crate::acp::Phase::Done } else { crate::acp::Phase::Failed });
            res.map(|_| ())
        });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            let ok = res.is_ok();
            _ = this.update(cx, |this, cx| {
                if let Some(p) = this.agent.updates.iter_mut().find(|p| p.id == id) {
                    p.step = match res {
                        Ok(()) => Step::Done,
                        Err(e) => Step::Failed(e),
                    };
                }
                cx.notify();
            });
            if ok {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                _ = this.update(cx, |this, cx| {
                    this.agent.updates.retain(|p| !(p.id == id && matches!(p.step, Step::Done)));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn agent_update_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self.agent.updates.iter().filter(|p| matches!(p.step, Step::Ready | Step::Failed(_))).map(|p| p.id.clone()).collect();
        for id in ids {
            self.agent_update(id, cx);
        }
    }

    pub(crate) fn render_agent_updates(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let list = &self.agent.updates;
        if list.is_empty() || self.agent.updates_hidden {
            return None;
        }
        if list.iter().any(|p| matches!(p.step, Step::Running(_))) {
            window.request_animation_frame();
        }
        let ready = list.iter().filter(|p| matches!(p.step, Step::Ready | Step::Failed(_))).count();
        let busy = list.iter().any(|p| matches!(p.step, Step::Running(_)));
        let head = h_flex()
            .pl_1()
            .gap_1()
            .items_center()
            .child(div().flex_1().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_MUTED)).child(tr("Agent updates")))
            .when(ready > 1, |h| {
                h.child(Button::new("ag-upd-all").label(crate::trf!("Update all ({0})"; 0 => ready)).xsmall().primary().on_click(cx.listener(|this, _, _, cx| this.agent_update_all(cx))))
            })
            .child(
                Button::new("ag-upd-hide")
                    .icon(IconName::Close)
                    .xsmall()
                    .ghost()
                    .tooltip(if busy { tr("Hide (the update goes on)") } else { tr("Later") })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.agent.updates_hidden = true;
                        this.agent.updates.retain(|p| !matches!(p.step, Step::Ready | Step::Failed(_)));
                        cx.notify();
                    })),
            );
        let cards = list.iter().enumerate().map(|(ix, p)| {
            let name = crate::acp::display_name(&p.id);
            let (title, accent) = match &p.step {
                Step::Ready => (crate::trf!("{0} has an update"; 0 => name), false),
                Step::Running(_) => (crate::trf!("Updating {0}"; 0 => name), true),
                Step::Done => (crate::trf!("{0} is up to date"; 0 => name), false),
                Step::Failed(_) => (crate::trf!("{0} was not updated"; 0 => name), false),
            };
            let versions = div().text_xs().text_color(c(theme::FG_MUTED)).child(format!("{} → {}", v(&p.from), v(&p.to)));
            let mut card = v_flex()
                .w_full()
                .px_3()
                .py_2()
                .gap_1p5()
                .rounded(theme::u(10.))
                .bg(c(theme::BG_HOVER).opacity(0.6))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().flex_none().size(theme::u(18.)).flex().items_center().justify_center().child(crate::agent::agent_glyph(&p.id, 16.)))
                        .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
                        .when(matches!(p.step, Step::Ready), |h| {
                            let id = p.id.clone();
                            h.child(Button::new(("ag-upd", ix)).label(tr("Update")).xsmall().primary().on_click(cx.listener(move |this, _, _, cx| this.agent_update(id.clone(), cx))))
                        })
                        .when(matches!(p.step, Step::Failed(_)), |h| {
                            let id = p.id.clone();
                            h.child(Button::new(("ag-upd-retry", ix)).label(tr("Try again")).xsmall().on_click(cx.listener(move |this, _, _, cx| this.agent_update(id.clone(), cx))))
                        }),
                )
                .child(div().pl(theme::u(26.)).child(versions));
            match &p.step {
                Step::Running(t) => {
                    let snap = t.get();
                    let frac = snap.fraction().filter(|_| matches!(snap.phase, crate::acp::Phase::Download | crate::acp::Phase::Unpack));
                    let track = div().relative().flex_1().h(px(4.)).rounded_full().bg(c(theme::BG_HOVER)).overflow_hidden().child(match frac {
                        Some(f) => div().absolute().left_0().top_0().bottom_0().rounded_full().bg(c(if accent { theme::ACCENT } else { theme::FG })).w(relative(f)).into_any_element(),
                        None => div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .w(relative(0.3))
                            .rounded_full()
                            .bg(c(theme::ACCENT))
                            .with_animation(SharedString::from(format!("ag-upd-slide-{ix}")), Animation::new(Duration::from_millis(1300)).repeat(), |d, t| d.left(relative(-0.3 + 1.3 * t)))
                            .into_any_element(),
                    });
                    let pct = frac.map(|f| format!("{}%", (f * 100.).round() as u32)).unwrap_or_else(|| tr(snap.phase.label()).to_string());
                    card = card.child(
                        h_flex()
                            .pl(theme::u(26.))
                            .gap_3()
                            .items_center()
                            .child(track)
                            .child(div().flex_none().min_w(theme::u(36.)).text_right().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG)).child(pct)),
                    );
                }
                Step::Done => {
                    card = card.child(div().pl(theme::u(26.)).text_xs().text_color(c(theme::GREEN)).child(tr("The next chat uses the new version.")));
                }
                Step::Failed(e) => {
                    card = card.child(div().pl(theme::u(26.)).text_xs().text_color(c(theme::DANGER)).line_clamp(3).child(e.clone()));
                }
                Step::Ready => {}
            }
            card.into_any_element()
        });
        Some(
            div()
                .absolute()
                .top(theme::u(44.))
                .right(theme::u(12.))
                .w(theme::u(340.))
                .child(deferred(v_flex().id("ag-updates").occlude().p_2().gap_2().glass_pop().child(head).children(cards)))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn compares_versions_as_numbers() {
        assert!(newer("2.3.2", "2.3.0"));
        assert!(newer("2.10.0", "2.9.3"));
        assert!(newer("v1.0.113", "1.0.112"));
        assert!(!newer("2.3.0", "2.3.0"));
        assert!(!newer("2.3.0", "2.3.2"));
        assert!(newer("nightly-b", "nightly-a"));
    }
}
