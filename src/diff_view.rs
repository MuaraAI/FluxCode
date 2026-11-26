use crate::files;
use crate::i18n::tr;
use crate::scm_git::Group;
use crate::theme::{self, c};
use crate::workbench::{Doc, Workbench};
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    diff::{
        Diff, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffMode, DiffParseOptions,
        DiffState,
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
    pub(crate) scm_stage: Option<(PathBuf, String, Group)>,
    scm_patch: Option<std::sync::Arc<str>>,
    conflict_original: Option<std::sync::Arc<str>>,
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
        scm_stage: None,
        scm_patch: None,
        conflict_original: None,
        state: None,
        multi_file: false,
        empty_note: None,
        virtual_tab: false,
    }
}

pub fn untracked_patch(rel_path: &str, bytes: &[u8]) -> Option<String> {
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return None;
    }
    let content = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = content.lines().collect();
    let path = |prefix: &str| {
        if rel_path.contains(['\\', '"', '\n', '\t', '\r']) {
            let escaped = rel_path
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\t', "\\t")
                .replace('\r', "\\r");
            format!("\"{prefix}/{escaped}\"")
        } else {
            format!("{prefix}/{rel_path}")
        }
    };
    let (old, new) = (path("a"), path("b"));
    let mut out =
        format!("diff --git {old} {new}\nnew file mode 100644\n--- /dev/null\n+++ {new}\n");
    if !lines.is_empty() {
        out.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
        for line in lines {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
        if !content.ends_with('\n') {
            out.push_str("\\ No newline at end of file\n");
        }
    }
    Some(out)
}

pub fn parse_with_state(text: &str, cx: &mut Context<Workbench>) -> DiffDoc {
    let mut doc = parse(text);
    let files = parse_diff_files(text, None);
    if !files.is_empty() {
        doc.added = files.iter().map(DiffFile::additions).sum();
        doc.removed = files.iter().map(DiffFile::deletions).sum();
        doc.multi_file = files.len() > 1;
        let state = cx.new(|cx| {
            DiffState::new(files, cx)
                .with_mode(DiffMode::Split)
                .with_context_lines(None)
        });
        doc.state = Some(state);
    } else if text.trim().is_empty() {
        doc.empty_note =
            Some(tr("No textual changes to compare (binary file or unchanged content).").into());
    }
    doc
}

