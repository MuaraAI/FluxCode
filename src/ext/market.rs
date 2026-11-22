//! VS Code Marketplace: search it, show an extension, install its VSIX.

use crate::i18n::tr;
use serde_json::{json, Value};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const GALLERY: &str = "https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery";
const ACCEPT: &str = "Accept: application/json;api-version=3.0-preview.1";
pub const ICON_SMALL: &str = "Microsoft.VisualStudio.Services.Icons.Small";
pub const ICON_DEFAULT: &str = "Microsoft.VisualStudio.Services.Icons.Default";
pub const DETAILS: &str = "Microsoft.VisualStudio.Services.Content.Details";
pub const VSIX: &str = "Microsoft.VisualStudio.Services.VSIXPackage";
/// Unpacked size an install may reach; past it the VSIX is refused.
const MAX_UNPACKED: u64 = 1 << 30;
/// README text kept for the page: the rest is cut.
const MAX_README: usize = 200 * 1024;

/// One extension from a gallery query (its latest version for this platform).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    /// The publisher's id (`ms-python`), half of the identifier.
    pub publisher: String,
    pub publisher_display: String,
    pub verified: bool,
    pub name: String,
    pub display: String,
    pub short: String,
    pub version: String,
    pub asset_uri: String,
    /// Asset type → URL, from the version's file list.
    pub files: Vec<(String, String)>,
    pub installs: u64,
    pub rating: Option<f32>,
    pub rating_count: u64,
    /// `YYYY-MM-DD`.
    pub updated: String,
    pub categories: Vec<String>,
}

impl Item {
    /// `publisher.name`, lowercase: VS Code ids ignore case.
    pub fn id(&self) -> String {
        format!("{}.{}", self.publisher, self.name).to_lowercase()
    }

    /// An asset's URL: the listed file, else built from the asset URI.
    pub fn asset(&self, kind: &str) -> Option<String> {
        self.files
            .iter()
            .find(|(k, _)| k == kind)
            .map(|(_, u)| u.clone())
            .or_else(|| (!self.asset_uri.is_empty()).then(|| format!("{}/{kind}", self.asset_uri)))
    }

    /// Where its icon is kept on disk once fetched.
    pub fn icon_cache(&self) -> Option<PathBuf> {
        let file = folder_name(&self.publisher, &self.name, &self.version)?;
        Some(
            crate::settings::config_dir()
                .join("vsx-icons")
                .join(format!("{file}.png")),
        )
    }
}

/// Extensions per gallery page.
pub const PAGE_SIZE: usize = 50;

/// VS Code's Marketplace categories, as the gallery names them.
pub const CATEGORIES: &[&str] = &[
    "Programming Languages",
    "Themes",
    "Snippets",
    "Linters",
    "Formatters",
    "Debuggers",
    "Keymaps",
    "AI",
    "Chat",
    "Data Science",
    "Machine Learning",
    "Notebooks",
    "Testing",
    "SCM Providers",
    "Language Packs",
    "Extension Packs",
    "Visualization",
    "Education",
    "Azure",
    "Other",
];

/// List orders the gallery can sort by.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sort {
    Relevance,
    Installs,
    Rating,
    Updated,
    Name,
}

impl Sort {
    pub const ALL: [Sort; 5] = [
        Sort::Relevance,
        Sort::Installs,
        Sort::Rating,
        Sort::Updated,
        Sort::Name,
    ];

    /// The gallery's `sortBy` and `sortOrder` (0: its default, 1: ascending).
    fn code(self) -> (u32, u32) {
        match self {
            Sort::Relevance => (0, 0),
            Sort::Installs => (4, 0),
            Sort::Rating => (12, 0),
            Sort::Updated => (1, 0),
            Sort::Name => (2, 1),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Sort::Relevance => tr("Relevance"),
            Sort::Installs => tr("Installs"),
            Sort::Rating => tr("Rating"),
            Sort::Updated => tr("Updated"),
            Sort::Name => tr("Name"),
        }
    }
}

/// What the list asks the gallery for.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Query {
    pub text: String,
    pub sort: Sort,
    pub category: Option<&'static str>,
}

impl Query {
    /// The text trimmed with single spaces.
    pub fn new(text: &str, sort: Option<Sort>, category: Option<&'static str>) -> Self {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let sort = match sort {
            Some(Sort::Relevance) | None if text.is_empty() => Sort::Installs,
            None => Sort::Relevance,
            Some(s) => s,
        };
        Query {
            text,
            sort,
            category,
        }
    }
}

