//! Pictures the agent shows: screenshots its tools take (a browser or desktop screenshot, an image a tool read) and images in its answers.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

/// A picture larger than this (decoded) is left out: no tool screenshot is near it.
const MAX_BYTES: usize = 40 * 1024 * 1024;
/// Cached pictures kept; the oldest go first.
const KEEP: usize = 300;
/// Preview height in the chat (before UI scaling).
const THUMB_H: f32 = 132.;

#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub path: PathBuf,
    /// Pixel size, when the header could be read.
    pub size: Option<(u32, u32)>,
}

/// The full-screen viewer: the pictures of one strip and the one shown.
pub struct Lightbox {
    pub shots: Vec<Shot>,
    pub ix: usize,
    pub focus: FocusHandle,
}

fn cache_dir() -> PathBuf {
    crate::settings::config_dir().join("agent-images")
}

/// Every image block in `v` (any depth: tool content wraps them differently per agent), saved to the cache.
pub fn collect(v: &Value) -> Vec<Shot> {
    let mut out = Vec::new();
    walk(v, &mut out, 0);
    out
}

fn walk(v: &Value, out: &mut Vec<Shot>, depth: usize) {
    if depth > 8 || out.len() >= 16 {
        return;
    }
    match v {
        Value::Object(m) => {
            let is_image = m.get("type").and_then(Value::as_str) == Some("image");
            if let (true, Some(data)) = (is_image, m.get("data").and_then(Value::as_str)) {
                let mime = m.get("mimeType").or_else(|| m.get("mime_type")).and_then(Value::as_str).unwrap_or("image/png");
                if let Some(s) = save(data, mime) {
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
                return;
            }
            // Anthropic-style blocks: {"type":"image","source":{"type":"base64","data":..,"media_type":..}}
            if is_image {
                if let Some(src) = m.get("source") {
                    let data = src.get("data").and_then(Value::as_str);
                    let mime = src.get("media_type").and_then(Value::as_str).unwrap_or("image/png");
                    if let Some(s) = data.and_then(|d| save(d, mime)) {
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                    return;
                }
            }
            for x in m.values() {
                walk(x, out, depth + 1);
            }
        }
        Value::Array(a) => {
            for x in a {
                walk(x, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// Decode and write one picture to the cache; the same picture twice is one file.
fn save(data: &str, mime: &str) -> Option<Shot> {
    use base64::Engine as _;
    if data.len() > MAX_BYTES / 3 * 4 + 8 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).ok()?;
    let ext = match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        _ => "png",
    };
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!("{}.{ext}", &sha1_smol::Sha1::from(&bytes).digest().to_string()[..20]);
    let path = dir.join(name);
    if !path.is_file() {
        std::fs::write(&path, &bytes).ok()?;
    }
    let size = image::image_dimensions(&path).ok();
    Some(Shot { path, size })
}

/// Keep the cache small: the newest `KEEP` pictures stay (run off the UI thread).
pub fn prune_cache() {
    let Ok(rd) = std::fs::read_dir(cache_dir()) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
    if files.len() <= KEEP {
        return;
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in files.into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(p);
    }
}

impl Workbench {
    /// Open the full-screen viewer on picture `ix` of `shots`.
    pub(crate) fn agent_open_lightbox(&mut self, shots: Vec<Shot>, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.agent.lightbox = Some(Lightbox { ix: ix.min(shots.len().saturating_sub(1)), shots, focus });
        cx.notify();
    }

    fn lightbox_step(&mut self, d: isize, cx: &mut Context<Self>) {
        if let Some(lb) = self.agent.lightbox.as_mut() {
            let n = lb.shots.len() as isize;
            if n > 1 {
                lb.ix = ((lb.ix as isize + d).rem_euclid(n)) as usize;
                cx.notify();
            }
        }
    }

    fn lightbox_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.agent.lightbox = None;
        let h = self.agent.input.focus_handle(cx);
        h.focus(window, cx);
        cx.notify();
    }

    /// Under a step: the pictures it produced, as previews in a row (one tall preview when it is a single picture), with a header to fold them away.
    pub(crate) fn render_shots(&self, i: usize, shots: &[Shot], hidden: bool, cx: &mut Context<Self>) -> AnyElement {
        let n = shots.len();
        let label: SharedString = if n == 1 { tr("Screenshot").into() } else { crate::trf!("{n} screenshots"; n => n).into() };
        let header = h_flex()
            .id(("ag-shots-h", i))
            .gap_1p5()
            .h(theme::u(22.))
            .px_1()
            .rounded(px(6.))
            .cursor_pointer()
            .text_xs()
            .text_color(c(theme::FG_DIM))
            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_MUTED)))
            .child(Icon::new(Ico::Image).size(theme::u(12.)))
            .child(label)
            .when(hidden, |h| {
                // Folded: the first picture as a tiny thumbnail, so it's clear what is there.
                h.child(img(shots[0].path.clone()).h(theme::u(16.)).max_w(theme::u(28.)).rounded(px(3.)).object_fit(ObjectFit::Cover))
            })
            .child(Icon::new(if hidden { Ico::ChevronRight } else { Ico::ChevronDown }).size(theme::u(11.)))
            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(if hidden { tr("Show the preview") } else { tr("Hide the preview") }).build(w, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(super::Item::Images { hidden, .. }) = this.agent.items.get_mut(i) {
                    *hidden = !*hidden;
                }
                this.agent.remeasure.borrow_mut().push(i..i + 1);
                cx.notify();
            }));
        let mut col = v_flex().w_full().gap_1().pl(theme::u(22.)).child(header);
        if !hidden {
            let h = if n == 1 { THUMB_H * 1.35 } else { THUMB_H };
            let mut row = h_flex().w_full().flex_wrap().gap_1p5();
            for (k, s) in shots.iter().enumerate() {
                let (w, hh) = s.size.unwrap_or((16, 10));
                let aspect = (w.max(1) as f32 / hh.max(1) as f32).clamp(0.3, 4.);
                let all = shots.to_vec();
                row = row.child(
                    div()
                        .id(("ag-shot", i * 64 + k))
                        .group("ag-shot")
                        .relative()
                        .h(theme::u(h))
                        .w(theme::u(h * aspect))
                        .max_w_full()
                        .rounded(px(8.))
                        .overflow_hidden()
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .bg(c(theme::BG_ELEVATED))
                        .cursor_pointer()
                        .hover(|s| s.border_color(c(theme::ACCENT)))
                        .child(img(s.path.clone()).size_full().object_fit(ObjectFit::Contain))
                        // A full-screen mark in the corner while the pointer is on it.
                        .child(
                            div()
                                .absolute()
                                .top(px(6.))
                                .right(px(6.))
                                .size(theme::u(22.))
                                .rounded(px(6.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::black().opacity(0.55))
                                .text_color(gpui_kit::white())
                                .invisible()
                                .group_hover("ag-shot", |s| s.visible())
                                .child(Icon::new(Ico::Maximize2).size(theme::u(12.))),
                        )
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Full screen")).build(w, cx))
                        .on_click(cx.listener(move |this, _, w, cx| this.agent_open_lightbox(all.clone(), k, w, cx))),
                );
            }
            col = col.child(row);
        }
        col.into_any_element()
    }

    /// The full-screen viewer over the whole window: the picture fitted, a dim backdrop that fades in, arrows between pictures, and its actions on top.
    pub(crate) fn render_lightbox(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let lb = self.agent.lightbox.as_ref()?;
        let shot = lb.shots.get(lb.ix)?.clone();
        let n = lb.shots.len();
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        // Fit inside the window with room for the bar and the arrows; never past 2x.
        let (aw, ah) = ((vw - 160.).max(120.), (vh - 150.).max(120.));
        let scale = window.scale_factor().max(0.5);
        let (pw, ph) = match shot.size {
            Some((w, h)) => {
                let (lw, lh) = (w as f32 / scale, h as f32 / scale);
                let k = (aw / lw).min(ah / lh).min(2.);
                (lw * k, lh * k)
            }
            None => (aw, ah),
        };
        let info: SharedString = match shot.size {
            Some((w, h)) if n > 1 => format!("{} / {n} · {w} × {h}", lb.ix + 1).into(),
            Some((w, h)) => format!("{w} × {h}").into(),
            None if n > 1 => format!("{} / {n}", lb.ix + 1).into(),
            None => SharedString::default(),
        };
        let bar_btn = |id: &'static str, icon: Ico, tip: &'static str| {
            div()
                .id(id)
                .size(theme::u(32.))
                .rounded(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(gpui_kit::white().opacity(0.85))
                .hover(|s| s.bg(gpui_kit::white().opacity(0.12)).text_color(gpui_kit::white()))
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tr(tip)).build(w, cx))
                .child(Icon::new(icon).size(theme::u(16.)))
        };
        let arrow = |id: &'static str, icon: Ico, d: isize, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(theme::u(44.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .bg(gpui_kit::black().opacity(0.45))
                .text_color(gpui_kit::white().opacity(0.9))
                .hover(|s| s.bg(gpui_kit::black().opacity(0.7)))
                .child(Icon::new(icon).size(theme::u(20.)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.lightbox_step(d, cx)
                }))
        };
        let open_path = shot.path.clone();
        let copy_path = shot.path.to_string_lossy().into_owned();
        let picture = div()
            .id("ag-lb-pic")
            .w(px(pw))
            .h(px(ph))
            .rounded(px(10.))
            .overflow_hidden()
            .shadow_lg()
            // A click on the picture itself doesn't close the viewer.
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(img(shot.path.clone()).size_full().object_fit(ObjectFit::Contain))
            .with_animation(SharedString::from(format!("ag-lb-in-{}", shot.path.display())), Animation::new(Duration::from_millis(180)).with_easing(gpui_kit::ease_out_quint()), |d, t| d.opacity(t));
        Some(
            div()
                .id("ag-lightbox")
                .track_focus(&lb.focus)
                .occlude()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .flex_col()
                .bg(gpui_kit::black().opacity(0.86))
                .on_click(cx.listener(|this, _, w, cx| this.lightbox_close(w, cx)))
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| match e.keystroke.key.as_str() {
                    "escape" => this.lightbox_close(w, cx),
                    "left" => this.lightbox_step(-1, cx),
                    "right" => this.lightbox_step(1, cx),
                    _ => {}
                }))
                .child(
                    h_flex()
                        .id("ag-lb-bar")
                        .flex_none()
                        .h(theme::u(52.))
                        .px_4()
                        .gap_1()
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(Icon::new(Ico::Image).size(theme::u(15.)).text_color(gpui_kit::white().opacity(0.7)))
                        .child(div().pl_1().text_sm().text_color(gpui_kit::white().opacity(0.9)).child(tr("Screenshot")))
                        .child(div().pl_2().text_xs().text_color(gpui_kit::white().opacity(0.55)).child(info))
                        .child(div().flex_1())
                        .child(bar_btn("ag-lb-open", Ico::ExternalLink, "Open in a tab").on_click(cx.listener(move |this, _, w, cx| {
                            cx.stop_propagation();
                            let p = open_path.clone();
                            this.lightbox_close(w, cx);
                            this.open_file(p, w, cx);
                        })))
                        .child(bar_btn("ag-lb-copy", Ico::Copy, "Copy the file path").on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
                        }))
                        .child(bar_btn("ag-lb-close", Ico::Close, "Close (Esc)").on_click(cx.listener(|this, _, w, cx| {
                            cx.stop_propagation();
                            this.lightbox_close(w, cx)
                        }))),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .px_4()
                        .pb(theme::u(28.))
                        .items_center()
                        .justify_center()
                        .gap_4()
                        .when(n > 1, |h| h.child(arrow("ag-lb-prev", Ico::ChevronLeft, -1, cx)))
                        .child(picture)
                        .when(n > 1, |h| h.child(arrow("ag-lb-next", Ico::ChevronRight, 1, cx))),
                )
                .with_animation("ag-lightbox-fade", Animation::new(Duration::from_millis(160)).with_easing(gpui_kit::ease_out_quint()), |d, t| d.opacity(t))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::collect;

    // A 1x1 PNG.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn images_are_found_in_tool_content_and_raw_output() {
        let tool = serde_json::json!({
            "content": [{"type": "content", "content": {"type": "image", "data": PNG, "mimeType": "image/png"}}],
            "rawOutput": {"content": [{"type": "text", "text": "ok"}, {"type": "image", "source": {"type": "base64", "data": PNG, "media_type": "image/png"}}]}
        });
        let shots = collect(&tool);
        // The same picture twice is one file.
        assert_eq!(shots.len(), 1);
        assert_eq!(shots[0].size, Some((1, 1)));
        assert!(shots[0].path.is_file());
        assert!(collect(&serde_json::json!({"content": [{"type": "text", "text": "no picture"}]})).is_empty());
        assert!(collect(&serde_json::json!({"type": "image", "data": "not base64!"})).is_empty());
    }
}
