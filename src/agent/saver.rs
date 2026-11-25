
use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;
use gpui_kit::component::{

    h_flex,
    switch::Switch,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

const REMIND_EVERY: u32 = 8;

pub const PACKS: [(&str, &str, &str); 3] = [
    ("caveman", "Caveman", "Short answers: no filler, no recap."),
    ("lean", "Lean code", "Diffs and changed lines, not whole files."),
    ("quiet", "Quiet tools", "Reads and prints only what the task needs."),
];

pub const LEVELS: [(&str, &str); 3] = [("lite", "Lite"), ("full", "Full"), ("ultra", "Ultra")];

pub const COMPACT_AT: [u8; 3] = [50, 70, 85];

const DEFAULT_WINDOW: u64 = 200_000;

#[derive(Default)]
pub struct SaverState {
    pub open: bool,
    sent: Option<(String, String)>,
    turns: u32,
    pub changed: Option<std::time::Instant>,
}

fn caveman(level: &str) -> &'static str {
    match level {
        "lite" => "- Be brief: no greetings, no restating the question, no closing summary. Full sentences are fine.",
        "ultra" => "- Telegraphic: the shortest wording that keeps the meaning (keywords, arrows, symbols). No prose around code.",
        _ => "- Terse: drop filler, hedging and pleasantries; fragments are fine.",
    }
}

fn pack_line(id: &str) -> &'static str {
    match id {
        "lean" => "- Code: show only the changed lines or a minimal diff; never re-print unchanged files or code the user already has; no comments unless asked.",
        "quiet" => "- Tools: search before reading, read line ranges instead of whole files, cap command output (quiet flags, head/tail), don't re-read files already read, don't narrate tool calls.",
        _ => "",
    }
}

pub fn guidance(packs: &[String], level: &str) -> Option<String> {
    let mut lines = Vec::new();
    for (id, ..) in PACKS {
        if packs.iter().any(|p| p == id) {
            lines.push(if id == "caveman" { caveman(level) } else { pack_line(id) });
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Response style (FluxCode token saver; keep following it until told otherwise):\n{}\n- Code, commands, file paths and error text always stay exact and complete.",
        lines.join("\n")
    ))
}

pub fn estimate(packs: &[String], level: &str) -> f32 {
    let keep = packs.iter().fold(1.0f32, |k, p| {
        let s = match p.as_str() {
            "caveman" => match level {
                "lite" => 0.12,
                "ultra" => 0.35,
                _ => 0.25,
            },
            "lean" => 0.12,
            "quiet" => 0.15,
            _ => 0.,
        };
        k * (1. - s)
    });
    1. - keep
}

impl Workbench {
    fn saver_key(&self) -> String {
        let s = &self.settings;
        if s.saver_packs.is_empty() {
            String::new()
        } else {
            format!("{}:{}", s.saver_packs.join(","), s.saver_level)
        }
    }

    pub(crate) fn agent_saver_block(&mut self) -> Option<String> {
        let session = self.agent.session.clone().unwrap_or_default();
        let key = self.saver_key();
        let st = &mut self.agent.saver;
        let same = st.sent.as_ref().is_some_and(|(s, k)| *s == session && *k == key);
        if same {
            st.turns += 1;
            if key.is_empty() || st.turns < REMIND_EVERY {
                return None;
            }
        } else if key.is_empty() {
            let had = st.sent.as_ref().is_some_and(|(s, k)| *s == session && !k.is_empty());
            if !had {
                st.sent = Some((session, key));
                return None;
            }
        }
        st.sent = Some((session, key));
        st.turns = 0;
        Some(guidance(&self.settings.saver_packs, &self.settings.saver_level).unwrap_or_else(|| "Response style: the earlier FluxCode token saver rules no longer apply. Answer normally.".into()))
    }

