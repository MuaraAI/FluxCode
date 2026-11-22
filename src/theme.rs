//! Color themes: FluxDark, FluxCream, FluxMono, FluxRed, FluxMuara (light and dark) and FluxSky (the website's dusk, with its clouds behind the panels).

use gpui_kit::component::highlighter::HighlightThemeStyle;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{linear_color_stop, linear_gradient, rgb, App, Background, Hsla};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering};
use std::sync::RwLock;

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

#[derive(Clone, Copy)]
pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    /// One line for the Settings card.
    pub note: &'static str,
    pub dark: bool,
    pub(crate) title: u32,
    pub(crate) side: u32,
    pub(crate) editor: u32,
    pub(crate) elevated: u32,
    pub(crate) hover: u32,
    pub(crate) border: u32,
    pub(crate) border_strong: u32,
    pub(crate) fg: u32,
    pub(crate) strong: u32,
    pub(crate) muted: u32,
    pub(crate) dim: u32,
    pub(crate) accent: u32,
    pub(crate) on_accent: u32,
    pub(crate) accent_text: u32,
    pub(crate) blue: u32,
    pub(crate) green: u32,
    pub(crate) warn: u32,
    pub(crate) danger: u32,
    pub(crate) folder: u32,
    pub(crate) select: u32,
    pub(crate) selection: u32,
    /// Code colors of its own (a VS Code theme's `tokenColors`); `None` keeps gpui-kit's default syntax theme for dark or light.
    pub(crate) syntax: Option<&'static HighlightThemeStyle>,
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

// The palettes come from the icon: ink ground, ivory text, one ochre accent (the colors of a meander painted on a Geometric-period pot).
pub static PALETTES: [Palette; 7] = [
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
        syntax: None,
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
        syntax: None,
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
        syntax: None,
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
        syntax: None,
    },

    Palette {
        id: "flux-muara",
        name: "FluxMuara",
        note: "Mist canvas, river-blue accent",
        dark: false,
        // The Muara AI brand palette (DESIGN.md, UBSI-API landing page): mist water canvas, white crystal surfaces, riverbed ink and one river current blue.
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
        syntax: None,
    },

    Palette {
        id: "flux-muara-dark",
        name: "FluxMuara Dark",
        note: "Night water, river-blue accent",
        dark: true,
        // The same Muara AI palette at night: riverbed ink surfaces (the design's code-block colors) with the river current blue lifted for the dark canvas.
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
        syntax: None,
    },

    Palette {
        id: "flux-sky",
        name: "FluxSky",
        note: "Dusk sky and clouds, peach accent",
        dark: true,
        // The website's dusk (fluxcode.muaraai.com): violet night surfaces from the top of its sky, the warm horizon glow as the accent.
        title: 0x15131f,
        side: 0x191626,
        editor: 0x1c1929,
        elevated: 0x252136,
        hover: 0x2e2942,
        border: 0x332d47,
        border_strong: 0x433b5a,
        fg: 0xe4def0,
        strong: 0xf6f2fc,
        muted: 0xb3abc6,
        dim: 0xa9a1bd,
        accent: 0xe9a07c,
        on_accent: 0x1a1220,
        accent_text: 0xf2b896,
        blue: 0x9fb6ec,
        green: 0x93cfa8,
        warn: 0xe8c27a,
        danger: 0xf08a9a,
        folder: 0xd9ad8c,
        select: 0x2d2840,
        selection: 0x4a3a58,
        syntax: None,
    },
];

