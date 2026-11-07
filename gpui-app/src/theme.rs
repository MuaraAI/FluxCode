//! "Blocky" theme: dark neo-brutalism. Square corners, 2px borders, hard
//! offset shadows, solid accent blocks. Applied on top of gpui-component's
//! dark theme so its widgets (inputs, menus, buttons) follow it too.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{rgb, App, Hsla};

// Surfaces, darkest to lightest.
pub const BG_TITLE: u32 = 0x0c0c0f;
pub const BG_SIDE: u32 = 0x15151a;
pub const BG_EDITOR: u32 = 0x1a1a20;
pub const BG_ELEVATED: u32 = 0x24242c;
pub const BG_HOVER: u32 = 0x2c2c36;
// Lines and the hard shadow.
pub const BORDER: u32 = 0x34343f;
pub const BORDER_STRONG: u32 = 0x55556a;
pub const INK: u32 = 0x000000;
// Text.
pub const FG: u32 = 0xe6e6ea;
pub const FG_STRONG: u32 = 0xffffff;
pub const FG_MUTED: u32 = 0x9d9daa;
pub const FG_DIM: u32 = 0x62626f;
// Solid accent blocks.
pub const ACCENT: u32 = 0xffd60a; // yellow: primary, active
pub const BLUE: u32 = 0x4d7cfe;
pub const PINK: u32 = 0xff4f8b;
pub const GREEN: u32 = 0x2bd97c;
pub const FOLDER: u32 = 0xffb84d;
pub const SELECT: u32 = 0x2e2a12; // yellow-tinted row selection

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |t| {
        t.font_size = gpui_kit::px(13.);
        t.mono_font_family = "Cascadia Code".into();
        t.mono_font_size = gpui_kit::px(14.);
        t.radius = gpui_kit::px(0.);
        t.radius_lg = gpui_kit::px(0.);

        let k = &mut t.colors;
        k.background = c(BG_EDITOR);
        k.foreground = c(FG);
        k.muted = c(BG_ELEVATED);
        k.muted_foreground = c(FG_MUTED);
        k.border = c(BORDER);
        k.input = c(BORDER_STRONG);
        k.ring = c(ACCENT);
        k.accent = c(BG_HOVER);
        k.accent_foreground = c(FG_STRONG);
        k.primary = c(ACCENT);
        k.primary_hover = c(0xffe14d);
        k.primary_active = c(0xe6c000);
        k.primary_foreground = c(INK);
        k.button_primary = c(ACCENT);
        k.button_primary_hover = c(0xffe14d);
        k.button_primary_active = c(0xe6c000);
        k.button_primary_foreground = c(INK);
        k.secondary = c(BG_ELEVATED);
        k.secondary_hover = c(BG_HOVER);
        k.secondary_foreground = c(FG);
        k.danger = c(PINK);
        k.caret = c(ACCENT);
        k.selection = c(0x4d4316);
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
        k.list_hover = c(BG_HOVER);
        k.list_active = c(SELECT);
        k.list_active_border = c(ACCENT);
        k.popover = c(BG_ELEVATED);
        k.popover_foreground = c(FG);
        k.window_border = c(BORDER);
        k.scrollbar = gpui_kit::transparent_black();
        k.scrollbar_thumb = c(BORDER_STRONG);
        k.scrollbar_thumb_hover = c(ACCENT);
    });
}
