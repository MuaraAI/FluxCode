
use gpui_kit::component::{h_flex, v_flex, Icon, IconName, Sizable};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::Workbench;

pub const BUILD: &str = env!("FLUXCODE_BUILD");
const RELEASES: &str = "https://api.github.com/repos/MuaraAI/FluxCode/releases?per_page=20";
const FEED: &str = "https://github.com/MuaraAI/FluxCode/releases.atom";
const DOWNLOAD: &str = "https://github.com/MuaraAI/FluxCode/releases/download";
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const FIRST: Duration = Duration::from_secs(4);

#[cfg(windows)]
const ASSET_SUFFIX: &str = "windows-x64.zip";
#[cfg(target_os = "macos")]
const ASSET_SUFFIX: &str = "macos-universal.tar.gz";
#[cfg(not(any(windows, target_os = "macos")))]
const ASSET_SUFFIX: &str = "linux-x64.tar.gz";

#[cfg(windows)]
const XZ_SUFFIX: &str = "windows-x64.update.xz";
#[cfg(target_os = "macos")]
const XZ_SUFFIX: &str = "macos-universal.update.xz";
#[cfg(not(any(windows, target_os = "macos")))]
const XZ_SUFFIX: &str = "linux-x64.update.xz";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    pub notes: String,
    pub page: String,
    pub url: String,
    pub size: u64,
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
    pub done: u64,
    pub hidden: Option<String>,
    pub manual: bool,
}

fn curl() -> std::process::Command {
    crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
}

pub fn check() -> Result<Option<Release>, String> {
    match get(RELEASES, true).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string())) {
        Ok(list) => Ok(newest(&list, BUILD)),
        Err(api) => check_feed().map_err(|feed| if feed == api { api } else { format!("{api}; {feed}") }),
    }
}

fn get(url: &str, api: bool) -> Result<Vec<u8>, String> {
    let mut cmd = curl();
    cmd.args(["-s", "-S", "-f", "-L", "--max-time", "20", "-H", "User-Agent: FluxCode"]);
    if api {
        cmd.args(["-H", "Accept: application/vnd.github+json"]);
    }
    let out = cmd.arg(url).output().map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "GitHub did not answer".into() } else { err });
    }
    Ok(out.stdout)
}

fn check_feed() -> Result<Option<Release>, String> {
    let body = get(FEED, false)?;
    let list = feed_releases(&String::from_utf8_lossy(&body));
    let Some(mut best) = newest(&list, BUILD) else { return Ok(None) };
    match size_of(&best.url) {
        Some(n) => best.size = n,
        None => {
            let archive = best.url.replace(XZ_SUFFIX, ASSET_SUFFIX);
            best.size = size_of(&archive).ok_or("the build's file is not on GitHub yet")?;
            best.url = archive;
        }
    }
    Ok(Some(best))
}

fn size_of(url: &str) -> Option<u64> {
    let out = curl().args(["-s", "-f", "-I", "-L", "--max-time", "20", "-H", "User-Agent: FluxCode", url]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse::<u64>().ok()).flatten()
        })
        .last()
        .filter(|n| *n > 0)
}

