
use super::{text, Span};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Box,
    Round,
    Pill,
    Diamond,
    Circle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    pub fill: Option<u32>,
    pub text: Option<u32>,
    pub border: Option<u32>,
    pub layer: usize,
    pub rect: (f32, f32, f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: String,
    pub dashed: bool,
    pub back: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flow {
    pub right: bool,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub size: (f32, f32),
}

fn label_w(s: &str) -> f32 {
    s.chars().map(|c| if c.is_ascii_uppercase() || c.is_ascii_digit() { 8.2 } else if c.is_ascii() { 7.0 } else { 12.0 }).sum()
}

use super::hex;

fn node_ref<'a>(s: &'a str, f: &mut Flow) -> Option<(usize, &'a str)> {
    let s = s.trim_start();
    let id_len = s.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
    if id_len == 0 {
        return None;
    }
    let id = &s[..id_len];
    let rest = &s[id_len..];
    let (shape, label, rest) = if let Some(r) = rest.strip_prefix("((") {
        let e = r.find("))")?;
        (Shape::Circle, &r[..e], &r[e + 2..])
    } else if let Some(r) = rest.strip_prefix("([") {
        let e = r.find("])")?;
        (Shape::Pill, &r[..e], &r[e + 2..])
    } else if let Some(r) = rest.strip_prefix('[') {
        let e = r.find(']')?;
        (Shape::Box, &r[..e], &r[e + 1..])
    } else if let Some(r) = rest.strip_prefix('(') {
        let e = r.find(')')?;
        (Shape::Round, &r[..e], &r[e + 1..])
    } else if let Some(r) = rest.strip_prefix('{') {
        let e = r.find('}')?;
        (Shape::Diamond, &r[..e], &r[e + 1..])
    } else {
        (Shape::Box, "", rest)
    };
    let ix = match f.nodes.iter().position(|n| n.id == id) {
        Some(i) => {
            if !label.is_empty() {
                f.nodes[i].label = label.trim().to_string();
                f.nodes[i].shape = shape;
            }
            i
        }
        None => {
            f.nodes.push(Node { id: id.to_string(), label: if label.is_empty() { id.to_string() } else { label.trim().to_string() }, shape, fill: None, text: None, border: None, layer: 0, rect: (0., 0., 0., 0.) });
            f.nodes.len() - 1
        }
    };
    Some((ix, rest))
}

fn arrow(s: &str) -> Option<(String, bool, &str)> {
    let s = s.trim_start();
    if let Some(r) = s.strip_prefix("-->").or_else(|| s.strip_prefix("->")) {
        return Some((String::new(), false, r));
    }
    if let Some(r) = s.strip_prefix("..>") {
        return Some((String::new(), true, r));
    }
    for (open, close, dashed) in [("-", "->", false), (".", ".>", true)] {
        if let Some(r) = s.strip_prefix(open) {
            if let Some(e) = r.find(close) {
                let label = r[..e].trim();
                if !label.is_empty() {
                    return Some((label.to_string(), dashed, &r[e + close.len()..]));
                }
            }
        }
    }
    None
}

pub fn parse(src: &str, args: Span, body: Span) -> Flow {
    let mut f = Flow { right: text(src, args).split_whitespace().any(|w| matches!(w, "right" | "lr" | "LR")), ..Default::default() };
    for line in text(src, body).lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        if let Some(r) = l.strip_prefix("style ") {
            let mut w = r.split_whitespace();
            let Some(id) = w.next() else { continue };
            let Some(ix) = f.nodes.iter().position(|n| n.id == id) else { continue };
            for kv in w {
                if let Some((k, v)) = kv.split_once('=') {
                    let c = hex(v);
                    match k {
                        "fill" => f.nodes[ix].fill = c,
                        "text" | "color" => f.nodes[ix].text = c,
                        "border" | "stroke" => f.nodes[ix].border = c,
                        _ => {}
                    }
                }
            }
            continue;
        }
        let Some((mut prev, mut rest)) = node_ref(l, &mut f) else { continue };
        while let Some((label, dashed, r)) = arrow(rest) {
            let Some((next, r2)) = node_ref(r, &mut f) else { break };
            f.edges.push(Edge { from: prev, to: next, label, dashed, back: false });
            prev = next;
            rest = r2;
        }
    }
    layout(&mut f);
    f
}

pub const NODE_H: f32 = 38.;
const GAP: f32 = 28.;
const LAYER_GAP: f32 = 64.;

