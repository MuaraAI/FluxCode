
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod i18n_id;
mod accel;
mod acp;
mod boot;
mod browser;
mod commands;
mod copilot;
mod editor_cmds;
mod agent;
mod agent_ask;
mod agent_ctx;
mod complete;
mod core;
mod diagnostics;
mod diff;
mod diff_view;
mod docs;
mod explorer;
mod ext;
mod files;
mod fluxicon;
mod flux_anim;
mod fluxdoc;
mod github;
mod instance;
mod nbt;
mod panel;
mod memtrim;
mod preview;
mod preview_ui;
mod proc;
mod quick;
mod remote;
mod scm;
mod scm_incoming;
mod search;
mod scene;
mod scene_art;
mod motion_check;
mod sidedock;
mod settings;
mod shell;
mod term;
mod theme;
mod theme_menu;
mod ui;
mod update;
mod viewer;
mod whatsnew;
mod wintitle;
mod wiztree;
mod workbench;
mod workspace;
mod ws_context;

use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::path::PathBuf;

fn main() {
    if std::env::args().any(|a| a == "--remote-server") {
        match remote::serve() {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("remote server: {e}");
                std::process::exit(1);
            }
        }
    }
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) == Some("--flux-to-md") {
        std::process::exit(fluxdoc::md::cli(&argv[2..]));
    }
    match argv.get(1).map(String::as_str) {
        Some("--register-shell") => std::process::exit(shell::cli(true)),
        Some("--unregister-shell") => std::process::exit(shell::cli(false)),
        Some("--wiztree") => std::process::exit(wiztree::cli(&argv[2..])),
        _ => {}
    }
    boot::start();
    let paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .filter_map(|a| PathBuf::from(a).canonicalize().ok())
        .map(|p| {
            let s = p.to_string_lossy();
            let plain = match s.strip_prefix(r"\\?\UNC\") {
                Some(rest) => format!(r"\\{rest}"),
                None => s.trim_start_matches(r"\\?\").to_string(),
            };
            PathBuf::from(plain)
        })
        .collect();
    let files: Vec<PathBuf> = paths.iter().filter(|p| p.is_file()).cloned().collect();
    let root = paths.iter().find(|p| p.is_dir()).cloned();
    if !argv.iter().any(|a| a == "--new-window") && root.is_none() {
        if !files.is_empty() && instance::forward(&files) {
            return;
        }
        if files.is_empty() && instance::wake_hidden() {
            return;
        }
    }

    std::thread::spawn(|| {
        let _ = acp::installed();
    });
    boot::mark("args");
    let app = gpui_kit::application();
    boot::mark("platform created");
    app.with_assets(AppAssets).run(move |cx| {
        boot::mark("app running");
        gpui_kit::init(cx);
        cx.set_cursor_hide_mode(CursorHideMode::Never);
        boot::mark("gpui-kit init");
        let saved = settings::Settings::load();
        ext::native::load();
        boot::mark("vscode extensions");
        theme::set_ui_size(saved.ui_size);
        saved.apply_backdrop();
        theme::apply(cx);
        boot::mark("settings + theme");
        workbench::bind_keys(cx);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1280.), px(760.)), cx);
        let mut options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(600.), px(400.))),
            app_id: Some("fluxcode".into()),
            ..TitleBar::window_options()
        };
        if let Some(t) = options.titlebar.as_mut() {
            t.title = Some(format!("FluxCode Development ({})", update::BUILD).into());
        }
        boot::mark("open window");
        gpui_kit::open_window(options, cx, move |window, cx| {
            boot::mark("window created");
            let wb = cx.new(|cx| workbench::Workbench::new(root.clone(), files.clone(), window, cx));
            cx.new(|cx| scene::Shell::new(wb, cx))
        })
        .expect("failed to open the FluxCode window");
    });
}

struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(bytes) = fluxicon::load(path) {
            return Ok(Some(std::borrow::Cow::Borrowed(bytes)));
        }
        if let Some(bytes) = files::FileIcons.load(path)? {
            return Ok(Some(bytes));
        }
        if path.starts_with(acp::ICON_ASSETS) {
            return Ok(acp::load_icon(path).map(std::borrow::Cow::Owned));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(files::FileIcons.list(path)?);
        paths.extend(fluxicon::paths().filter(|p| p.starts_with(path)).map(SharedString::from));
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
