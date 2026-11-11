//! Development-build updates from GitHub Releases.
//!
//! Every push to `main` publishes a release tagged `BuildDev-YYYY-MM-DD-HH-MM-SS`
//! (see `.github/workflows/build.yml`). FluxCode asks GitHub for the release list
//! at start and every 6 hours, and offers the newest build when its tag sorts after
//! the running one. The request carries no identifier: no build ID, no machine or
//! user ID, no cookie. It is one plain `GET` through the system `curl`, the same
//! way the GitHub sign-in works, so no HTTP stack is compiled in.
//!
//! Installing replaces the running executable in place: the new file is written
//! next to it, the running one is renamed to `<name>.old` (Windows allows renaming
//! a running exe, Linux allows replacing it), the new one takes its name, and the
//! app restarts. The `.old` file is removed on the next start.

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;

/// This build, e.g. `BuildDev-2026-11-09-17-30-12` (set by build.rs).
pub const BUILD: &str = env!("FLUXCODE_BUILD");
const RELEASES: &str = "https://api.github.com/repos/MuaraAI/FluxCode/releases?per_page=20";
/// How often a running FluxCode looks again.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// First look, after start-up work has settled.
const FIRST: Duration = Duration::from_secs(4);

#[cfg(windows)]
const ASSET_SUFFIX: &str = "windows-x64.zip";
#[cfg(target_os = "macos")]
const ASSET_SUFFIX: &str = "macos-universal.tar.gz";
#[cfg(not(any(windows, target_os = "macos")))]
const ASSET_SUFFIX: &str = "linux-x64.tar.gz";

/// The executable alone, xz-compressed: about 30% smaller than the archive above. A
/// release that has it is updated from it; the archive stays in every release for the
/// copies of FluxCode made before this file existed (they only know the archive).
#[cfg(windows)]
const XZ_SUFFIX: &str = "windows-x64.update.xz";
#[cfg(target_os = "macos")]
const XZ_SUFFIX: &str = "macos-universal.update.xz";
#[cfg(not(any(windows, target_os = "macos")))]
const XZ_SUFFIX: &str = "linux-x64.update.xz";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    /// Release notes, Markdown as written on GitHub.
    pub notes: String,
    pub page: String,
    pub url: String,
    pub size: u64,
    /// Every build newer than this one, newest first: its tag and the first point of its notes.
    pub between: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    Available(Release),
    Downloading(Release),
    Installing(Release),
    Failed(Option<Release>, String),
}

#[derive(Default)]
pub struct UpdateUi {
    pub phase: Phase,
    /// Bytes on disk while downloading (polled from the file size).
    pub done: u64,
    /// "Later" hides the card for this tag until the next start.
    pub hidden: Option<String>,
    /// A check started from the menu reports "up to date" too.
    pub manual: bool,
}

fn curl() -> std::process::Command {
    crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
}

/// Ask GitHub for the newest BuildDev release that has a file for this platform.
/// `Ok(None)`: this build is the newest.
pub fn check() -> Result<Option<Release>, String> {
    let out = curl()
        .args(["-s", "-S", "-f", "-L", "--max-time", "20"])
        .args(["-H", "Accept: application/vnd.github+json", "-H", "User-Agent: FluxCode", RELEASES])
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "GitHub did not answer".into() } else { err });
    }
    let list: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    Ok(newest(&list, BUILD))
}

/// The newest release in GitHub's list that is newer than `current` and carries
/// this platform's archive. Tags are zero-padded, so they sort as text.
fn newest(list: &serde_json::Value, current: &str) -> Option<Release> {
    let mut between: Vec<(String, String)> = list
        .as_array()?
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            (tag.starts_with("BuildDev-") && tag > current).then(|| {
                let first = highlights(r["body"].as_str().unwrap_or_default(), 1).into_iter().next().unwrap_or_default();
                (tag.to_string(), first)
            })
        })
        .collect();
    between.sort_by(|a, b| b.0.cmp(&a.0));
    let mut best = list.as_array()?
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            if !tag.starts_with("BuildDev-") || tag <= current {
                return None;
            }
            let assets = r["assets"].as_array()?;
            let named = |suffix: &str| assets.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(suffix)));
            let asset = named(XZ_SUFFIX).or_else(|| named(ASSET_SUFFIX))?;
            Some(Release {
                tag: tag.to_string(),
                notes: r["body"].as_str().unwrap_or_default().to_string(),
                page: r["html_url"].as_str().unwrap_or_default().to_string(),
                url: asset["browser_download_url"].as_str()?.to_string(),
                size: asset["size"].as_u64().unwrap_or(0),
                between: Vec::new(),
            })
        })
        .max_by(|a, b| a.tag.cmp(&b.tag))?;
    best.between = between;
    Some(best)
}

