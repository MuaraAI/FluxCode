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
    /// Warnings in the terminal (amber; FluxRed's errors are a warm orange, apart from its clay accent).
    Warn,
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
pub const WARN: Tok = Tok::Warn;
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
    warn: u32,
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
            Tok::Warn => self.warn,
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

// The palettes come from the icon: ink ground, ivory text, one ochre accent (the
// colors of a meander painted on a Geometric-period pot). Accents are muted on
// purpose; color marks state (focus, the primary action, errors), not decoration.
//
// Contrast (checked with a script, WCAG 2.1): fg, muted, dim, accent text and the
// state colors are >= 4.5:1 on every surface, and still >= 4.5:1 over a white (dark
// themes) or black (FluxCream) image pixel at the Balanced strength. Text on accent
// fills >= 5.1:1.
pub const PALETTES: [Palette; 6] = [
    Palette {
        id: "flux-dark",
        name: "FluxDark",
        note: "Ink and ivory, ochre accent",
        dark: true,
        title: 0x111214,
        side: 0x151619,
        editor: 0x18191c,
        elevated: 0x202226,
        hover: 0x2a2c31,
        border: 0x2c2e33,
        border_strong: 0x3a3d43,
        fg: 0xdcd7cd,
        strong: 0xf3eee4,
        muted: 0xaaa59b,
        dim: 0xa29d93,
        accent: 0xc29466,
        on_accent: 0x16120e,
        accent_text: 0xd6aa7c,
        blue: 0x93b6dc,
        green: 0x92c79a,
        warn: 0xdcb86e,
        danger: 0xe8867a,
        folder: 0xc4a47a,
        select: 0x26282d,
        selection: 0x3d3426,
    },
    Palette {
        id: "flux-cream",
        name: "FluxCream",
        note: "Paper and ink, umber accent",
        dark: false,
        title: 0xe6e1d6,
        side: 0xede9e0,
        editor: 0xf6f3ec,
        elevated: 0xffffff,
        hover: 0xe2ddd1,
        border: 0xd5cfc2,
        border_strong: 0xc2bbab,
        fg: 0x2b2925,
        strong: 0x15130f,
        muted: 0x534e46,
        dim: 0x58534a,
        accent: 0x7d5230,
        on_accent: 0xffffff,
        accent_text: 0x6c4526,
        blue: 0x2a5282,
        green: 0x285c33,
        warn: 0x6b4c0e,
        danger: 0x95322a,
        folder: 0x7d5a30,
        select: 0xe6dccb,
        selection: 0xe5d5bb,
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
        warn: 0xd9c38c,
        danger: 0xff6670,
        folder: 0xbdbdbd,
        select: 0x2c2c2c,
        selection: 0x3d3d3d,
    },
    Palette {
        id: "flux-red",
        name: "FluxRed",
        note: "Dark oxblood, clay accent",
        dark: true,
        title: 0x150f10,
        side: 0x1b1314,
        editor: 0x1f1617,
        elevated: 0x2a1f20,
        hover: 0x35282a,
        border: 0x3d2e2f,
        border_strong: 0x4b393a,
        fg: 0xe6dad6,
        strong: 0xf7efec,
        muted: 0xb7a7a2,
        dim: 0xae9e99,
        accent: 0xc26e62,
        on_accent: 0x150f10,
        accent_text: 0xdc8f83,
        blue: 0x9bb8d8,
        green: 0x9cc79f,
        warn: 0xdcbc7a,
        danger: 0xf0a27a,
        folder: 0xd0a37f,
        select: 0x3a2a2b,
        selection: 0x55393a,
    },

    Palette {
        id: "flux-muara",
        name: "FluxMuara",
        note: "Mist canvas, river-blue accent",
        dark: false,
        // The Muara AI brand palette (DESIGN.md, UBSI-API landing page): mist water
        // canvas, white crystal surfaces, riverbed ink and one river current blue.
        title: 0xeaf3f9,
        side: 0xf0f7fb,
        editor: 0xffffff,
        elevated: 0xffffff,
        hover: 0xf0f7fb,
        border: 0xd9e5ed,
        border_strong: 0xb9cfe0,
        fg: 0x072033,
        strong: 0x031a28,
        muted: 0x3b5a70,
        dim: 0x47627a,
        accent: 0x0369a1,
        on_accent: 0xffffff,
        accent_text: 0x0369a1,
        blue: 0x075985,
        green: 0x0f766e,
        warn: 0x9a3412,
        danger: 0xbe123c,
        folder: 0x075985,
        select: 0xeaf3f9,
        selection: 0xdcebf5,
    },

    Palette {
        id: "flux-muara-dark",
        name: "FluxMuara Dark",
        note: "Night water, river-blue accent",
        dark: true,
        // The same Muara AI palette at night: riverbed ink surfaces (the design's
        // code-block colors) with the river current blue lifted for the dark canvas.
        title: 0x0e1922,
        side: 0x111e2a,
        editor: 0x131b26,
        elevated: 0x1a2836,
        hover: 0x223140,
        border: 0x263848,
        border_strong: 0x354b5e,
        fg: 0xcdd6f4,
        strong: 0xe8eefb,
        muted: 0x9aabc0,
        dim: 0x8da0b4,
        accent: 0x358fcc,
        on_accent: 0x07141f,
        accent_text: 0x7cc7ef,
        blue: 0x7dd3fc,
        green: 0x5fc9ab,
        warn: 0xe8b06a,
        danger: 0xef7d92,
        folder: 0x8fb9d9,
        select: 0x1a2836,
        selection: 0x28496b,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance per WCAG 2.1 (sRGB, linearized).
    fn luminance(h: u32) -> f64 {
        let channel = |shift: u32| {
            let c = ((h >> shift) & 0xff) as f64 / 255.;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(fg: u32, bg: u32) -> f64 {
        let (a, b) = (luminance(fg), luminance(bg));
        let (hi, lo) = (a.max(b), a.min(b));
        (hi + 0.05) / (lo + 0.05)
    }

    /// The contract stated in the palette comment above: text colors reach 4.5:1
    /// on every surface the UI paints text over, and text on an accent fill is
    /// even clearer. A new palette must pass this before it ships.
    #[test]
    fn every_palette_is_readable_on_every_surface() {
        let text = [Tok::Fg, Tok::FgStrong, Tok::FgMuted, Tok::FgDim, Tok::AccentText, Tok::Blue, Tok::Green, Tok::Warn, Tok::Danger, Tok::Folder];
        let surfaces = [Tok::BgTitle, Tok::BgSide, Tok::BgEditor, Tok::BgElevated, Tok::BgHover, Tok::Select];
        for p in PALETTES.iter() {
            for t in text {
                for s in surfaces {
                    let r = contrast(p.hex(t), p.hex(s));
                    assert!(r >= 4.5, "{}: {:?} on {:?} is only {r:.2}:1", p.id, t, s);
                }
            }
            let r = contrast(p.hex(Tok::OnAccent), p.hex(Tok::Accent));
            assert!(r >= 5.1, "{}: OnAccent on Accent is only {r:.2}:1", p.id);
        }
    }

    #[test]
    fn flux_muara_comes_in_light_and_dark() {
        for id in ["flux-muara", "flux-muara-dark"] {
            assert!(PALETTES.iter().any(|p| p.id == id), "{id} palette is missing");
        }
    }
}
