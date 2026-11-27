use crate::scm::{Commit, ROW_H};
use crate::theme::{self, c};
use gpui_kit::*;

const LANE_PITCH: f32 = 10.;
const LEFT_PAD: f32 = 6.;
const MAX_VISIBLE_LANES: usize = 6;
const STROKE_W: f32 = 1.6;

pub const GRAPH_ROW_H: f32 = 22.;

const LANE_TOKS: [theme::Tok; 5] = [
    theme::BLUE,
    theme::WARN,
    theme::ACCENT,
    theme::GREEN,
    theme::DANGER,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Head,
    Branch,
    Remote,
    Tag,
}

impl RefKind {
    fn sort_key(self) -> u8 {
        match self {
            Self::Head => 0,
            Self::Branch => 1,
            Self::Remote => 2,
            Self::Tag => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefBadge {
    pub kind: RefKind,
    pub label: String,
}

fn parse_ref_badge(raw: &str) -> RefBadge {
    let (kind, label) = if let Some(head) = raw.strip_prefix("HEAD -> ") {
        (
            RefKind::Head,
            head.strip_prefix("refs/heads/").unwrap_or(head),
        )
    } else if raw == "HEAD" {
        (RefKind::Head, raw)
    } else if let Some(tag) = raw.strip_prefix("tag: ") {
        (RefKind::Tag, tag.strip_prefix("refs/tags/").unwrap_or(tag))
    } else if let Some(remote) = raw.strip_prefix("refs/remotes/") {
        (RefKind::Remote, remote)
    } else {
        (
            RefKind::Branch,
            raw.strip_prefix("refs/heads/").unwrap_or(raw),
        )
    };
    RefBadge {
        kind,
        label: label.to_string(),
    }
}

pub fn sort_ref_badges(refs: &[String]) -> Vec<RefBadge> {
    let mut badges: Vec<RefBadge> = refs.iter().map(|r| parse_ref_badge(r)).collect();
    badges.sort_by_key(|b| b.kind.sort_key());
    badges
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRow {
    pub col: usize,
    pub color: usize,
    pub merge: bool,
    pub is_head: bool,
    pub continues_up: bool,
    pub continues_down: bool,
    pub row_lanes: usize,
    pub pass_lanes: Vec<(usize, usize)>,
    pub in_edges: Vec<(usize, usize)>,
    pub out_edges: Vec<(usize, usize)>,
    pub after_lanes: Vec<(usize, usize)>,
}

impl GraphRow {
    pub fn row_width(&self) -> Pixels {
        let cols = self.row_lanes.clamp(1, MAX_VISIBLE_LANES + 1) as f32;
        theme::u(LEFT_PAD + cols * LANE_PITCH + 4.)
    }
}

pub fn rails_width(lanes: &[(usize, usize)]) -> Pixels {
    let cols = lanes
        .iter()
        .map(|(c, _)| c + 1)
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_VISIBLE_LANES + 1) as f32;
    theme::u(LEFT_PAD + cols * LANE_PITCH + 4.)
}

#[derive(Clone, Debug, Default)]
pub struct GraphLayout {
    pub rows: Vec<GraphRow>,
}

#[derive(Clone)]
struct Slot {
    hash: String,
    color: usize,
}

pub fn compute_graph(commits: &[Commit]) -> GraphLayout {
    let mut slots: Vec<Option<Slot>> = Vec::new();
    let mut next_color: usize = 0;
    let mut rows = Vec::with_capacity(commits.len());

    for cm in commits {
        let matching: Vec<usize> = slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().filter(|s| s.hash == cm.hash).map(|_| i))
            .collect();

        let (col, color, continues_up, mut in_edges) = if let Some(&primary) = matching.first() {
            let color = slots[primary].as_ref().map(|s| s.color).unwrap_or(0);
            let mut merges = Vec::new();
            for &dup in &matching[1..] {
                if let Some(s) = slots[dup].take() {
                    merges.push((dup, s.color));
                }
            }
            (primary, color, true, merges)
        } else {
            let col = alloc_slot(&mut slots);
            let color = next_color % LANE_TOKS.len();
            next_color += 1;
            (col, color, false, Vec::new())
        };

        let mut pass_lanes = Vec::new();
        for (i, s) in slots.iter().enumerate() {
            if i != col {
                if let Some(slot) = s {
                    pass_lanes.push((i, slot.color));
                }
            }
        }

        let mut continues_down = false;
        let mut out_edges = Vec::new();

        if let Some(first_parent) = cm.parents.first() {
            if let Some(target) = find_slot(&slots, first_parent, Some(col)) {
                slots[col] = None;
                out_edges.push((target, color));
            } else {
                slots[col] = Some(Slot {
                    hash: first_parent.clone(),
                    color,
                });
                continues_down = true;
            }
        } else {
            slots[col] = None;
        }

        for extra_parent in cm.parents.iter().skip(1) {
            if let Some(target) = find_slot(&slots, extra_parent, None) {
                let target_color = slots[target].as_ref().map(|s| s.color).unwrap_or(color);
                out_edges.push((target, target_color));
            } else {
                let new_col = alloc_slot(&mut slots);
                let new_color = next_color % LANE_TOKS.len();
                next_color += 1;
                slots[new_col] = Some(Slot {
                    hash: extra_parent.clone(),
                    color: new_color,
                });
                out_edges.push((new_col, new_color));
            }
        }

        trim_slots(&mut slots);

        let after_lanes: Vec<(usize, usize)> = slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|slot| (i, slot.color)))
            .collect();

        // Size each row to only the lanes drawn on that row so commit text hugs the graph.
        let row_lanes = [col + 1]
            .into_iter()
            .chain(pass_lanes.iter().map(|(c, _)| c + 1))
            .chain(in_edges.iter().map(|(c, _)| c + 1))
            .chain(out_edges.iter().map(|(c, _)| c + 1))
            .max()
            .unwrap_or(1);

        in_edges.sort_by_key(|(c, _)| *c);
        rows.push(GraphRow {
            col,
            color,
            merge: cm.merge,
            is_head: cm.is_head,
            continues_up,
            continues_down,
            row_lanes,
            pass_lanes,
            in_edges,
            out_edges,
            after_lanes,
        });
    }

