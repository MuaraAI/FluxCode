use crate::files;
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    diff::{
        Diff, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffMode, DiffOverviewMark,
        DiffParseOptions, DiffState,
    },
    h_flex, v_flex, Icon,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    File,
    Hunk,
    Add,
    Del,
    Same,
}

pub struct Line {
    pub kind: Kind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: SharedString,
}

pub struct DiffDoc {
    pub lines: Rc<Vec<Line>>,
    pub added: usize,
    pub removed: usize,
    pub source: bool,
    pub name: SharedString,
    pub badge: Option<SharedString>,
    pub target: Option<PathBuf>,
    pub state: Option<Entity<DiffState>>,
    pub multi_file: bool,
    pub empty_note: Option<SharedString>,
    pub virtual_tab: bool,
}

fn hunk_starts(h: &str) -> Option<(u32, u32)> {
    let mut it = h.split_whitespace().skip(1);
    let num = |s: Option<&str>, sign: char| -> Option<u32> {
        s?.strip_prefix(sign)?.split(',').next()?.parse().ok()
    };
    Some((num(it.next(), '-')?, num(it.next(), '+')?))
}

pub fn parse(text: &str) -> DiffDoc {
    let (mut old, mut new) = (0u32, 0u32);
    let (mut added, mut removed) = (0, 0);
    let mut in_hunk = false;
    let lines = text
        .lines()
        .map(|l| {
            let (kind, o, n) = if l.starts_with("@@") {
                if let Some((a, b)) = hunk_starts(l) {
                    (old, new) = (a, b);
                }
                in_hunk = true;
                (Kind::Hunk, None, None)
            } else if l.starts_with("diff ")
                || l.starts_with("index ")
                || (l.starts_with("--- ") || l.starts_with("+++ ")) && !in_hunk
            {
                in_hunk = false;
                (Kind::File, None, None)
            } else if l.starts_with('+') {
                added += 1;
                new += 1;
                (Kind::Add, None, Some(new - 1))
            } else if l.starts_with('-') {
                removed += 1;
                old += 1;
                (Kind::Del, Some(old - 1), None)
            } else if in_hunk && l.starts_with(' ') {
                old += 1;
                new += 1;
                (Kind::Same, Some(old - 1), Some(new - 1))
            } else {
                (Kind::Same, None, None)
            };
            if l.starts_with("diff ") {
                in_hunk = false;
            }
            Line {
                kind,
                old: o,
                new: n,
                text: l.to_string().into(),
            }
        })
        .filter(|l: &Line| l.kind != Kind::File)
        .collect::<Vec<_>>();
    let name = text
        .lines()
        .find_map(|l| l.strip_prefix("+++ ").or_else(|| l.strip_prefix("--- ")))
        .map(|n| n.trim().trim_start_matches("b/").trim_start_matches("a/").to_string())
        .unwrap_or_default();
    DiffDoc {
        lines: Rc::new(lines),
        added,
        removed,
        source: false,
        name: name.into(),
        badge: None,
        target: None,
        state: None,
        multi_file: false,
        empty_note: None,
        virtual_tab: false,
    }
}

pub fn parse_with_state(text: &str, cx: &mut Context<Workbench>) -> DiffDoc {
    let files = parse_diff_files(text, None);
    if !files.is_empty() {
        let added = files.iter().map(DiffFile::additions).sum();
        let removed = files.iter().map(DiffFile::deletions).sum();
        let multi_file = files.len() > 1;
        let name = files[0].path().clone();
        let state = cx.new(|cx| {
            DiffState::new(files, cx)
                .with_mode(DiffMode::Split)
                .with_context_lines(None)
        });
        return DiffDoc {
            lines: Rc::new(Vec::new()),
            added,
            removed,
            source: false,
            name,
            badge: None,
            target: None,
            state: Some(state),
            multi_file,
            empty_note: None,
            virtual_tab: false,
        };
    }
    let mut doc = parse(text);
    if text.trim().is_empty() {
        doc.empty_note =
            Some(tr("No textual changes to compare (binary file or unchanged content).").into());
    }
    doc
}