    fn saver_set(&mut self, pack: &str, on: bool, cx: &mut Context<Self>) {
        let packs = &mut self.settings.saver_packs;
        packs.retain(|p| p != pack);
        if on {
            packs.push(pack.into());
        }
        self.agent.saver.changed = Some(std::time::Instant::now());
        self.settings.save();
        cx.notify();
    }

    pub(crate) fn render_saver_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.settings.saver_packs.len();
        let open = self.agent.saver.open;
        h_flex()
            .id("ag-saver")
            .flex_none()
            .h(theme::u(24.))
            .px_1p5()
            .gap_1()
            .rounded(px(6.))
            .cursor_pointer()
            .text_xs()
            .text_color(c(if n > 0 { theme::GREEN } else { theme::FG_DIM }))
            .when(n > 0, |d| d.bg(c(theme::GREEN).opacity(0.12)))
            .when(open, |d| d.bg(c(theme::BG_HOVER)))
            .hover(|s| s.bg(c(theme::BG_HOVER)))
            .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Token saver")).build(w, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                let may = this.pop_may_open("saver");
                this.agent.saver.open = !this.agent.saver.open && may;
                cx.notify();
            }))
            .child(Icon::new(gpui_kit::assets::IconName::Leaf).size(theme::u(13.)))
            .when(n > 0, |d| d.child(n.to_string()))
    }

    pub(crate) fn agent_context_fill(&self) -> f32 {
        if let Some(l) = self.agent.usage.live.as_ref().filter(|l| l.size > 0 && l.used > 0) {
            return l.used as f32 / l.size as f32;
        }
        let from = self.agent.compact_from.min(self.agent.items.len());
        let chars: usize = self.agent.items[from..]
            .iter()
            .map(|i| match i {
                super::Item::User(t) | super::Item::Text(t) | super::Item::Thought(t) => t.len(),
                _ => 200,
            })
            .sum();
        chars as f32 / 4. / DEFAULT_WINDOW as f32
    }

    pub(crate) fn agent_auto_compact(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.settings.saver_compact || self.agent.compacting {
            return false;
        }
        let fill = self.agent_context_fill();
        if fill < self.settings.saver_compact_at as f32 / 100. {
            return false;
        }
        let pct = (fill * 100.).round() as u32;
        self.agent.compact_from = self.agent.items.len();
        if let Some(l) = self.agent.usage.live.as_mut() {
            l.used = 0;
        }
        let cmd = self.agent.commands.iter().find(|c| c.name == "compact" || c.name == "summarize").map(|c| format!("/{}", c.name));
        if let Some(cmd) = cmd {
            self.agent.compacting = true;
            self.agent.items.push(super::Item::Note(crate::trf!("Context {pct}% full: compacting it…"; pct => pct), false));
            self.agent_send_probe(&cmd);
        } else {
            self.agent.items.push(super::Item::Note(crate::trf!("Context {pct}% full: the chat goes on in a fresh session that carries its recent messages."; pct => pct), false));
            self.agent.carry = Some(super::transcript(&self.agent.items));
            self.agent_save();
            self.agent_stop(cx);
            self.agent_start(cx);
        }
        true
    }

    pub(crate) fn agent_compact_done(&mut self, ok: bool) {
        let text = if ok { crate::i18n::tr("Context compacted: the conversation continues with a summary, using fewer tokens.").to_string() } else { crate::i18n::tr("Couldn't compact the context this time; it will try again after the next answer.").to_string() };
        if let Some(super::Item::Note(t, err)) = self.agent.items.iter_mut().rev().find(|i| matches!(i, super::Item::Note(..))) {
            *t = text;
            *err = !ok;
        }
    }

    pub(crate) fn render_saver_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.agent.saver.open {
            return None;
        }
        use gpui_kit::assets::IconName as Ico;
        let s = &self.settings;
        let on = |id: &str| s.saver_packs.iter().any(|p| p == id);
        let pct = (estimate(&s.saver_packs, &s.saver_level) * 100.).round() as u32;
        let (panel_top, composer_top) = self.agent.geom.get();
        let room = if composer_top > panel_top + 120. { composer_top - panel_top - 24. } else { 420. };

        let row = |key: &'static str, icon: Ico, title: &'static str, tip: SharedString, checked: bool, cx: &mut Context<Self>, set: fn(&mut Workbench, bool, &mut Context<Workbench>)| {
            h_flex()
                .id(SharedString::from(format!("sv-row-{key}")))
                .h(theme::u(30.))
                .gap_2()
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                .child(Icon::new(icon).flex_none().size(theme::u(14.)).text_color(c(if checked { theme::FG_STRONG } else { theme::FG_DIM })))
                .child(div().flex_1().min_w_0().truncate().text_sm().text_color(c(if checked { theme::FG_STRONG } else { theme::FG_MUTED })).child(tr(title)))
                .child(Switch::new(SharedString::from(format!("sv-{key}"))).checked(checked).small().on_click(cx.listener(move |this, v: &bool, _, cx| set(this, *v, cx))))
        };
        let seg = |id: &'static str, opts: Vec<(SharedString, bool, Box<dyn Fn(&mut Workbench)>)>, cx: &mut Context<Self>| {
            let mut h = h_flex().flex_1().min_w_0().p(px(2.)).gap(px(2.)).rounded(px(4.)).bg(c(theme::FG_DIM).opacity(0.12));
            for (k, (label, picked, f)) in opts.into_iter().enumerate() {
                h = h.child(
                    div()
                        .id((id, k))
                        .flex_1()
                        .h(theme::u(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .text_xs()
                        .cursor_pointer()
                        .text_color(c(if picked { theme::FG_STRONG } else { theme::FG_MUTED }))
                        .when(picked, |d| d.bg(theme::glass_pop_fill()).border_1().border_color(theme::glass_pop_edge()).shadow_sm())
                        .when(!picked, |d| d.hover(|s| s.text_color(c(theme::FG))))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            f(this);
                            this.agent.saver.changed = Some(std::time::Instant::now());
                            this.settings.save();
                            cx.notify();
                        }))
                        .child(label),
                );
            }
            h
        };
        let indent = theme::u(22.);
        let mut body = v_flex().gap_0p5();

        let level_tip: SharedString = match s.saver_level.as_str() {
            "lite" => tr("Short answers, Lite: brief, in full sentences; no greetings, no recap."),
            "ultra" => tr("Short answers, Ultra: telegraphic, keywords and symbols; no prose around code."),
            _ => tr("Short answers, Full: terse; no filler or pleasantries, fragments are fine."),
        }
        .into();
        body = body.child(row("caveman", Ico::TextAlignStart, "Short answers", level_tip, on("caveman"), cx, |w, v, cx| w.saver_set("caveman", v, cx)));
        if on("caveman") {
            let opts = LEVELS
                .iter()
                .map(|&(lv, label)| {
                    let f: Box<dyn Fn(&mut Workbench)> = Box::new(move |w| w.settings.saver_level = lv.into());
                    (SharedString::from(tr(label)), s.saver_level == lv, f)
                })
                .collect();
            body = body.child(h_flex().pl(indent).pb_1().child(seg("sv-lv", opts, cx)));
        }
        body = body
            .child(row("lean", Ico::Code, "Lean code", tr("Shows changed lines or a small diff, never whole files again.").into(), on("lean"), cx, |w, v, cx| w.saver_set("lean", v, cx)))
            .child(row("quiet", Ico::SquareTerminal, "Quiet tools", tr("Reads only the lines it needs and keeps command output short.").into(), on("quiet"), cx, |w, v, cx| w.saver_set("quiet", v, cx)));

        let fill = self.agent_context_fill().clamp(0., 1.);
        let at = s.saver_compact_at;
        let meter = if fill * 100. >= at as f32 { theme::WARN } else { theme::GREEN };
        body = body.child(div().my_1().h(px(1.)).bg(theme::glass_pop_edge())).child(row(
            "compact",
            Ico::Shrink,
            "Auto compact",
            crate::trf!("When the context is {at}% full, the chat is summarized so later messages cost less."; at => at).into(),
            s.saver_compact,
            cx,
            |w, v, cx| {
                w.settings.saver_compact = v;
                w.settings.save();
                cx.notify();
            },
        ));
        if s.saver_compact {
            let opts = COMPACT_AT
                .iter()
                .map(|&p| {
                    let f: Box<dyn Fn(&mut Workbench)> = Box::new(move |w| w.settings.saver_compact_at = p);
                    (SharedString::from(format!("{p}%")), at == p, f)
                })
                .collect();
            body = body.child(
                v_flex()
                    .pl(indent)
                    .gap_1()
                    .child(seg("sv-at", opts, cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .relative()
                                    .flex_1()
                                    .h(px(3.))
                                    .rounded_full()
                                    .bg(c(theme::FG_DIM).opacity(0.18))
                                    .child(div().absolute().left_0().top_0().h_full().w(relative(fill.max(0.02))).rounded_full().bg(c(meter)))
                                    .child(div().absolute().top(px(-2.)).h(px(7.)).w(px(1.5)).left(relative(at as f32 / 100.)).bg(c(theme::FG_MUTED))),
                            )
                            .child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(crate::trf!("{n}% now"; n => (fill * 100.).round() as u32))),
                    ),
            );
        }
        let any_on = !s.saver_packs.is_empty() || s.saver_compact;
        Some(
            v_flex()
                .id("ag-saver-panel")
                .occlude()
                .w(theme::u(272.))
                .max_h(px(room.max(160.)))
                .p_3()
                .gap_2()
                .glass_pop()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.pop_closed_outside("saver");
                    this.agent.saver.open = false;
                    cx.notify();
                }))
                .child(
                    h_flex()
                        .gap_2()
                        .child(Icon::new(Ico::Leaf).size(theme::u(14.)).text_color(c(theme::GREEN)))
                        .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Token saver")))
                        .when(pct > 0, |h| {
                            h.child(
                                div()
                                    .id("sv-pct")
                                    .px_1p5()
                                    .rounded_full()
                                    .bg(c(theme::GREEN).opacity(0.14))
                                    .text_xs()
                                    .text_color(c(theme::GREEN))
                                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("About this many fewer output tokens (an estimate). Guidance goes out with your next message, then every few turns.")).build(w, cx))
                                    .child(format!("−{pct}%")),
                            )
                        }),
                )
                .child(v_flex().id("sv-body").flex_1().min_h_0().overflow_y_scroll().child(body))
                .when(any_on, |v| {
                    v.child(
                        h_flex()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(div().flex_1().min_w_0().truncate().child(if self.agent.saver.changed.is_some() { tr("Applies from your next message") } else { "" }))
                            .child(
                                div()
                                    .id("sv-off")
                                    .px_1()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(c(theme::FG_STRONG)))
                                    .child(tr("Turn all off"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.saver_packs.clear();
                                        this.settings.saver_compact = false;
                                        this.agent.saver.changed = Some(std::time::Instant::now());
                                        this.settings.save();
                                        cx.notify();
                                    })),
                            ),
                    )
                })
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{estimate, guidance};

    #[test]
    fn guidance_lists_only_the_packs_that_are_on() {
        assert!(guidance(&[], "full").is_none());
        let g = guidance(&["quiet".into(), "caveman".into()], "ultra").unwrap();
        assert!(g.contains("Telegraphic") && g.contains("Tools:") && !g.contains("Code: show"));
        assert!(g.find("Telegraphic").unwrap() < g.find("Tools:").unwrap());
    }

    #[test]
    fn savings_combine_without_passing_100_percent() {
        assert_eq!(estimate(&[], "full"), 0.);
        let all = estimate(&["caveman".into(), "lean".into(), "quiet".into()], "ultra");
        assert!(all > 0.35 && all < 0.6, "{all}");
    }
}
