//! Color themes: FluxDark, FluxCream, FluxMono and FluxRed.
//!
//! Every color in the UI is a token (`theme::FG`, `theme::ACCENT`, ...). `c(token)`
//! reads the active palette, so switching themes only needs `set` + `apply` and a
//! redraw; no call site changes. Raw hex colors (file-type badges) go through `c`
//! too and stay as they are.
//!
//! With a background image, the panel surfaces become translucent: the image
//! shows through every panel at the chosen strength, and more on the right side
//! of the editor than on the code side (see `editor_wash`). The secondary text
//! colors were chosen so they still reach WCAG AA (4.5:1) over a bright image at
//! the default strength; see `Visibility`.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{linear_color_stop, linear_gradient, rgb, App, Background, Hsla};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tok {
    BgTitle,
    BgSide,
    BgEditor,
    BgElevated,
    BgHover,
    Border,
    BorderStrong,
    Fg,
    FgStrong,
    FgMuted,
    FgDim,
    /// Fills: active tab line, primary button, focus ring.
    Accent,
    /// Text drawn on an accent fill.
    OnAccent,
    /// Accent used as text on a panel surface (lighter or darker than the fill, for contrast).
    AccentText,
    Blue,
    Green,
    Danger,
    Folder,
    /// Selected row in a list or tree.
    Select,
    /// Selected text in the editor.
    Selection,
}

pub const BG_TITLE: Tok = Tok::BgTitle;
pub const BG_SIDE: Tok = Tok::BgSide;
pub const BG_EDITOR: Tok = Tok::BgEditor;
pub const BG_ELEVATED: Tok = Tok::BgElevated;
pub const BG_HOVER: Tok = Tok::BgHover;
pub const BORDER: Tok = Tok::Border;
pub const BORDER_STRONG: Tok = Tok::BorderStrong;
pub const FG: Tok = Tok::Fg;
pub const FG_STRONG: Tok = Tok::FgStrong;
pub const FG_MUTED: Tok = Tok::FgMuted;
pub const FG_DIM: Tok = Tok::FgDim;
pub const ACCENT: Tok = Tok::Accent;
pub const ON_ACCENT: Tok = Tok::OnAccent;
pub const ACCENT_TEXT: Tok = Tok::AccentText;
pub const BLUE: Tok = Tok::Blue;
pub const GREEN: Tok = Tok::Green;
pub const DANGER: Tok = Tok::Danger;
pub const FOLDER: Tok = Tok::Folder;
pub const SELECT: Tok = Tok::Select;

pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    /// One line for the Settings card.
    pub note: &'static str,
    pub dark: bool,
    title: u32,
    side: u32,
    editor: u32,
    elevated: u32,
    hover: u32,
    border: u32,
    border_strong: u32,
    fg: u32,
    strong: u32,
    muted: u32,
    dim: u32,
    accent: u32,
    on_accent: u32,
    accent_text: u32,
    blue: u32,
    green: u32,
    danger: u32,
    folder: u32,
    select: u32,
    selection: u32,
}

impl Palette {
    fn hex(&self, t: Tok) -> u32 {
        match t {
            Tok::BgTitle => self.title,
            Tok::BgSide => self.side,
            Tok::BgEditor => self.editor,
            Tok::BgElevated => self.elevated,
            Tok::BgHover => self.hover,
            Tok::Border => self.border,
            Tok::BorderStrong => self.border_strong,
            Tok::Fg => self.fg,
            Tok::FgStrong => self.strong,
            Tok::FgMuted => self.muted,
            Tok::FgDim => self.dim,
            Tok::Accent => self.accent,
            Tok::OnAccent => self.on_accent,
            Tok::AccentText => self.accent_text,
            Tok::Blue => self.blue,
            Tok::Green => self.green,
            Tok::Danger => self.danger,
            Tok::Folder => self.folder,
            Tok::Select => self.select,
            Tok::Selection => self.selection,
        }
    }

    /// Swatches for the Settings card: surface, side bar, accent, text.
    pub fn swatches(&self) -> [u32; 4] {
        [self.editor, self.side, self.accent, self.fg]
    }
}