/// The first sentence of up to `n` bullets from the notes, without Markdown marks.
/// A bullet can wrap over several indented lines.
pub fn highlights(notes: &str, n: usize) -> Vec<String> {
    let mut bullets: Vec<String> = Vec::new();
    let mut open = false;
    for line in notes.lines() {
        let t = line.trim();
        if let Some(b) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            bullets.push(b.to_string());
            open = true;
        } else if open && !t.is_empty() && line.starts_with(' ') {
            let last = bullets.last_mut().expect("open bullet");
            last.push(' ');
            last.push_str(t);
        } else {
            open = false;
        }
    }
    bullets
        .iter()
        .map(|b| {
            let b = plain(b);
            match b.find(". ") {
                Some(i) => b[..=i].to_string(),
                None => b,
            }
        })
        .filter(|l| !l.is_empty())
        .take(n)
        .collect()
}

/// `**bold**`, `` `code` `` and `[text](url)` as plain text.
fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        match (tail.find("]("), tail.find(')')) {
            (Some(close), Some(end)) if close < end => {
                out.push_str(&tail[..close]);
                rest = &tail[end + 1..];
            }
            _ => {
                out.push('[');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

fn download_path(r: &Release) -> PathBuf {
    let suffix = if r.url.ends_with(XZ_SUFFIX) { XZ_SUFFIX } else { ASSET_SUFFIX };
    std::env::temp_dir().join(format!("FluxCode-{}-{suffix}", r.tag))
}

/// The executable from an `.update.xz` download (the format checks its own CRC).
fn unpack_xz(archive: &Path) -> Result<Vec<u8>, String> {
    let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    lzma_rs::xz_decompress(&mut std::io::BufReader::new(f), &mut out).map_err(|e| format!("the download is damaged ({e})"))?;
    Ok(out)
}

/// An executable for this system, by its first bytes: nothing else is ever put in place
/// of the running one.
fn looks_like_exe(b: &[u8]) -> bool {
    if cfg!(windows) {
        b.starts_with(b"MZ")
    } else if cfg!(target_os = "macos") {
        // Universal (fat) or a single 64-bit Mach-O.
        b.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) || b.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
    } else {
        b.starts_with(b"\x7fELF")
    }
}

fn download(r: &Release, to: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(to);
    let out = curl()
        .args(["-s", "-S", "-f", "-L", "--max-time", "900", "-H", "User-Agent: FluxCode", "-o"])
        .arg(to)
        .arg(&r.url)
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let got = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    if r.size > 0 && got != r.size {
        return Err(format!("download incomplete ({got} of {} bytes)", r.size));
    }
    Ok(())
}

/// The new executable's bytes from the downloaded archive.
#[cfg(windows)]
fn unpack(archive: &Path) -> Result<Vec<u8>, String> {
    let b = std::fs::read(archive).map_err(|e| e.to_string())?;
    let entries = crate::docs::zip_entries(&b).ok_or("the download is not a valid ZIP")?;
    let (_, off, csize, method, _) = entries
        .iter()
        .find(|e| e.0.rsplit('/').next().is_some_and(|n| n.eq_ignore_ascii_case("FluxCode.exe")))
        .ok_or("FluxCode.exe is missing from the download")?;
    let data = b.get(*off..*off + *csize).ok_or("the ZIP is cut short")?;
    match method {
        0 => Ok(data.to_vec()),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, 512 << 20).map_err(|e| format!("{e:?}")),
        m => Err(format!("unsupported ZIP compression ({m})")),
    }
}