pub fn parse_diff_files(patch: &str, lang_hint: Option<&str>) -> Vec<DiffFile> {
    let opts = DiffParseOptions::new().truncated_hunks(true);
    let Ok(mut files) = DiffFile::parse_with(patch, opts) else {
        return Vec::new();
    };
    for file in &mut files {
        let lang = lang_hint.or_else(|| {
            let p = Path::new(file.path().as_ref());
            let (l, _) = files::language_for(p, "");
            (l != "text").then_some(l)
        });
        if let Some(l) = lang {
            *file = file.clone().with_language(l);
        }
    }
    files
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

pub(crate) type DiffRulerDrag = (Entity<DiffState>, Rc<Cell<(f32, f32)>>);

pub struct ScmDiffSpec {
    pub virtual_key: PathBuf,
    pub tab_title: String,
    pub display_name: String,
    pub badge: String,
    pub patch: String,
    pub conflict_text: Option<String>,
    pub target: Option<PathBuf>,
    pub(crate) scm_stage: Option<(PathBuf, String, Group)>,
    pub multi_file: bool,
}

impl Workbench {
    pub(crate) fn diff_drag_ruler(&mut self, y: f32, cx: &mut Context<Self>) {
        let Some((st, rect)) = self.diff_ruler_drag.as_ref() else {
            return;
        };
        let (top, h) = rect.get();
        if h <= 1. {
            return;
        }
        let total = st.read(cx).row_count();
        let row = jump_row_for_y(y, top, h, total);
        st.update(cx, |s, cx| s.scroll_to_row(row, cx));
    }

    pub(crate) fn open_scm_diff_tab(
        &mut self,
        spec: ScmDiffSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing_ix = self.docs.iter().position(|d| d.path == spec.virtual_key);
        let prev_mode = existing_ix
            .and_then(|ix| self.docs[ix].diff.as_ref())
            .and_then(|d| d.state.as_ref())
            .map(|st| st.read(cx).mode())
            .unwrap_or(DiffMode::Split);
        let lang_hint = spec.target.as_deref().and_then(|p| {
            let (l, _) = files::language_for(p, "");
            (l != "text").then_some(l)
        });
        let files = if let Some(conflict_text) = &spec.conflict_text {
            match DiffFile::parse_conflicts(spec.display_name.clone(), conflict_text) {
                Ok(mut f) => {
                    if let Some(l) = lang_hint {
                        f = f.with_language(l);
                    }
                    vec![f]
                }
                Err(_) => parse_diff_files(&spec.patch, lang_hint),
            }
        } else {
            parse_diff_files(&spec.patch, lang_hint)
        };

        let (added, removed, state, empty_note) = if !files.is_empty() {
            let added = files.iter().map(DiffFile::additions).sum();
            let removed = files.iter().map(DiffFile::deletions).sum();
            let context = if spec.multi_file || spec.scm_stage.is_some() {
                Some(3)
            } else {
                None
            };
            let st = cx.new(|cx| {
                DiffState::new(files, cx)
                    .with_mode(prev_mode)
                    .with_context_lines(context)
            });
            if !spec.multi_file && spec.scm_stage.is_none() {
                st.update(cx, |s, cx| s.next_change(cx));
            }
            (added, removed, Some(st), None)
        } else {
            (
                0,
                0,
                None,
                Some(
                    tr("No textual changes to compare (binary file or unchanged content).").into(),
                ),
            )
        };

        let scm_patch = spec
            .scm_stage
            .as_ref()
            .filter(|(_, _, group)| matches!(group, Group::Changes | Group::Staged))
            .map(|_| std::sync::Arc::from(spec.patch.as_str()));
        let diff_doc = DiffDoc {
            lines: Rc::new(Vec::new()),
            added,
            removed,
            source: false,
            name: spec.display_name.into(),
            badge: Some(spec.badge.into()),
            target: spec.target,
            scm_stage: spec.scm_stage,
            scm_patch,
            conflict_original: spec.conflict_text.as_deref().map(std::sync::Arc::from),
            state,
            multi_file: spec.multi_file,
            empty_note,
            virtual_tab: true,
        };

        if let Some(ix) = existing_ix {
            self.docs[ix].name = spec.tab_title.into();
            self.docs[ix].diff = Some(diff_doc);
            self.activate(ix, window, cx);
            return;
        }

        self.anim_seq = self.anim_seq.wrapping_add(1);
        self.opened_tab = Some((spec.virtual_key.clone(), self.anim_seq));
        self.docs.push(Doc {
            path: spec.virtual_key,
            name: spec.tab_title.into(),
            lang: "diff",
            label: "Diff",
            read_only: true,
            nbt_format: None,
            banner: None,
            crlf: false,
            editor: None,
            dirty: false,
            gen: 0,
            _sub: None,
            image: None,
            flux: None,
            flux_source: false,
            flux_lazy: false,
            diff: Some(diff_doc),
            long_lines: false,
            complex: false,
            wrap_override: None,
            saved_cursor: Default::default(),
            saved_scroll: Default::default(),
            last_used: 0,
            loading: false,
            diagnostics: Vec::new(),
            error_count: 0,
            warn_count: 0,
            diag_ranges: None,
            diag_ruler: None,
            runnable_runtime: None,
        });
        let ix = self.docs.len() - 1;
        self.activate(ix, window, cx);
    }

    pub(crate) fn render_diff_doc(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = self.docs.get(ix).and_then(|d| d.diff.as_ref()) else {
            return div().into_any_element();
        };
        let (green, red) = (c(theme::GREEN), c(theme::DANGER));
        let target = d.target.clone();
        let scm_stage = d.scm_stage.clone();
        let virtual_tab = d.virtual_tab;
        let is_file_diff = scm_stage.is_some();
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
            if is_file_diff {
                if let Some((hunk, count)) = st.read(cx).hunk_position() {
                    bar = bar.child(
                        div()
                            .flex_none()
                            .text_color(c(theme::FG_MUTED))
                            .child(crate::trf!("Hunk {n}/{total}"; n => hunk + 1, total => count)),
                    );
                }
            }
            let st_prev = st.clone();
            let st_next = st.clone();
            let st_mode = st.clone();
            let st_ctx = st.clone();
            bar = bar
                .child(
                    icon_btn(
                        "diff-prev",
                        Ico::ArrowUp,
                        if is_file_diff {
                            "Previous hunk"
                        } else {
                            "Previous change"
                        },
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        st_prev.update(cx, |s, cx| {
                            if is_file_diff {
                                s.move_hunk(false, cx)
                            } else {
                                s.previous_change(cx)
                            }
                        });
                    })),
                )
                .child(
                    icon_btn(
                        "diff-next",
                        Ico::ArrowDown,
                        if is_file_diff {
                            "Next hunk"
                        } else {
                            "Next change"
                        },
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        st_next.update(cx, |s, cx| {
                            if is_file_diff {
                                s.move_hunk(true, cx)
                            } else {
                                s.next_change(cx)
                            }
                        });
                    })),
                )
                .child(
                    btn("diff-mode", if is_split { "Inline" } else { "Side by side" }, false)
                        .on_click(cx.listener(move |_, _, _, cx| {
                            let next = if is_split {
                                DiffMode::Unified
                            } else {
                                DiffMode::Split
                            };
                            st_mode.update(cx, |s, cx| s.set_mode(next, cx));
                        })),
                )
                .when(!is_file_diff, |h| {
                    h.child(
                        btn("diff-ctx", if is_full { "Full file" } else { "Changes only" }, false)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                let next = if is_full { Some(3) } else { None };
                                st_ctx.update(cx, |s, cx| s.set_context_lines(next, cx));
                            })),
                    )
                });
        }

        if let Some((repo, rel, group)) = scm_stage {
            if self.scm.repo.as_ref() == Some(&repo)
                && !self.scm.is_busy()
                && self
                    .scm
                    .status
                    .files(group)
                    .iter()
                    .any(|file| file.path == rel)
            {
                if let Some((original, conflict_state)) =
                    d.conflict_original.clone().zip(state.clone())
                {
                    let path = rel.clone();
                    let conflict_repo = repo.clone();
                    bar = bar.child(btn("save-conflict-resolution", "Save Resolution", false).on_click(
                        cx.listener(move |this, _, _, cx| {
                            if this.scm.repo.as_ref() != Some(&conflict_repo) {
                                return;
                            }
                            let full = conflict_repo.join(&path);
                            let unsaved = this
                                .docs
                                .iter()
                                .any(|doc| doc.dirty && crate::panel::same_path(&doc.path, &full));
                            let resolved = conflict_state.read(cx).resolved_text(&path);
                            if unsaved || resolved.is_none() {
                                let message = if unsaved {
                                    tr("Save unsaved editors before changing Git branches or files.")
                                } else {
                                    tr("Choose a resolution for every conflict first.")
                                };
                                this.status = message.into();
                                cx.notify();
                                return;
                            }
                            if let Some(resolved) = resolved {
                                let step = crate::scm_git::Step::Resolve {
                                    path: path.clone(),
                                    expected: original.to_string(),
                                    resolved,
                                };
                                this.scm_run(tr("Running Git…"), vec![step], cx, |this, _, _| {
                                    this.status = tr("Conflict resolution saved and staged.").into();
                                });
                            }
                        }),
                    ));
                }
                if let Some((patch, (index, _))) = d
                    .scm_patch
                    .clone()
                    .zip(state.as_ref().and_then(|state| state.read(cx).hunk_position()))
                {
                    if self
                        .scm
                        .status
                        .files(group)
                        .iter()
                        .any(|file| file.path == rel && file.code == 'M')
                    {
                        let path = rel.clone();
                        let hunk_repo = repo.clone();
                        bar = bar.child(
                            btn(
                                "diff-hunk-stage",
                                if group == Group::Staged {
                                    "Unstage Hunk"
                                } else {
                                    "Stage Hunk"
                                },
                                false,
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.scm.repo.as_ref() != Some(&hunk_repo) {
                                    return;
                                }
                                this.scm_stage_hunk(
                                    path.clone(),
                                    group,
                                    patch.to_string(),
                                    index,
                                    window,
                                    cx,
                                );
                            })),
                        );
                    }
                }
                bar = bar.child(
                    icon_btn(
                        "diff-stage",
                        if group == Group::Staged {
                            Ico::Minus
                        } else {
                            Ico::Plus
                        },
                        if group == Group::Staged {
                            "Unstage"
                        } else {
                            "Stage"
                        },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.scm.repo.as_ref() == Some(&repo) {
                            this.scm_paths(group, Some(rel.clone()), false, cx);
                        }
                    })),
                );
            }
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
                    for m in marks {
                        let my = y0 + (m.start_row as f32 / denom) * h;
                        let mh = (((m.end_row - m.start_row) as f32 / denom) * h).max(3.);
                        let my = my.min((y0 + h - mh).max(y0));
                        if m.conflict {
                            let r = Bounds::new(
                                point(px(x0 + 1.5), px(my)),
                                size(px((w - 3.).max(2.)), px(mh)),
                            );
                            window.paint_quad(quad(
                                r,
                                px(1.),
                                warn.opacity(0.92),
                                px(0.),
                                transparent_black(),
                                BorderStyle::default(),
                            ));
                        } else {
                            if m.removed {
                                let r = Bounds::new(
                                    point(px(x0 + 1.5), px(my)),
                                    size(px(half), px(mh)),
                                );
                                window.paint_quad(quad(
                                    r,
                                    px(0.5),
                                    red.opacity(0.92),
                                    px(0.),
                                    transparent_black(),
                                    BorderStyle::default(),
                                ));
                            }
                            if m.added {
                                let r = Bounds::new(
                                    point(px(x0 + 1.5 + half), px(my)),
                                    size(px(half), px(mh)),
                                );
                                window.paint_quad(quad(
                                    r,
                                    px(0.5),
                                    green.opacity(0.92),
                                    px(0.),
                                    transparent_black(),
                                    BorderStyle::default(),
                                ));
                            }
                        }
                    }
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
                            st_jump.update(cx, |s, cx| s.scroll_to_row(row, cx));
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
            let mono = self.settings.font_family.clone();
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
    use super::{jump_row_for_y, parse, parse_diff_files, untracked_patch, Kind};

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
    fn untracked_patch_handles_text_and_skips_binary() {
        let p = untracked_patch("src/new.kt", b"fun main() {}\n").unwrap();
        assert!(p.contains("+++ b/src/new.kt\n@@ -0,0 +1,1 @@\n+fun main() {}\n"));
        assert!(untracked_patch("img.png", b"\x89PNG\0\0").is_none());
        let quoted = untracked_patch("tabs\tand\nlines.rs", b"content").unwrap();
        let parsed = parse_diff_files(&quoted, None);
        assert_eq!(parsed[0].path().as_ref(), "tabs\tand\nlines.rs");
        assert_eq!(parsed[0].additions(), 1);
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
}