/// One page of answers is cached under this: the gallery's search ignores case.
pub type CacheKey = (String, Sort, Option<&'static str>, usize);

pub fn cache_key(q: &Query, page: usize) -> CacheKey {
    (q.text.to_lowercase(), q.sort, q.category, page)
}

/// The JSON a gallery query sends for one page (from 1): VS Code extensions without unpublished ones, maybe a search text and a category.
pub fn query_body(q: &Query, page: usize) -> String {
    let mut criteria = vec![json!({"filterType": 8, "value": "Microsoft.VisualStudio.Code"})];
    if !q.text.is_empty() {
        criteria.push(json!({"filterType": 10, "value": q.text}));
    }
    if let Some(c) = q.category {
        criteria.push(json!({"filterType": 5, "value": c}));
    }
    criteria.push(json!({"filterType": 12, "value": "4096"}));
    let (sort, order) = q.sort.code();
    // 918 = files (2) + categories and tags (4) + version properties (16) + asset URI (128) + statistics (256) + latest version only (512): every row's icon, numbers, categories and VSIX in one answer.
    json!({"filters": [{"criteria": criteria, "pageNumber": page, "pageSize": PAGE_SIZE, "sortBy": sort, "sortOrder": order}], "flags": 918}).to_string()
}

/// The VSIX target this build runs on (`win32-x64`); platform-specific extensions publish one version per target.
fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "win32-x64",
        ("windows", "aarch64") => "win32-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("macos", "aarch64") => "darwin-arm64",
        _ => "",
    }
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn item(e: &Value) -> Option<Item> {
    let p = e.get("publisher")?;
    let versions = e.get("versions").and_then(Value::as_array)?;
    fn plat(v: &Value) -> Option<&str> {
        v.get("targetPlatform").and_then(Value::as_str)
    }
    let ver = versions
        .iter()
        .find(|v| plat(v) == Some(target()))
        .or_else(|| {
            versions
                .iter()
                .find(|v| matches!(plat(v), None | Some("universal")))
        })
        .or(versions.first())?;
    let stat = |name: &str| {
        e.get("statistics")
            .and_then(Value::as_array)
            .and_then(|a| {
                a.iter()
                    .find(|s| s.get("statisticName").and_then(Value::as_str) == Some(name))
            })
            .and_then(|s| s.get("value"))
            .and_then(Value::as_f64)
    };
    let (publisher, name) = (text(p, "publisherName"), text(e, "extensionName"));
    if publisher.is_empty() || name.is_empty() {
        return None;
    }
    // Some names start with spaces to sort first by name: trimmed.
    let or = |s: String, d: &str| {
        let t = s.trim();
        if t.is_empty() {
            d.to_string()
        } else {
            t.to_string()
        }
    };
    let files = ver
        .get("files")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|f| (text(f, "assetType"), text(f, "source")))
                .filter(|(k, u)| !k.is_empty() && u.starts_with("https://"))
                .collect()
        })
        .unwrap_or_default();
    Some(Item {
        publisher_display: or(text(p, "displayName"), &publisher),
        verified: text(p, "flags").to_lowercase().contains("verified")
            || p.get("isDomainVerified")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        display: or(text(e, "displayName"), &name),
        short: text(e, "shortDescription"),
        version: text(ver, "version"),
        asset_uri: text(ver, "assetUri"),
        files,
        installs: stat("install").unwrap_or(0.0).max(0.0) as u64,
        rating: stat("averagerating").filter(|r| *r > 0.0).map(|r| r as f32),
        rating_count: stat("ratingcount").unwrap_or(0.0).max(0.0) as u64,
        updated: text(e, "lastUpdated").chars().take(10).collect(),
        categories: e
            .get("categories")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        publisher,
        name,
    })
}

/// One page of a gallery answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub items: Vec<Item>,
    /// How many extensions match in all (the answer's ResultCount).
    pub total: Option<u64>,
}

/// `resultMetadata`'s TotalCount.
fn result_count(result: &Value) -> Option<u64> {
    result
        .get("resultMetadata")?
        .as_array()?
        .iter()
        .find(|m| m.get("metadataType").and_then(Value::as_str) == Some("ResultCount"))?
        .get("metadataItems")?
        .as_array()?
        .iter()
        .find(|i| i.get("name").and_then(Value::as_str) == Some("TotalCount"))?
        .get("count")?
        .as_u64()
}

/// The extensions in a gallery answer, and how many match in all.
pub fn parse(body: &str) -> Result<Page, String> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| format!("The Marketplace sent something unexpected ({e})."))?;
    let result = v
        .pointer("/results/0")
        .ok_or_else(|| "The Marketplace answer has no results.".to_string())?;
    let list = result
        .get("extensions")
        .and_then(Value::as_array)
        .ok_or_else(|| "The Marketplace answer has no results.".to_string())?;
    Ok(Page {
        items: list.iter().filter_map(item).collect(),
        total: result_count(result),
    })
}

/// A count with thousands separators: 1,234,567.
pub fn format_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// An install count the way VS Code shows it: 950, 5.6K, 48.4M.
pub fn format_count(n: u64) -> String {
    let short = |v: f64, unit: &str| {
        let r = (v * 10.0).round() / 10.0;
        if r.fract() == 0.0 {
            format!("{}{unit}", r as u64)
        } else {
            format!("{r:.1}{unit}")
        }
    };
    match n {
        999_950_000.. => short(n as f64 / 1e9, "B"),
        999_950.. => short(n as f64 / 1e6, "M"),
        1_000.. => short(n as f64 / 1e3, "K"),
        _ => n.to_string(),
    }
}

