//! The color theme from the status bar: its name there opens a small list above it,
//! each theme drawn as a little editor in its own colors (surface, side bar, a line of
//! text, the accent), so you pick by looking. Escape or a click outside closes it.

use crate::i18n::tr;
use crate::theme::{self, c, PALETTES};
use crate::workbench::*;
use gpui_kit::component::{h_flex, v_flex, Icon, IconName};
use gpui_kit::{prelude::FluentBuilder, *};

fn rgb_of(v: u32) -> Hsla {
    rgb(v).into()
}

/// A theme in miniature: the side bar, the editor with two lines of text, the accent.
fn thumb(p: &theme::Palette) -> Div {
    let [editor, side, accent, fg] = p.swatches();
    h_flex()
        .flex_none()
        .w(theme::u(46.))
        .h(theme::u(30.))
        .rounded(px(4.))
        .overflow_hidden()
        .border_1()
        .border_color(c(theme::BORDER_STRONG))
        .child(div().w(theme::u(11.)).h_full().bg(rgb_of(side)))
        .child(
            v_flex()
                .flex_1()
                .h_full()
                .px(theme::u(5.))
                .py(theme::u(6.))
                .gap(theme::u(4.))
                .bg(rgb_of(editor))
                .child(div().w(relative(0.8)).h(px(2.)).rounded(px(1.)).bg(rgb_of(fg)).opacity(0.85))
                .child(div().w(relative(0.5)).h(px(2.)).rounded(px(1.)).bg(rgb_of(accent)))
                .child(div().w(relative(0.65)).h(px(2.)).rounded(px(1.)).bg(rgb_of(fg)).opacity(0.5)),
        )
}

impl Workbench {
    /// The theme's name at the right of the status bar: a dot of its accent, the name.
    pub(crate) fn render_theme_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette();
        let open = self.theme_menu;
        h_flex()
            .id("sb-theme")
            .h_full()
            .px_2()
            .mr_1()
            .gap_1p5()
            .cursor_pointer()
            .when(open, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            .hover(|d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            .child(div().size(theme::u(8.)).rounded_full().bg(rgb_of(p.swatches()[2])).border_1().border_color(c(theme::BORDER_STRONG)))
            .child(p.name)
            // No tooltip while the list is open: it would cover the list's last row.
            .when(!open, |d| d.tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Color theme (Ctrl+K Ctrl+T for the next one)")).build(w, cx)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.theme_menu = !this.theme_menu;
                cx.notify();
            }))
            .into_any_element()
    }

    /// The list above the status bar's right end.
    pub(crate) fn render_theme_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.theme_menu {
            return None;
        }
        let current = theme::palette().id;
        let mut list = v_flex().p_1().gap_0p5();
        for (i, p) in PALETTES.iter().enumerate() {
            let on = p.id == current;
            let id = p.id;
            list = list.child(
                h_flex()
                    .id(("theme-pick", i))
                    .px_2()
                    .py_1p5()
                    .gap_2p5()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(c(theme::BG_HOVER)))
                    .hover(|d| d.bg(c(theme::BG_HOVER)))
                    .child(thumb(p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_sm().font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).text_color(c(theme::FG_STRONG)).child(p.name))
                            .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(tr(p.note))),
                    )
                    .child(div().flex_none().w(theme::u(16.)).when(on, |d| d.child(Icon::new(IconName::Check).size(theme::u(14.)).text_color(c(theme::ACCENT_TEXT)))))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.theme_menu = false;
                        this.set_theme(id, w, cx);
                        cx.notify();
                    })),
            );
        }
        Some(
            v_flex()
                .id("theme-menu")
                .key_context("ThemeMenu")
                .absolute()
                .right(px(8.))
                .bottom(theme::u(30.))
                .w(theme::u(272.))
                .max_w(relative(0.92))
                .rounded(px(8.))
                .border_1()
                .border_color(c(theme::BORDER_STRONG))
                .bg(c(theme::BG_ELEVATED))
                .shadow_md()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.theme_menu = false;
                    cx.notify();
                }))
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "escape" {
                        this.theme_menu = false;
                        cx.notify();
                    }
                }))
                .child(div().px_3().pt_2().pb_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr("Color theme")))
                .child(list)
                .child(
                    h_flex()
                        .id("theme-more")
                        .mx_1()
                        .mb_1()
                        .px_2()
                        .py_1p5()
                        .gap_2()
                        .rounded(px(5.))
                        .border_t_1()
                        .border_color(c(theme::BORDER))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .hover(|d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                        .child(Icon::new(IconName::Settings).size(theme::u(13.)))
                        .child(div().flex_1().child(tr("Background image and more in Settings")))
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.theme_menu = false;
                            w.dispatch_action(Box::new(ShowSettings), cx);
                        })),
                )
                .into_any_element(),
        )
    }
}