#[cfg(not(windows))]
fn unpack(archive: &Path) -> Result<Vec<u8>, String> {
    let dir = std::env::temp_dir().join("fluxcode-update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = std::process::Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(&dir).output().map_err(|e| format!("tar could not start ({e})"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    // The archive holds one folder with the `fluxcode` binary in it (on macOS, inside
    // FluxCode.app).
    let found = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .flat_map(|p| [p.join("FluxCode.app/Contents/MacOS/fluxcode"), p.join("fluxcode"), p])
        .find(|p| p.is_file() && p.file_name().is_some_and(|n| n == "fluxcode"))
        .ok_or("fluxcode is missing from the download")?;
    let bytes = std::fs::read(found).map_err(|e| e.to_string());
    let _ = std::fs::remove_dir_all(&dir);
    bytes
}

fn sibling(exe: &Path, ext: &str) -> PathBuf {
    let name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    exe.with_file_name(format!("{name}.{ext}"))
}

/// Put the downloaded build in place of the running executable.
fn install(archive: &Path, tag: &str) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bytes = if archive.to_string_lossy().ends_with(".xz") { unpack_xz(archive)? } else { unpack(archive)? };
    if bytes.len() < 1 << 20 || !looks_like_exe(&bytes) {
        return Err("the download is not a FluxCode executable".into());
    }
    let new = sibling(&exe, "new");
    let old = sibling(&exe, "old");
    std::fs::write(&new, &bytes).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("no permission to write to {}", exe.parent().unwrap_or(&exe).display()),
        _ => e.to_string(),
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new, &exe) {
        let _ = std::fs::rename(&old, &exe);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_file(archive);
    // macOS: the app's signature covers its executable; sign the bundle again (ad hoc,
    // as it was made) so it still verifies with the new one.
    #[cfg(target_os = "macos")]
    if let Some(app) = exe.ancestors().nth(3).filter(|a| a.extension().is_some_and(|e| e == "app")) {
        // The old binary would be sealed in with it (a running one can be unlinked here).
        let _ = std::fs::remove_file(&old);
        let _ = std::process::Command::new("/usr/bin/codesign").args(["--force", "--deep", "--sign", "-"]).arg(app).output();
    }
    refresh_icon(&exe);
    // Settings > Apps shows the build that is installed.
    #[cfg(windows)]
    {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\FluxCode";
        if crate::proc::command("reg").args(["query", key]).output().is_ok_and(|o| o.status.success()) {
            let _ = crate::proc::command("reg").args(["add", key, "/v", "DisplayVersion", "/d", tag, "/f"]).output();
        }
    }
    let _ = tag;
    Ok(exe)
}

/// Remove what the last update left behind.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        if std::fs::remove_file(sibling(&exe, "old")).is_ok() {
            // First start after an update: in case the shell drew the taskbar from its cache
            // before the restart, tell it again.
            refresh_icon(&exe);
        }
        let _ = std::fs::remove_file(sibling(&exe, "new"));
    }
}

/// Tell Windows the exe changed. The taskbar and Start Menu take the icon of the shortcut
/// that points at the exe, and the shell caches it by path: an update that brings a new
/// icon kept showing the old one until the cache was rebuilt.
fn refresh_icon(exe: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "shell32")]
        extern "system" {
            fn SHChangeNotify(event: i32, flags: u32, item1: *const std::ffi::c_void, item2: *const std::ffi::c_void);
        }
        const SHCNE_UPDATEITEM: i32 = 0x0000_2000;
        const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
        const SHCNF_IDLIST: u32 = 0x0000;
        const SHCNF_PATHW: u32 = 0x0005;
        let path: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
        unsafe {
            SHChangeNotify(SHCNE_UPDATEITEM, SHCNF_PATHW, path.as_ptr().cast(), std::ptr::null());
            // Makes Explorer drop its icon cache, the same as `ie4uinit -show`.
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null());
        }
    }
    let _ = exe;
}