/// How long a theme change takes to fade from the old colors to the new.
pub const FADE: std::time::Duration = std::time::Duration::from_millis(340);
/// The palette a theme change fades from (null: no fade running).
static FROM: AtomicPtr<Palette> = AtomicPtr::new(std::ptr::null_mut());
/// When that fade started, in ms since `CLOCK`.
static FADE_AT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CLOCK: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn now_ms() -> u64 {
    CLOCK.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

/// Fade from `old` into whatever `set` selects next.
pub fn fade_from(old: &'static Palette) {
    FADE_AT.store(now_ms(), Ordering::Relaxed);
    FROM.store(old as *const Palette as *mut Palette, Ordering::Relaxed);
}

/// The running fade: the palette it comes from and how far it is (0..1, eased).
fn fade() -> Option<(&'static Palette, f32)> {
    let p = FROM.load(Ordering::Relaxed);
    if p.is_null() {
        return None;
    }
    let t = (now_ms().saturating_sub(FADE_AT.load(Ordering::Relaxed))) as f32 / FADE.as_millis() as f32;
    if t >= 1. {
        FROM.store(std::ptr::null_mut(), Ordering::Relaxed);
        return None;
    }
    // Ease in and out (cubic): it leaves gently and settles gently.
    let e = if t < 0.5 { 4. * t * t * t } else { 1. - (-2. * t + 2.).powi(3) / 2. };
    // SAFETY: as `ACTIVE`, only ever a `&'static Palette`.
    Some((unsafe { &*p }, e))
}

/// A theme change is still fading: the window needs another frame.
pub fn fading() -> bool {
    fade().is_some()
}

/// A token's color right now: the active palette's, or mid-fade a mix of both.
fn now(t: Tok) -> Hsla {
    let to = palette().hex(t);
    match fade() {
        Some((from, e)) => rgb(mix(from.hex(t), to, e)).into(),
        None => rgb(to).into(),
    }
}

/// A token's color right now as hex, mid-fade the mix: for drawings that take their colors from the theme (the animated backgrounds), so they change with it.
pub fn live(t: Tok) -> u32 {
    let to = palette().hex(t);
    match fade() {
        Some((from, e)) => mix(from.hex(t), to, e),
        None => to,
    }
}

/// Whether the theme is dark right now (mid-fade, it turns halfway through).
pub fn live_dark() -> bool {
    match fade() {
        Some((from, e)) if e < 0.5 => from.dark,
        _ => palette().dark,
    }
}

/// Mix two colors channel by channel (`e` 0: `a`, 1: `b`).
pub fn mix(a: u32, b: u32, e: f32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xff) as f32, ((b >> s) & 0xff) as f32);
        ((x + (y - x) * e).round().clamp(0., 255.) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}


/// How much of the background image reaches the eye through the panels.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
pub enum Visibility {
    Subtle,
    #[default]
    Balanced,
    /// Above the level the text colors were checked at: dim text can drop below 4.5:1 over very bright pictures.
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

/// Size of the whole interface (text, rows, bars, icons).
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

/// The active palette; null until `set` runs, which reads as FluxDark.
static ACTIVE: AtomicPtr<Palette> = AtomicPtr::new(std::ptr::null_mut());
/// Palettes from installed VS Code themes, after the built-in ones.
static EXTRA: RwLock<Vec<&'static Palette>> = RwLock::new(Vec::new());
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
static GLASS: AtomicBool = AtomicBool::new(false);
/// The backdrop is a drawn scene, not a photo: its colors are calm and mid-dark, so the panels can let much more of it through and text keeps its contrast.
static SCENE: AtomicBool = AtomicBool::new(false);

pub fn set_scene_backdrop(on: bool) {
    SCENE.store(on, Ordering::Relaxed);
}

fn strength(v: Visibility) -> f32 {
    v.strength() * if SCENE.load(Ordering::Relaxed) { 2.2 } else { 1. }
}

/// Liquid glass on: the window takes the system blur and the panels let it through.
pub fn set_glass(on: bool) {
    GLASS.store(on, Ordering::Relaxed);
}

pub fn glass() -> bool {
    GLASS.load(Ordering::Relaxed)
}

pub fn palette() -> &'static Palette {
    let p = ACTIVE.load(Ordering::Relaxed);
    if p.is_null() {
        &PALETTES[0]
    } else {
        // SAFETY: `ACTIVE` only ever holds a pointer made from a `&'static Palette` (an element of `PALETTES` or a leaked extension palette), never freed.
        unsafe { &*p }
    }
}

/// Every palette to choose from: the built-in ones, then the installed VS Code themes.
pub fn all() -> Vec<&'static Palette> {
    let extra = EXTRA.read().unwrap_or_else(|e| e.into_inner());
    PALETTES.iter().chain(extra.iter().copied()).collect()
}

/// A palette by id (built-in or from an installed theme).
pub fn find(id: &str) -> Option<&'static Palette> {
    if let Some(p) = PALETTES.iter().find(|p| p.id == id) {
        return Some(p);
    }
    EXTRA.read().unwrap_or_else(|e| e.into_inner()).iter().copied().find(|p| p.id == id)
}

