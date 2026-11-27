use crate::i18n::tr;
use crate::theme::{self, c};
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, AnyElement, App, ClickEvent, ClipboardItem, FontWeight, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Window,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read as _;
use std::path::PathBuf;
use std::process::Stdio;

const MAX_CACHE_ENTRIES: usize = 64;
const MAX_IN_FLIGHT: usize = 3;
const MAX_HTML_BYTES: u64 = 65_536;
const MAX_THUMB_BYTES: u64 = 131_072;
const MAX_THUMBS_ON_DISK: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    YouTubeVideo,
    YouTubeShort,
    YouTubePlaylist,
    YouTubeChannel,
    GitPullRequest,
    GitIssue,
    GitCommit,
    GitFile,
    GitRepo,
    Package,
    Docs,
    MediaVideo,
    MediaAudio,
    MediaImage,
    Discussion,
    Design,
    Document,
    Email,
    Web,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmartLink {
    pub url: String,
    pub clean_url: String,
    pub host: String,
    pub kind: LinkKind,
    pub provider: String,
    pub label: String,
    pub subtitle: String,
    pub video_id: Option<String>,
    pub timestamp: Option<String>,
    pub github_api: Option<String>,
}

impl SmartLink {
    pub fn icon(&self) -> IconName {
        match self.kind {
            LinkKind::YouTubeVideo | LinkKind::YouTubeShort | LinkKind::YouTubePlaylist => {
                IconName::Play
            }
            LinkKind::YouTubeChannel | LinkKind::MediaVideo => IconName::FileVideoCamera,
            LinkKind::GitPullRequest => IconName::GitPullRequest,
            LinkKind::GitIssue => IconName::CircleAlert,
            LinkKind::GitCommit => IconName::GitMerge,
            LinkKind::GitFile => IconName::FileCode,
            LinkKind::GitRepo => IconName::GitBranch,
            LinkKind::Package => IconName::Package,
            LinkKind::Docs => IconName::ScrollText,
            LinkKind::MediaImage | LinkKind::Design => IconName::Image,
            LinkKind::MediaAudio => IconName::Play,
            LinkKind::Discussion => IconName::MessagesSquare,
            LinkKind::Document => IconName::FileText,
            LinkKind::Email => IconName::AtSign,
            LinkKind::Web => IconName::Globe,
        }
    }

    pub fn icon_color(&self) -> theme::Tok {
        match self.kind {
            LinkKind::YouTubeVideo
            | LinkKind::YouTubeShort
            | LinkKind::YouTubePlaylist
            | LinkKind::YouTubeChannel
            | LinkKind::MediaVideo => theme::DANGER,
            LinkKind::GitPullRequest | LinkKind::GitIssue => theme::GREEN,
            LinkKind::GitCommit | LinkKind::Package => theme::WARN,
            LinkKind::GitRepo => theme::FG_MUTED,
            LinkKind::MediaImage
            | LinkKind::MediaAudio
            | LinkKind::Design
            | LinkKind::Discussion => theme::ACCENT_TEXT,
            LinkKind::GitFile
            | LinkKind::Docs
            | LinkKind::Document
            | LinkKind::Email
            | LinkKind::Web => theme::BLUE,
        }
    }