impl Workbench {
    /// Look for a new build now and then every few hours (when the setting is on).
    pub(crate) fn start_update_checks(&mut self, cx: &mut Context<Self>) {
        cleanup();
        cx.spawn(async move |this, cx| {
            let mut wait = FIRST;
            loop {
                cx.background_executor().timer(wait).await;
                wait = EVERY;
                let on = this.update(cx, |this, _| this.settings.update_check).unwrap_or(false);
                if !on {
                    continue;
                }
                let busy = this
                    .update(cx, |this, _| matches!(this.update.phase, Phase::Downloading(_) | Phase::Installing(_)))
                    .unwrap_or(true);
                if busy {
                    continue;
                }
                let r = cx.background_spawn(async move { check() }).await;
                if this.update(cx, |this, cx| this.update_checked(r, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Help → Check for Updates.
    pub(crate) fn check_updates_now(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update.phase, Phase::Downloading(_) | Phase::Installing(_)) {
            return;
        }
        self.update.manual = true;
        self.update.hidden = None;
        self.status = tr("Checking for a new build…").into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = cx.background_spawn(async move { check() }).await;
            _ = this.update(cx, |this, cx| this.update_checked(r, cx));
        })
        .detach();
    }

    fn update_checked(&mut self, r: Result<Option<Release>, String>, cx: &mut Context<Self>) {
        let manual = std::mem::take(&mut self.update.manual);
        match r {
            Ok(Some(rel)) => {
                if manual {
                    self.status = crate::trf!("{0} is available"; 0 => rel.tag).into();
                }
                // Same build as before: keep its state (the card may be hidden by Later).
                if !matches!(&self.update.phase, Phase::Available(a) if a.tag == rel.tag) {
                    self.update.phase = Phase::Available(rel);
                }
            }
            Ok(None) if manual => self.status = crate::trf!("You have the newest build ({0})"; 0 => BUILD).into(),
            Err(e) if manual => self.status = crate::trf!("Could not check for updates: {0}"; 0 => e).into(),
            _ => {}
        }
        cx.notify();
    }

    pub(crate) fn update_start(&mut self, cx: &mut Context<Self>) {
        let rel = match &self.update.phase {
            Phase::Available(r) | Phase::Failed(Some(r), _) => r.clone(),
            _ => return,
        };
        let path = download_path(&rel);
        self.update.phase = Phase::Downloading(rel.clone());
        self.update.done = 0;
        cx.notify();
        // Progress: the file's size on disk, a few times a second.
        let poll = path.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(120)).await;
            let size = std::fs::metadata(&poll).map(|m| m.len()).unwrap_or(0);
            let going = this
                .update(cx, |this, cx| {
                    let going = matches!(this.update.phase, Phase::Downloading(_));
                    if going && size != this.update.done {
                        this.update.done = size;
                        cx.notify();
                    }
                    going
                })
                .unwrap_or(false);
            if !going {
                break;
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let r2 = rel.clone();
            let p2 = path.clone();
            let got = cx.background_spawn(async move { download(&r2, &p2) }).await;
            if let Err(e) = got {
                _ = this.update(cx, |this, cx| {
                    this.update.phase = Phase::Failed(Some(rel), e);
                    cx.notify();
                });
                return;
            }
            _ = this.update(cx, |this, cx| {
                this.update.phase = Phase::Installing(rel.clone());
                cx.notify();
            });
            let tag = rel.tag.clone();
            let done = cx.background_spawn(async move { install(&path, &tag) }).await;
            _ = this.update(cx, |this, cx| match done {
                Ok(exe) => {
                    this.save_session(cx);
                    match std::process::Command::new(&exe).spawn() {
                        Ok(_) => cx.quit(),
                        Err(e) => {
                            this.update.phase = Phase::Failed(None, crate::trf!("Installed, but FluxCode could not restart ({0}). Open it again."; 0 => e));
                            cx.notify();
                        }
                    }
                }
                Err(e) => {
                    this.update.phase = Phase::Failed(Some(rel), e);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// A new build was found and its toast was put away: the title bar keeps a small
    /// Update button (like Zed's and Chrome's) so it isn't lost.
    pub(crate) fn update_waiting(&self) -> Option<&Release> {
        match &self.update.phase {
            Phase::Available(r) if self.update.hidden.as_deref() == Some(r.tag.as_str()) => Some(r),
            _ => None,
        }
    }

    /// The toast in the bottom-right corner: new build, download progress, or what failed.
    /// Short on purpose: one title, one line about the build, one about what changed,
    /// and the actions. The full notes are behind What's new.
    pub(crate) fn render_update(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let u = &self.update;
        let rel = match &u.phase {
            Phase::Idle => return None,
            Phase::Available(r) => {
                if u.hidden.as_deref() == Some(r.tag.as_str()) {
                    return None;
                }
                Some(r)
            }
            Phase::Downloading(r) | Phase::Installing(r) => Some(r),
            Phase::Failed(r, _) => r.as_ref(),
        };
        let tag: SharedString = rel.map(|r| r.tag.clone()).unwrap_or_default().into();
        let close = matches!(u.phase, Phase::Available(_) | Phase::Failed(..));
        let failed = matches!(u.phase, Phase::Failed(..));
        let title = match &u.phase {
            Phase::Downloading(_) => tr("Downloading the update"),
            Phase::Installing(_) => tr("Installing…"),
            Phase::Failed(..) => tr("The update did not finish"),
            _ => tr("Update ready"),
        };
        let head = h_flex()
            .gap_2()
            .items_center()
            .child(
                Icon::new(if failed { IconName::TriangleAlert } else { IconName::ArrowDown })
                    .size(theme::u(14.))
                    .text_color(c(if failed { theme::DANGER } else { theme::FG_MUTED })),
            )
            .child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
            .when(close, |h| {
                h.child(
                    Button::new("upd-close")
                        .icon(IconName::Close)
                        .ghost()
                        .xsmall()
                        .tooltip(tr("Later (the Update button stays in the title bar)"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            match &this.update.phase {
                                Phase::Available(r) => this.update.hidden = Some(r.tag.clone()),
                                _ => this.update.phase = Phase::Idle,
                            }
                            cx.notify();
                        })),
                )
            });
        // Lines under the title line up with its text, not with the icon.
        let mut body = v_flex().pl(theme::u(22.)).gap_0p5().text_xs().min_w_0();
        if !tag.is_empty() {
            let full: SharedString = crate::trf!("{0}\nInstalled: {1}"; 0 => &tag, 1 => BUILD).into();
            let when = build_time(&tag).map(|t| crate::trf!("Build from {0}"; 0 => t)).unwrap_or_else(|| tag.to_string());
            body = body.child(
                div()
                    .id("upd-build")
                    .truncate()
                    .text_color(c(theme::FG_DIM))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx))
                    .child(when),
            );
        }
        match &u.phase {
            Phase::Available(r) => {
                if let Some(line) = highlights(&r.notes, 1).into_iter().next() {
                    body = body.child(div().text_color(c(theme::FG)).line_clamp(2).child(line));
                }
            }
            Phase::Downloading(r) => {
                let frac = if r.size > 0 { (u.done as f32 / r.size as f32).clamp(0., 1.) } else { 0. };
                let mb = |b: u64| format!("{:.1}", b as f64 / 1_048_576.);
                body = body
                    .child(
                        div()
                            .mt_1()
                            .h(px(3.))
                            .w_full()
                            .rounded(px(2.))
                            .bg(c(theme::BG_HOVER))
                            .child(div().h_full().rounded(px(2.)).bg(c(theme::ACCENT)).w(relative(frac))),
                    )
                    .child(div().text_color(c(theme::FG_DIM)).child(crate::trf!("{0} of {1} MB"; 0 => mb(u.done), 1 => mb(r.size))));
            }
            Phase::Installing(_) => {
                body = body.child(div().text_color(c(theme::FG_DIM)).child(tr("FluxCode restarts on its own in a moment. Your open files and folders come back.")));
            }
            Phase::Failed(_, e) => {
                body = body.child(div().text_color(c(theme::FG)).line_clamp(3).child(e.clone()));
            }
            Phase::Idle => {}
        }
        let page = rel.map(|r| r.page.clone()).unwrap_or_default();
        let notes_for = rel.cloned();
        let actions = match &u.phase {
            Phase::Available(_) | Phase::Failed(Some(_), _) => Some(
                h_flex()
                    .pl(theme::u(22.))
                    .pt_1()
                    .gap_1()
                    .flex_wrap()
                    .child(
                        Button::new("upd-go")
                            .label(if failed { tr("Try again") } else { tr("Update and restart") })
                            .primary()
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| this.update_start(cx))),
                    )
                    .when(!page.is_empty(), |h| {
                        // The notes open in FluxCode: what changed between this build and the new one.
                        h.child(Button::new("upd-notes").label(tr("What's new")).ghost().xsmall().on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(r) = notes_for.clone() {
                                this.show_update_notes(r, cx);
                            }
                        })))
                    }),
            ),
            _ => None,
        };
        // Slides up and fades in once when it appears (240 ms), then stays still.
        let card = v_flex()
            .id("update-card")
            .absolute()
            .right(px(12.))
            .w(theme::u(300.))
            .max_w(relative(0.92))
            .px_3()
            .py_2p5()
            .gap_1()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER_STRONG))
            .bg(c(theme::BG_ELEVATED))
            .shadow_md()
            .occlude()
            .child(head)
            .child(body)
            .children(actions);
        let key: SharedString = format!("update-in-{tag}").into();
        Some(
            card.with_animation(key, Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint()), |el, t| {
                el.bottom(theme::u(34.) + px((1. - t) * 10.)).opacity(t)
            })
            .into_any_element(),
        )
    }
}