pub fn parse_diff_files(patch: &str, lang_hint: Option<&str>) -> Vec<DiffFile> {
    let opts = DiffParseOptions::new().truncated_hunks(true);
    let Ok(files) = DiffFile::parse_with(patch, opts) else {
        return Vec::new();
    };
    files
        .into_iter()
        .map(|file| {
            let lang = lang_hint.or_else(|| {
                let p = Path::new(file.path().as_ref());
                let (l, _) = files::language_for(p, "");
                (l != "text").then_some(l)
            });
            match lang {
                Some(l) => file.with_language(l),
                None => file,
            }
        })
        .collect()
}

pub fn is_diff(lang: &str) -> bool {
    lang == "diff"
}

pub(crate) fn jump_row_for_y(y: f32, top: f32, height: f32, total: usize) -> usize {
    if total <= 1 {
        return 0;
    }
    let frac = ((y - top) / height.max(1.)).clamp(0., 1.);
    ((frac * (total - 1) as f32).round() as usize).min(total - 1)
}

/// Coalesces consecutive overview marks per lane (`0`=conflict, `1`=removed, `2`=added) in
/// pixel space with zero heap allocation and hoisted reciprocal scaling.
pub(crate) fn emit_coalesced_marks(
    marks: &[DiffOverviewMark],
    total_rows: usize,
    y0: f32,
    h: f32,
    mut emit: impl FnMut(u8, f32, f32),
) {
    if total_rows == 0 || h <= 1. {
        return;
    }
    let scale = h / total_rows as f32;
    let y_max = y0 + h;
    let mut active: [Option<(f32, f32)>; 3] = [None; 3];
    let mut push_lane = |lane: usize, sy: f32, ey: f32| {
        if let Some((cs, ce)) = active[lane] {
            if sy <= ce {
                active[lane] = Some((cs, ce.max(ey)));
                return;
            }
            emit(lane as u8, cs, (ce - cs).max(3.));
        }
        active[lane] = Some((sy, ey));
    };

    for m in marks {
        let mh = ((m.end_row.saturating_sub(m.start_row)) as f32 * scale).max(3.);
        let sy = (y0 + m.start_row as f32 * scale).min((y_max - mh).max(y0));
        let ey = (sy + mh).min(y_max);
        if m.conflict {
            push_lane(0, sy, ey);
        } else {
            if m.removed {
                push_lane(1, sy, ey);
            }
            if m.added {
                push_lane(2, sy, ey);
            }
        }
    }
    for (lane, span) in active.into_iter().enumerate() {
        if let Some((sy, ey)) = span {
            emit(lane as u8, sy, (ey - sy).max(3.));
        }
    }
}

pub(crate) type DiffRulerDrag = (Entity<DiffState>, Rc<Cell<(f32, f32)>>);

impl Workbench {
    pub(crate) fn diff_drag_ruler(&mut self, y: f32, cx: &mut Context<Self>) -> bool {
        let Some((st, rect)) = self.diff_ruler_drag.as_ref() else {
            return false;
        };
        let (top, h) = rect.get();
        if h <= 1. {
            return false;
        }
        let total = st.read(cx).row_count();
        let row = jump_row_for_y(y, top, h, total);
        st.update(cx, |s, cx| s.scroll_to_row(row, cx))
    }