// Contrast (checked with a script, WCAG 2.1): fg, muted, dim and accent text are all
// >= 4.5:1 on every surface, and still >= 4.5:1 over a white (dark themes) or black
// (FluxCream) image pixel at the Balanced strength. Text on accent fills >= 5.2:1.
pub const PALETTES: [Palette; 4] = [
    Palette {
        id: "flux-dark",
        name: "FluxDark",
        note: "Near-black, crimson accent",
        dark: true,
        title: 0x0d0a0b,
        side: 0x141012,
        editor: 0x181314,
        elevated: 0x231b1d,
        hover: 0x2c2225,
        border: 0x3d3033,
        border_strong: 0x4d3b3f,
        fg: 0xebe3e4,
        strong: 0xffffff,
        muted: 0xb3a4a7,
        dim: 0xaa9c9f,
        accent: 0xc8323f,
        on_accent: 0xffffff,
        accent_text: 0xf07d86,
        blue: 0x7fb2ff,
        green: 0x6fd39a,
        danger: 0xff7a85,
        folder: 0xd9a86c,
        select: 0x35191e,
        selection: 0x5a2830,
    },
    Palette {
        id: "flux-cream",
        name: "FluxCream",
        note: "Warm paper, red accent",
        dark: false,
        title: 0xefe3dc,
        side: 0xf5ebe5,
        editor: 0xfbf6f2,
        elevated: 0xffffff,
        hover: 0xeadbd3,
        border: 0xd6c3ba,
        border_strong: 0xc4ada3,
        fg: 0x2b2124,
        strong: 0x170f11,
        muted: 0x5e4b4f,
        dim: 0x685459,
        accent: 0xa82a37,
        on_accent: 0xffffff,
        accent_text: 0x9a2431,
        blue: 0x2457b8,
        green: 0x1a7444,
        danger: 0xb42332,
        folder: 0x9a6224,
        select: 0xf0d6d4,
        selection: 0xefc6c8,
    },
    Palette {
        id: "flux-mono",
        name: "FluxMono",
        note: "Grey UI, red only for errors",
        dark: true,
        title: 0x0b0b0b,
        side: 0x121212,
        editor: 0x161616,
        elevated: 0x202020,
        hover: 0x282828,
        border: 0x383838,
        border_strong: 0x444444,
        fg: 0xe6e6e6,
        strong: 0xffffff,
        muted: 0xa9a9a9,
        dim: 0xa2a2a2,
        accent: 0xe2e2e2,
        on_accent: 0x111111,
        accent_text: 0xffffff,
        blue: 0xc4c4c4,
        green: 0xd4d4d4,
        danger: 0xff5a64,
        folder: 0xbdbdbd,
        select: 0x2c2c2c,
        selection: 0x3d3d3d,
    },
    Palette {
        id: "flux-red",
        name: "FluxRed",
        note: "Surfaces tinted deep red",
        dark: true,
        title: 0x1c080b,
        side: 0x260b10,
        editor: 0x2b0d12,
        elevated: 0x3a131a,
        hover: 0x46171f,
        border: 0x5a2129,
        border_strong: 0x6e2a33,
        fg: 0xf6e6e8,
        strong: 0xffffff,
        muted: 0xd9b3b8,
        dim: 0xc79aa0,
        accent: 0xf0606c,
        on_accent: 0x1c080b,
        accent_text: 0xff8f99,
        blue: 0x9cc2ff,
        green: 0x86e3ad,
        danger: 0xffc46b,
        folder: 0xf2b48a,
        select: 0x531a24,
        selection: 0x6e2331,
    },
];

/// How much of the background image reaches the eye through the panels.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
pub enum Visibility {
    Subtle,
    #[default]
    Balanced,
    /// Above the level the text colors were checked at: dim text can drop below
    /// 4.5:1 over very bright pictures. Settings says so next to the option.
    Strong,
}

impl Visibility {
    pub const ALL: [Visibility; 3] = [Visibility::Subtle, Visibility::Balanced, Visibility::Strong];