    pub fn is_youtube(&self) -> bool {
        matches!(
            self.kind,
            LinkKind::YouTubeVideo
                | LinkKind::YouTubeShort
                | LinkKind::YouTubePlaylist
                | LinkKind::YouTubeChannel
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkPreview {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub site_name: Option<String>,
    pub badge: Option<String>,
    pub badge_tok: Option<theme::Tok>,
    pub thumb_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewState {
    Loading,
    Ready(LinkPreview),
    Failed,
}

#[derive(Default)]
pub struct LinkPreviewStore {
    entries: HashMap<String, PreviewState>,
    order: VecDeque<String>,
    pub in_flight: usize,
    pub dismissed: HashSet<String>,
}

impl LinkPreviewStore {
    pub fn get(&self, url: &str) -> Option<&PreviewState> {
        self.entries.get(url)
    }

    pub fn should_fetch(&self, url: &str) -> bool {
        !self.entries.contains_key(url) && self.in_flight < MAX_IN_FLIGHT && is_safe_public_url(url)
    }

    pub fn dismiss(&mut self, url: &str) {
        self.dismissed.insert(url.to_string());
    }

    pub fn mark_loading(&mut self, url: &str) {
        if self.entries.contains_key(url) {
            return;
        }
        self.evict_if_full();
        self.entries.insert(url.to_string(), PreviewState::Loading);
        self.order.push_back(url.to_string());
        self.in_flight += 1;
    }

    pub fn finish(&mut self, url: &str, preview: Option<LinkPreview>) {
        self.in_flight = self.in_flight.saturating_sub(1);
        if !self.entries.contains_key(url) {
            self.evict_if_full();
            self.order.push_back(url.to_string());
        }
        let state = match preview {
            Some(p) => PreviewState::Ready(p),
            None => PreviewState::Failed,
        };
        self.entries.insert(url.to_string(), state);
    }

    fn evict_if_full(&mut self) {
        while self.entries.len() >= MAX_CACHE_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextPart {
    Text(String),
    Link {
        link: Box<SmartLink>,
        custom_label: Option<String>,
    },
}

pub fn parse_link(raw_url: &str) -> SmartLink {
    let raw = raw_url.trim();
    if let Some(addr) = raw.strip_prefix("mailto:") {
        let addr = addr.split('?').next().unwrap_or(addr).trim();
        return SmartLink {
            url: raw.to_string(),
            clean_url: format!("mailto:{addr}"),
            host: addr.to_string(),
            kind: LinkKind::Email,
            provider: "Email".into(),
            label: addr.to_string(),
            subtitle: "mailto".into(),
            video_id: None,
            timestamp: None,
            github_api: None,
        };
    }

    let (scheme, after_scheme) = match raw.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => ("https".to_string(), raw),
    };
    let (authority_and_path, fragment) = match after_scheme.split_once('#') {
        Some((before, frag)) => (before, Some(frag)),
        None => (after_scheme, None),
    };
    let (authority_path, query) = match authority_and_path.split_once('?') {
        Some((before, q)) => (before, Some(q)),
        None => (authority_and_path, None),
    };
    let (authority, path_raw) = match authority_path.split_once('/') {
        Some((a, p)) => (a, p),
        None => (authority_path, ""),
    };
    let host_only = authority
        .rsplit('@')
        .next()
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or(authority)
        .to_ascii_lowercase();
    let host = strip_common_subdomain(&host_only).to_string();
    let segs: Vec<String> = path_raw
        .split('/')
        .filter(|s| !s.is_empty())
        .map(percent_decode)
        .collect();
    let params = parse_query(query);
    let clean_url = build_clean_url(&scheme, authority, path_raw, query, fragment);

    if is_youtube_host(&host) {
        return parse_youtube(raw, &clean_url, &host, &segs, &params);
    }
    if host == "github.com" {
        return parse_github(raw, &clean_url, &host, &segs, fragment);
    }
    if matches!(host.as_str(), "gitlab.com" | "codeberg.org") {
        return parse_gitlab_like(raw, &clean_url, &host, &segs);
    }
    if let Some(pkg) = parse_package_registry(raw, &clean_url, &host, &segs) {
        return pkg;
    }
    if let Some(docs) = parse_docs_site(raw, &clean_url, &host, &segs, fragment) {
        return docs;
    }
    if let Some(disc) = parse_discussion_or_tool(raw, &clean_url, &host, &segs) {
        return disc;
    }
    if let Some(media) = parse_media_extension(raw, &clean_url, &host, &segs) {
        return media;
    }

    let provider = pretty_provider(&host);
    let (label, subtitle) = build_generic_labels(&host, &segs, &params, fragment);
    SmartLink {
        url: raw.to_string(),
        clean_url,
        host,
        kind: LinkKind::Web,
        provider,
        label,
        subtitle,
        video_id: None,
        timestamp: None,
        github_api: None,
    }
}

pub fn inline_display_label(raw_url: &str) -> String {
    let parsed = parse_link(raw_url);
    parsed.label
}

pub fn split_text_links(text: &str) -> Vec<TextPart> {
    if !text.contains("://") && !text.contains("mailto:") {
        return vec![TextPart::Text(text.to_string())];
    }
    let mut parts = Vec::new();
    let mut text_start = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some((custom, url, end_ix)) = parse_markdown_link(&text[i..]) {
                if i > text_start {
                    parts.push(TextPart::Text(text[text_start..i].to_string()));
                }
                let link = Box::new(parse_link(url));
                let custom_label = (!custom.trim().is_empty() && custom.trim() != url)
                    .then(|| custom.trim().to_string());
                parts.push(TextPart::Link { link, custom_label });
                i += end_ix;
                text_start = i;
                continue;
            }
        }

        let rest = &text[i..];
        let is_url = (rest.starts_with("https://")
            || rest.starts_with("http://")
            || rest.starts_with("mailto:"))
            && (i == 0 || !is_word_byte(bytes[i - 1]));

        if is_url {
            let url = scan_url_token(rest);
            if !url.is_empty() && has_valid_authority(url) {
                if i > text_start {
                    parts.push(TextPart::Text(text[text_start..i].to_string()));
                }
                parts.push(TextPart::Link {
                    link: Box::new(parse_link(url)),
                    custom_label: None,
                });
                i += url.len();
                text_start = i;
                continue;
            }
        }

        i += rest.chars().next().map_or(1, char::len_utf8);
    }

    if text_start < text.len() {
        parts.push(TextPart::Text(text[text_start..].to_string()));
    }
    if parts.is_empty() {
        parts.push(TextPart::Text(text.to_string()));
    }
    parts
}

pub fn extract_unique_urls(text: &str, skip_code_blocks: bool) -> Vec<SmartLink> {
    if !text.contains("://") {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut in_fence = false;
    let mut in_inline_code = false;
    let mut line_start = true;

    while i < bytes.len() {
        let rest = &text[i..];
        if skip_code_blocks {
            if line_start {
                let trimmed = rest.trim_start_matches(' ');
                if trimmed.starts_with("```") {
                    in_fence = !in_fence;
                    i += rest.find('\n').map_or(rest.len(), |n| n + 1);
                    line_start = true;
                    continue;
                }
            }
            if bytes[i] == b'\n' {
                in_inline_code = false;
                line_start = true;
                i += 1;
                continue;
            }
            line_start = false;
            if in_fence {
                i += rest.chars().next().map_or(1, char::len_utf8);
                continue;
            }
            if bytes[i] == b'`' {
                in_inline_code = !in_inline_code;
                i += 1;
                continue;
            }
            if in_inline_code {
                i += rest.chars().next().map_or(1, char::len_utf8);
                continue;
            }
        }

        if (rest.starts_with("https://") || rest.starts_with("http://"))
            && (i == 0 || !is_word_byte(bytes[i - 1]))
        {
            let url = scan_url_token(rest);
            if !url.is_empty() && has_valid_authority(url) && seen.insert(url.to_string()) {
                out.push(parse_link(url));
            }
            i += url.len().max(1);
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }

    out
}

pub fn is_safe_public_url(raw: &str) -> bool {
    let Some((scheme, rest)) = raw.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    if authority.starts_with('[') {
        let inner = authority
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("");
        let low = inner.to_ascii_lowercase();
        if low == "::1" || low.starts_with("fc") || low.starts_with("fd") || low.starts_with("fe80")
        {
            return false;
        }
        return true;
    }
    let host = authority
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".localhost")
    {
        return false;
    }
    if let Some(ip) = parse_ipv4(&host) {
        let [a, b, ..] = ip;
        let private = a == 0
            || a == 10
            || a == 127
            || (a == 169 && b == 254)
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
            || (a == 100 && (64..=127).contains(&b));
        if private {
            return false;
        }
    }
    host.contains('.')
}

pub fn fetch_preview(link: &SmartLink) -> Option<LinkPreview> {
    if !is_safe_public_url(&link.url) {
        return None;
    }
    if link.is_youtube() {
        if let Some(meta) = fetch_youtube_oembed(link) {
            return Some(meta);
        }
    }
    if let Some(api_url) = &link.github_api {
        if let Some(meta) = fetch_github_api(api_url, link) {
            return Some(meta);
        }
    }
    let html_bytes = curl_get_bounded(&link.url, MAX_HTML_BYTES, &[])?;
    let html = String::from_utf8_lossy(&html_bytes);
    let mut meta = parse_html_meta(&html);
    if meta.title.is_none() && meta.description.is_none() {
        return None;
    }
    if meta.site_name.is_none() {
        meta.site_name = Some(link.provider.clone());
    }
    Some(meta)
}

pub fn parse_html_meta(html: &str) -> LinkPreview {
    let mut preview = LinkPreview::default();
    let mut og_title: Option<String> = None;
    let mut tw_title: Option<String> = None;
    let mut doc_title: Option<String> = None;
    let mut og_desc: Option<String> = None;
    let mut meta_desc: Option<String> = None;

    let head_end = find_ascii_ci(html, "</head>")
        .unwrap_or(html.len())
        .min(html.len());
    let head = &html[..head_end];

    let mut pos = 0usize;
    while let Some(rel_lt) = head[pos..].find('<') {
        let tag_start = pos + rel_lt;
        let Some(rel_gt) = head[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + rel_gt + 1;
        let tag = &head[tag_start + 1..tag_end - 1];
        let trimmed = tag.trim_start();
        if starts_with_ascii_ci(trimmed, "meta")
            && trimmed
                .as_bytes()
                .get(4)
                .is_some_and(|b| b.is_ascii_whitespace())
        {
            let key = attr_value(trimmed, "property")
                .or_else(|| attr_value(trimmed, "name"))
                .map(|s| s.to_ascii_lowercase());
            let content = attr_value(trimmed, "content").map(clean_meta_text);
            if let (Some(k), Some(v)) = (key, content) {
                if !v.is_empty() {
                    let slot = match k.as_str() {
                        "og:title" => Some(&mut og_title),
                        "twitter:title" => Some(&mut tw_title),
                        "og:description" | "twitter:description" => Some(&mut og_desc),
                        "description" => Some(&mut meta_desc),
                        "og:site_name" => Some(&mut preview.site_name),
                        "author" => Some(&mut preview.author),
                        _ => None,
                    };
                    if let Some(slot) = slot {
                        slot.get_or_insert(v);
                    }
                }
            }
        } else if starts_with_ascii_ci(trimmed, "title")
            && trimmed
                .as_bytes()
                .get(5)
                .is_none_or(|b| b.is_ascii_whitespace())
            && doc_title.is_none()
        {
            if let Some(close_rel) = find_ascii_ci(&head[tag_end..], "</title") {
                let raw_t = clean_meta_text(&head[tag_end..tag_end + close_rel]);
                if !raw_t.is_empty() {
                    doc_title = Some(raw_t);
                }
            }
        }
        pos = tag_end;
    }

    preview.title = og_title.or(tw_title).or(doc_title);
    preview.description = og_desc.or(meta_desc);
    preview
}

pub fn render_inline_chip(
    id: SharedString,
    link: &SmartLink,
    custom_label: Option<&str>,
    state: Option<&PreviewState>,
) -> AnyElement {
    let url = link.url.clone();
    let display = resolved_inline_label(link, custom_label, state);
    let tip_link = link.clone();
    let tip_state = state.cloned();

    h_flex()
        .id(id)
        .flex_none()
        .max_w(theme::u(240.))
        .px_1p5()
        .py(px(1.))
        .gap_1()
        .items_center()
        .rounded(px(4.))
        .bg(c(theme::BG_SIDE))
        .border_1()
        .border_color(c(theme::BORDER))
        .cursor_pointer()
        .hover(|s| {
            s.bg(c(theme::BG_HOVER))
                .border_color(c(theme::BORDER_STRONG))
        })
        .active(|s| s.opacity(0.84))
        .child(
            Icon::new(link.icon())
                .flex_none()
                .size(theme::u(12.))
                .text_color(c(link.icon_color())),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(theme::BLUE))
                .child(display),
        )
        .tooltip(move |w, cx| {
            let l = tip_link.clone();
            let st = tip_state.clone();
            gpui_kit::component::tooltip::Tooltip::element(move |_, _| {
                render_tooltip_body(&l, st.as_ref())
            })
            .build(w, cx)
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            cx.open_url(&url);
        })
        .into_any_element()
}

pub fn render_composer_chip(
    id: SharedString,
    link: &SmartLink,
    state: Option<&PreviewState>,
    on_dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let url = link.url.clone();
    let display = resolved_inline_label(link, None, state);
    let tip_link = link.clone();
    let tip_state = state.cloned();

    h_flex()
        .id(id.clone())
        .group("ag-chip")
        .flex_none()
        .max_w(theme::u(200.))
        .h(theme::u(22.))
        .pr_0p5()
        .gap_0p5()
        .rounded(px(3.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_SIDE))
        .text_xs()
        .text_color(c(theme::FG))
        .hover(|s| {
            s.bg(c(theme::BG_HOVER))
                .border_color(c(theme::BORDER_STRONG))
                .text_color(c(theme::FG_STRONG))
        })
        .active(|s| s.opacity(0.84))
        .child(
            h_flex()
                .id(SharedString::from(format!("{id}-open")))
                .flex_1()
                .min_w_0()
                .h_full()
                .items_center()
                .pl_1p5()
                .gap_1()
                .cursor_pointer()
                .tooltip(move |w, cx| {
                    let l = tip_link.clone();
                    let st = tip_state.clone();
                    gpui_kit::component::tooltip::Tooltip::element(move |_, _| {
                        render_tooltip_body(&l, st.as_ref())
                    })
                    .build(w, cx)
                })
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(&url);
                })
                .child(
                    Icon::new(link.icon())
                        .flex_none()
                        .size(theme::u(12.))
                        .text_color(c(link.icon_color())),
                )
                .child(div().min_w_0().truncate().child(display)),
        )
        .child(
            div()
                .id(SharedString::from(format!("{id}-x")))
                .flex_none()
                .size(theme::u(16.))
                .ml_px()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.))
                .opacity(0.)
                .group_hover("ag-chip", |s| s.opacity(1.))
                .cursor_pointer()
                .text_color(c(theme::FG_DIM))
                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .tooltip(|w, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tr("Dismiss preview")).build(w, cx)
                })
                .on_click(move |ev, w, cx| {
                    cx.stop_propagation();
                    on_dismiss(ev, w, cx);
                })
                .child(Icon::new(IconName::Close).size(theme::u(11.))),
        )
        .into_any_element()
}