fn layout(f: &mut Flow) {
    let n = f.nodes.len();
    if n == 0 {
        return;
    }
    let out: Vec<Vec<usize>> = (0..n).map(|i| f.edges.iter().enumerate().filter(|(_, e)| e.from == i).map(|(k, _)| k).collect()).collect();
    let mut state = vec![0u8; n];
    fn dfs(v: usize, f: &mut Flow, out: &[Vec<usize>], state: &mut [u8]) {
        state[v] = 1;
        for &k in &out[v] {
            let to = f.edges[k].to;
            match state[to] {
                1 => f.edges[k].back = true,
                0 => dfs(to, f, out, state),
                _ => {}
            }
        }
        state[v] = 2;
    }
    let has_in: Vec<bool> = (0..n).map(|i| f.edges.iter().any(|e| e.to == i && e.from != i)).collect();
    for v in (0..n).filter(|&v| !has_in[v]).chain(0..n) {
        if state[v] == 0 {
            dfs(v, f, &out, &mut state);
        }
    }
    let mut layer = vec![0usize; n];
    for _ in 0..n {
        let mut moved = false;
        for e in f.edges.iter().filter(|e| !e.back && e.from != e.to) {
            if layer[e.to] < layer[e.from] + 1 {
                layer[e.to] = layer[e.from] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let layers = layer.iter().max().copied().unwrap_or(0) + 1;
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); layers];
    for v in 0..n {
        rows[layer[v]].push(v);
    }
    for _ in 0..2 {
        let pos: Vec<f32> = {
            let mut p = vec![0f32; n];
            for r in &rows {
                for (i, &v) in r.iter().enumerate() {
                    p[v] = i as f32;
                }
            }
            p
        };
        for r in rows.iter_mut().skip(1) {
            let key = |v: usize| {
                let ps: Vec<f32> = f.edges.iter().filter(|e| e.to == v && !e.back).map(|e| pos[e.from]).collect();
                if ps.is_empty() { pos[v] } else { ps.iter().sum::<f32>() / ps.len() as f32 }
            };
            r.sort_by(|a, b| key(*a).partial_cmp(&key(*b)).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
    for node in &mut f.nodes {
        let (w, h) = if node.shape == Shape::Diamond {
            ((label_w(&node.label) * 1.4 + 60.).max(104.), NODE_H + 24.)
        } else {
            ((label_w(&node.label) + 28.).max(72.), NODE_H)
        };
        node.rect = (0., 0., w, h);
    }
    let span = |r: &Vec<usize>, f: &Flow| -> f32 {
        let along: f32 = r.iter().map(|&v| if f.right { f.nodes[v].rect.3 } else { f.nodes[v].rect.2 }).sum();
        along + GAP * (r.len().saturating_sub(1)) as f32
    };
    let widest = rows.iter().map(|r| span(r, f)).fold(0f32, f32::max);
    let mut depth_at = 0f32;
    for r in &rows {
        let thick = r.iter().map(|&v| if f.right { f.nodes[v].rect.2 } else { f.nodes[v].rect.3 }).fold(0f32, f32::max);
        let mut at = (widest - span(r, f)) / 2.;
        for &v in r {
            let (w, h) = (f.nodes[v].rect.2, f.nodes[v].rect.3);
            f.nodes[v].layer = layer[v];
            f.nodes[v].rect = if f.right { (depth_at + (thick - w) / 2., at, w, h) } else { (at, depth_at + (thick - h) / 2., w, h) };
            at += (if f.right { h } else { w }) + GAP;
        }
        depth_at += thick + LAYER_GAP;
    }
    let depth = depth_at - LAYER_GAP;
    let back = if f.edges.iter().any(|e| e.back) { 40. } else { 0. };
    f.size = if f.right { (depth, widest + back) } else { (widest + back, depth) };
}

pub fn to_mermaid(f: &Flow) -> String {
    let mut s = format!("flowchart {}\n", if f.right { "LR" } else { "TD" });
    let q = |l: &str| l.replace('"', "#quot;");
    for n in &f.nodes {
        let l = q(&n.label);
        let shape = match n.shape {
            Shape::Box => format!("[\"{l}\"]"),
            Shape::Round => format!("(\"{l}\")"),
            Shape::Pill => format!("([\"{l}\"])"),
            Shape::Diamond => format!("{{\"{l}\"}}"),
            Shape::Circle => format!("((\"{l}\"))"),
        };
        s.push_str(&format!("    {}{shape}\n", n.id));
    }
    for e in &f.edges {
        let arrow = if e.dashed { "-.->" } else { "-->" };
        let label = if e.label.is_empty() { String::new() } else { format!("|{}|", q(&e.label)) };
        s.push_str(&format!("    {} {arrow}{label} {}\n", f.nodes[e.from].id, f.nodes[e.to].id));
    }
    for n in &f.nodes {
        let mut parts = Vec::new();
        if let Some(c) = n.fill {
            parts.push(format!("fill:#{c:06x}"));
        }
        if let Some(c) = n.text {
            parts.push(format!("color:#{c:06x}"));
        }
        if let Some(c) = n.border {
            parts.push(format!("stroke:#{c:06x}"));
        }
        if !parts.is_empty() {
            s.push_str(&format!("    style {} {}\n", n.id, parts.join(",")));
        }
    }
    s
}