/// `BuildDev-2026-11-10-16-49-52` → `2026-11-10 16:49` (when the build was made, UTC).
pub fn build_time(tag: &str) -> Option<String> {
    let rest = tag.strip_prefix("BuildDev-")?;
    let p: Vec<&str> = rest.split('-').collect();
    if p.len() < 5 || !p.iter().all(|x| x.chars().all(|ch| ch.is_ascii_digit())) {
        return None;
    }
    Some(format!("{}-{}-{} {}:{}", p[0], p[1], p[2], p[3], p[4]))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in gpui's own `test` attribute.
    use super::{build_time, highlights, newest, ASSET_SUFFIX, BUILD, XZ_SUFFIX};

    #[test]
    fn prefers_the_xz_file_and_reads_it() {
        let list: serde_json::Value = serde_json::from_str(
            &r#"[{ "tag_name": "BuildDev-2999-01-01-00-00-00", "draft": false, "body": "", "html_url": "p",
                  "assets": [{ "name": "a-OLD", "browser_download_url": "old", "size": 9 },
                             { "name": "a-NEW", "browser_download_url": "new", "size": 5 }] }]"#
                .replace("OLD", ASSET_SUFFIX)
                .replace("NEW", XZ_SUFFIX),
        )
        .unwrap();
        let r = newest(&list, BUILD).unwrap();
        assert_eq!((r.url.as_str(), r.size), ("new", 5));
        // A release without the xz file: the archive, as before.
        let old: serde_json::Value = serde_json::from_str(
            &r#"[{ "tag_name": "BuildDev-2999-01-01-00-00-00", "draft": false, "assets": [{ "name": "a-OLD", "browser_download_url": "old", "size": 9 }] }]"#.replace("OLD", ASSET_SUFFIX),
        )
        .unwrap();
        assert_eq!(newest(&old, BUILD).unwrap().url, "old");
        let dir = std::env::temp_dir().join(format!("fx-xz-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("t.update.xz");
        // An xz file as CI makes it: Python's lzma, preset 9 extreme, CRC64.
        const PY: [u8; 84] = [
            0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00, 0x00, 0x04, 0xe6, 0xd6, 0xb4, 0x46, 0x02, 0x00, 0x21, 0x01, 0x1c, 0x00, 0x00, 0x00, 0x10, 0xcf, 0x58, 0xcc, 0xe0, 0x01, 0xc1, 0x00,
            0x12, 0x5d, 0x00, 0x23, 0x1b, 0x0a, 0xa7, 0xb6, 0x1f, 0x53, 0x09, 0xdb, 0x20, 0x7c, 0xa6, 0x60, 0x59, 0x09, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x9c, 0x56, 0x6a, 0x82,
            0x1a, 0x6d, 0x51, 0x92, 0x00, 0x01, 0x2e, 0xc2, 0x03, 0x00, 0x00, 0x00, 0xce, 0xa4, 0x93, 0x7f, 0xb1, 0xc4, 0x67, 0xfb, 0x02, 0x00, 0x00, 0x00, 0x00, 0x04, 0x59, 0x5a,
        ];
        std::fs::write(&f, PY).unwrap();
        assert_eq!(super::unpack_xz(&f).unwrap(), b"FluxCode ".repeat(50));
        // Round trip, and a cut-off download is refused.
        let data = b"FluxCode update test ".repeat(1000);
        let mut xz = Vec::new();
        lzma_rs::xz_compress(&mut &data[..], &mut xz).unwrap();
        std::fs::write(&f, &xz).unwrap();
        assert_eq!(super::unpack_xz(&f).unwrap(), data);
        std::fs::write(&f, &xz[..xz.len() - 4]).unwrap();
        assert!(super::unpack_xz(&f).is_err(), "a cut-off download is refused");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_time_reads_the_tag() {
        assert_eq!(build_time("BuildDev-2026-11-10-16-49-52").as_deref(), Some("2026-11-10 16:49"));
        assert_eq!(build_time("gpui-v0.9.1-dev"), None);
    }

    #[test]
    fn picks_the_newest_newer_build_with_this_platforms_file() {
        let text = r#"[
            { "tag_name": "BuildDev-2026-11-10-08-00-00", "draft": false, "body": "- **Faster** start\n- See [notes](https://x)",
              "html_url": "https://github.com/r/1", "assets": [{ "name": "FluxCode-BuildDev-2026-11-10-08-00-00-SUFFIX", "browser_download_url": "https://dl/1", "size": 10 }] },
            { "tag_name": "BuildDev-2026-11-11-08-00-00", "draft": true, "assets": [] },
            { "tag_name": "BuildDev-2026-11-12-08-00-00", "draft": false, "assets": [{ "name": "something-else.zip", "browser_download_url": "x", "size": 1 }] },
            { "tag_name": "gpui-v0.9.1-dev", "draft": false, "assets": [] }
        ]"#
        .replace("SUFFIX", ASSET_SUFFIX);
        let list: serde_json::Value = serde_json::from_str(&text).unwrap();
        let r = newest(&list, "BuildDev-2026-11-09-17-00-00").unwrap();
        assert_eq!(r.tag, "BuildDev-2026-11-10-08-00-00");
        assert_eq!(r.size, 10);
        assert_eq!(highlights(&r.notes, 3), vec!["Faster start", "See notes"]);
        let wrapped = "## What changed\n\n- One thing that wraps\n  onto a second line. More detail.\n- Two\n\nText";
        assert_eq!(highlights(wrapped, 3), vec!["One thing that wraps onto a second line.", "Two"]);
        assert!(newest(&list, "BuildDev-2026-11-10-08-00-00").is_none());
    }

    #[test]
    fn build_id_has_the_expected_shape() {
        // BuildDev-YYYY-MM-DD-HH-MM-SS
        assert_eq!(BUILD.len(), "BuildDev-2026-11-09-17-30-12".len());
        assert!(BUILD.starts_with("BuildDev-"));
    }
}

#[cfg(test)]
mod real_file {
    /// `FX_XZ=path cargo test --bin fluxcode real_update_file -- --ignored`: a real
    /// `.update.xz` from CI decodes to an executable.
    #[test]
    #[ignore]
    fn real_update_file() {
        let p = std::path::PathBuf::from(std::env::var("FX_XZ").expect("FX_XZ"));
        let t = std::time::Instant::now();
        let b = super::unpack_xz(&p).unwrap();
        assert!(super::looks_like_exe(&b));
        println!("{} bytes in {:?}", b.len(), t.elapsed());
    }
}