    GraphLayout { rows }
}

fn find_slot(slots: &[Option<Slot>], hash: &str, exclude: Option<usize>) -> Option<usize> {
    slots.iter().enumerate().find_map(|(i, s)| {
        if Some(i) == exclude {
            return None;
        }
        s.as_ref().filter(|slot| slot.hash == hash).map(|_| i)
    })
}

fn alloc_slot(slots: &mut Vec<Option<Slot>>) -> usize {
    if let Some(empty) = slots.iter().position(Option::is_none) {
        empty
    } else {
        slots.push(None);
        slots.len() - 1
    }
}

fn trim_slots(slots: &mut Vec<Option<Slot>>) {
    while slots.last().is_some_and(Option::is_none) {
        slots.pop();
    }
}

pub fn lane_color(color_ix: usize) -> Hsla {
    c(LANE_TOKS[color_ix % LANE_TOKS.len()])
}

fn lane_x(col: usize, scale: f32) -> f32 {
    (LEFT_PAD + (col as f32 + 0.5) * LANE_PITCH) * scale
}

fn paint_overflow(window: &mut Window, x0: f32, mid: f32, scale: f32) {
    let x = x0 + lane_x(MAX_VISIBLE_LANES, scale);
    let diameter = px(1.5 * scale);
    for offset in [-2.5, 0., 2.5] {
        let center = point(px(x + offset * scale), px(mid));
        window.paint_quad(quad(
            Bounds::centered_at(center, size(diameter, diameter)),
            diameter,
            c(theme::FG_MUTED),
            px(0.),
            transparent_black(),
            BorderStyle::default(),
        ));
    }
}