pub type DismissCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub fn render_preview_card(
    id: SharedString,
    link: &SmartLink,
    state: Option<&PreviewState>,
    on_dismiss: Option<DismissCallback>,
) -> AnyElement {
    let url_open = link.url.clone();
    let url_copy = link.url.clone();
    let (loading, meta) = match state {
        Some(PreviewState::Loading) => (true, None),
        Some(PreviewState::Ready(m)) => (false, Some(m)),
        _ => (false, None),
    };

    let title = meta
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| link.label.clone());
    let site = meta
        .and_then(|m| m.site_name.clone())
        .unwrap_or_else(|| link.provider.clone());
    let author = meta.and_then(|m| m.author.clone());
    let badge = meta.and_then(|m| m.badge.clone());
    let badge_tok = meta.and_then(|m| m.badge_tok);
    let desc = meta.and_then(|m| m.description.clone());
    let thumb = meta
        .and_then(|m| m.thumb_path.as_ref())
        .filter(|p| p.is_file())
        .cloned();

    let lead: AnyElement = if link.is_youtube() {
        div()
            .relative()
            .flex_none()
            .w(theme::u(96.))
            .h(theme::u(54.))
            .rounded(px(4.))
            .overflow_hidden()
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_SIDE))
            .when_some(thumb, |d, p| {
                d.child(img(p).size_full().object_fit(ObjectFit::Cover))
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(theme::u(24.))
                            .rounded_full()
                            .bg(c(theme::BG_TITLE).opacity(0.78))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::Play)
                                    .size(theme::u(12.))
                                    .text_color(c(theme::FG_STRONG)),
                            ),
                    ),
            )
            .when_some(link.timestamp.clone(), |d, ts| {
                d.child(
                    div()
                        .absolute()
                        .right(px(4.))
                        .bottom(px(3.))
                        .px_1()
                        .rounded(px(3.))
                        .bg(c(theme::BG_TITLE).opacity(0.88))
                        .text_size(theme::u(9.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(theme::FG_STRONG))
                        .child(ts),
                )
            })
            .into_any_element()
    } else {
        div()
            .flex_none()
            .size(theme::u(32.))
            .rounded(px(5.))
            .bg(c(theme::BG_SIDE))
            .border_1()
            .border_color(c(theme::BORDER))
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(link.icon())
                    .size(theme::u(15.))
                    .text_color(c(link.icon_color())),
            )
            .into_any_element()
    };

    let mut top_meta = h_flex()
        .min_w_0()
        .gap_1p5()
        .items_center()
        .text_xs()
        .text_color(c(theme::FG_MUTED))
        .child(
            Icon::new(link.icon())
                .flex_none()
                .size(theme::u(11.))
                .text_color(c(link.icon_color())),
        )
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .child(site),
        );

    if let Some(b) = badge {
        let col = badge_tok.unwrap_or(theme::FG_MUTED);
        top_meta = top_meta.child(
            div()
                .flex_none()
                .px_1()
                .rounded(px(3.))
                .border_1()
                .border_color(c(col).opacity(0.45))
                .text_size(theme::u(10.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(col))
                .child(b),
        );
    }
    if let Some(auth) = author {
        top_meta = top_meta.child(
            div()
                .min_w_0()
                .truncate()
                .text_color(c(theme::FG_DIM))
                .child(auth),
        );
    } else if link.host != link.provider.to_ascii_lowercase() {
        top_meta = top_meta.child(
            div()
                .min_w_0()
                .truncate()
                .text_color(c(theme::FG_DIM))
                .child(link.host.clone()),
        );
    }

    let detail_line: Option<String> = if let Some(d) = desc {
        Some(d)
    } else if loading {
        Some(tr("Fetching preview…").into())
    } else if !link.subtitle.is_empty() && link.subtitle != link.host {
        Some(link.subtitle.clone())
    } else {
        Some(link.clean_url.clone())
    };

    h_flex()
        .id(id.clone())
        .w_full()
        .min_w_0()
        .p_2()
        .gap_2p5()
        .items_center()
        .rounded(px(6.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .cursor_pointer()
        .hover(|s| {
            s.bg(c(theme::BG_HOVER))
                .border_color(c(theme::BORDER_STRONG))
        })
        .active(|s| s.opacity(0.9))
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            cx.open_url(&url_open);
        })
        .child(lead)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(top_meta)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_STRONG))
                        .child(title),
                )
                .when_some(detail_line, |v, d| {
                    v.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(d),
                    )
                }),
        )
        .child(
            h_flex()
                .flex_none()
                .gap_0p5()
                .items_center()
                .child(
                    div()
                        .id(SharedString::from(format!("{id}-copy")))
                        .size(theme::u(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_color(c(theme::FG_MUTED))
                        .hover(|s| s.bg(c(theme::BG_SIDE)).text_color(c(theme::FG_STRONG)))
                        .tooltip(|w, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tr("Copy link")).build(w, cx)
                        })
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            cx.write_to_clipboard(ClipboardItem::new_string(url_copy.clone()));
                        })
                        .child(Icon::new(IconName::Copy).size(theme::u(12.))),
                )
                .when_some(on_dismiss, |h, f| {
                    h.child(
                        div()
                            .id(SharedString::from(format!("{id}-dismiss")))
                            .size(theme::u(22.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_color(c(theme::FG_MUTED))
                            .hover(|s| s.bg(c(theme::BG_SIDE)).text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(tr("Dismiss preview"))
                                    .build(w, cx)
                            })
                            .on_click(move |ev, w, cx| {
                                cx.stop_propagation();
                                f(ev, w, cx);
                            })
                            .child(Icon::new(IconName::Close).size(theme::u(12.))),
                    )
                }),
        )
        .into_any_element()
}

