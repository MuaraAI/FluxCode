//! Window placement: open inside the monitor's *work area* (never under the
//! taskbar), and reopen where the user left the window last time.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
struct Bounds {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    maximized: bool,
}

fn state_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("window.json"))
}

/// Does the saved rectangle still fit on one of the connected monitors?
fn fits(b: &Bounds, win: &WebviewWindow) -> bool {
    win.available_monitors().unwrap_or_default().iter().any(|m| {
        let wa = m.work_area();
        let (ax, ay) = (wa.position.x, wa.position.y);
        let (aw, ah) = (wa.size.width as i32, wa.size.height as i32);
        b.x >= ax - 16 && b.y >= ay - 16 && b.x + b.w as i32 <= ax + aw + 16 && b.y + b.h as i32 <= ay + ah + 16
    })
}

/// Place the (still hidden) window, then show it.
pub fn place(app: &AppHandle, win: &WebviewWindow) -> tauri::Result<()> {
    let saved: Option<Bounds> = state_file(app)
        .and_then(|f| fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .filter(|b: &Bounds| b.w >= 400 && b.h >= 300);

    match saved.filter(|b| fits(b, win)) {
        Some(b) => {
            win.set_size(PhysicalSize::new(b.w, b.h))?;
            win.set_position(PhysicalPosition::new(b.x, b.y))?;
            if b.maximized {
                win.maximize()?;
            }
        }
        None => {
            if let Some(m) = win.current_monitor()?.or(win.primary_monitor()?) {
                let wa = *m.work_area();
                let scale = m.scale_factor();
                let (lw, lh) = (wa.size.width as f64 / scale, wa.size.height as f64 / scale);
                if lw < 1500.0 || lh < 900.0 {
                    // Small screens: use all of the work area, like most editors do.
                    win.maximize()?;
                } else {
                    let (w, h) = ((1400.0 * scale) as u32, (900.0 * scale) as u32);
                    win.set_size(PhysicalSize::new(w, h))?;
                    win.set_position(PhysicalPosition::new(
                        wa.position.x + (wa.size.width as i32 - w as i32) / 2,
                        wa.position.y + (wa.size.height as i32 - h as i32) / 2,
                    ))?;
                }
            }
        }
    }
    win.show()?;
    track(app, win, saved.unwrap_or_default());
    Ok(())
}

/// Remember the normal (restored) bounds and the maximized flag; written on close.
fn track(app: &AppHandle, win: &WebviewWindow, initial: Bounds) {
    let current = Arc::new(Mutex::new(initial));
    let file = state_file(app);
    let w = win.clone();
    win.on_window_event(move |event| match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            if w.is_minimized().unwrap_or(false) {
                return;
            }
            let maximized = w.is_maximized().unwrap_or(false);
            let mut b = current.lock().unwrap();
            b.maximized = maximized;
            if !maximized {
                if let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) {
                    (b.x, b.y, b.w, b.h) = (p.x, p.y, s.width, s.height);
                }
            }
        }
        WindowEvent::Destroyed => {
            if let Some(f) = &file {
                if let Some(dir) = f.parent() {
                    let _ = fs::create_dir_all(dir);
                }
                let b = *current.lock().unwrap();
                if b.w > 0 {
                    let _ = fs::write(f, serde_json::to_string(&b).unwrap_or_default());
                }
            }
        }
        _ => {}
    });
}
