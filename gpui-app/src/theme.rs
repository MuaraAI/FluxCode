//! FluxCode's dark palette (same values as `src/styles.css`) applied on top
//! of gpui-component's dark theme.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{rgb, App, Hsla};

pub const BG_EDITOR: u32 = 0x1e1f22;
pub const BG_SIDE: u32 = 0x18191c;
pub const BG_TITLE: u32 = 0x141518;
pub const BG_ELEVATED: u32 = 0x24262b;
pub const BORDER: u32 = 0x2a2c31;
pub const FG: u32 = 0xcfd2d8;
pub const FG_STRONG: u32 = 0xf1f3f6;
pub const FG_MUTED: u32 = 0x949aa4;
pub const FG_DIM: u32 = 0x6b717c;
pub const ACCENT: u32 = 0x3b8eea;
pub const FOLDER: u32 = 0xd9a74a;
pub const SELECT: u32 = 0x1d3b5c;

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |t| {
        t.font_size = gpui_kit::px(13.);
        t.mono_font_family = "Cascadia Code".into();
        t.mono_font_size = gpui_kit::px(14.);
        t.radius = gpui_kit::px(6.);

        let k = &mut t.colors;
        k.background = c(BG_EDITOR);
        k.foreground = c(FG);
        k.muted_foreground = c(FG_MUTED);
        k.border = c(BORDER);
        k.accent = c(BG_ELEVATED);
        k.accent_foreground = c(FG_STRONG);
        k.primary = c(ACCENT);
        k.ring = c(ACCENT);
        k.caret = c(0xc8ccd4);
        k.selection = c(0x264f78);
        k.sidebar = c(BG_SIDE);
        k.sidebar_foreground = c(FG);
        k.sidebar_border = c(BORDER);
        k.title_bar = c(BG_TITLE);
        k.title_bar_border = c(BORDER);
        k.status_bar = c(BG_TITLE);
        k.status_bar_border = c(BORDER);
        k.tab_bar = c(BG_SIDE);
        k.tab = c(BG_SIDE);
        k.tab_active = c(BG_EDITOR);
        k.tab_foreground = c(FG_MUTED);
        k.tab_active_foreground = c(FG_STRONG);
        k.list = c(BG_SIDE);
        k.list_hover = c(0x232529);
        k.list_active = c(SELECT);
        k.list_active_border = c(ACCENT);
        k.popover = c(BG_ELEVATED);
        k.input = c(0x2a2c31);
        k.window_border = c(BORDER);
        k.scrollbar = gpui_kit::transparent_black();
    });
}