pub fn render_rich_text_with_links(
    id_prefix: &str,
    text: &str,
    store: &LinkPreviewStore,
) -> AnyElement {
    let mut lines_el = v_flex().w_full().min_w_0().gap_1();
    let mut link_seq = 0usize;

    for (line_ix, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            lines_el = lines_el.child(div().h(theme::u(6.)));
            continue;
        }
        let parts = split_text_links(line);
        let has_links = parts.iter().any(|p| matches!(p, TextPart::Link { .. }));
        if !has_links {
            lines_el = lines_el.child(div().w_full().min_w_0().child(line.to_string()));
            continue;
        }
        let mut row = h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .items_center()
            .gap_y(px(2.));
        let mut word_count = 0usize;
        for part in parts {
            match part {
                TextPart::Text(seg) => {
                    for (byte_off, token) in split_keep_spaces_offsets(&seg) {
                        word_count += 1;
                        if word_count > 160 {
                            row = row.child(div().child(seg[byte_off..].to_string()));
                            break;
                        }
                        if token == " " {
                            row = row.child(div().w(px(4.)));
                        } else {
                            row = row.child(div().child(token.to_string()));
                        }
                    }
                }
                TextPart::Link { link, custom_label } => {
                    let chip_id = SharedString::from(format!("{id_prefix}-{line_ix}-{link_seq}"));
                    link_seq += 1;
                    let st = store.get(&link.url);
                    row = row.child(render_inline_chip(
                        chip_id,
                        &link,
                        custom_label.as_deref(),
                        st,
                    ));
                }
            }
        }
        lines_el = lines_el.child(row);
    }

    lines_el.into_any_element()
}

fn split_keep_spaces_offsets(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_space = false;
    for (i, ch) in s.char_indices() {
        let sp = ch.is_whitespace();
        if i > 0 && sp != in_space {
            let chunk = &s[start..i];
            out.push((start, if in_space { " " } else { chunk }));
            start = i;
        }
        in_space = sp;
    }
    if start < s.len() {
        let chunk = &s[start..];
        out.push((start, if in_space { " " } else { chunk }));
    }
    out
}

fn resolved_inline_label(
    link: &SmartLink,
    custom_label: Option<&str>,
    state: Option<&PreviewState>,
) -> String {
    if let Some(custom) = custom_label {
        return truncate_chars(custom, 44);
    }
    if let Some(PreviewState::Ready(meta)) = state {
        if let Some(title) = meta.title.as_deref() {
            if !title.trim().is_empty() {
                if link.is_youtube() {
                    return format!("{} · {}", link.provider, truncate_chars(title.trim(), 34));
                }
                if matches!(
                    link.kind,
                    LinkKind::Web | LinkKind::Docs | LinkKind::Discussion
                ) {
                    return truncate_chars(title.trim(), 40);
                }
            }
        }
    }
    link.label.clone()
}

fn render_tooltip_body(link: &SmartLink, state: Option<&PreviewState>) -> AnyElement {
    let meta = match state {
        Some(PreviewState::Ready(m)) => Some(m),
        _ => None,
    };
    let loading = matches!(state, Some(PreviewState::Loading));
    let title = meta
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| link.label.clone());
    let thumb = meta
        .and_then(|m| m.thumb_path.as_ref())
        .filter(|p| p.is_file())
        .cloned();
    let desc = meta
        .and_then(|m| m.description.clone())
        .or_else(|| loading.then(|| tr("Fetching preview…").into()));

    v_flex()
        .max_w(theme::u(260.))
        .gap_1()
        .when_some(thumb, |v, p| {
            v.child(
                img(p)
                    .w(theme::u(220.))
                    .h(theme::u(124.))
                    .rounded(px(4.))
                    .object_fit(ObjectFit::Cover),
            )
        })
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(
                    Icon::new(link.icon())
                        .size(theme::u(12.))
                        .text_color(c(link.icon_color())),
                )
                .child(link.provider.clone())
                .when_some(meta.and_then(|m| m.author.clone()), |h, auth| {
                    h.child(format!("· {auth}"))
                }),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_STRONG))
                .child(title),
        )
        .when_some(desc, |v, d| {
            v.child(
                div()
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .child(truncate_chars(&d, 120)),
            )
        })
        .child(
            div()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(truncate_chars(&link.clean_url, 64)),
        )
        .into_any_element()
}

fn strip_common_subdomain(host: &str) -> &str {
    for prefix in ["www.", "m.", "mobile."] {
        if let Some(rest) = host.strip_prefix(prefix) {
            if rest.contains('.') {
                return rest;
            }
        }
    }
    host
}

fn is_youtube_host(host: &str) -> bool {
    matches!(
        host,
        "youtube.com" | "youtu.be" | "music.youtube.com" | "youtube-nocookie.com"
    )
}