/// A file or folder name built from gallery data: letters, digits, `.`, `-`, `_`, not starting with a dot, so it can't climb out of its folder or hide.
pub fn safe_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 160
        && !s.starts_with('.')
        && !s.contains("..")
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// `publisher.name-version`, lowercase like VS Code's own folders; None when a part isn't a safe name.
pub fn folder_name(publisher: &str, name: &str, version: &str) -> Option<String> {
    let f = format!(
        "{}.{}-{}",
        publisher.to_lowercase(),
        name.to_lowercase(),
        version
    );
    (safe_name(publisher) && safe_name(name) && safe_name(version) && safe_name(&f)).then_some(f)
}

/// Where a VSIX entry goes inside the extension's folder: files under `extension/` only, and none whose path has an empty, `.`, `..` or drive part.
pub fn entry_path(name: &str) -> Option<PathBuf> {
    let rest = name.strip_prefix("extension/")?;
    if rest.is_empty() || rest.ends_with('/') {
        return None;
    }
    let mut p = PathBuf::new();
    for seg in rest.split(['/', '\\']) {
        if seg.is_empty()
            || seg == "."
            || seg == ".."
            || seg.contains(':')
            || seg.chars().any(char::is_control)
        {
            return None;
        }
        p.push(seg);
    }
    Some(p)
}


fn curl() -> Command {
    crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
}

fn finish(out: std::process::Output) -> Result<Vec<u8>, String> {
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(crate::acp::curl_error(&String::from_utf8_lossy(
            &out.stderr,
        )))
    }
}

/// One page (from 1) of a gallery query (blocking: run it off the UI thread).
pub fn search(query: &Query, page: usize) -> Result<Page, String> {
    use std::io::Write;
    let mut child = curl()
        .args([
            "-s",
            "-S",
            "-f",
            "-L",
            "--compressed",
            "--max-time",
            "20",
            "-H",
            "User-Agent: FluxCode",
            "-H",
            ACCEPT,
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            GALLERY,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl could not start ({e})"))?;
    // The body goes through stdin: no quoting of JSON on a Windows command line.
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(query_body(query, page).as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let out = finish(child.wait_with_output().map_err(|e| e.to_string())?)?;
    parse(&String::from_utf8_lossy(&out))
}

/// Download `url` into `to` (blocking).
fn download(url: &str, to: &Path, max_secs: u32, max_bytes: u64) -> Result<(), String> {
    let out = curl()
        .args([
            "-s",
            "-S",
            "-f",
            "-L",
            "--compressed",
            "-H",
            "User-Agent: FluxCode",
            "--max-time",
            &max_secs.to_string(),
            "--max-filesize",
            &max_bytes.to_string(),
            "-o",
        ])
        .arg(to)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    finish(out).map(|_| ())
}

/// Fetch an extension's icon into the cache (the small one, else the default); the cached file when it is there already.
pub fn fetch_icon(urls: &[String], to: &Path) -> bool {
    if std::fs::metadata(to).is_ok_and(|m| m.len() > 0) {
        return true;
    }
    let Some(dir) = to.parent() else { return false };
    let _ = std::fs::create_dir_all(dir);
    let part = to.with_extension("part");
    for url in urls {
        if download(url, &part, 20, 2_000_000).is_ok()
            && std::fs::metadata(&part).is_ok_and(|m| m.len() > 0)
            && std::fs::rename(&part, to).is_ok()
        {
            return true;
        }
    }
    let _ = std::fs::remove_file(&part);
    false
}

/// An extension's README: from its folder when installed, else from the gallery.
pub fn readme(item: &Item, installed: Option<&Path>) -> Result<String, String> {
    let local = installed
        .and_then(|d| std::fs::read_dir(d).ok())
        .and_then(|rd| {
            rd.flatten()
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case("readme.md")
                })
                .and_then(|e| std::fs::read_to_string(e.path()).ok())
        });
    let mut text = match local {
        Some(t) => t,
        None => {
            let url = item
                .asset(DETAILS)
                .ok_or_else(|| tr("This extension has no README.").to_string())?;
            let out = curl()
                .args([
                    "-s",
                    "-S",
                    "-f",
                    "-L",
                    "--compressed",
                    "--max-time",
                    "20",
                    "--max-filesize",
                    "4000000",
                    "-H",
                    "User-Agent: FluxCode",
                    &url,
                ])
                .stdin(Stdio::null())
                .output()
                .map_err(|e| format!("curl could not start ({e})"))?;
            String::from_utf8_lossy(&finish(out)?).into_owned()
        }
    };
    if text.len() > MAX_README {
        let mut cut = MAX_README;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n\n…");
    }
    Ok(text)
}

/// The pictures a README shows: `![alt](url)` and `<img src="url">`, in order, without repeats.
pub fn readme_image_urls(md: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |u: &str| {
        let u = u.trim().trim_matches(['<', '>']).split_whitespace().next().unwrap_or("").to_string();
        if !u.is_empty() && !u.starts_with("data:") && !out.contains(&u) {
            out.push(u);
        }
    };
    let mut rest = md;
    while let Some(i) = rest.find("](") {
        let before = &rest[..i];
        let after = &rest[i + 2..];
        let end = after.find(')').unwrap_or(after.len());
        // Only an image: the `[` that opens this link text follows a `!`.
        if before.rfind('[').is_some_and(|o| o > 0 && before.as_bytes()[o - 1] == b'!') {
            add(&after[..end]);
        }
        rest = &after[end.min(after.len())..];
    }
    let lower = md.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<img") {
        let tag_start = from + i;
        let tag_end = lower[tag_start..].find('>').map(|e| tag_start + e).unwrap_or(lower.len());
        let tag = &md[tag_start..tag_end];
        let tl = &lower[tag_start..tag_end];
        if let Some(p) = tl.find("src=") {
            let v = &tag[p + 4..];
            let q = v.chars().next().unwrap_or(' ');
            let val = if q == '"' || q == '\'' { v[1..].split(q).next().unwrap_or("") } else { v.split_whitespace().next().unwrap_or("") };
            add(val);
        }
        from = tag_end.max(tag_start + 4);
    }
    out
}