/// Replace the palettes from installed VS Code themes.
pub fn set_extra(list: Vec<&'static Palette>) {
    *EXTRA.write().unwrap_or_else(|e| e.into_inner()) = list;
}

fn image() -> Option<Visibility> {
    match IMAGE.load(Ordering::Relaxed) {
        0 => None,
        n => Visibility::ALL.get(n as usize - 1).copied(),
    }
}

/// Select the theme (by id, falling back to FluxDark) and whether a background image shows.
pub fn set(id: &str, image: Option<Visibility>) {
    let p: &'static Palette = find(id).unwrap_or(&PALETTES[0]);
    ACTIVE.store(p as *const Palette as *mut Palette, Ordering::Relaxed);
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
            let col: Hsla = now(t);
            match (image(), t) {
                // Panels become translucent over the image; the title bar and popovers stay nearly solid (menus and window buttons sit on them).
                (Some(v), Tok::BgTitle) => col.opacity(1. - strength(v) * 0.5),
                (Some(v), Tok::BgSide) => col.opacity(1. - strength(v) * 0.8),
                (Some(v), Tok::BgEditor) => col.opacity(1. - strength(v) * 0.6),
                (Some(v), Tok::BgElevated) => col.opacity(1. - strength(v) * 0.3),
                // Glass over the blurred desktop: the code surface stays nearly solid for contrast, the chrome around it lets more through.
                (None, Tok::BgTitle) if glass() => col.opacity(0.72),
                (None, Tok::BgSide) if glass() => col.opacity(0.76),
                (None, Tok::BgEditor) if glass() => col.opacity(0.88),
                (None, Tok::BgElevated) if glass() => col.opacity(0.9),
                _ => col,
            }
        }
    }
}

/// A file-type color for icons.
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

/// The editor surface.
pub fn welcome_tint() -> Background {
    let base: Hsla = rgb(palette().editor).into();
    let a = if palette().dark { 0.6 } else { 0.62 };
    linear_gradient(180., linear_color_stop(base.opacity(a * 0.8), 0.), linear_color_stop(base.opacity(a), 1.))
}

pub fn editor_wash() -> Background {
    let base: Hsla = now(Tok::BgEditor);
    match image() {
        None if glass() => base.opacity(0.88).into(),
        None => base.into(),
        // The code side keeps the photo-level strength even over a scene, so text always sits on a calm surface.
        Some(v) => linear_gradient(
            90.,
            linear_color_stop(base.opacity(1. - v.strength() * 0.6), 0.45),
            linear_color_stop(base.opacity(1. - strength(v)), 1.),
        ),
    }
}