fn parse_youtube(
    raw: &str,
    clean_url: &str,
    host: &str,
    segs: &[String],
    params: &[(String, String)],
) -> SmartLink {
    let ts = query_val(params, "t").and_then(format_yt_timestamp);
    let list_id = query_val(params, "list");
    let provider = if host == "music.youtube.com" {
        "YouTube Music"
    } else {
        "YouTube"
    };

    let (kind, video_id, label) = if host == "youtu.be" {
        let vid = segs.first().filter(|s| is_yt_id(s)).cloned();
        let lbl = match &vid {
            Some(id) => format!("{provider} · {id}"),
            None => provider.to_string(),
        };
        (LinkKind::YouTubeVideo, vid, lbl)
    } else if segs.first().map(String::as_str) == Some("shorts") {
        let vid = segs.get(1).filter(|s| is_yt_id(s)).cloned();
        let lbl = match &vid {
            Some(id) => format!("YouTube Shorts · {id}"),
            None => "YouTube Shorts".to_string(),
        };
        (LinkKind::YouTubeShort, vid, lbl)
    } else if matches!(segs.first().map(String::as_str), Some("live" | "embed")) {
        let vid = segs.get(1).filter(|s| is_yt_id(s)).cloned();
        let lbl = match &vid {
            Some(id) => format!("{provider} · {id}"),
            None => provider.to_string(),
        };
        (LinkKind::YouTubeVideo, vid, lbl)
    } else if segs.first().map(String::as_str) == Some("playlist")
        || list_id.is_some() && query_val(params, "v").is_none()
    {
        (
            LinkKind::YouTubePlaylist,
            None,
            "YouTube Playlist".to_string(),
        )
    } else if let Some(first) = segs.first().filter(|s| s.starts_with('@')) {
        (LinkKind::YouTubeChannel, None, format!("YouTube · {first}"))
    } else if let Some(v) = query_val(params, "v").filter(|s| is_yt_id(s)) {
        (
            LinkKind::YouTubeVideo,
            Some(v.to_string()),
            format!("{provider} · {v}"),
        )
    } else {
        (LinkKind::YouTubeVideo, None, provider.to_string())
    };

    let subtitle = match (&kind, &ts) {
        (LinkKind::YouTubeShort, _) => "YouTube Shorts".into(),
        (LinkKind::YouTubePlaylist, _) => "YouTube Playlist".into(),
        (LinkKind::YouTubeChannel, _) => "YouTube Channel".into(),
        (_, Some(t)) => format!("youtube.com · {t}"),
        _ => "youtube.com".into(),
    };

    SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind,
        provider: provider.to_string(),
        label,
        subtitle,
        video_id,
        timestamp: ts,
        github_api: None,
    }
}

fn parse_github(
    raw: &str,
    clean_url: &str,
    host: &str,
    segs: &[String],
    fragment: Option<&str>,
) -> SmartLink {
    let (kind, label, subtitle, github_api) = match segs {
        [owner, repo, kind, num, ..]
            if (kind == "pull" || kind == "issues") && num.chars().all(|c| c.is_ascii_digit()) =>
        {
            let is_pr = kind == "pull";
            (
                if is_pr {
                    LinkKind::GitPullRequest
                } else {
                    LinkKind::GitIssue
                },
                format!("{repo}#{num}"),
                format!(
                    "{owner}/{repo} · {}",
                    if is_pr { "Pull Request" } else { "Issue" }
                ),
                Some(format!(
                    "https://api.github.com/repos/{owner}/{repo}/issues/{num}"
                )),
            )
        }
        [owner, repo, kind, sha, ..] if kind == "commit" && sha.len() >= 7 => {
            let short = &sha[..7.min(sha.len())];
            (
                LinkKind::GitCommit,
                format!("{repo}@{short}"),
                format!("{owner}/{repo} · Commit"),
                None,
            )
        }
        [owner, repo, kind, _branch, rest @ ..]
            if (kind == "blob" || kind == "tree") && !rest.is_empty() =>
        {
            let tail = rest.last().map(String::as_str).unwrap_or("");
            let line = fragment
                .and_then(|f| f.strip_prefix('L'))
                .filter(|l| !l.is_empty());
            let lbl = match line {
                Some(l) => format!("{repo}/{tail}:{l}"),
                None => format!("{repo}/{tail}"),
            };
            (LinkKind::GitFile, lbl, format!("{owner}/{repo}"), None)
        }
        [owner, repo, kind, _, tag, ..] if kind == "releases" => (
            LinkKind::Package,
            format!("{repo} {tag}"),
            format!("{owner}/{repo} · Release"),
            None,
        ),
        [owner, repo] => (
            LinkKind::GitRepo,
            format!("{owner}/{repo}"),
            "GitHub Repository".into(),
            Some(format!("https://api.github.com/repos/{owner}/{repo}")),
        ),
        [owner] => (
            LinkKind::GitRepo,
            format!("@{owner}"),
            "GitHub".into(),
            None,
        ),
        _ => (LinkKind::Web, "GitHub".into(), "github.com".into(), None),
    };

    SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind,
        provider: "GitHub".into(),
        label,
        subtitle,
        video_id: None,
        timestamp: None,
        github_api,
    }
}

fn parse_gitlab_like(raw: &str, clean_url: &str, host: &str, segs: &[String]) -> SmartLink {
    let provider = if host == "gitlab.com" {
        "GitLab"
    } else {
        "Codeberg"
    };
    let dash_pos = segs.iter().position(|s| s == "-");
    let repo = match dash_pos {
        Some(ix) if ix > 0 => segs[ix - 1].as_str(),
        _ => segs
            .get(1)
            .or(segs.first())
            .map(String::as_str)
            .unwrap_or(provider),
    };
    let (kind, label, subtitle) = if let Some(ix) = dash_pos {
        match (
            segs.get(ix + 1).map(String::as_str),
            segs.get(ix + 2).map(String::as_str),
        ) {
            (Some("merge_requests"), Some(num)) => (
                LinkKind::GitPullRequest,
                format!("{repo}!{num}"),
                format!("{provider} Merge Request"),
            ),
            (Some("issues"), Some(num)) => (
                LinkKind::GitIssue,
                format!("{repo}#{num}"),
                format!("{provider} Issue"),
            ),
            _ => (LinkKind::GitRepo, repo.to_string(), host.to_string()),
        }
    } else if segs.len() >= 2 {
        (
            LinkKind::GitRepo,
            format!("{}/{}", segs[0], segs[1]),
            host.to_string(),
        )
    } else {
        (LinkKind::GitRepo, provider.to_string(), host.to_string())
    };
    SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind,
        provider: provider.into(),
        label,
        subtitle,
        video_id: None,
        timestamp: None,
        github_api: None,
    }
}

fn parse_package_registry(
    raw: &str,
    clean_url: &str,
    host: &str,
    segs: &[String],
) -> Option<SmartLink> {
    let (provider, pkg) = match (host, segs.first().map(String::as_str), segs.get(1)) {
        ("crates.io", Some("crates"), Some(p)) => ("crates.io", p.as_str()),
        ("npmjs.com" | "www.npmjs.com", Some("package"), Some(p)) => ("npm", p.as_str()),
        ("pypi.org", Some("project"), Some(p)) => ("PyPI", p.as_str()),
        ("modrinth.com", Some("mod" | "plugin" | "datapack" | "resourcepack"), Some(p)) => {
            ("Modrinth", p.as_str())
        }
        ("hub.docker.com", Some("r"), Some(p)) => ("Docker Hub", p.as_str()),
        _ => return None,
    };
    Some(SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind: LinkKind::Package,
        provider: provider.into(),
        label: format!("{provider} · {pkg}"),
        subtitle: host.to_string(),
        video_id: None,
        timestamp: None,
        github_api: None,
    })
}