fn feed_releases(xml: &str) -> serde_json::Value {
    let list: Vec<serde_json::Value> = xml
        .split("<entry>")
        .skip(1)
        .filter_map(|e| {
            let title = between(e, "<title>", "</title>")?;
            let tag = title.split_whitespace().find(|w| w.starts_with("BuildDev-"))?.to_string();
            let notes = between(e, "<content type=\"html\">", "</content>").map(|h| feed_notes(&unescape(h))).unwrap_or_default();
            let page = between(e, "<link rel=\"alternate\" type=\"text/html\" href=\"", "\"").unwrap_or_default().to_string();
            let name = format!("FluxCode-{tag}-{XZ_SUFFIX}");
            Some(serde_json::json!({
                "tag_name": tag, "draft": false, "body": notes, "html_url": page,
                "assets": [{ "name": name, "browser_download_url": format!("{DOWNLOAD}/{tag}/{name}"), "size": 0 }],
            }))
        })
        .collect();
    serde_json::Value::Array(list)
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let i = s.find(open)? + open.len();
    let j = s[i..].find(close)? + i;
    Some(&s[i..j])
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

fn feed_notes(html: &str) -> String {
    let html = html.split("<h2>Download</h2>").next().unwrap_or_default();
    let mut md = html
        .replace("<h2>", "\n## ")
        .replace("</h2>", "\n")
        .replace("<li>", "- ")
        .replace("</li>\n", "\n")
        .replace("</li>", "\n")
        .replace("</p>", "\n\n")
        .replace("<code>", "`")
        .replace("</code>", "`");
    let mut out = String::with_capacity(md.len());
    let mut tag = false;
    for ch in md.drain(..) {
        match ch {
            '<' => tag = true,
            '>' if tag => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    let out = unescape(&out);
    let mut lines: Vec<&str> = Vec::new();
    for l in out.lines().map(str::trim_end) {
        if !(l.is_empty() && lines.last().is_none_or(|p| p.is_empty())) {
            lines.push(l);
        }
    }
    lines.join("\n").trim().to_string()
}

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
            let url = asset["browser_download_url"].as_str()?;
            if !url.starts_with("https://") {
                return None;
            }
            Some(Release {
                tag: tag.to_string(),
                notes: r["body"].as_str().unwrap_or_default().to_string(),
                page: r["html_url"].as_str().unwrap_or_default().to_string(),
                url: url.to_string(),
                size: asset["size"].as_u64().unwrap_or(0),
                between: Vec::new(),
            })
        })
        .max_by(|a, b| a.tag.cmp(&b.tag))?;
    best.between = between;
    Some(best)
}

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

fn staging_dir() -> PathBuf {
    let dir = crate::settings::config_dir().join("update-staging");
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    dir
}

fn download_path(r: &Release) -> PathBuf {
    let suffix = if r.url.ends_with(XZ_SUFFIX) { XZ_SUFFIX } else { ASSET_SUFFIX };
    staging_dir().join(format!("FluxCode-{}-{suffix}", r.tag))
}

fn unpack_xz(archive: &Path) -> Result<Vec<u8>, String> {
    let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    lzma_rs::xz_decompress(&mut std::io::BufReader::new(f), &mut out).map_err(|e| format!("the download is damaged ({e})"))?;
    Ok(out)
}

fn looks_like_exe(b: &[u8]) -> bool {
    if cfg!(windows) {
        b.starts_with(b"MZ")
    } else if cfg!(target_os = "macos") {
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
    let dir = staging_dir().join(format!("unpack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = std::process::Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(&dir).output().map_err(|e| format!("tar could not start ({e})"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
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
    #[cfg(target_os = "macos")]
    if let Some(app) = exe.ancestors().nth(3).filter(|a| a.extension().is_some_and(|e| e == "app")) {
        let _ = std::fs::remove_file(&old);
        let _ = std::process::Command::new("/usr/bin/codesign").args(["--force", "--deep", "--sign", "-"]).arg(app).output();
    }
    refresh_icon(&exe);
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

pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        if std::fs::remove_file(sibling(&exe, "old")).is_ok() {
            refresh_icon(&exe);
        }
        let _ = std::fs::remove_file(sibling(&exe, "new"));
    }
}

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
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null());
        }
    }
    let _ = exe;
}

impl Workbench {
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