/// What kind of picture `bytes` are, as a file extension gpui can read.
fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(256)]).to_ascii_lowercase();
    Some(match bytes {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, ..] => "jpg",
        [b'G', b'I', b'F', ..] => "gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        [b'B', b'M', ..] => "bmp",
        _ if head.contains("<svg") => "svg",
        _ => return None,
    })
}

/// Fetch a README's pictures into the cache, so the page can show them (gpui has no HTTP client here.
pub fn fetch_readme_images(md: &str, installed: Option<&Path>) -> HashMap<String, PathBuf> {
    let cache = dir().join(".readme-images");
    let _ = std::fs::create_dir_all(&cache);
    let mut out = HashMap::new();
    for url in readme_image_urls(md).into_iter().take(30) {
        if url.starts_with("http://") || url.starts_with("https://") {
            let key = sha1_smol::Sha1::from(url.as_bytes()).digest().to_string();
            let known = ["png", "jpg", "gif", "webp", "bmp", "svg"].iter().map(|e| cache.join(format!("{key}.{e}"))).find(|p| p.is_file());
            if let Some(p) = known {
                out.insert(url, p);
                continue;
            }
            let part = cache.join(format!("{key}.part"));
            if download(&url, &part, 15, 6_000_000).is_ok() {
                if let Some(ext) = std::fs::read(&part).ok().as_deref().and_then(sniff_image) {
                    let to = cache.join(format!("{key}.{ext}"));
                    if std::fs::rename(&part, &to).is_ok() {
                        out.insert(url, to);
                        continue;
                    }
                }
            }
            let _ = std::fs::remove_file(&part);
        } else if let Some(folder) = installed {
            let rel = url.trim_start_matches("./").split(['?', '#']).next().unwrap_or("");
            let p = folder.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            if p.is_file() && p.starts_with(folder) {
                out.insert(url, p);
            }
        }
    }
    out
}


/// `~/.fluxcode/vscode-extensions`.
pub fn dir() -> PathBuf {
    super::home().join(".fluxcode").join("vscode-extensions")
}

/// An extension unpacked into `dir()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Installed {
    pub id: String,
    pub folder: PathBuf,
    pub publisher: String,
    pub name: String,
    pub display: String,
    pub description: String,
    pub version: String,
    pub categories: Vec<String>,
    pub icon: Option<PathBuf>,
    /// What it declares in `contributes`, in words ("3 color themes").
    pub contributes: Vec<String>,
    /// It has a `main` or `browser` script: code for VS Code's extension host.
    pub has_code: bool,
}

impl Installed {
    /// As a gallery item, for the page when the search doesn't list it.
    pub fn item(&self) -> Item {
        Item {
            publisher: self.publisher.clone(),
            publisher_display: self.publisher.clone(),
            name: self.name.clone(),
            display: self.display.clone(),
            short: self.description.clone(),
            version: self.version.clone(),
            categories: self.categories.clone(),
            ..Default::default()
        }
    }
}

/// `contributes` in words, the parts FluxCode can read as data first.
pub fn contributes(pkg: &Value) -> Vec<String> {
    let Some(c) = pkg.get("contributes") else {
        return Vec::new();
    };
    let n = |k: &str| c.get(k).and_then(Value::as_array).map_or(0, Vec::len);
    let kinds: [(&str, &'static str, &'static str); 8] = [
        ("themes", "1 color theme", "{n} color themes"),
        ("iconThemes", "1 file icon theme", "{n} file icon themes"),
        ("languages", "1 language", "{n} languages"),
        ("grammars", "1 grammar", "{n} grammars"),
        ("snippets", "snippets", "snippets"),
        ("commands", "1 command", "{n} commands"),
        ("keybindings", "1 keybinding", "{n} keybindings"),
        ("debuggers", "1 debugger", "{n} debuggers"),
    ];
    kinds
        .into_iter()
        .filter_map(|(k, one, many)| match n(k) {
            0 => None,
            1 => Some(tr(one).to_string()),
            m => Some(crate::i18n::fill(
                tr(many),
                &[("n", &m as &dyn std::fmt::Display)],
            )),
        })
        .collect()
}