fn parse_docs_site(
    raw: &str,
    clean_url: &str,
    host: &str,
    segs: &[String],
    fragment: Option<&str>,
) -> Option<SmartLink> {
    let (provider, label) = if host == "docs.rs" {
        let krate = segs.first().map(String::as_str).unwrap_or("docs.rs");
        let item = segs.last().filter(|s| *s != krate).map(String::as_str);
        let lbl = match item {
            Some(i) => format!("docs.rs · {krate}::{i}"),
            None => format!("docs.rs · {krate}"),
        };
        ("docs.rs", lbl)
    } else if host.ends_with(".wikipedia.org") && segs.first().map(String::as_str) == Some("wiki") {
        let title = segs
            .get(1)
            .map(|s| s.replace('_', " "))
            .unwrap_or_else(|| "Wikipedia".into());
        (
            "Wikipedia",
            format!("Wikipedia · {}", truncate_chars(&title, 30)),
        )
    } else if host == "minecraft.wiki" && segs.first().map(String::as_str) == Some("w") {
        let title = segs
            .get(1)
            .map(|s| s.replace('_', " "))
            .unwrap_or_else(|| "Minecraft Wiki".into());
        (
            "Minecraft Wiki",
            format!("MC Wiki · {}", truncate_chars(&title, 30)),
        )
    } else if host == "developer.mozilla.org" {
        let tail = segs
            .last()
            .map(|s| s.replace('_', " "))
            .unwrap_or_else(|| "MDN".into());
        ("MDN", format!("MDN · {}", truncate_chars(&tail, 30)))
    } else if host == "learn.microsoft.com" {
        let tail = segs
            .last()
            .map(|s| humanize_slug(s))
            .unwrap_or_else(|| "Microsoft Learn".into());
        (
            "Microsoft Learn",
            format!("MS Learn · {}", truncate_chars(&tail, 28)),
        )
    } else if host == "doc.rust-lang.org" {
        let tail = segs
            .last()
            .map(|s| humanize_slug(s))
            .unwrap_or_else(|| "Rust Docs".into());
        (
            "Rust Docs",
            format!("Rust Docs · {}", truncate_chars(&tail, 28)),
        )
    } else if host == "arxiv.org" && segs.len() >= 2 {
        ("arXiv", format!("arXiv:{}", segs[1]))
    } else if host.starts_with("docs.") || segs.first().map(String::as_str) == Some("docs") {
        let tail = segs
            .last()
            .filter(|s| *s != "docs")
            .map(|s| humanize_slug(s))
            .unwrap_or_else(|| host.to_string());
        let p = pretty_provider(host);
        return Some(SmartLink {
            url: raw.to_string(),
            clean_url: clean_url.to_string(),
            host: host.to_string(),
            kind: LinkKind::Docs,
            provider: p,
            label: format!("{host} · {}", truncate_chars(&tail, 28)),
            subtitle: fragment
                .map(|f| format!("#{f}"))
                .unwrap_or_else(|| host.to_string()),
            video_id: None,
            timestamp: None,
            github_api: None,
        });
    } else {
        return None;
    };

    Some(SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind: LinkKind::Docs,
        provider: provider.into(),
        label,
        subtitle: host.to_string(),
        video_id: None,
        timestamp: None,
        github_api: None,
    })
}

fn parse_discussion_or_tool(
    raw: &str,
    clean_url: &str,
    host: &str,
    segs: &[String],
) -> Option<SmartLink> {
    let (kind, provider, label) = match host {
        "stackoverflow.com" if segs.first().map(String::as_str) == Some("questions") => {
            let slug = segs
                .get(2)
                .map(|s| humanize_slug(s))
                .or_else(|| segs.get(1).cloned())
                .unwrap_or_else(|| "Question".into());
            (
                LinkKind::Discussion,
                "Stack Overflow",
                format!("SO · {}", truncate_chars(&slug, 30)),
            )
        }
        "reddit.com" | "old.reddit.com" => {
            let lbl = if segs.first().map(String::as_str) == Some("r") && segs.len() >= 2 {
                format!("r/{}", segs[1])
            } else {
                "Reddit".into()
            };
            (LinkKind::Discussion, "Reddit", lbl)
        }
        "news.ycombinator.com" => (LinkKind::Discussion, "Hacker News", "Hacker News".into()),
        "x.com" | "twitter.com" => {
            let lbl = match segs.first() {
                Some(user) => format!("X · @{user}"),
                None => "X".into(),
            };
            (LinkKind::Discussion, "X", lbl)
        }
        "discord.gg" | "discord.com" => (LinkKind::Discussion, "Discord", "Discord".into()),
        "figma.com" => {
            let name = segs
                .get(2)
                .map(|s| humanize_slug(s))
                .unwrap_or_else(|| "Figma".into());
            (
                LinkKind::Design,
                "Figma",
                format!("Figma · {}", truncate_chars(&name, 28)),
            )
        }
        "docs.google.com" => (LinkKind::Document, "Google Docs", "Google Docs".into()),
        "notion.so" | "notion.site" => (LinkKind::Document, "Notion", "Notion".into()),
        "twitch.tv" => {
            let ch = segs.first().map(String::as_str).unwrap_or("Twitch");
            (LinkKind::MediaVideo, "Twitch", format!("Twitch · {ch}"))
        }
        "vimeo.com" => (LinkKind::MediaVideo, "Vimeo", "Vimeo".into()),
        "open.spotify.com" => (LinkKind::MediaAudio, "Spotify", "Spotify".into()),
        "soundcloud.com" => (LinkKind::MediaAudio, "SoundCloud", "SoundCloud".into()),
        _ => return None,
    };

    Some(SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind,
        provider: provider.into(),
        label,
        subtitle: host.to_string(),
        video_id: None,
        timestamp: None,
        github_api: None,
    })
}

fn parse_media_extension(
    raw: &str,
    clean_url: &str,
    host: &str,
    segs: &[String],
) -> Option<SmartLink> {
    let last = segs.last()?;
    let ext = last.rsplit('.').next()?.to_ascii_lowercase();
    let kind = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "avif" => LinkKind::MediaImage,
        "mp4" | "webm" | "mov" | "mkv" => LinkKind::MediaVideo,
        "mp3" | "wav" | "ogg" | "flac" => LinkKind::MediaAudio,
        "pdf" => LinkKind::Document,
        _ => return None,
    };
    Some(SmartLink {
        url: raw.to_string(),
        clean_url: clean_url.to_string(),
        host: host.to_string(),
        kind,
        provider: pretty_provider(host),
        label: truncate_chars(last, 38),
        subtitle: host.to_string(),
        video_id: None,
        timestamp: None,
        github_api: None,
    })
}

fn build_generic_labels(
    host: &str,
    segs: &[String],
    params: &[(String, String)],
    fragment: Option<&str>,
) -> (String, String) {
    let meaningful = segs
        .iter()
        .rev()
        .find(|s| !s.is_empty() && *s != "index.html" && *s != "index.php");
    let label = match meaningful {
        Some(seg) => {
            let slug = humanize_slug(seg);
            if slug.is_empty() {
                host.to_string()
            } else {
                format!("{host} · {}", truncate_chars(&slug, 28))
            }
        }
        None => {
            if let Some(q) = query_val(params, "q") {
                format!("{host} · {}", truncate_chars(&percent_decode(q), 26))
            } else {
                host.to_string()
            }
        }
    };
    let subtitle = match fragment.filter(|f| !f.is_empty()) {
        Some(f) => format!("{host}#{f}"),
        None => host.to_string(),
    };
    (label, subtitle)
}

fn pretty_provider(host: &str) -> String {
    let base = host.split('.').next().unwrap_or(host);
    if base.len() <= 2 {
        return host.to_string();
    }
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => host.to_string(),
    }
}

fn humanize_slug(seg: &str) -> String {
    let no_ext = seg
        .strip_suffix(".html")
        .or_else(|| seg.strip_suffix(".htm"))
        .or_else(|| seg.strip_suffix(".md"))
        .unwrap_or(seg);
    let mut out = String::with_capacity(no_ext.len());
    let mut prev_space = true;
    for ch in no_ext.chars() {
        if ch == '-' || ch == '_' || ch == '+' {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = ch.is_whitespace();
        }
    }
    out.trim().to_string()
}