pub fn render_graph_cell(row: GraphRow) -> AnyElement {
    let width = row.row_width();
    let scale = theme::scale();
    let bg_side = c(theme::BG_SIDE);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let x0 = f32::from(bounds.origin.x);
            let y0 = f32::from(bounds.origin.y);
            let h = f32::from(bounds.size.height);
            let mid = y0 + h * 0.5;
            let bot = y0 + h;
            let pt = |x: f32, y: f32| point(px(x0 + x), px(y));
            let stroke = px(STROKE_W);
            if row.row_lanes > MAX_VISIBLE_LANES {
                paint_overflow(window, x0, mid, scale);
            }

            for &(lane, col_ix) in row
                .pass_lanes
                .iter()
                .filter(|(lane, _)| *lane < MAX_VISIBLE_LANES)
            {
                let lx = lane_x(lane, scale);
                let mut pb = PathBuilder::stroke(stroke);
                pb.move_to(pt(lx, y0));
                pb.line_to(pt(lx, bot));
                if let Ok(p) = pb.build() {
                    window.paint_path(p, lane_color(col_ix).opacity(0.9));
                }
            }

            if row.col >= MAX_VISIBLE_LANES {
                return;
            }
            let cx = lane_x(row.col, scale);
            let node_col = lane_color(row.color);

            if row.continues_up {
                let mut pb = PathBuilder::stroke(stroke);
                pb.move_to(pt(cx, y0));
                pb.line_to(pt(cx, mid));
                if let Ok(p) = pb.build() {
                    window.paint_path(p, node_col.opacity(0.92));
                }
            }
            if row.continues_down {
                let mut pb = PathBuilder::stroke(stroke);
                pb.move_to(pt(cx, mid));
                pb.line_to(pt(cx, bot));
                if let Ok(p) = pb.build() {
                    window.paint_path(p, node_col.opacity(0.92));
                }
            }

            for &(from_lane, col_ix) in row
                .in_edges
                .iter()
                .filter(|(lane, _)| *lane < MAX_VISIBLE_LANES)
            {
                let fx = lane_x(from_lane, scale);
                let mut pb = PathBuilder::stroke(stroke);
                pb.move_to(pt(fx, y0));
                pb.cubic_bezier_to(pt(cx, mid), pt(fx, y0 + h * 0.36), pt(fx, mid));
                if let Ok(p) = pb.build() {
                    window.paint_path(p, lane_color(col_ix).opacity(0.9));
                }
            }

            for &(to_lane, col_ix) in row
                .out_edges
                .iter()
                .filter(|(lane, _)| *lane < MAX_VISIBLE_LANES)
            {
                let tx = lane_x(to_lane, scale);
                let mut pb = PathBuilder::stroke(stroke);
                pb.move_to(pt(cx, mid));
                pb.cubic_bezier_to(pt(tx, bot), pt(tx, mid), pt(tx, y0 + h * 0.64));
                if let Ok(p) = pb.build() {
                    window.paint_path(p, lane_color(col_ix).opacity(0.9));
                }
            }

            let center = pt(cx, mid);
            let halo_r = if row.merge || row.is_head {
                5.4 * scale
            } else {
                4.4 * scale
            };
            let halo = Bounds::centered_at(center, size(px(halo_r * 2.), px(halo_r * 2.)));
            window.paint_quad(quad(
                halo,
                px(halo_r),
                bg_side,
                px(0.),
                transparent_black(),
                BorderStyle::default(),
            ));

            if row.is_head {
                let r = 4.0 * scale;
                let ring = Bounds::centered_at(center, size(px(r * 2.), px(r * 2.)));
                window.paint_quad(quad(
                    ring,
                    px(r),
                    bg_side,
                    px(1.8),
                    node_col,
                    BorderStyle::default(),
                ));
            } else if row.merge {
                let outer_r = 4.2 * scale;
                let outer = Bounds::centered_at(center, size(px(outer_r * 2.), px(outer_r * 2.)));
                window.paint_quad(quad(
                    outer,
                    px(outer_r),
                    bg_side,
                    px(1.3),
                    node_col,
                    BorderStyle::default(),
                ));
                let inner_r = 1.7 * scale;
                let inner = Bounds::centered_at(center, size(px(inner_r * 2.), px(inner_r * 2.)));
                window.paint_quad(quad(
                    inner,
                    px(inner_r),
                    node_col,
                    px(0.),
                    transparent_black(),
                    BorderStyle::default(),
                ));
            } else {
                let r = 3.1 * scale;
                let dot = Bounds::centered_at(center, size(px(r * 2.), px(r * 2.)));
                window.paint_quad(quad(
                    dot,
                    px(r),
                    node_col,
                    px(0.),
                    transparent_black(),
                    BorderStyle::default(),
                ));
            }
        },
    )
    .w(width)
    .h_full()
    .flex_none()
    .into_any_element()
}

