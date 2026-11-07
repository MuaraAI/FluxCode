//! FluxCode, native GPUI edition.
//!
//! One process, no WebView: the UI is drawn by GPUI (DirectX 11 on Windows,
//! Metal on macOS, Vulkan on Linux) with widgets from gpui-component.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod complete;
mod core;
mod explorer;
mod files;
mod github;
mod panel;
mod preview;
mod preview_ui;
mod proc;
mod search;
mod settings;
mod theme;
mod ui;
mod workbench;

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::path::PathBuf;

fn main() {
    // `fluxcode-gpui [folder] [files...]` like the Tauri app; with no
    // arguments the last session is restored (or the welcome screen shown).
    let paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .filter_map(|a| PathBuf::from(a).canonicalize().ok())
        // canonicalize() gives `\\?\C:\...` on Windows; keep the plain form.
        .map(|p| PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\")))
        .collect();
    let files: Vec<PathBuf> = paths.iter().filter(|p| p.is_file()).cloned().collect();
    let root = paths.iter().find(|p| p.is_dir()).cloned();

    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        theme::apply(cx);
        workbench::bind_keys(cx);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1280.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(600.), px(400.))),
            ..TitleBar::window_options()
        };
        gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| workbench::Workbench::new(root.clone(), files.clone(), window, cx))
        })
        .expect("failed to open the FluxCode window");
    });
}