fn build_clean_url(
    scheme: &str,
    authority: &str,
    path_raw: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> String {
    let mut out = format!("{scheme}://{authority}");
    if !path_raw.is_empty() {
        out.push('/');
        out.push_str(path_raw);
    }
    if let Some(q) = query {
        let kept: Vec<&str> = q
            .split('&')
            .filter(|kv| {
                let k = kv.split('=').next().unwrap_or("").to_ascii_lowercase();
                !k.is_empty()
                    && !k.starts_with("utm_")
                    && !matches!(
                        k.as_str(),
                        "fbclid" | "gclid" | "si" | "feature" | "pp" | "ref_src" | "igshid"
                    )
            })
            .collect();
        if !kept.is_empty() {
            out.push('?');
            out.push_str(&kept.join("&"));
        }
    }
    if let Some(f) = fragment.filter(|f| !f.is_empty()) {
        out.push('#');
        out.push_str(f);
    }
    out
}

fn parse_query(query: Option<&str>) -> Vec<(String, String)> {
    let Some(q) = query else {
        return Vec::new();
    };
    q.split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((k.to_ascii_lowercase(), percent_decode(v)))
        })
        .collect()
}

fn query_val<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn is_yt_id(s: &str) -> bool {
    s.len() == 11
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn format_yt_timestamp(raw: &str) -> Option<String> {
    let digits: String = raw.trim_end_matches('s').chars().collect();
    let secs: u64 = digits.parse().ok()?;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    Some(if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    })
}

fn parse_markdown_link(rest: &str) -> Option<(&str, &str, usize)> {
    let close_bracket = rest.find("](")?;
    let label = &rest[1..close_bracket];
    if label.contains('\n') || label.contains('[') {
        return None;
    }
    let url_part = &rest[close_bracket + 2..];
    let close_paren = url_part.find(')')?;
    let url = url_part[..close_paren].trim();
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:")) {
        return None;
    }
    Some((label, url, close_bracket + 2 + close_paren + 1))
}

fn scan_url_token(rest: &str) -> &str {
    let mut end = 0usize;
    let mut paren_depth = 0usize;
    for (i, ch) in rest.char_indices() {
        if ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\'' | '`') {
            break;
        }
        if ch == '(' {
            paren_depth += 1;
        } else if ch == ')' {
            if paren_depth == 0 {
                break;
            }
            paren_depth -= 1;
        } else if ch == ']' {
            break;
        }
        end = i + ch.len_utf8();
    }
    rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?'])
}

fn has_valid_authority(url: &str) -> bool {
    if let Some(addr) = url.strip_prefix("mailto:") {
        return addr.contains('@') && addr.contains('.');
    }
    let Some((_, rest)) = url.split_once("://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && (host.contains('.') || host.starts_with("localhost"))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn parse_ipv4(host: &str) -> Option<[u8; 4]> {
    let mut parts = host.split('.');
    let a = parts.next()?.parse().ok()?;
    let b = parts.next()?.parse().ok()?;
    let c = parts.next()?.parse().ok()?;
    let d = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([a, b, c, d])
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(b[i + 1]), hex_nibble(b[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xf) as usize] as char);
        }
    }
    out
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    for (count, (ix, _)) in s.char_indices().enumerate() {
        if count == max {
            return format!("{}…", s[..ix].trim_end());
        }
    }
    s.to_string()
}

fn fetch_youtube_oembed(link: &SmartLink) -> Option<LinkPreview> {
    let oembed_url = format!(
        "https://www.youtube.com/oembed?url={}&format=json",
        percent_encode_component(&link.clean_url)
    );
    let raw = curl_get_bounded(&oembed_url, 32_768, &[])?;
    let json: Value = serde_json::from_slice(&raw).ok()?;
    let title = json
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let author = json
        .get("author_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let thumb_path = link.video_id.as_deref().and_then(fetch_youtube_thumbnail);
    let badge = match link.kind {
        LinkKind::YouTubeShort => Some("Shorts".to_string()),
        LinkKind::YouTubePlaylist => Some("Playlist".to_string()),
        _ => None,
    };
    Some(LinkPreview {
        title,
        description: None,
        author,
        site_name: Some(link.provider.clone()),
        badge,
        badge_tok: Some(theme::DANGER),
        thumb_path,
    })
}

fn fetch_youtube_thumbnail(video_id: &str) -> Option<PathBuf> {
    if !is_yt_id(video_id) {
        return None;
    }
    let dir = std::env::temp_dir().join("fluxcode-link-thumbs");
    let path = dir.join(format!("{video_id}.jpg"));
    if path.is_file() {
        return Some(path);
    }
    let thumb_url = format!("https://i.ytimg.com/vi/{video_id}/mqdefault.jpg");
    let bytes = curl_get_bounded(&thumb_url, MAX_THUMB_BYTES, &[])?;
    let valid_img = bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"\x89PNG");
    if !valid_img {
        return None;
    }
    if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&path, &bytes).is_ok() {
        trim_thumb_cache(&dir);
        return Some(path);
    }
    None
}

fn trim_thumb_cache(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((modified, p))
        })
        .collect();
    if files.len() <= MAX_THUMBS_ON_DISK {
        return;
    }
    files.sort_by_key(|(t, _)| *t);
    for (_, old) in files.iter().take(files.len() - MAX_THUMBS_ON_DISK) {
        let _ = std::fs::remove_file(old);
    }
}

fn fetch_github_api(api_url: &str, link: &SmartLink) -> Option<LinkPreview> {
    let raw = curl_get_bounded(
        api_url,
        32_768,
        &[("Accept", "application/vnd.github+json")],
    )?;
    let json: Value = serde_json::from_slice(&raw).ok()?;
    if json.get("message").is_some()
        && json.get("title").is_none()
        && json.get("full_name").is_none()
    {
        return None;
    }
    if let Some(title) = json.get("title").and_then(Value::as_str) {
        let state = json.get("state").and_then(Value::as_str).unwrap_or("");
        let merged = json
            .pointer("/pull_request/merged_at")
            .is_some_and(|v| !v.is_null());
        let (badge, badge_tok) = if merged {
            (Some("Merged".to_string()), Some(theme::ACCENT_TEXT))
        } else if state == "open" {
            (Some("Open".to_string()), Some(theme::GREEN))
        } else if state == "closed" {
            (Some("Closed".to_string()), Some(theme::DANGER))
        } else {
            (None, None)
        };
        let author = json
            .pointer("/user/login")
            .and_then(Value::as_str)
            .map(|u| format!("@{u}"));
        let desc = json
            .get("body")
            .and_then(Value::as_str)
            .map(|b| clean_meta_text(b.lines().find(|l| !l.trim().is_empty()).unwrap_or("")))
            .filter(|s| !s.is_empty());
        return Some(LinkPreview {
            title: Some(title.trim().to_string()),
            description: desc,
            author,
            site_name: Some("GitHub".to_string()),
            badge,
            badge_tok,
            thumb_path: None,
        });
    }
    let full_name = json.get("full_name").and_then(Value::as_str)?;
    let desc = json
        .get("description")
        .and_then(Value::as_str)
        .map(clean_meta_text)
        .filter(|s| !s.is_empty());
    let lang = json
        .get("language")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(LinkPreview {
        title: Some(full_name.to_string()),
        description: desc,
        author: lang,
        site_name: Some(link.provider.clone()),
        badge: None,
        badge_tok: None,
        thumb_path: None,
    })
}

fn curl_get_bounded(url: &str, max_bytes: u64, headers: &[(&str, &str)]) -> Option<Vec<u8>> {
    let mut cmd = crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" });
    cmd.args([
        "-sS",
        "-f",
        "-L",
        "--max-redirs",
        "3",
        "--connect-timeout",
        "3",
        "--max-time",
        "5",
        "-H",
        "User-Agent: FluxCode/1.0",
    ]);
    for (k, v) in headers {
        cmd.arg("-H").arg(format!("{k}: {v}"));
    }
    cmd.arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let mut buf = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = stdout.take(max_bytes).read_to_end(&mut buf);
    }
    let _ = child.kill();
    let _ = child.wait();
    (!buf.is_empty()).then_some(buf)
}