/// Push the active palette into gpui-component's theme (inputs, menus, editor).
pub fn apply(cx: &mut App) {
    let p = palette();
    // Mid-fade, the controls switch between light and dark halfway through.
    let dark = match fade() {
        Some((from, e)) if e < 0.5 => from.dark,
        _ => p.dark,
    };
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    Theme::update(cx, |t| {
        t.font_size = u(13.);
        t.mono_font_family = "Cascadia Code".into();
        t.mono_font_size = gpui_kit::px(14.);
        t.radius = gpui_kit::px(4.);
        t.radius_lg = gpui_kit::px(6.);

        let k = &mut t.colors;
        // Inputs and the title bar (which blends title_bar with this) use it.
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
        // Menus and dropdowns from gpui-kit: the same light glass as FluxCode's own.
        k.popover = if glass() { now(Tok::BgElevated).opacity(0.94) } else { now(Tok::BgElevated) };
        k.popover_foreground = c(FG);
        k.window_border = c(BORDER);
        k.scrollbar = gpui_kit::transparent_black();
        k.scrollbar_thumb = c(BORDER_STRONG);
        k.scrollbar_thumb_hover = c(FG_DIM);

        // The code editor paints its own background and gutter from the syntax theme's style block, not from the colors above: point those at the palette too.
        let mut hl = (*t.highlight_theme).clone();
        // A VS Code theme brings its own code and diagnostic colors.
        if let Some(own) = p.syntax {
            hl.style.syntax = own.syntax.clone();
            hl.style.status = own.status.clone();
        }
        let st = &mut hl.style;
        st.editor_background = Some(if image().is_some() { gpui_kit::transparent_black() } else { c(BG_EDITOR) });
        // The line numbers sit on the same surface as the code (the syntax theme's own gutter color would be a solid stripe over a backdrop).
        st.editor_gutter_background = st.editor_background;
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

    /// The contract stated in the palette comment above: text colors reach 4.5:1 on every surface the UI paints text over, and text on an accent fill is even clearer.
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
    fn extension_palettes_join_the_list_and_can_be_selected() {
        let mut p = Palette { ..PALETTES[1] };
        p.id = "vsx:test.theme/one";
        p.name = "One";
        let leaked: &'static Palette = Box::leak(Box::new(p));
        set_extra(vec![leaked]);
        assert!(all().iter().any(|p| p.id == "vsx:test.theme/one"));
        assert_eq!(find("vsx:test.theme/one").map(|p| p.name), Some("One"));
        set("vsx:test.theme/one", None);
        assert_eq!(palette().id, "vsx:test.theme/one");
        set_extra(Vec::new());
        set("vsx:test.theme/one", None);
        assert_eq!(palette().id, PALETTES[0].id);
    }

    #[test]
    fn a_theme_change_fades_between_the_palettes() {
        assert_eq!(mix(0x000000, 0xffffff, 0.), 0x000000);
        assert_eq!(mix(0x000000, 0xffffff, 1.), 0xffffff);
        assert_eq!(mix(0x102030, 0x304050, 0.5), 0x203040);
        assert!(find("flux-sky").is_some_and(|p| p.dark));
    }

    #[test]
    fn flux_muara_comes_in_light_and_dark() {
        for id in ["flux-muara", "flux-muara-dark"] {
            assert!(PALETTES.iter().any(|p| p.id == id), "{id} palette is missing");
        }
    }
}

/// The fill of a liquid glass card: translucent, lit from the top edge down.
pub fn glass_fill() -> Background {
    glass_with(0.9, 0.84)
}

/// A floating surface's liquid glass (menus, popovers, toasts, the token saver): translucent, a little lighter at the top like light caught in glass, over a soft shadow.
pub fn glass_pop_fill() -> Background {
    let base: Hsla = now(Tok::BgElevated);
    if !glass() {
        return base.into();
    }
    let dark = palette().dark;
    let top = Hsla { l: (base.l + if dark { 0.05 } else { 0.03 }).clamp(0., 1.), ..base };
    // Nearly solid: without a blur, even a tenth of the chat's white text under a dark panel reads through it.
    let (a_top, a_bottom) = (0.97, 0.95);
    linear_gradient(180., linear_color_stop(top.opacity(a_top), 0.), linear_color_stop(base.opacity(a_bottom), 1.))
}

/// The bright hairline on a floating glass surface's edge.
pub fn glass_pop_edge() -> Hsla {
    if palette().dark { gpui_kit::hsla(0., 0., 1., 0.14) } else { gpui_kit::hsla(0., 0., 0., 0.09) }
}

fn glass_with(top_a: f32, bottom_a: f32) -> Background {
    let base: Hsla = now(Tok::BgElevated);
    if !glass() && image().is_none() {
        return base.into();
    }
    let lift = if palette().dark { 0.06 } else { -0.03 };
    let top = Hsla { l: (base.l + lift).clamp(0., 1.), ..base };
    linear_gradient(180., linear_color_stop(top.opacity(top_a), 0.), linear_color_stop(base.opacity(bottom_a), 1.))
}

/// The hairline that catches the light on a glass card's edge.
pub fn glass_edge() -> Hsla {
    if palette().dark { gpui_kit::hsla(0., 0., 1., 0.10) } else { gpui_kit::hsla(0., 0., 0., 0.10) }
}

/// `.glass()` on any element: a floating, rounded liquid glass card.
pub trait GlassExt: gpui_kit::Styled + Sized {
    fn glass(self) -> Self {
        self.bg(glass_fill()).border_1().border_color(glass_edge()).rounded(gpui_kit::px(14.)).shadow_lg()
    }
    /// A floating popover, menu or toast in liquid glass.
    fn glass_pop(self) -> Self {
        self.bg(glass_pop_fill()).border_1().border_color(glass_pop_edge()).rounded(gpui_kit::px(14.)).shadow_lg()
    }
}
impl<T: gpui_kit::Styled> GlassExt for T {}

/// An image (or scene) shows behind the panels.
pub fn has_backdrop() -> bool {
    image().is_some()
}