/// `%key%` names come from package.nls.json.
pub(crate) fn localized(folder: &Path, s: &str, nls: &mut Option<Value>) -> String {
    let Some(key) = s.strip_prefix('%').and_then(|k| k.strip_suffix('%')) else {
        return s.to_string();
    };
    let v = nls.get_or_insert_with(|| {
        std::fs::read_to_string(folder.join("package.nls.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null)
    });
    match v.get(key) {
        Some(Value::String(t)) => t.clone(),
        Some(o) => o
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(s)
            .to_string(),
        None => s.to_string(),
    }
}

fn read_installed(folder: &Path) -> Option<Installed> {
    let pkg: Value =
        serde_json::from_str(&std::fs::read_to_string(folder.join("package.json")).ok()?).ok()?;
    let (publisher, name) = (text(&pkg, "publisher"), text(&pkg, "name"));
    if publisher.is_empty() || name.is_empty() {
        return None;
    }
    let mut nls = None;
    let display = localized(folder, &text(&pkg, "displayName"), &mut nls);
    let icon = pkg
        .get("icon")
        .and_then(Value::as_str)
        .filter(|i| !i.contains(".."))
        .map(|i| folder.join(i))
        .filter(|p| p.is_file());
    Some(Installed {
        id: format!("{publisher}.{name}").to_lowercase(),
        folder: folder.to_path_buf(),
        display: if display.trim().is_empty() {
            name.clone()
        } else {
            display
        },
        description: localized(folder, &text(&pkg, "description"), &mut nls),
        version: text(&pkg, "version"),
        categories: pkg
            .get("categories")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        icon,
        contributes: contributes(&pkg),
        has_code: pkg.get("main").is_some() || pkg.get("browser").is_some(),
        publisher,
        name,
    })
}

/// The installed extensions, by name (blocking: a few small files).
pub fn installed_list() -> Vec<Installed> {
    let mut out: Vec<Installed> = std::fs::read_dir(dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| safe_name(&e.file_name().to_string_lossy()))
                .filter_map(|e| read_installed(&e.path()))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|i| i.display.to_lowercase());
    out
}

/// Download the VSIX and unpack it; replaces other versions of the same extension.
pub fn install(item: &Item) -> Result<(), String> {
    let folder = folder_name(&item.publisher, &item.name, &item.version).ok_or_else(bad_name)?;
    let url = item
        .asset(VSIX)
        .ok_or_else(|| tr("The Marketplace lists no package for it.").to_string())?;
    let tmp = std::env::temp_dir().join(format!("fluxcode-{folder}.vsix"));
    download(&url, &tmp, 900, 1 << 30)?;
    let bytes = std::fs::read(&tmp).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&tmp);
    unpack(&bytes?, &folder, &item.id())
}

/// Install a `.vsix` file from disk; returns the extension's name.
pub fn install_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let entries = super::zip::list(&bytes).ok_or_else(not_vsix)?;
    let pkg = entries
        .iter()
        .find(|e| e.name == "extension/package.json")
        .and_then(|e| super::zip::read(&bytes, e, 4 << 20))
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .ok_or_else(|| tr("The package has no extension/package.json.").to_string())?;
    let (publisher, name, version) = (
        text(&pkg, "publisher"),
        text(&pkg, "name"),
        text(&pkg, "version"),
    );
    let folder = folder_name(&publisher, &name, &version).ok_or_else(bad_name)?;
    unpack(
        &bytes,
        &folder,
        &format!("{publisher}.{name}").to_lowercase(),
    )?;
    let display = text(&pkg, "displayName");
    Ok(if display.is_empty() || display.starts_with('%') {
        name
    } else {
        display
    })
}

fn bad_name() -> String {
    tr("The extension's name or version has characters FluxCode won't use in a folder name.")
        .to_string()
}

fn not_vsix() -> String {
    tr("The file is not a VSIX package.").to_string()
}

/// Unpack a VSIX's `extension/` folder into `dir()/<folder>`, through a hidden partial folder renamed at the end: a failed install never leaves a half folder that looks installed.
fn unpack(bytes: &[u8], folder: &str, id: &str) -> Result<(), String> {
    let entries = super::zip::list(bytes).ok_or_else(not_vsix)?;
    let root = dir();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let partial = root.join(format!(".partial-{folder}"));
    let _ = std::fs::remove_dir_all(&partial);
    let write = || -> Result<(), String> {
        let mut total = 0u64;
        for e in &entries {
            let Some(rel) = entry_path(&e.name) else {
                continue;
            };
            total += e.size as u64;
            if total > MAX_UNPACKED {
                return Err(tr("The package unpacks to more than 1 GB.").into());
            }
            let data = super::zip::read(bytes, e, (MAX_UNPACKED - total + e.size as u64) as usize)
                .ok_or_else(|| format!("{}: unreadable entry", e.name))?;
            let to = partial.join(rel);
            if let Some(d) = to.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::write(&to, data).map_err(|e| e.to_string())?;
        }
        if !partial.join("package.json").is_file() {
            return Err(tr("The package has no extension/package.json.").into());
        }
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(e);
    }
    for old in installed_list().into_iter().filter(|i| i.id == id) {
        let _ = std::fs::remove_dir_all(&old.folder);
    }
    let dest = root.join(folder);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&partial, &dest).map_err(|e| e.to_string())
}