fn starts_with_ascii_ci(haystack: &str, prefix: &str) -> bool {
    haystack
        .get(..prefix.len())
        .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
}

fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let nb = needle.as_bytes();
    let hb = haystack.as_bytes();
    if nb.is_empty() || hb.len() < nb.len() {
        return None;
    }
    for i in 0..=hb.len() - nb.len() {
        if hb[i..i + nb.len()].eq_ignore_ascii_case(nb) {
            return Some(i);
        }
    }
    None
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut pos = 0usize;
    let bytes = tag.as_bytes();
    while pos < bytes.len() {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let key_start = pos;
        while pos < bytes.len() && !matches!(bytes[pos], b'=' | b' ' | b'\t' | b'\r' | b'\n') {
            pos += 1;
        }
        let key = &tag[key_start..pos];
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] != b'=' {
            continue;
        }
        pos += 1;
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let (val, next_pos) = if bytes[pos] == b'"' || bytes[pos] == b'\'' {
            let quote = bytes[pos];
            let v_start = pos + 1;
            let v_end = tag[v_start..].find(quote as char).map(|x| v_start + x)?;
            (&tag[v_start..v_end], v_end + 1)
        } else {
            let v_start = pos;
            while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            (&tag[v_start..pos], pos)
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(val);
        }
        pos = next_pos;
    }
    None
}

fn clean_meta_text(s: &str) -> String {
    let decoded = decode_html_entities(s);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';').filter(|&n| n <= 10) {
            let ent = &after[..semi];
            let decoded = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                }
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(ch) = decoded {
                out.push(ch);
                rest = &after[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{
        extract_unique_urls, is_safe_public_url, parse_html_meta, parse_link, split_text_links,
        LinkKind, LinkPreviewStore, TextPart, MAX_CACHE_ENTRIES,
    };

    #[test]
    fn parses_youtube_variants_and_strips_tracking() {
        let a = parse_link("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90s&si=abc123");
        assert_eq!(a.kind, LinkKind::YouTubeVideo);
        assert_eq!(a.provider, "YouTube");
        assert_eq!(a.video_id.as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(a.timestamp.as_deref(), Some("1:30"));
        assert_eq!(a.label, "YouTube · dQw4w9WgXcQ");
        assert!(!a.clean_url.contains("si="));

        let b = parse_link("https://youtu.be/jNQXAC9IVRw?t=3665");
        assert_eq!(b.video_id.as_deref(), Some("jNQXAC9IVRw"));
        assert_eq!(b.timestamp.as_deref(), Some("1:01:05"));

        let c = parse_link("https://youtube.com/shorts/abcdefghijk");
        assert_eq!(c.kind, LinkKind::YouTubeShort);
        assert_eq!(c.label, "YouTube Shorts · abcdefghijk");

        let d = parse_link("https://www.youtube.com/@MuaraAI");
        assert_eq!(d.kind, LinkKind::YouTubeChannel);
        assert_eq!(d.label, "YouTube · @MuaraAI");
    }

    #[test]
    fn parses_github_and_registries_and_docs_and_any_site() {
        let pr = parse_link("https://github.com/MuaraAI/FluxCode/pull/138");
        assert_eq!(pr.kind, LinkKind::GitPullRequest);
        assert_eq!(pr.label, "FluxCode#138");
        assert_eq!(
            pr.github_api.as_deref(),
            Some("https://api.github.com/repos/MuaraAI/FluxCode/issues/138")
        );

        let issue = parse_link("https://github.com/MuaraAI/FluxCode/issues/42");
        assert_eq!(issue.kind, LinkKind::GitIssue);
        assert_eq!(issue.label, "FluxCode#42");

        let file =
            parse_link("https://github.com/MuaraAI/FluxCode/blob/main/src/diff_view.rs#L120");
        assert_eq!(file.kind, LinkKind::GitFile);
        assert_eq!(file.label, "FluxCode/diff_view.rs:120");

        let krate = parse_link("https://crates.io/crates/gpui-component");
        assert_eq!(krate.kind, LinkKind::Package);
        assert_eq!(krate.label, "crates.io · gpui-component");

        let wiki = parse_link("https://en.wikipedia.org/wiki/Amdahl%27s_law");
        assert_eq!(wiki.kind, LinkKind::Docs);
        assert_eq!(wiki.label, "Wikipedia · Amdahl's law");

        let any = parse_link("https://example.org/blog/how-we-built-fast-diffs?utm_source=x");
        assert_eq!(any.kind, LinkKind::Web);
        assert_eq!(any.label, "example.org · how we built fast diffs");
        assert_eq!(
            any.clean_url,
            "https://example.org/blog/how-we-built-fast-diffs"
        );
    }

    #[test]
    fn extracts_urls_respecting_code_blocks_and_punctuation() {
        let text = "Cek https://github.com/MuaraAI/FluxCode/pull/138, dan video (https://youtu.be/dQw4w9WgXcQ).";
        let parts = split_text_links(text);
        let links: Vec<_> = parts
            .iter()
            .filter_map(|p| match p {
                TextPart::Link { link, .. } => Some(link.url.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            links,
            vec![
                "https://github.com/MuaraAI/FluxCode/pull/138",
                "https://youtu.be/dQw4w9WgXcQ"
            ]
        );

        let wiki_and_md = "Read https://en.wikipedia.org/wiki/Rust_(programming_language) and [Demo](https://youtu.be/dQw4w9WgXcQ)!";
        let wiki_parts = split_text_links(wiki_and_md);
        let wiki_links: Vec<_> = wiki_parts
            .iter()
            .filter_map(|p| match p {
                TextPart::Link { link, custom_label } => {
                    Some((link.url.as_str(), custom_label.as_deref()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            wiki_links,
            vec![
                (
                    "https://en.wikipedia.org/wiki/Rust_(programming_language)",
                    None
                ),
                ("https://youtu.be/dQw4w9WgXcQ", Some("Demo")),
            ]
        );

        let md = "Skip `https://ignored.dev` and:\n```\nhttps://ignored-fence.dev\n```\nKeep https://docs.rs/gpui.";
        let extracted = extract_unique_urls(md, true);
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].url, "https://docs.rs/gpui");
    }

    #[test]
    fn parses_html_meta_and_blocks_private_ssrf_hosts() {
        let html = r#"<!doctype html><html><head>
            <meta property="og:title" content="FluxCode &amp; Fast Diffs">
            <meta name="description" content="Built for &#39;low-spec&#39; laptops.">
            <meta property="og:site_name" content="MuaraAI">
            <title>Fallback</title>
        </head></html>"#;
        let meta = parse_html_meta(html);
        assert_eq!(meta.title.as_deref(), Some("FluxCode & Fast Diffs"));
        assert_eq!(
            meta.description.as_deref(),
            Some("Built for 'low-spec' laptops.")
        );
        assert_eq!(meta.site_name.as_deref(), Some("MuaraAI"));

        assert!(!is_safe_public_url("http://127.0.0.1:8080/secret"));
        assert!(!is_safe_public_url("http://192.168.1.1/admin"));
        assert!(!is_safe_public_url("http://localhost:3000"));
        assert!(!is_safe_public_url("file:///C:/Windows/win.ini"));
        assert!(is_safe_public_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
    }

    #[test]
    fn store_bounds_lru_entries() {
        let mut store = LinkPreviewStore::default();
        for i in 0..80 {
            let u = format!("https://example.com/{i}");
            store.mark_loading(&u);
            store.finish(&u, None);
        }
        assert_eq!(store.len(), MAX_CACHE_ENTRIES);
    }
}