    pub fn label(self) -> &'static str {
        match self {
            Visibility::Subtle => "Subtle",
            Visibility::Balanced => "Balanced",
            Visibility::Strong => "Strong",
        }
    }

    /// Image strength at the most exposed spot (the editor's right edge).
    fn strength(self) -> f32 {
        match self {
            Visibility::Subtle => 0.10,
            Visibility::Balanced => 0.15,
            Visibility::Strong => 0.25,
        }
    }
}

/// Size of the whole interface (text, rows, bars, icons). The editor's own font
/// size is a separate setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
pub enum UiSize {
    Compact,
    #[default]
    Medium,
    Large,
}

impl UiSize {
    pub const ALL: [UiSize; 3] = [UiSize::Compact, UiSize::Medium, UiSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            UiSize::Compact => "Compact",
            UiSize::Medium => "Medium",
            UiSize::Large => "Large",
        }
    }

    fn factor(self) -> f32 {
        match self {
            UiSize::Compact => 1.0,
            UiSize::Medium => 1.15,
            UiSize::Large => 1.3,
        }
    }
}

static ACTIVE: AtomicU8 = AtomicU8::new(0);
static UI_SIZE: AtomicU8 = AtomicU8::new(1);

pub fn set_ui_size(size: UiSize) {
    UI_SIZE.store(UiSize::ALL.iter().position(|s| *s == size).unwrap_or(1) as u8, Ordering::Relaxed);
}

/// The interface scale factor (1.0 = Compact).
pub fn scale() -> f32 {
    UiSize::ALL[UI_SIZE.load(Ordering::Relaxed) as usize % 3].factor()
}

/// A fixed size in the chrome (bar heights, rows, icons), scaled with the interface size.
pub fn u(v: f32) -> gpui_kit::Pixels {
    gpui_kit::px((v * scale()).round())
}

/// The window's rem size: GPUI's spacing and text utilities (`px_3`, `text_sm`, ...) follow it.
pub fn rem() -> gpui_kit::Pixels {
    gpui_kit::px(16. * scale())
}
/// 0 = no image, otherwise Visibility index + 1.
static IMAGE: AtomicU8 = AtomicU8::new(0);

pub fn palette() -> &'static Palette {
    &PALETTES[ACTIVE.load(Ordering::Relaxed) as usize % PALETTES.len()]
}

fn image() -> Option<Visibility> {
    match IMAGE.load(Ordering::Relaxed) {
        0 => None,
        n => Visibility::ALL.get(n as usize - 1).copied(),
    }
}

/// Select the theme (by id, falling back to FluxDark) and whether a background image shows.
pub fn set(id: &str, image: Option<Visibility>) {
    let ix = PALETTES.iter().position(|p| p.id == id).unwrap_or(0);
    ACTIVE.store(ix as u8, Ordering::Relaxed);
    let v = image.map_or(0, |v| Visibility::ALL.iter().position(|x| *x == v).unwrap_or(1) as u8 + 1);
    IMAGE.store(v, Ordering::Relaxed);
}

/// A token from the active palette, or a raw hex color.
pub enum Paint {
    Tok(Tok),
    Hex(u32),
}
impl From<Tok> for Paint {
    fn from(t: Tok) -> Self {
        Paint::Tok(t)
    }
}
impl From<u32> for Paint {
    fn from(h: u32) -> Self {
        Paint::Hex(h)
    }
}

pub fn c(p: impl Into<Paint>) -> Hsla {
    match p.into() {
        Paint::Hex(h) => rgb(h).into(),
        Paint::Tok(t) => {
            let col: Hsla = rgb(palette().hex(t)).into();
            match (image(), t) {
                // Panels become translucent over the image; the title bar and popovers stay
                // nearly solid (menus and window buttons sit on them).
                (Some(v), Tok::BgTitle) => col.opacity(1. - v.strength() * 0.5),
                (Some(v), Tok::BgSide) => col.opacity(1. - v.strength() * 0.8),
                (Some(v), Tok::BgEditor) => col.opacity(1. - v.strength() * 0.6),
                (Some(v), Tok::BgElevated) => col.opacity(1. - v.strength() * 0.3),
                _ => col,
            }
        }
    }
}