pub fn render_rails_cell(lanes: Vec<(usize, usize)>) -> AnyElement {
    let width = rails_width(&lanes);
    let scale = theme::scale();
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let x0 = f32::from(bounds.origin.x);
            let y0 = f32::from(bounds.origin.y);
            let bot = y0 + f32::from(bounds.size.height);
            let stroke = px(STROKE_W);
            if lanes.iter().any(|(lane, _)| *lane >= MAX_VISIBLE_LANES) {
                paint_overflow(window, x0, (y0 + bot) / 2., scale);
            }
            for &(lane, col_ix) in lanes.iter().filter(|(lane, _)| *lane < MAX_VISIBLE_LANES) {
                let lx = lane_x(lane, scale);
                let mut pb = PathBuilder::stroke(stroke);
                pb.move_to(point(px(x0 + lx), px(y0)));
                pb.line_to(point(px(x0 + lx), px(bot)));
                if let Ok(p) = pb.build() {
                    window.paint_path(p, lane_color(col_ix).opacity(0.82));
                }
            }
        },
    )
    .w(width)
    .h(theme::u(ROW_H))
    .flex_none()
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{compute_graph, sort_ref_badges, RefKind};
    use crate::scm::Commit;

    fn cm(hash: &str, parents: &[&str], head: bool) -> Commit {
        Commit {
            hash: hash.into(),
            short: hash.into(),
            subject: hash.into(),
            author: "dev".into(),
            when: "1h ago".into(),
            refs: Vec::new(),
            merge: parents.len() > 1,
            parents: parents.iter().map(|s| (*s).to_string()).collect(),
            is_head: head,
        }
    }

    #[test]
    fn merge_creates_second_lane_and_row_lanes_hug_each_row() {
        let log = vec![
            cm("tip", &["m"], true),
            cm("m", &["a", "b"], false),
            cm("b", &["base"], false),
            cm("a", &["base"], false),
            cm("base", &[], false),
        ];
        let g = compute_graph(&log);
        assert_eq!(g.rows[0].row_lanes, 1);
        assert_eq!(g.rows[1].row_lanes, 2);
        assert_eq!(g.rows[2].row_lanes, 2);
        assert_eq!(g.rows[3].row_lanes, 2);
        assert_eq!(g.rows[1].out_edges[0].0, 1);
        assert_eq!(g.rows[2].col, 1);
        assert_eq!(g.rows[4].col, 1);
        assert!(!g.rows[4].continues_down);
    }

    #[test]
    fn wide_merges_reserve_a_separate_overflow_position() {
        let log = [cm("merge", &["a", "b", "c", "d", "e", "f", "g", "h"], true)];
        let graph = compute_graph(&log);
        let row = &graph.rows[0];
        let limit = super::MAX_VISIBLE_LANES;
        assert!(row.row_lanes > limit + 1);
        assert!(super::lane_x(limit, 1.) > super::lane_x(limit - 1, 1.));
        assert_eq!(row.row_width(), super::rails_width(&row.after_lanes));
    }

    #[test]
    fn ref_badges_prioritize_branches_and_remotes_over_tags() {
        let refs = vec![
            "tag: refs/tags/v1.0.168".into(),
            "refs/remotes/fork/main".into(),
            "refs/heads/origin/topic".into(),
        ];
        let sorted = sort_ref_badges(&refs);
        assert_eq!(sorted[0].kind, RefKind::Branch);
        assert_eq!(sorted[1].kind, RefKind::Remote);
        assert_eq!(sorted[1].label, "fork/main");
        assert_eq!(sorted[2].kind, RefKind::Tag);
        assert_eq!(sorted[2].label, "v1.0.168");

        let head_first = sort_ref_badges(&[
            "refs/remotes/fork/test/fix".into(),
            "HEAD -> refs/heads/test/fix".into(),
        ]);
        assert_eq!(head_first[0].kind, RefKind::Head);
        assert_eq!(head_first[0].label, "test/fix");
        assert_eq!(head_first[1].kind, RefKind::Remote);
    }
}
