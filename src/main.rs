//! FluxCode: a native code editor on GPUI.
//!
//! One process, no WebView: the UI is drawn by GPUI (DirectX 11 on Windows,
//! Metal on macOS, Vulkan on Linux) with widgets from gpui-component.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod i18n_id;
mod acp;
mod boot;
mod commands;
mod editor_cmds;
mod agent;
mod agent_ask;
mod agent_ctx;
mod complete;
mod core;
mod diff;
mod docs;
mod explorer;
mod files;
mod flux_anim;
mod github;
mod panel;
mod memory;
mod preview;
mod preview_ui;
mod proc;
mod quick;
mod scm;
mod search;
mod settings;
mod term;
mod theme;
mod ui;
mod update;
mod viewer;
mod whatsnew;
mod wintitle;
mod workbench;
mod workspace;
mod ws_context;

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::path::PathBuf;

fn main() {
    boot::start();
    // `fluxcode [folder] [files...]`; with no
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

    // Which agents are installed (PATH and ~/.fluxcode/agents), worked out while
    // GPUI sets up the window system, so the agent panel never waits for it.
    std::thread::spawn(|| {
        let _ = acp::installed();
    });
    boot::mark("args");
    let app = gpui_kit::application();
    boot::mark("platform created");
    app.with_assets(AppAssets).run(move |cx| {
        boot::mark("app running");
        gpui_kit::init(cx);
        boot::mark("gpui-kit init");
        // The saved theme is active before the first frame, so the window never flashes another one.
        let saved = settings::Settings::load();
        theme::set_ui_size(saved.ui_size);
        theme::set(&saved.theme, saved.background().map(|_| saved.bg_visibility));
        theme::apply(cx);
        boot::mark("settings + theme");
        workbench::bind_keys(cx);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1280.), px(760.)), cx);
        let mut options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(600.), px(400.))),
            // Linux: matches fluxcode.desktop, so the dock groups the window under its icon.
            app_id: Some("fluxcode".into()),
            ..TitleBar::window_options()
        };
        // The name the taskbar preview, Alt+Tab and the window list show.
        if let Some(t) = options.titlebar.as_mut() {
            t.title = Some(format!("FluxCode Development ({})", update::BUILD).into());
        }
        boot::mark("open window");
        gpui_kit::open_window(options, cx, move |window, cx| {
            boot::mark("window created");
            cx.new(|cx| workbench::Workbench::new(root.clone(), files.clone(), window, cx))
        })
        .expect("failed to open the FluxCode window");
    });
}

/// gpui-kit's default icons, the file-type icons the explorer uses, and the agent icons.
struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(bytes) = files::FileIcons.load(path)? {
            return Ok(Some(bytes));
        }
        // Agent icons from the ACP registry, kept on disk (see acp::fetch_icons).
        if path.starts_with(acp::ICON_ASSETS) {
            return Ok(acp::load_icon(path).map(std::borrow::Cow::Owned));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(files::FileIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
