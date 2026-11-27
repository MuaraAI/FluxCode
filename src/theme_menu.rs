use crate::i18n::tr;
use crate::settings::theme_swatch;
use crate::theme::{self, c, GlassExt};
use crate::workbench::*;
use gpui_kit::component::{h_flex, v_flex, Icon, IconName};
use gpui_kit::{prelude::FluentBuilder, *};

fn rgb_of(v: u32) -> Hsla {
    rgb(v).into()
}

impl Workbench {
    pub(crate) fn render_theme_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette();
        let open = self.theme_menu;
        h_flex()
            .id("sb-theme")
            .flex_none()
            .whitespace_nowrap()
            .h_full()
            .px_2()
            .mr_1()
            .gap_1p5()
            .cursor_pointer()
            .when(open, |d| {
                d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG))
            })
            .hover(|d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
            .child(
                div()
                    .flex_none()
                    .size(theme::u(8.))
                    .rounded_full()
                    .bg(rgb_of(p.swatches()[2]))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG)),
            )
            .child(p.name)
            .when(!open, |d| {
                d.tooltip(|w, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tr(
                        "Color theme (Ctrl+K Ctrl+T for the next one)",
                    ))
                    .build(w, cx)
                })
            })
            .on_click(cx.listener(|this, _, w, cx| {
                let may = this.pop_may_open("theme");
                let opening = !this.theme_menu && may;
                this.theme_menu = opening;
                if opening {
                    this.begin_theme_preview();
                } else {
                    this.cancel_theme_preview(w, cx);
                }
                cx.notify();
            }))
            .into_any_element()
    }

    pub(crate) fn render_theme_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.theme_menu {
            return None;
        }
        let committed = self.committed_theme_id();
        let current_live = theme::palette().id;
        let icon_themes = crate::ext::vsx_icons::list();
        let active_icon = crate::ext::vsx_icons::active_id();
        let mut list = v_flex()
            .id("theme-list")
            .p_1()
            .gap_0p5()
            .max_h(theme::u(if icon_themes.is_empty() { 380. } else { 240. }))
            .overflow_y_scroll();
        for (i, p) in theme::all().into_iter().enumerate() {
            let on = p.id == committed;
            let live = p.id == current_live;
            let id = p.id;
            list = list.child(
                h_flex()
                    .id(("theme-pick", i))
                    .h(theme::u(34.))
                    .px_2()
                    .gap_2p5()
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(live, |d| d.bg(c(theme::BG_HOVER)))
                    .hover(|d| d.bg(c(theme::BG_HOVER)))
                    .on_hover(cx.listener(move |this, entered: &bool, w, cx| {
                        if *entered {
                            this.preview_theme(id, w, cx);
                        }
                    }))
                    .child(theme_swatch(p))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(if on {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(c(theme::FG_STRONG))
                                    .child(p.name),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(tr(p.note)),
                            ),
                    )
                    .child(div().flex_none().w(theme::u(16.)).when(on, |d| {
                        d.child(
                            Icon::new(IconName::Check)
                                .size(theme::u(14.))
                                .text_color(c(theme::ACCENT_TEXT)),
                        )
                    }))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.theme_menu = false;
                        this.commit_theme(id, w, cx);
                        cx.notify();
                    })),
            );
        }
        let icon_section = (!icon_themes.is_empty()).then(|| {
            let mut ilist = v_flex()
                .id("icon-theme-list")
                .p_1()
                .gap_0p5()
                .max_h(theme::u(140.))
                .overflow_y_scroll();
            for (i, m) in icon_themes.iter().enumerate() {
                let on = m.id == active_icon;
                let tid = m.id.clone();
                ilist = ilist.child(
                    h_flex()
                        .id(("icon-theme-pick", i))
                        .h(theme::u(28.))
                        .px_2()
                        .gap_2()
                        .items_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(c(theme::BG_HOVER)))
                        .hover(|d| d.bg(c(theme::BG_HOVER)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .font_weight(if on {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(c(theme::FG_STRONG))
                                .child(m.label.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .max_w(theme::u(72.))
                                .truncate()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(m.ext_display.clone()),
                        )
                        .child(div().flex_none().w(theme::u(16.)).when(on, |d| {
                            d.child(
                                Icon::new(IconName::Check)
                                    .size(theme::u(13.))
                                    .text_color(c(theme::ACCENT_TEXT)),
                            )
                        }))
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.theme_menu = false;
                            this.cancel_theme_preview(w, cx);
                            this.set_icon_theme(&tid, cx);
                        })),
                );
            }
            v_flex()
                .border_t_1()
                .border_color(c(theme::BORDER))
                .child(
                    div()
                        .px_3()
                        .pt_2()
                        .pb_0p5()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_MUTED))
                        .child(tr("File icon theme")),
                )
                .child(ilist)
        });
        let menu = v_flex()
            .id("theme-menu")
            .key_context("ThemeMenu")
            .absolute()
            .right(px(8.))
            .bottom(theme::u(30.))
            .w(theme::u(264.))
            .max_w(relative(0.92))
            .glass_pop()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, w, cx| {
                this.pop_closed_outside("theme");
                this.theme_menu = false;
                this.cancel_theme_preview(w, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" {
                    this.theme_menu = false;
                    this.cancel_theme_preview(w, cx);
                    cx.notify();
                }
            }))
            .child(
                h_flex()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(theme::FG_MUTED))
                            .child(tr("Color theme")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(tr("Hover to preview")),
                    ),
            )
            .child(list)
            .children(icon_section)
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
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(tr("Background image and more in Settings")),
                    )
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.theme_menu = false;
                        this.cancel_theme_preview(w, cx);
                        w.dispatch_action(Box::new(ShowSettings), cx);
                    })),
            );
        let el = if cx.reduce_motion() {
            menu.into_any_element()
        } else {
            menu.with_animation(
                "theme-menu-in",
                Animation::new(std::time::Duration::from_millis(140))
                    .with_easing(gpui_kit::ease_out_quint()),
                |el, t| el.opacity(t).bottom(theme::u(30.) - px((1. - t) * 6.)),
            )
            .into_any_element()
        };
        Some(el)
    }
}