/// Remove an installed extension's folder (only ever one inside `dir()`).
pub fn uninstall(folder: &Path) -> Result<(), String> {
    let ok = folder.parent() == Some(dir().as_path())
        && folder
            .file_name()
            .is_some_and(|n| safe_name(&n.to_string_lossy()));
    if !ok {
        return Err(format!(
            "{} is not in {}",
            folder.display(),
            dir().display()
        ));
    }
    std::fs::remove_dir_all(folder).map_err(|e| e.to_string())
}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Busy {
    Installing,
    Removing,
}

/// Pages kept in memory before the cache starts over.
const CACHE_PAGES: usize = 400;

/// The Marketplace list: the pages loaded so far for one query, and a cache of every page fetched, so going back to an earlier query (Backspace) is instant.
#[derive(Default)]
pub struct Market {
    /// The sort the user picked (None: relevance for a search, else installs).
    pub sort: Option<Sort>,
    pub category: Option<&'static str>,
    /// The query `items` answer (None: nothing shown yet).
    pub shown: Option<Query>,
    pub items: Vec<Item>,
    pub total: Option<u64>,
    /// Pages of `shown` loaded; the last one came back short.
    pub pages: usize,
    pub done: bool,
    /// A new query's first page is on its way: the old list stays, dimmed.
    pub loading: bool,
    pub loading_more: bool,
    pub error: Option<String>,
    pub more_error: Option<String>,
    /// Bumped per query: a wait or an answer that belongs to an older one is dropped.
    pub generation: u64,
    /// The first search was started (set while drawing, hence a Cell).
    pub kicked: Cell<bool>,
    pub cache: HashMap<CacheKey, Page>,
    pub scroll: gpui_kit::UniformListScrollHandle,
    /// Icons on disk, by id; the ones asked for or failed; downloads running.
    pub icons: HashMap<String, PathBuf>,
    pub icons_tried: HashSet<String>,
    pub icons_running: usize,
    /// Icons the rows near the viewport still need, nearest first.
    pub icon_queue: Vec<(String, Vec<String>, PathBuf)>,
    pub installed: Vec<Installed>,
    pub busy: HashMap<String, Busy>,
    /// The extension the page shows.
    pub open: Option<Item>,
    /// README per id; missing while it loads.
    pub readme: HashMap<String, Result<String, String>>,
    /// Each README's pictures, fetched into the cache (URL as written → file).
    pub readme_images: HashMap<String, std::sync::Arc<HashMap<String, PathBuf>>>,
}

impl Market {
    pub fn installed(&self, id: &str) -> Option<&Installed> {
        self.installed.iter().find(|i| i.id == id)
    }

    pub fn remember(&mut self, key: CacheKey, page: Page) {
        if self.cache.len() >= CACHE_PAGES {
            self.cache.clear();
        }
        self.cache.insert(key, page);
    }

    /// Show a query's first page in place of the list.
    pub fn show_first(&mut self, q: Query, page: Page) {
        self.done = page.items.len() < PAGE_SIZE;
        self.items = page.items;
        self.total = page.total;
        self.pages = 1;
        self.shown = Some(q);
        self.loading = false;
        self.loading_more = false;
        self.error = None;
        self.more_error = None;
        self.icon_queue.clear();
    }