    pub(crate) fn render_diff_doc(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.docs.get(ix).and_then(|d| d.diff.as_ref()) else {
            return div().into_any_element();
        };
        let (green, red) = (c(theme::GREEN), c(theme::DANGER));
        let target = d.target.clone();
        let virtual_tab = d.virtual_tab;
        let state = d.state.clone();
        let (is_split, is_full) = match &state {
            Some(st) => {
                let s = st.read(cx);
                (s.mode() == DiffMode::Split, s.context_lines().is_none())
            }
            None => (true, true),
        };

        let btn = |id: &'static str, label: &'static str, active: bool| {
            h_flex()
                .id((id, ix))
                .flex_none()
                .h(theme::u(22.))
                .px_2()
                .gap_1()
                .items_center()
                .rounded(px(4.))
                .cursor_pointer()
                .when(active, |d| {
                    d.bg(c(theme::SELECT)).text_color(c(theme::FG_STRONG))
                })
                .when(!active, |d| d.text_color(c(theme::FG_MUTED)))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .child(tr(label))
        };

        let icon_btn = |id: &'static str, icon: Ico, tip: &'static str| {
            div()
                .id((id, ix))
                .flex_none()
                .size(theme::u(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(c(theme::FG_MUTED))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .tooltip(move |w, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tr(tip)).build(w, cx)
                })
                .child(Icon::new(icon).size(theme::u(13.)))
        };

        let mut bar = h_flex()
            .flex_none()
            .min_w_0()
            .overflow_hidden()
            .h(theme::u(26.))
            .px_3()
            .gap_1()
            .items_center()
            .text_xs()
            .bg(c(theme::BG_EDITOR))
            .border_b_1()
            .border_color(c(theme::BORDER))
            .child(
                Icon::new(Ico::FileDiff)
                    .flex_none()
                    .size(theme::u(13.))
                    .text_color(c(theme::FG_MUTED)),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(theme::FG_STRONG))
                    .child(d.name.clone()),
            )
            .when_some(d.badge.clone(), |h, b| {
                h.child(div().flex_none().text_color(c(theme::FG_MUTED)).child(b))
            })
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(green)
                    .child(format!("+{}", d.added)),
            )
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(red)
                    .child(format!("-{}", d.removed)),
            )
            .child(div().flex_1());

        if let Some(st) = &state {
            let st_prev = st.clone();
            let st_next = st.clone();
            let st_mode = st.clone();
            let st_ctx = st.clone();
            bar = bar
                .child(
                    icon_btn("diff-prev", Ico::ArrowUp, "Previous change").on_click(
                        cx.listener(move |_, _, _, cx| {
                            st_prev.update(cx, |s, cx| s.previous_change(cx));
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    icon_btn("diff-next", Ico::ArrowDown, "Next change").on_click(
                        cx.listener(move |_, _, _, cx| {
                            st_next.update(cx, |s, cx| s.next_change(cx));
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    btn("diff-mode", if is_split { "Inline" } else { "Side by side" }, false)
                        .on_click(cx.listener(move |_, _, _, cx| {
                            st_mode.update(cx, |s, cx| {
                                let next = if s.mode() == DiffMode::Split {
                                    DiffMode::Unified
                                } else {
                                    DiffMode::Split
                                };
                                s.set_mode(next, cx);
                            });
                            cx.notify();
                        })),
                )
                .child(
                    btn("diff-ctx", if is_full { "Full file" } else { "Changes only" }, false)
                        .on_click(cx.listener(move |_, _, _, cx| {
                            st_ctx.update(cx, |s, cx| {
                                let next = if s.context_lines().is_none() {
                                    Some(3)
                                } else {
                                    None
                                };
                                s.set_context_lines(next, cx);
                            });
                            cx.notify();
                        })),
                );
        }

        if let Some(path) = target.clone() {
            bar = bar.child(icon_btn("diff-open", Ico::FileCode, "Open file").on_click(
                cx.listener(move |this, _, w, cx| {
                    this.agent.user_nav = Some(std::time::Instant::now());
                    this.open_file(path.clone(), w, cx);
                }),
            ));
        }

        if !virtual_tab {
            bar = bar.child(btn("diff-source", "Edit as text", false).on_click(
                cx.listener(move |this, _, w, cx| {
                    if let Some(d) = this.docs.get_mut(ix).and_then(|d| d.diff.as_mut()) {
                        d.source = true;
                    }
                    this.activate(ix, w, cx);
                }),
            ));
        }

        let content: AnyElement = if let Some(st) = &state {
            let ruler_bounds: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0., 0.)));
            let warn = c(theme::WARN);
            let vp_fill = c(theme::FG_MUTED).opacity(0.2);
            let vp_edge = c(theme::FG_MUTED).opacity(0.35);
            let bounds_paint = ruler_bounds.clone();
            let st_paint = st.clone();
            let ruler_canvas = canvas(
                move |bounds, _, _| {
                    bounds_paint.set((f32::from(bounds.origin.y), f32::from(bounds.size.height)));
                },
                move |bounds, _, window, cx| {
                    let s = st_paint.read(cx);
                    let total_rows = s.row_count();
                    if total_rows == 0 {
                        return;
                    }
                    let marks = s.overview_marks();
                    let (vp_top, vp_vis) = s.viewport_rows();
                    let x0 = f32::from(bounds.origin.x);
                    let y0 = f32::from(bounds.origin.y);
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    if h <= 1. {
                        return;
                    }
                    let denom = total_rows as f32;
                    let half = ((w - 3.) * 0.5).max(2.);
                    emit_coalesced_marks(marks, total_rows, y0, h, |lane, my, mh| match lane {
                        0 => window.paint_quad(quad(
                            Bounds::new(point(px(x0 + 1.5), px(my)), size(px((w - 3.).max(2.)), px(mh))),
                            px(1.),
                            warn.opacity(0.92),
                            px(0.),
                            transparent_black(),
                            BorderStyle::default(),
                        )),
                        1 => window.paint_quad(quad(
                            Bounds::new(point(px(x0 + 1.5), px(my)), size(px(half), px(mh))),
                            px(0.5),
                            red.opacity(0.92),
                            px(0.),
                            transparent_black(),
                            BorderStyle::default(),
                        )),
                        _ => window.paint_quad(quad(
                            Bounds::new(point(px(x0 + 1.5 + half), px(my)), size(px(half), px(mh))),
                            px(0.5),
                            green.opacity(0.92),
                            px(0.),
                            transparent_black(),
                            BorderStyle::default(),
                        )),
                    });
                    let vp_h = ((vp_vis as f32 / denom) * h).clamp(18., h);
                    let vp_y =
                        (y0 + (vp_top as f32 / denom) * h).clamp(y0, (y0 + h - vp_h).max(y0));
                    let vp = Bounds::new(
                        point(px(x0 + 0.5), px(vp_y)),
                        size(px((w - 1.).max(2.)), px(vp_h)),
                    );
                    window.paint_quad(quad(
                        vp,
                        px(1.),
                        vp_fill,
                        px(1.),
                        vp_edge,
                        BorderStyle::default(),
                    ));
                },
            )
            .size_full();

            let st_jump = st.clone();
            let bounds_click = ruler_bounds.clone();
            let ruler = div()
                .id(("diff-overview-ruler", ix))
                .flex_none()
                .w(px(14.))
                .h_full()
                .bg(c(theme::BG_EDITOR))
                .border_l_1()
                .border_color(c(theme::BORDER))
                .cursor_pointer()
                .child(ruler_canvas)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        let (top, h) = bounds_click.get();
                        if h > 1. {
                            let total = st_jump.read(cx).row_count();
                            let row = jump_row_for_y(f32::from(ev.position.y), top, h, total);
                            st_jump.update(cx, |s, cx| {
                                s.scroll_to_row(row, cx);
                            });
                        }
                        this.diff_ruler_drag = Some((st_jump.clone(), bounds_click.clone()));
                        this.resizing = 6;
                        cx.notify();
                    }),
                );

            h_flex()
                .size_full()
                .min_h_0()
                .min_w_0()
                .child(
                    div().flex_1().min_h_0().min_w_0().size_full().child(
                        Diff::new(st)
                            .header_visible(d.multi_file)
                            .hover_highlight(DiffHoverHighlight::Both)
                            .change_indicator(DiffChangeIndicator::Signs)
                            .border_0()
                            .bg(gpui_kit::transparent_black()),
                    ),
                )
                .child(ruler)
                .into_any_element()
        } else if !d.lines.is_empty() {
            let lines = d.lines.clone();
            let mono = theme::resolve_mono_font(&self.settings.font_family, cx);
            let fs = px(self.settings.font_size.max(10.));
            let rh = px((self.settings.font_size * 1.55).round().max(18.));
            let (fg, dim, blue) = (c(theme::FG), c(theme::FG_DIM), c(theme::BLUE));
            let (add_bg, del_bg, hunk_bg) = (green.alpha(0.14), red.alpha(0.14), blue.alpha(0.1));
            uniform_list(("diff-fallback", ix), lines.len(), move |range, _, _| {
                range
                    .map(|i| {
                        let l = &lines[i];
                        let (bg, col) = match l.kind {
                            Kind::Add => (Some(add_bg), green),
                            Kind::Del => (Some(del_bg), red),
                            Kind::Hunk => (Some(hunk_bg), blue),
                            _ => (None, fg),
                        };
                        let num = |n: Option<u32>| n.map(|v| v.to_string()).unwrap_or_default();
                        h_flex()
                            .h(rh)
                            .w_full()
                            .items_center()
                            .font_family(mono.clone())
                            .text_size(fs)
                            .when_some(bg, |h, bg| h.bg(bg))
                            .child(
                                div()
                                    .w(px(44.))
                                    .pr_2()
                                    .text_right()
                                    .text_color(dim)
                                    .child(num(l.old)),
                            )
                            .child(
                                div()
                                    .w(px(44.))
                                    .pr_2()
                                    .text_right()
                                    .text_color(dim)
                                    .child(num(l.new)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px_2()
                                    .text_color(col)
                                    .whitespace_nowrap()
                                    .child(l.text.clone()),
                            )
                    })
                    .collect()
            })
            .size_full()
            .into_any_element()
        } else {
            let note = d.empty_note.clone().unwrap_or_else(|| {
                tr("No textual changes to compare (binary file or unchanged content).").into()
            });
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .p_6()
                .text_sm()
                .text_color(c(theme::FG_MUTED))
                .child(note)
                .when_some(target, |v, path| {
                    v.child(
                        div()
                            .id(("diff-empty-open", ix))
                            .px_3()
                            .py_1()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(theme::BORDER_STRONG))
                            .bg(c(theme::BG_ELEVATED))
                            .text_xs()
                            .text_color(c(theme::FG_STRONG))
                            .cursor_pointer()
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(tr("Open file"))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.open_file(path.clone(), w, cx);
                            })),
                    )
                })
                .into_any_element()
        };

        v_flex()
            .size_full()
            .child(bar)
            .child(div().flex_1().min_h_0().size_full().child(content))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        emit_coalesced_marks, jump_row_for_y, parse, parse_diff_files, DiffOverviewMark, Kind,
    };

    #[test]
    fn numbers_follow_the_hunks() {
        let d = parse("--- a/f.rs\n+++ b/f.rs\n@@ -3,3 +3,4 @@\n same\n-old\n+new\n+more\n tail\n");
        let k: Vec<Kind> = d.lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            k,
            vec![
                Kind::Hunk,
                Kind::Same,
                Kind::Del,
                Kind::Add,
                Kind::Add,
                Kind::Same
            ]
        );
        assert_eq!(d.name.as_ref(), "f.rs");
        assert_eq!((d.lines[1].old, d.lines[1].new), (Some(3), Some(3)));
        assert_eq!(d.lines[2].old, Some(4));
        assert_eq!(d.lines[3].new, Some(4));
        assert_eq!((d.lines[5].old, d.lines[5].new), (Some(5), Some(6)));
        assert_eq!((d.added, d.removed), (2, 1));
    }

    #[test]
    fn no_newline_markers_do_not_advance_line_numbers() {
        let diff = parse("--- a/file\n+++ b/file\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n");
        assert_eq!(diff.lines[3].new, Some(1));
        assert_eq!((diff.added, diff.removed), (1, 1));
    }

    #[test]
    fn parse_diff_files_detects_language_and_counts() {
        let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,3 @@\n fn a() {}\n+fn b() {}\n fn c() {}\n";
        let files = parse_diff_files(patch, None);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].additions(), 1);
        assert_eq!(files[0].deletions(), 0);
    }

    #[test]
    fn overview_ruler_maps_click_y_to_clamped_row() {
        assert_eq!(jump_row_for_y(100., 100., 200., 101), 0);
        assert_eq!(jump_row_for_y(200., 100., 200., 101), 50);
        assert_eq!(jump_row_for_y(300., 100., 200., 101), 100);
        assert_eq!(jump_row_for_y(50., 100., 200., 101), 0);
        assert_eq!(jump_row_for_y(450., 100., 200., 101), 100);
        assert_eq!(jump_row_for_y(150., 100., 200., 0), 0);
    }

    #[test]
    fn overview_ruler_coalesces_dense_marks_without_overdraw() {
        assert_eq!(std::mem::size_of::<DiffOverviewMark>(), 12);
        let marks: Vec<DiffOverviewMark> = (0..5_000u32)
            .map(|i| DiffOverviewMark {
                start_row: i * 2,
                end_row: i * 2 + 1,
                removed: false,
                added: true,
                conflict: false,
            })
            .collect();
        let mut quads = Vec::new();
        emit_coalesced_marks(&marks, 10_000, 10., 300., |lane, y, mh| {
            quads.push((lane, y, mh));
        });
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].0, 2);
        assert!((quads[0].1 - 10.).abs() < 0.01);
        assert!((quads[0].2 - 300.).abs() < 1.0);
    }
}
