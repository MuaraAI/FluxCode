//! Animated backgrounds behind the panels: twelve scenes drawn live (`scene_art`) in the active theme's colors, or none.

use gpui_kit::{prelude::FluentBuilder, *};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Sky,
    Ocean,
    River,
    Beach,
    Mountains,
    Anime,
    Aurora,
    Rain,
    Snow,
    Desert,
    Forest,
    Space,
}

impl Scene {
    pub const ALL: [Scene; 12] = [
        Scene::Sky,
        Scene::Ocean,
        Scene::Mountains,
        Scene::Aurora,
        Scene::Forest,
        Scene::River,
        Scene::Beach,
        Scene::Desert,
        Scene::Anime,
        Scene::Rain,
        Scene::Snow,
        Scene::Space,
    ];

    /// The id saved in settings.json.
    pub fn id(self) -> &'static str {
        match self {
            Scene::Sky => "sky",
            Scene::Ocean => "ocean",
            Scene::River => "river",
            Scene::Beach => "beach",
            Scene::Mountains => "mountains",
            Scene::Anime => "anime",
            Scene::Aurora => "aurora",
            Scene::Rain => "rain",
            Scene::Snow => "snow",
            Scene::Desert => "desert",
            Scene::Forest => "forest",
            Scene::Space => "space",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scene::Sky => "Clouds",
            Scene::Ocean => "Ocean",
            Scene::River => "Lake",
            Scene::Beach => "Beach",
            Scene::Mountains => "Mountains",
            Scene::Anime => "City",
            Scene::Aurora => "Aurora",
            Scene::Rain => "Rain",
            Scene::Snow => "Snow",
            Scene::Desert => "Dunes",
            Scene::Forest => "Forest",
            Scene::Space => "Galaxy",
        }
    }

    pub fn from_id(id: &str) -> Option<Scene> {
        Scene::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// 0: no scene, n: `Scene::ALL[n - 1]`.
static SCENE: AtomicU8 = AtomicU8::new(0);
static ANIMATE: AtomicBool = AtomicBool::new(true);

/// Which scene shows (None: none) and whether it moves (off: it stands still).
pub fn set(scene: Option<Scene>, animate: bool) {
    let n = scene.and_then(|s| Scene::ALL.iter().position(|x| *x == s)).map_or(0, |i| i as u8 + 1);
    SCENE.store(n, Ordering::Relaxed);
    ANIMATE.store(animate, Ordering::Relaxed);
}

pub fn current() -> Option<Scene> {
    match SCENE.load(Ordering::Relaxed) {
        0 => None,
        n => Scene::ALL.get(n as usize - 1).copied(),
    }
}

pub fn active() -> bool {
    current().is_some()
}

/// Seconds since the first scene showed: every scene's clock.
fn clock() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// A scene drawn into its element's bounds, standing still at `t` seconds (Settings' previews), in the theme's colors at the time it is drawn.
pub fn preview(scene: Scene, t: f32) -> impl IntoElement {
    canvas(|_, _, _| (), move |b, _, window, _| crate::scene_art::paint(scene.id(), b, t, false, window)).size_full()
}

/// The window root: the scene, and the workbench over it as a cached view.
pub struct Shell {
    backdrop: Entity<Backdrop>,
    workbench: Entity<crate::workbench::Workbench>,
}

impl Shell {
    pub fn new(workbench: Entity<crate::workbench::Workbench>, cx: &mut Context<Self>) -> Self {
        Shell { backdrop: cx.new(|_| Backdrop::default()), workbench }
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // gpui-component's Root sets the rem to its 13 px text size on every render.
        window.set_rem_size(crate::theme::rem());
        let mut style = StyleRefinement::default();
        style.size.width = Some(relative(1.).into());
        style.size.height = Some(relative(1.).into());
        // FLUXCODE_SCENE_ONLY=1 shows the scene alone (the README's pictures of them).
        static SCENE_ONLY: OnceLock<bool> = OnceLock::new();
        let only = *SCENE_ONLY.get_or_init(|| std::env::var_os("FLUXCODE_SCENE_ONLY").is_some_and(|v| v == "1"));
        div()
            .size_full()
            .relative()
            .when(active(), |d| d.child(div().absolute().inset_0().child(self.backdrop.clone())))
            .when(!only, |d| d.child(self.workbench.clone().cached(style)))
    }
}

#[derive(Default)]
pub struct Backdrop {
    /// The frame timer is running.
    ticking: bool,
    /// The window is in the background: frames come slower.
    idle: bool,
}

/// 24 frames a second in front: the motions are slow (clouds, fog and stars move less than a pixel a frame), so this reads as smooth for about a fifth less work than 30.
const FRAME: Duration = Duration::from_millis(42);
const IDLE_FRAME: Duration = Duration::from_millis(400);

impl Backdrop {
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| loop {
            let idle = this.read_with(cx, |b, _| b.idle).unwrap_or(true);
            cx.background_executor().timer(if idle { IDLE_FRAME } else { FRAME }).await;
            let go = this
                .update(cx, |b, cx| {
                    let on = active() && ANIMATE.load(Ordering::Relaxed);
                    if on {
                        cx.notify();
                    } else {
                        b.ticking = false;
                    }
                    on
                })
                .unwrap_or(false);
            if !go {
                break;
            }
        })
        .detach();
    }
}

impl Render for Backdrop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.idle = !window.is_window_active();
        let scene = current();
        let animate = ANIMATE.load(Ordering::Relaxed);
        if scene.is_some() && animate {
            self.tick(cx);
        }
        // Standing still, every scene is shown at the same calm moment.
        let t = if animate { clock() } else { 12. };
        canvas(|_, _, _| (), move |b, _, window, _| {
            if let Some(s) = scene {
                crate::scene_art::paint(s.id(), b, t, true, window);
            }
        })
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::Scene;

    #[test]
    fn scene_ids_round_trip() {
        for s in Scene::ALL {
            assert_eq!(Scene::from_id(s.id()), Some(s));
        }
        assert_eq!(Scene::from_id("none"), None);
        // The pixel videos' ids still name a scene.
        for old in ["ocean", "river", "beach", "mountains", "anime"] {
            assert!(Scene::from_id(old).is_some(), "{old}");
        }
    }
}