    /// Add the next page under the list (an extension already listed is skipped: the order can shift between two requests).
    pub fn append(&mut self, page: Page) {
        self.done = page.items.len() < PAGE_SIZE;
        let seen: HashSet<String> = self.items.iter().map(Item::id).collect();
        self.items
            .extend(page.items.into_iter().filter(|i| !seen.contains(&i.id())));
        if page.total.is_some() {
            self.total = page.total;
        }
        self.pages += 1;
        self.loading_more = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{"results":[{"extensions":[
        {"publisher":{"publisherName":"ms-python","displayName":"Microsoft","flags":"verified"},
         "extensionName":"python","displayName":"Python","shortDescription":"Python language support",
         "lastUpdated":"2026-10-30T18:02:11.5Z","categories":["Programming Languages","Debuggers"],
         "versions":[
           {"version":"2026.18.0","targetPlatform":"linux-x64","assetUri":"https://cdn/linux"},
           {"version":"2026.18.0","targetPlatform":"win32-x64","assetUri":"https://cdn/win"},
           {"version":"2026.18.0","assetUri":"https://cdn/any","files":[{"assetType":"Microsoft.VisualStudio.Services.Icons.Small","source":"https://cdn/any/small.png"}]}],
         "statistics":[{"statisticName":"install","value":48412345.0},{"statisticName":"averagerating","value":4.21},{"statisticName":"ratingcount","value":610}]},
        {"publisher":{"publisherName":"someone","displayName":"","flags":"none","isDomainVerified":false},
         "extensionName":"theme","versions":[{"version":"1.0.0","assetUri":"https://cdn/t"}]},
        {"publisher":{"publisherName":""},"extensionName":"broken","versions":[{"version":"1"}]}
    ],
    "resultMetadata":[{"metadataType":"ResultCount","metadataItems":[{"name":"TotalCount","count":125913}]},
                      {"metadataType":"Categories","metadataItems":[{"name":"Themes","count":19044}]}]}]}"#;

    fn criteria(body: &str) -> (Value, Vec<(i64, String)>) {
        let v: Value = serde_json::from_str(body).unwrap();
        let crit = v["filters"][0]["criteria"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["filterType"].as_i64().unwrap(),
                    c["value"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        (v, crit)
    }

    #[test]
    fn paged_query_body_with_sort_and_category() {
        let (v, crit) = criteria(&query_body(&Query::new("", None, None), 1));
        let f = &v["filters"][0];
        assert_eq!(
            (
                f["sortBy"].as_i64(),
                f["sortOrder"].as_i64(),
                f["pageNumber"].as_i64()
            ),
            (Some(4), Some(0), Some(1))
        );
        assert_eq!(f["pageSize"], PAGE_SIZE);
        assert_eq!(v["flags"], 918);
        assert_eq!(
            crit,
            [
                (8, "Microsoft.VisualStudio.Code".to_string()),
                (12, "4096".to_string())
            ]
        );

        let q = Query::new("  python   \"x\" ", None, Some("Themes"));
        assert_eq!((q.text.as_str(), q.sort), ("python \"x\"", Sort::Relevance));
        let (v, crit) = criteria(&query_body(&q, 3));
        assert_eq!(
            (
                v["filters"][0]["sortBy"].as_i64(),
                v["filters"][0]["pageNumber"].as_i64()
            ),
            (Some(0), Some(3))
        );
        assert!(
            crit.contains(&(10, "python \"x\"".into())) && crit.contains(&(5, "Themes".into()))
        );

        let by_name = Query::new("go", Some(Sort::Name), None);
        let (v, _) = criteria(&query_body(&by_name, 2));
        assert_eq!(
            (
                v["filters"][0]["sortBy"].as_i64(),
                v["filters"][0]["sortOrder"].as_i64()
            ),
            (Some(2), Some(1))
        );
        // Relevance means nothing without text: most installed instead.
        assert_eq!(
            Query::new(" ", Some(Sort::Relevance), None).sort,
            Sort::Installs
        );
        assert_eq!(Query::new("", Some(Sort::Rating), None).sort, Sort::Rating);
    }

    #[test]
    fn readme_pictures_are_found() {
        let md = "# X\n![logo](images/logo.png)\n[a link](https://x.dev)\n<p><img src=\"https://img.shields.io/badge/a-b-green\" width=\"80\"></p>\n![again](images/logo.png \"title\")\n";
        assert_eq!(readme_image_urls(md), vec!["images/logo.png".to_string(), "https://img.shields.io/badge/a-b-green".to_string()]);
        assert_eq!(sniff_image(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(sniff_image(b"<?xml version=\"1.0\"?><svg xmlns=\"\">"), Some("svg"));
        assert_eq!(sniff_image(b"hello"), None);
    }

    #[test]
    fn cache_keys() {
        let a = Query::new("Python  Debugger", None, None);
        let b = Query::new(" python debugger ", None, None);
        assert_eq!(
            cache_key(&a, 1),
            cache_key(&b, 1),
            "case and spaces don't matter"
        );
        assert_ne!(cache_key(&a, 1), cache_key(&a, 2));
        assert_ne!(
            cache_key(&a, 1),
            cache_key(&Query::new("python debugger", Some(Sort::Rating), None), 1)
        );
        assert_ne!(
            cache_key(&a, 1),
            cache_key(&Query::new("python debugger", None, Some("Debuggers")), 1)
        );
    }

    #[test]
    fn result_counts() {
        assert_eq!(parse(FIXTURE).unwrap().total, Some(125_913));
        assert_eq!(
            parse(r#"{"results":[{"extensions":[]}]}"#).unwrap(),
            Page::default()
        );
        assert_eq!(format_thousands(125_913), "125,913");
        assert_eq!(format_thousands(1_234_567), "1,234,567");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(0), "0");
    }

    #[test]
    fn appending_pages_skips_repeats() {
        let it = |n: &str| Item {
            publisher: "p".into(),
            name: n.into(),
            ..Default::default()
        };
        let mut m = Market::default();
        m.show_first(
            Query::new("", None, None),
            Page {
                items: (0..PAGE_SIZE).map(|i| it(&i.to_string())).collect(),
                total: Some(60),
            },
        );
        assert!(!m.done && m.pages == 1);
        m.append(Page {
            items: vec![it("49"), it("x")],
            total: None,
        });
        assert_eq!(
            (m.items.len(), m.pages, m.done, m.total),
            (PAGE_SIZE + 1, 2, true, Some(60))
        );
    }

    #[test]
    fn install_counts() {
        assert_eq!(format_count(950), "950");
        assert_eq!(format_count(5_600), "5.6K");
        assert_eq!(format_count(12_345), "12.3K");
        assert_eq!(format_count(48_412_345), "48.4M");
        assert_eq!(format_count(1_000_000), "1M");
        assert_eq!(format_count(999_999), "1M");
        assert_eq!(format_count(2_000_000_000), "2B");
    }

    #[test]
    fn parses_a_gallery_answer() {
        let list = parse(FIXTURE).unwrap().items;
        assert_eq!(list.len(), 2, "the entry without a publisher is skipped");
        let py = &list[0];
        assert_eq!(py.id(), "ms-python.python");
        assert_eq!(
            (
                py.display.as_str(),
                py.publisher_display.as_str(),
                py.verified
            ),
            ("Python", "Microsoft", true)
        );
        assert_eq!(py.installs, 48_412_345);
        assert_eq!(py.rating_count, 610);
        assert!((py.rating.unwrap() - 4.21).abs() < 1e-4);
        assert_eq!(py.updated, "2026-10-30");
        assert_eq!(py.categories, ["Programming Languages", "Debuggers"]);
        let expect = match target() {
            "win32-x64" => "https://cdn/win",
            "linux-x64" => "https://cdn/linux",
            _ => "https://cdn/any",
        };
        assert_eq!(py.asset_uri, expect);
        assert_eq!(py.asset(VSIX).unwrap(), format!("{expect}/{VSIX}"));
        let t = &list[1];
        assert_eq!(
            (
                t.display.as_str(),
                t.publisher_display.as_str(),
                t.verified,
                t.rating
            ),
            ("theme", "someone", false, None)
        );
        assert_eq!(
            t.asset(ICON_SMALL).unwrap(),
            format!("https://cdn/t/{ICON_SMALL}")
        );
        assert!(parse("{}").is_err() && parse("not json").is_err());
    }

    #[test]
    fn listed_files_win_over_the_asset_uri() {
        let any = Item {
            asset_uri: "https://cdn/any".into(),
            files: vec![(ICON_SMALL.into(), "https://cdn/any/small.png".into())],
            ..Default::default()
        };
        assert_eq!(any.asset(ICON_SMALL).unwrap(), "https://cdn/any/small.png");
        assert_eq!(Item::default().asset(VSIX), None);
    }

    #[test]
    fn safe_folder_names() {
        assert!(safe_name("ms-python.python-2026.18.0"));
        assert!(!safe_name(""));
        assert!(!safe_name(".."));
        assert!(!safe_name(".hidden"));
        assert!(!safe_name("a/b") && !safe_name("a\\b") && !safe_name("C:x") && !safe_name("a..b"));
        assert_eq!(
            folder_name("MS-Python", "Python", "1.2.3").as_deref(),
            Some("ms-python.python-1.2.3")
        );
        assert_eq!(folder_name("x", "..", "1"), None);
        assert_eq!(folder_name("x", "y", "1/../../2"), None);
        assert_eq!(
            entry_path("extension/package.json"),
            Some(PathBuf::from("package.json"))
        );
        assert_eq!(
            entry_path("extension/out/a.js"),
            Some(Path::new("out").join("a.js"))
        );
        assert_eq!(entry_path("extension.vsixmanifest"), None);
        assert_eq!(entry_path("extension/../evil"), None);
        assert_eq!(entry_path("extension/a/..\\..\\evil"), None);
        assert_eq!(entry_path("extension/C:/evil"), None);
        assert_eq!(entry_path("extension/dir/"), None);
    }

    #[test]
    fn contributes_in_words() {
        let pkg: Value = serde_json::from_str(r#"{"contributes":{"themes":[{},{},{}],"languages":[{},{}],"snippets":[{}],"commands":[{}]}}"#).unwrap();
        assert_eq!(
            contributes(&pkg),
            ["3 color themes", "2 languages", "snippets", "1 command"]
        );
        assert!(contributes(&json!({})).is_empty());
    }

    #[test]
    fn unpacks_only_the_extension_folder() {
        let mut z = super::super::zip::Zip::default();
        z.add("extension.vsixmanifest", b"<x/>");
        z.add("extension/package.json", br#"{"publisher":"p","name":"n","version":"1.0.0","displayName":"%dn%","contributes":{"themes":[{}]}}"#);
        z.add("extension/package.nls.json", br#"{"dn":"Nice Theme"}"#);
        z.add("extension/../escape.txt", b"no");
        let bytes = z.finish();
        let root = std::env::temp_dir().join(format!("fx-vsx-{}", std::process::id()));
        let folder = root.join("p.n-1.0.0");
        for e in super::super::zip::list(&bytes).unwrap() {
            if let Some(rel) = entry_path(&e.name) {
                let to = folder.join(rel);
                std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                std::fs::write(to, super::super::zip::read(&bytes, &e, 1 << 20).unwrap()).unwrap();
            }
        }
        assert!(!root.join("escape.txt").exists());
        let i = read_installed(&folder).unwrap();
        assert_eq!(
            (i.id.as_str(), i.display.as_str(), i.has_code),
            ("p.n", "Nice Theme", false)
        );
        assert_eq!(i.contributes, ["1 color theme"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