/// A file-type color for icons. On the light theme the bright language colors
/// are darkened so the outline icons keep enough contrast on paper; `None` is
/// the neutral icon color.
pub fn tint(hex: Option<u32>) -> Hsla {
    match hex {
        None => c(FG_MUTED),
        Some(h) if palette().dark => rgb(h).into(),
        Some(h) => {
            let f = |sh: u32| (((h >> sh) & 0xff) as f32 * 0.62) as u32;
            rgb((f(16) << 16) | (f(8) << 8) | f(0)).into()
        }
    }
}

/// The editor surface. With an image, a left-to-right wash: the code side lets
/// 60% of the image strength through, the far right all of it, so the picture
/// lives where code usually is not.
pub fn editor_wash() -> Background {
    let base: Hsla = rgb(palette().editor).into();
    match image() {
        None => base.into(),
        Some(v) => linear_gradient(
            90.,
            linear_color_stop(base.opacity(1. - v.strength() * 0.6), 0.45),
            linear_color_stop(base.opacity(1. - v.strength()), 1.),
        ),
    }
}

/// Push the active palette into gpui-component's theme (inputs, menus, editor).
pub fn apply(cx: &mut App) {
    let p = palette();
    Theme::change(if p.dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    Theme::update(cx, |t| {
        t.font_size = u(13.);
        t.mono_font_family = "Cascadia Code".into();
        t.mono_font_size = gpui_kit::px(14.);
        t.radius = gpui_kit::px(4.);
        t.radius_lg = gpui_kit::px(6.);

        let k = &mut t.colors;
        // Inputs and the title bar (which blends title_bar with this) use it; the code
        // editor gets its own clear background below so the wash behind it shows.
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
        k.primary_hover = c(ACCENT).opacity(0.88);
        k.primary_active = c(ACCENT).opacity(0.76);
        k.primary_foreground = c(ON_ACCENT);
        k.button_primary = c(ACCENT);
        k.button_primary_hover = c(ACCENT).opacity(0.88);
        k.button_primary_active = c(ACCENT).opacity(0.76);
        k.button_primary_foreground = c(ON_ACCENT);
        k.secondary = c(BG_ELEVATED);
        k.secondary_hover = c(BG_HOVER);
        k.secondary_foreground = c(FG);
        k.danger = c(DANGER);
        k.caret = c(ACCENT_TEXT);
        k.selection = c(Tok::Selection);
        k.sidebar = c(BG_SIDE);
        k.sidebar_foreground = c(FG);
        k.sidebar_border = c(BORDER);
        k.title_bar = c(BG_TITLE);
        k.title_bar_border = c(BORDER);
        k.status_bar = c(BG_TITLE);
        k.status_bar_border = c(BORDER);
        k.tab_bar = c(BG_SIDE);
        k.tab = gpui_kit::transparent_black();
        k.tab_active = c(BG_EDITOR);
        k.tab_foreground = c(FG_MUTED);
        k.tab_active_foreground = c(FG_STRONG);
        k.list = gpui_kit::transparent_black();
        k.list_hover = c(BG_HOVER);
        k.list_active = c(SELECT);
        k.list_active_border = gpui_kit::transparent_black();
        k.popover = rgb(p.elevated).into();
        k.popover_foreground = c(FG);
        k.window_border = c(BORDER);
        k.scrollbar = gpui_kit::transparent_black();
        k.scrollbar_thumb = c(BORDER_STRONG);
        k.scrollbar_thumb_hover = c(FG_DIM);

        // The code editor paints its own background and gutter from the syntax theme's
        // style block, not from the colors above: point those at the palette too.
        let mut hl = (*t.highlight_theme).clone();
        let st = &mut hl.style;
        st.editor_background = Some(if image().is_some() { gpui_kit::transparent_black() } else { c(BG_EDITOR) });
        st.editor_foreground = Some(c(FG));
        st.editor_active_line = Some(c(FG).opacity(0.05));
        st.editor_line_number = Some(c(FG_DIM));
        st.editor_active_line_number = Some(c(FG_STRONG));
        // Whitespace dots: present but quiet, a step below the indent guides' weight.
        st.editor_invisible = Some(c(FG_DIM).opacity(0.32));
        t.highlight_theme = std::sync::Arc::new(hl);
    });
}