    pub(crate) fn update_waiting(&self) -> Option<&Release> {
        match &self.update.phase {
            Phase::Available(r) if self.update.hidden.as_deref() == Some(r.tag.as_str()) => Some(r),
            _ => None,
        }
    }

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
        let full: SharedString = if tag.is_empty() {
            SharedString::default()
        } else {
            let when = build_time(&tag).map(|t| crate::trf!("Build from {0}"; 0 => t)).unwrap_or_else(|| tag.to_string());
            format!("{when}\n{}", crate::trf!("{0}\nInstalled: {1}"; 0 => &tag, 1 => BUILD)).into()
        };
        let accent = if failed { theme::DANGER } else { theme::ACCENT_TEXT };
        let busy = matches!(u.phase, Phase::Downloading(_) | Phase::Installing(_));
        let page = rel.map(|r| r.page.clone()).unwrap_or_default();
        let notes_for = rel.cloned();
        let text_btn = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex_none()
                .h(theme::u(22.))
                .px_1p5()
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor_pointer()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .child(tr(label))
        };
        let can_act = matches!(u.phase, Phase::Available(_) | Phase::Failed(Some(_), _));
        let head = h_flex()
            .gap_2()
            .child(if busy {
                gpui_kit::component::spinner::Spinner::new().icon(IconName::LoaderCircle).xsmall().color(c(accent)).into_any_element()
            } else {
                Icon::new(if failed { IconName::TriangleAlert } else { IconName::ArrowDown }).flex_none().size(theme::u(13.)).text_color(c(accent)).into_any_element()
            })
            .child(
                div()
                    .id("upd-title")
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(theme::FG_STRONG))
                    .when(!full.is_empty(), |d| {
                        let full = full.clone();
                        d.tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx))
                    })
                    .child(title),
            )
            .when(can_act && !page.is_empty(), |h| {
                h.child(text_btn("upd-notes", "What's new").on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(r) = notes_for.clone() {
                        this.show_update_notes(r, cx);
                    }
                })))
            })
            .when(can_act, |h| {
                h.child(
                    div()
                        .id("upd-go")
                        .flex_none()
                        .h(theme::u(22.))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .bg(c(theme::ACCENT))
                        .text_color(c(theme::ON_ACCENT))
                        .hover(|s| s.opacity(0.88))
                        .child(if failed { tr("Try again") } else { tr("Restart") })
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Install the update and restart FluxCode; your open files come back.")).build(w, cx))
                        .on_click(cx.listener(|this, _, _, cx| this.update_start(cx))),
                )
            })
            .when(close, |h| {
                h.child(
                    div()
                        .id("upd-close")
                        .flex_none()
                        .size(theme::u(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_color(c(theme::FG_DIM))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Later (the Update button stays in the title bar)")).build(w, cx))
                        .child(Icon::new(IconName::Close).size(theme::u(11.)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            match &this.update.phase {
                                Phase::Available(r) => this.update.hidden = Some(r.tag.clone()),
                                _ => this.update.phase = Phase::Idle,
                            }
                            cx.notify();
                        })),
                )
            });
        let sub: Option<AnyElement> = match &u.phase {
            Phase::Available(r) => highlights(&r.notes, 1).into_iter().next().map(|l| div().truncate().child(l).into_any_element()),
            Phase::Downloading(r) => {
                let frac = if r.size > 0 { (u.done as f32 / r.size as f32).clamp(0., 1.) } else { 0. };
                let mb = |b: u64| format!("{:.1}", b as f64 / 1_048_576.);
                Some(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().h(px(3.)).rounded_full().bg(c(theme::FG_DIM).opacity(0.2)).child(div().h_full().rounded_full().bg(c(theme::ACCENT)).w(relative(frac))))
                        .child(div().flex_none().child(crate::trf!("{0} of {1} MB"; 0 => mb(u.done), 1 => mb(r.size))))
                        .into_any_element(),
                )
            }
            Phase::Installing(_) => Some(div().truncate().child(tr("Restarting in a moment; your files come back.")).into_any_element()),
            Phase::Failed(_, e) => Some(div().line_clamp(2).child(e.clone()).into_any_element()),
            Phase::Idle => None,
        };
        let card = v_flex()
            .id("update-card")
            .absolute()
            .right(px(12.))
            .w(theme::u(340.))
            .max_w(relative(0.9))
            .pl_3()
            .pr_2()
            .py_2()
            .gap_1()
            .glass_pop()
            .rounded(px(10.))
            .occlude()
            .child(head)
            .children(sub.map(|s| div().pl(theme::u(21.)).pr_1().text_xs().text_color(c(theme::FG_DIM)).child(s)));

        let key: SharedString = format!("update-in-{tag}").into();
        Some(
            card.with_animation(key, Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint()), |el, t| {
                el.bottom(theme::u(34.) + px((1. - t) * 10.)).opacity(t)
            })
            .into_any_element(),
        )
    }
}

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
    use super::{build_time, feed_releases, highlights, newest, ASSET_SUFFIX, BUILD, XZ_SUFFIX};

    #[test]
    fn reads_the_releases_feed() {
        let xml = r#"<feed><title>Release notes from FluxCode</title>
  <entry>
    <link rel="alternate" type="text/html" href="https://github.com/MuaraAI/FluxCode/releases/tag/BuildDev-2999-01-02-00-00-00"/>
    <title>FluxCode BuildDev-2999-01-02-00-00-00</title>
    <content type="html">&lt;p&gt;Development build, made from &lt;code&gt;0b745e2&lt;/code&gt;.&lt;/p&gt;
&lt;h2&gt;What changed&lt;/h2&gt;
&lt;ul&gt;
&lt;li&gt;French and Malay interface; quieter update toast&lt;/li&gt;
&lt;li&gt;Liquid glass across popups &amp;amp; menus&lt;/li&gt;
&lt;/ul&gt;
&lt;h2&gt;Download&lt;/h2&gt;
&lt;table&gt;&lt;tr&gt;&lt;td&gt;x&lt;/td&gt;&lt;/tr&gt;&lt;/table&gt;</content>
  </entry>
  <entry><title>FluxCode BuildDev-2999-01-01-00-00-00</title><content type="html">&lt;ul&gt;&lt;li&gt;Older&lt;/li&gt;&lt;/ul&gt;</content></entry>
</feed>"#;
        let list = feed_releases(xml);
        let r = newest(&list, BUILD).unwrap();
        assert_eq!(r.tag, "BuildDev-2999-01-02-00-00-00");
        assert_eq!(r.page, "https://github.com/MuaraAI/FluxCode/releases/tag/BuildDev-2999-01-02-00-00-00");
        assert!(r.url.ends_with(&format!("/BuildDev-2999-01-02-00-00-00/FluxCode-BuildDev-2999-01-02-00-00-00-{XZ_SUFFIX}")), "{}", r.url);
        assert!(r.url.starts_with("https://github.com/MuaraAI/FluxCode/releases/download/"));
        assert!(!r.notes.contains("Download") && !r.notes.contains('<'), "{}", r.notes);
        assert_eq!(highlights(&r.notes, 2), vec!["French and Malay interface; quieter update toast", "Liquid glass across popups & menus"]);
        assert_eq!(r.between.len(), 2);
        assert_eq!(r.between[1], ("BuildDev-2999-01-01-00-00-00".to_string(), "Older".to_string()));
    }

    #[test]
    fn prefers_the_xz_file_and_reads_it() {
        let list: serde_json::Value = serde_json::from_str(
            &r#"[{ "tag_name": "BuildDev-2999-01-01-00-00-00", "draft": false, "body": "", "html_url": "p",
                  "assets": [{ "name": "a-OLD", "browser_download_url": "https://example.com/old", "size": 9 },
                             { "name": "a-NEW", "browser_download_url": "https://example.com/new", "size": 5 }] }]"#
                .replace("OLD", ASSET_SUFFIX)
                .replace("NEW", XZ_SUFFIX),
        )
        .unwrap();
        let r = newest(&list, BUILD).unwrap();
        assert_eq!((r.url.as_str(), r.size), ("https://example.com/new", 5));
        let old: serde_json::Value = serde_json::from_str(
            &r#"[{ "tag_name": "BuildDev-2999-01-01-00-00-00", "draft": false, "assets": [{ "name": "a-OLD", "browser_download_url": "https://example.com/old", "size": 9 }] }]"#.replace("OLD", ASSET_SUFFIX),
        )
        .unwrap();
        assert_eq!(newest(&old, BUILD).unwrap().url, "https://example.com/old");
        let dir = std::env::temp_dir().join(format!("fx-xz-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("t.update.xz");
        const PY: [u8; 84] = [
            0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00, 0x00, 0x04, 0xe6, 0xd6, 0xb4, 0x46, 0x02, 0x00, 0x21, 0x01, 0x1c, 0x00, 0x00, 0x00, 0x10, 0xcf, 0x58, 0xcc, 0xe0, 0x01, 0xc1, 0x00,
            0x12, 0x5d, 0x00, 0x23, 0x1b, 0x0a, 0xa7, 0xb6, 0x1f, 0x53, 0x09, 0xdb, 0x20, 0x7c, 0xa6, 0x60, 0x59, 0x09, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x9c, 0x56, 0x6a, 0x82,
            0x1a, 0x6d, 0x51, 0x92, 0x00, 0x01, 0x2e, 0xc2, 0x03, 0x00, 0x00, 0x00, 0xce, 0xa4, 0x93, 0x7f, 0xb1, 0xc4, 0x67, 0xfb, 0x02, 0x00, 0x00, 0x00, 0x00, 0x04, 0x59, 0x5a,
        ];
        std::fs::write(&f, PY).unwrap();
        assert_eq!(super::unpack_xz(&f).unwrap(), b"FluxCode ".repeat(50));
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
        assert_eq!(BUILD.len(), "BuildDev-2026-11-09-17-30-12".len());
        assert!(BUILD.starts_with("BuildDev-"));
    }
}

#[cfg(test)]
mod real_file {
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
