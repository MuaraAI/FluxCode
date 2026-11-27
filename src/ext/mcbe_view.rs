use super::bedrock::{
    self, analyze_workspace, deploy_packs, generate_manifests, regenerate_duplicate_uuids,
    sync_pair, sync_world_packs, upsert_script_dep, validate, GenMode, Kind, McbeChannel,
    McbeWorkspace, SyncKind,
};
use super::mcbe_types::{self, InstalledMcbe};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::InputState,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McbeAssetIcon(pub &'static str);

impl gpui_kit::assets::IconNamed for McbeAssetIcon {
    fn path(self) -> SharedString {
        self.0.into()
    }
}

pub const MCBE_ICON: McbeAssetIcon = McbeAssetIcon("mcbe/mcbe.svg");
pub const MCBE_COLOR_ICON: McbeAssetIcon = McbeAssetIcon("mcbe/mcbe-color.svg");

pub fn load_mcbe_asset(path: &str) -> Option<&'static [u8]> {
    match path {
        "mcbe/mcbe.svg" => Some(include_bytes!("../../assets/mcbe/mcbe.svg")),
        "mcbe/mcbe-color.svg" => Some(include_bytes!("../../assets/mcbe/mcbe-color.svg")),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McbeTab {
    #[default]
    Packs,
    Sync,
    ScriptApi,
    Create,
}

pub struct McbeState {
    pub page: bool,
    pub tab: McbeTab,
    pub workspace: Option<McbeWorkspace>,
    pub installed: InstalledMcbe,
    pub channel: McbeChannel,
    pub gen_open: bool,
    pub gen_mode: GenMode,
    pub gen_script: bool,
    pub gen_ui_dep: bool,
    pub gen_name: Entity<InputState>,
    pub gen_desc: Entity<InputState>,
    pub busy_pkg: Option<String>,
    pub refreshing_tags: bool,
    pub note: Option<String>,
}

impl McbeState {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let installed = mcbe_types::detect_installed_mcbe();
        let channel = if installed.official.is_none() && installed.preview.is_some() {
            McbeChannel::Preview
        } else {
            McbeChannel::Official
        };
        Self {
            page: false,
            tab: McbeTab::Packs,
            workspace: None,
            installed,
            channel,
            gen_open: false,
            gen_mode: GenMode::BothSynced,
            gen_script: true,
            gen_ui_dep: false,
            gen_name: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Pack name"))),
            gen_desc: cx
                .new(|cx| InputState::new(window, cx).placeholder(tr("Description (optional)"))),
            busy_pkg: None,
            refreshing_tags: false,
            note: None,
        }
    }

    pub fn engine_for_channel(&self) -> [u32; 3] {
        match self.channel {
            McbeChannel::Official => self.installed.official_engine.unwrap_or([1, 21, 90]),
            McbeChannel::Preview => self.installed.preview_engine.unwrap_or([1, 26, 70]),
        }
    }
}

fn channel_pill(id: &'static str, label: &'static str, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .h(theme::u(24.))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .border_1()
        .border_color(c(if active { theme::ACCENT } else { theme::BORDER }))
        .when(active, |d| {
            d.bg(c(theme::ACCENT).opacity(0.18))
                .text_color(c(theme::FG_STRONG))
        })
        .when(!active, |d| {
            d.text_color(c(theme::FG_MUTED))
                .hover(|h| h.bg(c(theme::BG_HOVER)).text_color(c(theme::FG)))
        })
        .text_xs()
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .cursor_pointer()
        .child(tr(label))
}

fn option_chip(id: impl Into<ElementId>, label: SharedString, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(theme::u(22.))
        .px_2()
        .flex()
        .items_center()
        .rounded(px(4.))
        .border_1()
        .border_color(c(if active { theme::ACCENT } else { theme::BORDER }))
        .when(active, |d| {
            d.bg(c(theme::ACCENT).opacity(0.15))
                .text_color(c(theme::FG_STRONG))
        })
        .when(!active, |d| {
            d.text_color(c(theme::FG_MUTED))
                .hover(|h| h.bg(c(theme::BG_HOVER)))
        })
        .text_xs()
        .cursor_pointer()
        .child(label)
}

fn pack_icon_spec(kind: &Kind) -> (Ico, &'static str, crate::theme::Tok) {
    match kind {
        Kind::Behavior => (Ico::Braces, "Behavior Pack", theme::ACCENT_TEXT),
        Kind::Resource => (Ico::Image, "Resource Pack", theme::GREEN),
        Kind::Skin => (Ico::Sparkles, "Skin Pack", theme::BLUE),
        Kind::World => (Ico::Globe, "World Template", theme::WARN),
        Kind::Other => (Ico::Box, "Pack", theme::FG_MUTED),
    }
}

fn pack_icon_tile(kind: &Kind, sz: f32) -> Div {
    let (ico, _, col_k) = pack_icon_spec(kind);
    div()
        .flex_none()
        .size(theme::u(sz))
        .rounded(px(sz * 0.24))
        .bg(c(col_k).opacity(0.14))
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(ico)
                .size(theme::u(sz * 0.52))
                .text_color(c(col_k)),
        )
}

fn short_pack_badge(kind: &Kind) -> &'static str {
    match kind {
        Kind::Behavior => "BP",
        Kind::Resource => "RP",
        Kind::Skin => "Skin",
        Kind::World => "World",
        Kind::Other => "Pack",
    }
}

fn clean_sync_title(raw: &str) -> String {
    raw.replace(" <-> ", " ↔ ").replace(" -> ", " → ")
}

fn sync_item_help(item: &bedrock::SyncItem) -> String {
    match item.kind {
        SyncKind::Synced => {
            let ver = item.detail.trim_start_matches("Synced ").trim();
            if ver.is_empty() {
                tr("Linked and matching. Both packs load together in Minecraft.").to_string()
            } else {
                crate::trf!(
                    "Linked and matching ({ver}). Both packs load together in Minecraft.";
                    ver => ver
                )
            }
        }
        SyncKind::UnlinkedPair => tr(
            "Not linked yet. Click Link so enabling the Behavior Pack automatically loads its Resource Pack.",
        )
        .to_string(),
        SyncKind::VersionMismatch => tr(
            "Version mismatch between paired packs. Click Sync to match both manifests.",
        )
        .to_string(),
        SyncKind::WorldPackMismatch => tr(
            "World JSON activates this Behavior Pack, but its paired Resource Pack is missing or outdated.",
        )
        .to_string(),
        SyncKind::DuplicateUuid => tr(
            "Two packs share the same UUID, so Minecraft will skip one. Click Fix UUID to generate a fresh UUID.",
        )
        .to_string(),
        SyncKind::MissingCounterpart => {
            tr("This pack points to a pack UUID that is not in this folder.").to_string()
        }
    }
}

fn format_api_version(man_ver: &str, npm_ver: &str) -> String {
    if man_ver == npm_ver {
        format!("v{man_ver}")
    } else {
        format!("v{man_ver} (npm {npm_ver})")
    }
}

fn pkg_friendly_summary(pkg_name: &str, fallback: &'static str) -> &'static str {
    match pkg_name {
        "@minecraft/server" => tr("Core entities, blocks, worlds, events & system ticks."),
        "@minecraft/server-ui" => {
            tr("Popup menus and dialog forms (ActionForm, ModalForm, MessageForm).")
        }
        "@minecraft/server-gametest" => {
            tr("Automated test suites and simulated players for GameTest.")
        }
        "@minecraft/server-net" => tr("HTTP client requests for Bedrock Dedicated Server."),
        "@minecraft/server-admin" => {
            tr("Server secrets, variables, and player transfer helpers (BDS).")
        }
        "@minecraft/common" => tr("Shared Bedrock Script API types, vectors, and errors."),
        "@minecraft/debug-utilities" => tr("Profiler watchdog and in-world debug shape rendering."),
        _ => fallback,
    }
}

fn page_intro(title: &'static str, desc: &'static str) -> Div {
    v_flex()
        .w_full()
        .gap_1()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_STRONG))
                .child(tr(title)),
        )
        .child(
            div()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(tr(desc)),
        )
}

impl Workbench {
    pub(crate) fn has_mcbe_workspace(&self) -> bool {
        self.mcbe.workspace.is_some() || self.ext.found.iter().any(|p| p.ext == "bedrock")
    }

    pub(crate) fn open_mcbe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_pages(window, cx);
        self.agent_full = false;
        self.mcbe.page = true;
        if self.mcbe.workspace.is_none() {
            self.mcbe_refresh(cx);
        }
        cx.notify();
    }

    pub(crate) fn toggle_mcbe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mcbe.page && !self.settings_open {
            self.mcbe.page = false;
            cx.notify();
        } else {
            self.open_mcbe(window, cx);
        }
    }

    pub(crate) fn mcbe_sync_types_context(&self) {
        let mut versions = HashMap::new();
        if let Some(ws) = &self.mcbe.workspace {
            for p in &ws.packs {
                for dep in &p.module_deps {
                    versions
                        .entry(dep.module_name.clone())
                        .or_insert_with(|| dep.version.clone());
                }
            }
        }
        mcbe_types::set_workspace_context(self.has_mcbe_workspace(), self.mcbe.channel, versions);
    }

    pub(crate) fn mcbe_refresh(&mut self, cx: &mut Context<Self>) {
        let roots = self.roots();
        cx.spawn(async move |this, cx| {
            let ws = cx
                .background_spawn(async move { analyze_workspace(&roots) })
                .await;
            _ = this.update(cx, |this, cx| {
                if let Some(ref w) = ws {
                    let uses_beta = w.packs.iter().any(|p| {
                        p.module_deps
                            .iter()
                            .any(|d| d.version.contains("beta") || d.version.contains("rc"))
                    });
                    if uses_beta && this.mcbe.installed.preview.is_some() {
                        this.mcbe.channel = McbeChannel::Preview;
                    }
                }
                this.mcbe.workspace = ws;
                if !this.has_mcbe_workspace() && this.view == crate::workbench::SideView::Mcbe {
                    this.view = crate::workbench::SideView::Explorer;
                }
                this.mcbe_sync_types_context();
                cx.notify();
            });
        })
        .detach();
    }

    fn mcbe_set_channel(&mut self, ch: McbeChannel, cx: &mut Context<Self>) {
        self.mcbe.channel = ch;
        self.mcbe_sync_types_context();
        cx.notify();
    }

    fn mcbe_deploy_all(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = self.mcbe.workspace.as_ref() else {
            return;
        };
        let packs: Vec<_> = ws.packs.iter().map(|i| i.pack.clone()).collect();
        if packs.is_empty() {
            return;
        }
        let ch = self.mcbe.channel;
        let who = ws
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Minecraft Bedrock".into());
        self.ext_background(who, tr("Deploy to Minecraft").into(), cx, move || {
            deploy_packs(&packs, ch)
        });
    }

    fn mcbe_check_all(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = self.mcbe.workspace.as_ref() else {
            return;
        };
        let packs: Vec<_> = ws.packs.iter().map(|i| i.pack.clone()).collect();
        let who = "Minecraft Bedrock".to_string();
        self.ext_background(who, tr("Check the manifests").into(), cx, move || {
            if packs.is_empty() {
                return Err("No pack here (a folder with manifest.json).".into());
            }
            let probs = validate(&packs);
            if probs.is_empty() {
                Ok(packs
                    .iter()
                    .map(|p| format!("{}: manifest.json is fine", p.name))
                    .collect())
            } else {
                Err(probs
                    .iter()
                    .map(|(d, m)| {
                        format!(
                            "{}: {m}",
                            d.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
        });
    }

    fn mcbe_pack_all(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = self.mcbe.workspace.as_ref() else {
            return;
        };
        let proj = super::Project {
            ext: "bedrock".into(),
            dir: ws.root.clone(),
            name: ws
                .root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "addon".into()),
        };
        self.ext_background(
            proj.name.clone(),
            tr("Pack the add-on").into(),
            cx,
            move || bedrock::pack(&proj),
        );
    }

    fn mcbe_fix_sync(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(ws) = self.mcbe.workspace.as_ref() else {
            return;
        };
        let Some(item) = ws.sync_items.get(ix).cloned() else {
            return;
        };
        let packs = ws.packs.clone();
        let root = ws.root.clone();
        let res = match item.kind {
            SyncKind::DuplicateUuid => regenerate_duplicate_uuids(&packs).map(|v| {
                if v.is_empty() {
                    "All UUIDs unique".into()
                } else {
                    format!("Regenerated UUIDs for {}", v.join(", "))
                }
            }),
            SyncKind::WorldPackMismatch => sync_world_packs(&root, &packs),
            SyncKind::VersionMismatch | SyncKind::UnlinkedPair => {
                match (item.bp_dir, item.rp_dir) {
                    (Some(bp), Some(rp)) => sync_pair(&bp, &rp),
                    _ => Err("Need both a Behavior Pack and a Resource Pack to sync".into()),
                }
            }
            SyncKind::MissingCounterpart => {
                let bp = item.bp_dir.or_else(|| {
                    packs
                        .iter()
                        .find(|p| p.pack.kind == Kind::Behavior)
                        .map(|p| p.pack.dir.clone())
                });
                let rp = item.rp_dir.or_else(|| {
                    packs
                        .iter()
                        .find(|p| p.pack.kind == Kind::Resource)
                        .map(|p| p.pack.dir.clone())
                });
                match (bp, rp) {
                    (Some(b), Some(r)) => sync_pair(&b, &r),
                    _ => Err("No matching pack in workspace to link".into()),
                }
            }
            SyncKind::Synced => return,
        };
        match res {
            Ok(msg) => {
                self.mcbe.note = Some(msg);
                self.status = tr("Manifests synced").into();
                self.mcbe_refresh(cx);
            }
            Err(e) => self.mcbe.note = Some(e),
        }
        cx.notify();
    }

    fn mcbe_add_script_dep(
        &mut self,
        pkg: &'static str,
        ver: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bp) = self
            .mcbe
            .workspace
            .as_ref()
            .and_then(|w| w.packs.iter().find(|p| p.pack.kind == Kind::Behavior))
            .map(|p| p.pack.dir.clone())
        else {
            self.mcbe.note =
                Some(tr("Need a Behavior Pack first (generate one below)").to_string());
            self.mcbe.gen_open = true;
            self.mcbe.tab = McbeTab::Create;
            cx.notify();
            return;
        };
        match upsert_script_dep(&bp, pkg, &ver) {
            Ok(msg) => {
                self.mcbe.note = Some(msg);
                self.mcbe_refresh(cx);
                self.open_file(bp.join("manifest.json"), window, cx);
            }
            Err(e) => self.mcbe.note = Some(e),
        }
        cx.notify();
    }

    fn mcbe_hydrate_pkg(&mut self, pkg: &'static str, npm_ver: String, cx: &mut Context<Self>) {
        if self.mcbe.busy_pkg.is_some() {
            return;
        }
        self.mcbe.busy_pkg = Some(pkg.to_string());
        self.mcbe.note = Some(tr("Fetching .d.ts into global compressed cache…").to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move { mcbe_types::hydrate_package_types(pkg, &npm_ver) })
                .await;
            _ = this.update(cx, |this, cx| {
                this.mcbe.busy_pkg = None;
                match res {
                    Ok(bytes) => {
                        let kb = (bytes / 1024).max(1);
                        this.mcbe.note = Some(crate::trf!("Cached {pkg} ({kb} KB, 0 MB in workspace)"; pkg => pkg, kb => kb));
                        this.mcbe_sync_types_context();
                    }
                    Err(e) => this.mcbe.note = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn mcbe_refresh_dist_tags(&mut self, cx: &mut Context<Self>) {
        if self.mcbe.refreshing_tags {
            return;
        }
        self.mcbe.refreshing_tags = true;
        self.mcbe.note = Some(tr("Refreshing @minecraft/* dist-tags…").to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async { mcbe_types::refresh_npm_dist_tags() })
                .await;
            _ = this.update(cx, |this, cx| {
                this.mcbe.refreshing_tags = false;
                match res {
                    Ok(_) => {
                        this.mcbe.note = Some(tr("Updated @minecraft/* dist-tags").to_string());
                        this.mcbe_sync_types_context();
                    }
                    Err(e) => this.mcbe.note = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn mcbe_generate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self
            .mcbe
            .workspace
            .as_ref()
            .map(|w| w.root.clone())
            .or_else(|| self.root.clone())
        else {
            return;
        };
        let name = self.mcbe.gen_name.read(cx).value().to_string();
        let desc = self.mcbe.gen_desc.read(cx).value().to_string();
        let catalog = mcbe_types::catalog();
        let ch = self.mcbe.channel;
        let mut script_deps = Vec::new();
        if self.mcbe.gen_script {
            if let Some(s) = catalog.iter().find(|c| c.name == "@minecraft/server") {
                script_deps.push((s.name.to_string(), s.manifest_for(ch).to_string()));
            }
            if self.mcbe.gen_ui_dep {
                if let Some(u) = catalog.iter().find(|c| c.name == "@minecraft/server-ui") {
                    script_deps.push((u.name.to_string(), u.manifest_for(ch).to_string()));
                }
            }
        }
        let spec = bedrock::GenerateSpec {
            name,
            description: desc,
            mode: self.mcbe.gen_mode,
            min_engine_version: self.mcbe.engine_for_channel(),
            with_script_api: self.mcbe.gen_script,
            script_deps,
        };
        match generate_manifests(&root, &spec) {
            Ok(paths) => {
                let n = paths.len();
                self.mcbe.note = Some(crate::trf!("Generated {n} manifest(s)"; n => n));
                self.mcbe.gen_open = false;
                self.mcbe.tab = McbeTab::Packs;
                self.mcbe
                    .gen_name
                    .update(cx, |s, cx| s.set_value("", window, cx));
                self.mcbe
                    .gen_desc
                    .update(cx, |s, cx| s.set_value("", window, cx));
                if let Some(first) = paths.into_iter().next() {
                    self.open_file(first, window, cx);
                }
                self.refresh_tree(cx);
                self.mcbe_refresh(cx);
            }
            Err(e) => self.mcbe.note = Some(e),
        }
        cx.notify();
    }

    pub(crate) fn render_mcbe_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.mcbe;
        let ws = m.workspace.as_ref();
        let ch = m.channel;
        let mut col = v_flex()
            .id("mcbe-view")
            .size_full()
            .overflow_y_scroll()
            .px_3()
            .pt_1()
            .pb_5()
            .gap(theme::u(14.));

        // 1. Header Card + Channel Switcher
        let base_installed = match (ch, &m.installed.official, &m.installed.preview) {
            (McbeChannel::Official, Some(v), _) => crate::trf!("MCBE {v} detected"; v => v),
            (McbeChannel::Preview, _, Some(v)) => crate::trf!("MCBE {v} detected"; v => v),
            _ => tr("Not detected on PC").to_string(),
        };
        let installed_line: SharedString = if ws.is_some_and(|w| w.is_world_folder) {
            format!("{base_installed} · World").into()
        } else {
            base_installed.into()
        };
        let header_card = v_flex()
            .p_2p5()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .child(
                h_flex()
                    .gap_2p5()
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(theme::u(32.))
                            .rounded(px(7.))
                            .bg(c(theme::BG_SIDE))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                img(MCBE_COLOR_ICON.0)
                                    .flex_none()
                                    .size(theme::u(24.))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(tr("Minecraft Bedrock")),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(installed_line),
                            ),
                    )
                    .child(
                        Button::new("mcbe-sb-open-studio")
                            .label(tr("Open Studio"))
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(|this, _, w, cx| this.open_mcbe(w, cx))),
                    ),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        channel_pill("mcbe-ch-off", "Official", ch == McbeChannel::Official)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mcbe_set_channel(McbeChannel::Official, cx)
                            })),
                    )
                    .child(
                        channel_pill("mcbe-ch-prev", "Preview", ch == McbeChannel::Preview)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mcbe_set_channel(McbeChannel::Preview, cx)
                            })),
                    ),
            );
        col = col.child(header_card);

        // 2. Quick Action Strip (Deploy / Check / Pack)
        if ws.is_some_and(|w| !w.packs.is_empty()) {
            let action_btn = |id: &'static str,
                              ico: Ico,
                              label: &'static str,
                              tip: &'static str,
                              primary: bool| {
                let b = Button::new(id)
                    .icon(Icon::new(ico))
                    .label(tr(label))
                    .xsmall()
                    .flex_1()
                    .tooltip(tr(tip));
                if primary {
                    b.primary()
                } else {
                    b.outline()
                }
            };
            col = col.child(
                h_flex()
                    .w_full()
                    .gap_1p5()
                    .child(
                        action_btn(
                            "mcbe-deploy",
                            Ico::Upload,
                            "Deploy",
                            "Deploy to active channel's com.mojang folder",
                            true,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_deploy_all(cx))),
                    )
                    .child(
                        action_btn(
                            "mcbe-check",
                            Ico::CircleCheck,
                            "Check",
                            "Validate all manifests and UUIDs",
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_check_all(cx))),
                    )
                    .child(
                        action_btn(
                            "mcbe-pack",
                            Ico::Package,
                            "Pack",
                            "Bundle into .mcaddon / .mcpack",
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_pack_all(cx))),
                    ),
            );
        }

        if let Some(note) = &m.note {
            col = col.child(
                h_flex()
                    .px_2p5()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::ACCENT).opacity(0.4))
                    .bg(c(theme::ACCENT).opacity(0.1))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::FG_STRONG))
                            .child(note.clone()),
                    )
                    .child(
                        Button::new("mcbe-sb-note-dismiss")
                            .icon(Icon::new(Ico::Close))
                            .xsmall()
                            .ghost()
                            .tooltip(tr("Dismiss"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mcbe.note = None;
                                cx.notify();
                            })),
                    ),
            );
        }

        // 3. Packs & Sync Diagnostics
        let mut packs_sec = v_flex().w_full().gap_1p5();
        let issues = ws
            .map(|w| {
                w.sync_items
                    .iter()
                    .filter(|s| s.kind != SyncKind::Synced)
                    .count()
            })
            .unwrap_or(0);
        let badge_txt: SharedString = if issues == 0 {
            tr("All synced").into()
        } else if issues == 1 {
            crate::trf!("{n} issue"; n => issues).into()
        } else {
            crate::trf!("{n} issues"; n => issues).into()
        };
        let badge_col = if issues == 0 {
            theme::GREEN
        } else {
            theme::WARN
        };
        packs_sec = packs_sec.child(
            h_flex()
                .px_0p5()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_MUTED))
                        .child(tr("PACKS & SYNC")),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(badge_col))
                        .child(badge_txt),
                ),
        );

        if let Some(w) = ws {
            for (ix, item) in w.sync_items.iter().enumerate() {
                let (tint, ico) = match item.kind {
                    SyncKind::Synced => (theme::GREEN, Ico::Check),
                    SyncKind::MissingCounterpart | SyncKind::DuplicateUuid => {
                        (theme::DANGER, Ico::CircleAlert)
                    }
                    SyncKind::VersionMismatch
                    | SyncKind::UnlinkedPair
                    | SyncKind::WorldPackMismatch => (theme::WARN, Ico::TriangleAlert),
                };
                let btn_lbl = match item.kind {
                    SyncKind::UnlinkedPair | SyncKind::MissingCounterpart => tr("Link"),
                    SyncKind::DuplicateUuid => tr("Fix"),
                    _ => tr("Sync"),
                };
                packs_sec = packs_sec.child(
                    h_flex()
                        .id(("mcbe-sync", ix))
                        .p_2()
                        .gap_2()
                        .items_center()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(
                            c(if item.kind == SyncKind::Synced {
                                theme::BORDER
                            } else {
                                tint
                            })
                            .opacity(0.45),
                        )
                        .bg(c(theme::BG_ELEVATED))
                        .child(
                            Icon::new(ico)
                                .flex_none()
                                .size(theme::u(13.))
                                .text_color(c(tint)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(c(theme::FG_STRONG))
                                        .child(clean_sync_title(&item.title)),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .text_color(c(theme::FG_DIM))
                                        .child(item.detail.clone()),
                                ),
                        )
                        .when(item.fixable, |h| {
                            h.child(
                                Button::new(("mcbe-fix", ix))
                                    .label(btn_lbl)
                                    .xsmall()
                                    .outline()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mcbe_fix_sync(ix, cx)
                                    })),
                            )
                        }),
                );
            }

            if w.packs.is_empty() {
                packs_sec = packs_sec.child(
                    div()
                        .px_1()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(tr("No packs detected yet. Generate BP/RP below.")),
                );
            } else {
                let mut list = v_flex()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .overflow_hidden();
                for (ix, info) in w.packs.iter().enumerate() {
                    let (_, kind_label, _) = pack_icon_spec(&info.pack.kind);
                    let manifest_path: PathBuf = info.pack.dir.join("manifest.json");
                    let ver = format!(
                        "v{}.{}.{}",
                        info.version[0], info.version[1], info.version[2]
                    );
                    let sub_line = if let Some(entry) = &info.script_entry {
                        format!("{} · {ver} · {entry}", info.rel_dir)
                    } else {
                        format!("{} · {ver}", info.rel_dir)
                    };
                    list = list.child(
                        h_flex()
                            .id(("mcbe-pack-row", ix))
                            .px_2p5()
                            .py_1p5()
                            .gap_2()
                            .items_center()
                            .cursor_pointer()
                            .when(ix > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                            .hover(|h| h.bg(c(theme::BG_HOVER)))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.open_file(manifest_path.clone(), w, cx);
                            }))
                            .child(pack_icon_tile(&info.pack.kind, 24.))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(c(theme::FG_STRONG))
                                            .child(info.pack.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(c(theme::FG_DIM))
                                            .child(format!("{} · {sub_line}", tr(kind_label))),
                                    ),
                            ),
                    );
                }
                packs_sec = packs_sec.child(list);
            }
        }
        col = col.child(packs_sec);

        // 4. Script API & NPM Packages (Zero node_modules)
        let cache_kb = mcbe_types::cache_total_bytes() / 1024;
        let cache_label: SharedString = if cache_kb == 0 {
            tr("Zero node_modules").into()
        } else {
            crate::trf!("Cache: {kb} KB"; kb => cache_kb).into()
        };
        let mut api_sec = v_flex().w_full().gap_1p5().child(
            h_flex()
                .px_0p5()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_MUTED))
                        .child(tr("SCRIPT API & NPM TYPES")),
                )
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(cache_label),
                        )
                        .child(
                            Button::new("mcbe-tags-refresh")
                                .icon(Icon::new(Ico::RefreshCw))
                                .xsmall()
                                .ghost()
                                .loading(m.refreshing_tags)
                                .tooltip(tr("Refresh"))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.mcbe_refresh_dist_tags(cx)),
                                ),
                        ),
                ),
        );
        let mut api_list = v_flex()
            .rounded(px(6.))
            .border_1()
            .border_color(c(theme::BORDER))
            .overflow_hidden();
        for (ix, pkg) in mcbe_types::catalog().into_iter().enumerate() {
            let npm_ver = pkg.npm_for(ch).to_string();
            let man_ver = pkg.manifest_for(ch).to_string();
            let cached = mcbe_types::is_cached(pkg.name, &npm_ver);
            let busy = m.busy_pkg.as_deref() == Some(pkg.name);
            let pkg_name = pkg.name;
            let man_v_click = man_ver.clone();
            let npm_v_click = npm_ver.clone();
            api_list = api_list.child(
                h_flex()
                    .id(("mcbe-pkg", ix))
                    .px_2p5()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .when(ix > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(pkg.name),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(format_api_version(&man_ver, &npm_ver)),
                            ),
                    )
                    .child(
                        Button::new(("mcbe-pkg-add", ix))
                            .label(tr("+ Manifest"))
                            .xsmall()
                            .outline()
                            .tooltip(pkg.summary)
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.mcbe_add_script_dep(pkg_name, man_v_click.clone(), w, cx)
                            })),
                    )
                    .child(if cached {
                        div()
                            .flex_none()
                            .px_1p5()
                            .py(px(2.))
                            .rounded(px(4.))
                            .bg(c(theme::GREEN).opacity(0.14))
                            .text_xs()
                            .text_color(c(theme::GREEN))
                            .child(tr("Cached"))
                            .into_any_element()
                    } else {
                        Button::new(("mcbe-pkg-dts", ix))
                            .label(tr("Get types"))
                            .xsmall()
                            .ghost()
                            .loading(busy)
                            .tooltip(pkg.summary)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mcbe_hydrate_pkg(pkg_name, npm_v_click.clone(), cx)
                            }))
                            .into_any_element()
                    }),
            );
        }
        api_sec = api_sec.child(api_list);
        col = col.child(api_sec);

        // 5. BP / RP Manifest Generator
        let gen_toggle_lbl = if m.gen_open {
            tr("Hide")
        } else {
            tr("New Pack / Manifest")
        };
        let mut gen_sec = v_flex().w_full().gap_1p5().child(
            h_flex()
                .px_0p5()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_MUTED))
                        .child(tr("MANIFEST GENERATOR")),
                )
                .child(
                    Button::new("mcbe-gen-toggle")
                        .label(gen_toggle_lbl)
                        .xsmall()
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mcbe.gen_open = !this.mcbe.gen_open;
                            cx.notify();
                        })),
                ),
        );
        if m.gen_open {
            gen_sec = gen_sec.child(
                v_flex()
                    .p_2p5()
                    .gap_2()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_ELEVATED))
                    .child(crate::ui::McInput::new(&m.gen_name).small())
                    .child(crate::ui::McInput::new(&m.gen_desc).small())
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                option_chip(
                                    "mcbe-gm-both",
                                    tr("BP + RP").into(),
                                    m.gen_mode == GenMode::BothSynced,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.mcbe.gen_mode = GenMode::BothSynced;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                option_chip(
                                    "mcbe-gm-bp",
                                    tr("BP only").into(),
                                    m.gen_mode == GenMode::BehaviorOnly,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.mcbe.gen_mode = GenMode::BehaviorOnly;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                option_chip(
                                    "mcbe-gm-rp",
                                    tr("RP only").into(),
                                    m.gen_mode == GenMode::ResourceOnly,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.mcbe.gen_mode = GenMode::ResourceOnly;
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                option_chip("mcbe-g-script", tr("Script API").into(), m.gen_script)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mcbe.gen_script = !this.mcbe.gen_script;
                                        cx.notify();
                                    })),
                            )
                            .when(m.gen_script, |h| {
                                h.child(
                                    option_chip(
                                        "mcbe-g-ui",
                                        tr("Add @minecraft/server-ui").into(),
                                        m.gen_ui_dep,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.mcbe.gen_ui_dep = !this.mcbe.gen_ui_dep;
                                            cx.notify();
                                        },
                                    )),
                                )
                            }),
                    )
                    .child(
                        Button::new("mcbe-gen-go")
                            .label(tr("Generate"))
                            .small()
                            .primary()
                            .w_full()
                            .on_click(cx.listener(|this, _, w, cx| this.mcbe_generate(w, cx))),
                    ),
            );
        }
        col = col.child(gen_sec);

        col.into_any_element()
    }

    pub(crate) fn render_mcbe_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let m = &self.mcbe;
        let ws = m.workspace.as_ref();
        let ch = m.channel;
        let weak = cx.entity().downgrade();

        let base_installed = match (ch, &m.installed.official, &m.installed.preview) {
            (McbeChannel::Official, Some(v), _) => crate::trf!("MCBE {v} detected"; v => v),
            (McbeChannel::Preview, _, Some(v)) => crate::trf!("MCBE {v} detected"; v => v),
            _ => tr("Not detected on PC").to_string(),
        };
        let subtitle: SharedString = if ws.is_some_and(|w| w.is_world_folder) {
            format!("{base_installed} · {}", tr("World Folder")).into()
        } else {
            base_installed.into()
        };

        let ch_seg = crate::ui::SegmentedControl::new(
            "mcbe-page-ch",
            [
                (McbeChannel::Official, "mcbe-p-off", tr("Official")),
                (McbeChannel::Preview, "mcbe-p-prev", tr("Preview")),
            ]
            .into_iter()
            .map(|(chan, id, lbl)| {
                let w = weak.clone();
                crate::ui::SegItem::new(id, lbl, ch == chan, move |_, _, cx| {
                    _ = w.update(cx, |this, cx| this.mcbe_set_channel(chan, cx));
                })
            }),
        );

        let header = h_flex()
            .w_full()
            .flex_wrap()
            .gap_3()
            .pb_3()
            .border_b_1()
            .border_color(c(theme::BORDER))
            .items_center()
            .child(
                h_flex()
                    .flex_1()
                    .min_w(theme::u(240.))
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(theme::u(34.))
                            .rounded(px(8.))
                            .bg(c(theme::BG_ELEVATED))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                img(MCBE_COLOR_ICON.0)
                                    .flex_none()
                                    .size(theme::u(24.))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(tr("Minecraft Bedrock Studio")),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(subtitle),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .items_center()
                    .child(ch_seg)
                    .child(
                        Button::new("mcbe-pg-refresh")
                            .icon(Icon::new(Ico::RefreshCw))
                            .small()
                            .ghost()
                            .tooltip(tr("Refresh"))
                            .on_click(cx.listener(|this, _, _, cx| this.mcbe_refresh(cx))),
                    )
                    .child(
                        Button::new("mcbe-pg-close")
                            .icon(Icon::new(Ico::Close))
                            .small()
                            .ghost()
                            .tooltip(tr("Close (Esc)"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mcbe.page = false;
                                cx.notify();
                            })),
                    ),
            );

        let pack_count = ws.map(|w| w.packs.len()).unwrap_or(0);
        let issues = ws
            .map(|w| {
                w.sync_items
                    .iter()
                    .filter(|s| s.kind != SyncKind::Synced)
                    .count()
            })
            .unwrap_or(0);
        let packs_tab_lbl: SharedString = if pack_count > 0 {
            crate::trf!("My Packs ({n})"; n => pack_count).into()
        } else {
            tr("My Packs").into()
        };
        let sync_tab_lbl: SharedString = if issues > 0 {
            crate::trf!("Pack Links ({n})"; n => issues).into()
        } else {
            tr("Pack Links").into()
        };
        let nav_tabs = crate::ui::SegmentedControl::new(
            "mcbe-page-tabs",
            [
                (McbeTab::Packs, "mcbe-tab-packs", packs_tab_lbl),
                (McbeTab::Sync, "mcbe-tab-sync", sync_tab_lbl),
                (McbeTab::ScriptApi, "mcbe-tab-api", tr("Script API").into()),
                (McbeTab::Create, "mcbe-tab-new", tr("New Pack").into()),
            ]
            .into_iter()
            .map(|(tab, id, lbl)| {
                let w = weak.clone();
                crate::ui::SegItem::new(id, lbl, m.tab == tab, move |_, _, cx| {
                    _ = w.update(cx, |this, cx| {
                        this.mcbe.tab = tab;
                        cx.notify();
                    });
                })
            }),
        );

        let _ = window;
        let body: AnyElement = match m.tab {
            McbeTab::Packs => self.render_mcbe_tab_packs(ws, cx),
            McbeTab::Sync => self.render_mcbe_tab_sync(ws, cx),
            McbeTab::ScriptApi => self.render_mcbe_tab_api(ws, m, ch, cx),
            McbeTab::Create => self.render_mcbe_tab_create(m, ch, cx),
        };

        let container = v_flex()
            .w_full()
            .max_w(theme::u(780.))
            .mx_auto()
            .gap_4()
            .child(header)
            .child(h_flex().w_full().flex_wrap().child(nav_tabs))
            .when_some(m.note.clone(), |v, note| {
                v.child(
                    h_flex()
                        .px_3()
                        .py_2()
                        .gap_2()
                        .items_center()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(c(theme::ACCENT).opacity(0.4))
                        .bg(c(theme::ACCENT).opacity(0.1))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(c(theme::FG_STRONG))
                                .child(note),
                        )
                        .child(
                            Button::new("mcbe-pg-note-dismiss")
                                .icon(Icon::new(Ico::Close))
                                .xsmall()
                                .ghost()
                                .tooltip(tr("Dismiss"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.mcbe.note = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(body);

        v_flex()
            .id("mcbe-page")
            .key_context("McbePage")
            .size_full()
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .px_6()
            .py_5()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.mcbe.page = false;
                    cx.notify();
                }
            }))
            .child(container)
            .into_any_element()
    }

    fn render_mcbe_tab_packs(
        &self,
        ws: Option<&McbeWorkspace>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = v_flex().w_full().gap_4().child(page_intro(
            "Your Add-on Packs",
            "Manage every Behavior Pack and Resource Pack in this folder, test them in Minecraft, or export an .mcaddon file.",
        ));

        let has_packs = ws.is_some_and(|w| !w.packs.is_empty());
        if has_packs {
            let action_tile = |id: &'static str,
                               ico: Ico,
                               title: &'static str,
                               desc: &'static str,
                               accent: bool| {
                h_flex()
                    .id(id)
                    .flex_1()
                    .min_w(theme::u(200.))
                    .p_3()
                    .gap_2p5()
                    .items_start()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if accent {
                        c(theme::ACCENT).opacity(0.45)
                    } else {
                        c(theme::BORDER)
                    })
                    .bg(if accent {
                        c(theme::ACCENT).opacity(0.09)
                    } else {
                        c(theme::BG_ELEVATED)
                    })
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(if accent {
                            c(theme::ACCENT).opacity(0.16)
                        } else {
                            c(theme::BG_HOVER)
                        })
                    })
                    .child(
                        Icon::new(ico)
                            .flex_none()
                            .mt(px(2.))
                            .size(theme::u(16.))
                            .text_color(c(if accent {
                                theme::ACCENT_TEXT
                            } else {
                                theme::FG_STRONG
                            })),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(tr(title)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(tr(desc)),
                            ),
                    )
            };
            col = col.child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap_2p5()
                    .child(
                        action_tile(
                            "mcbe-tile-deploy",
                            Ico::Upload,
                            "Deploy to Game",
                            "Copy packs into Minecraft's com.mojang folder",
                            true,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_deploy_all(cx))),
                    )
                    .child(
                        action_tile(
                            "mcbe-tile-check",
                            Ico::CircleCheck,
                            "Check Manifests",
                            "Validate JSON syntax, UUIDs, and dependencies",
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_check_all(cx))),
                    )
                    .child(
                        action_tile(
                            "mcbe-tile-pack",
                            Ico::Package,
                            "Export .mcaddon",
                            "Bundle packs into a ready-to-share package",
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_pack_all(cx))),
                    ),
            );
        }

        let card_body: AnyElement = match ws {
            None => div()
                .p_4()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(tr(
                    "Open a folder or world to inspect Behavior & Resource Packs.",
                ))
                .into_any_element(),
            Some(w) if w.packs.is_empty() => v_flex()
                .p_5()
                .gap_3()
                .items_start()
                .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr(
                    "No Behavior or Resource Packs found in this folder yet.",
                )))
                .child(
                    Button::new("mcbe-packs-empty-new")
                        .label(tr("Create New Pack"))
                        .small()
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mcbe.tab = McbeTab::Create;
                            cx.notify();
                        })),
                )
                .into_any_element(),
            Some(w) => {
                let mut list = v_flex().w_full();
                for (ix, info) in w.packs.iter().enumerate() {
                    let badge = short_pack_badge(&info.pack.kind);
                    let manifest_path: PathBuf = info.pack.dir.join("manifest.json");
                    let ver = format!(
                        "v{}.{}.{}",
                        info.version[0], info.version[1], info.version[2]
                    );
                    let meta_line = if let Some(entry) = &info.script_entry {
                        format!("{} · {entry}", info.rel_dir)
                    } else {
                        info.rel_dir.clone()
                    };
                    list = list.child(
                        h_flex()
                            .id(("mcbe-pg-pack", ix))
                            .px_4()
                            .py_2p5()
                            .gap_3()
                            .items_center()
                            .cursor_pointer()
                            .when(ix > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                            .hover(|h| h.bg(c(theme::BG_HOVER)))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.open_file(manifest_path.clone(), w, cx);
                            }))
                            .child(pack_icon_tile(&info.pack.kind, 28.))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(c(theme::FG_STRONG))
                                                    .child(info.pack.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .px_1p5()
                                                    .py(px(1.))
                                                    .rounded(px(4.))
                                                    .bg(c(theme::BG_SIDE))
                                                    .border_1()
                                                    .border_color(c(theme::BORDER))
                                                    .text_xs()
                                                    .text_color(c(theme::FG_MUTED))
                                                    .child(badge),
                                            )
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .text_xs()
                                                    .text_color(c(theme::FG_DIM))
                                                    .child(ver),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(c(theme::FG_DIM))
                                            .child(meta_line),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_none()
                                    .gap_1()
                                    .items_center()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(tr("Open manifest"))
                                    .child(Icon::new(Ico::ChevronRight).size(theme::u(12.))),
                            ),
                    );
                }
                list.into_any_element()
            }
        };

        col.child(
            div()
                .w_full()
                .rounded(px(8.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_ELEVATED))
                .overflow_hidden()
                .child(card_body),
        )
        .into_any_element()
    }

    fn render_mcbe_tab_sync(
        &self,
        ws: Option<&McbeWorkspace>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = v_flex().w_full().gap_4().child(page_intro(
            "Pack Links & Sync",
            "Minecraft only loads a Behavior Pack and Resource Pack together when their manifest UUIDs and versions match. This page checks those links and fixes mismatches in one click.",
        ));

        let Some(w) = ws else {
            return col
                .child(
                    div()
                        .p_4()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .bg(c(theme::BG_ELEVATED))
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(tr(
                            "Open a folder or world to inspect Behavior & Resource Packs.",
                        )),
                )
                .into_any_element();
        };

        if w.sync_items.is_empty() {
            return col
                .child(
                    v_flex()
                        .p_5()
                        .gap_3()
                        .items_start()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .bg(c(theme::BG_ELEVATED))
                        .child(
                            div()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(tr(
                                    "No Behavior and Resource Pack pairs found yet. Create a linked BP + RP pair in New Pack.",
                                )),
                        )
                        .child(
                            Button::new("mcbe-sync-empty-new")
                                .label(tr("Go to New Pack"))
                                .small()
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.mcbe.tab = McbeTab::Create;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element();
        }

        let issues = w
            .sync_items
            .iter()
            .filter(|s| s.kind != SyncKind::Synced)
            .count();
        let (banner_col, banner_ico, banner_txt): (theme::Tok, Ico, SharedString) = if issues == 0 {
            (
                theme::GREEN,
                Ico::CircleCheck,
                tr("All pack links match. Minecraft will load your Behavior and Resource Packs together.").into(),
            )
        } else if issues == 1 {
            (
                theme::WARN,
                Ico::TriangleAlert,
                crate::trf!("{n} link issue needs attention."; n => issues).into(),
            )
        } else {
            (
                theme::WARN,
                Ico::TriangleAlert,
                crate::trf!("{n} link issues need attention."; n => issues).into(),
            )
        };
        col = col.child(
            h_flex()
                .w_full()
                .px_3p5()
                .py_2p5()
                .gap_2p5()
                .items_center()
                .rounded(px(8.))
                .border_1()
                .border_color(c(banner_col).opacity(0.4))
                .bg(c(banner_col).opacity(0.08))
                .child(
                    Icon::new(banner_ico)
                        .flex_none()
                        .size(theme::u(14.))
                        .text_color(c(banner_col)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(theme::FG_STRONG))
                        .child(banner_txt),
                ),
        );

        let mut list = v_flex()
            .w_full()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .overflow_hidden();

        for (ix, item) in w.sync_items.iter().enumerate() {
            let (tint, ico) = match item.kind {
                SyncKind::Synced => (theme::GREEN, Ico::Check),
                SyncKind::MissingCounterpart | SyncKind::DuplicateUuid => {
                    (theme::DANGER, Ico::CircleAlert)
                }
                SyncKind::VersionMismatch
                | SyncKind::UnlinkedPair
                | SyncKind::WorldPackMismatch => (theme::WARN, Ico::TriangleAlert),
            };
            let btn_lbl = match item.kind {
                SyncKind::UnlinkedPair | SyncKind::MissingCounterpart => tr("Link"),
                SyncKind::DuplicateUuid => tr("Fix UUID"),
                _ => tr("Sync"),
            };
            list = list.child(
                h_flex()
                    .id(("mcbe-pg-sync", ix))
                    .px_4()
                    .py_3()
                    .gap_3()
                    .items_center()
                    .when(ix > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                    .child(
                        Icon::new(ico)
                            .flex_none()
                            .size(theme::u(15.))
                            .text_color(c(tint)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(c(theme::FG_STRONG))
                                    .child(clean_sync_title(&item.title)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(sync_item_help(item)),
                            ),
                    )
                    .child(if item.fixable {
                        let btn = Button::new(("mcbe-pg-fix", ix)).label(btn_lbl).small();
                        let btn = if item.kind == SyncKind::Synced {
                            btn.outline()
                        } else {
                            btn.primary()
                        };
                        btn.on_click(cx.listener(move |this, _, _, cx| {
                            this.mcbe_fix_sync(ix, cx);
                        }))
                        .into_any_element()
                    } else {
                        div()
                            .flex_none()
                            .px_2()
                            .py(px(2.))
                            .rounded(px(4.))
                            .bg(c(theme::GREEN).opacity(0.12))
                            .text_xs()
                            .text_color(c(theme::GREEN))
                            .child(tr("Synced"))
                            .into_any_element()
                    }),
            );
        }

        col.child(list).into_any_element()
    }

    fn render_mcbe_tab_api(
        &self,
        ws: Option<&McbeWorkspace>,
        m: &McbeState,
        ch: McbeChannel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = v_flex().w_full().gap_4().child(page_intro(
            "Script API & Code Autocomplete",
            "Add official @minecraft/* modules to your Behavior Pack manifest, or download autocomplete types without creating a node_modules folder.",
        ));

        let cache_kb = mcbe_types::cache_total_bytes() / 1024;
        let cache_label: SharedString = if cache_kb == 0 {
            tr("Zero node_modules").into()
        } else {
            crate::trf!("Type cache: {kb} KB · 0 MB in project"; kb => cache_kb).into()
        };
        let ch_name = if ch == McbeChannel::Official {
            tr("Official")
        } else {
            tr("Preview")
        };

        col = col.child(
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(format!("{ch_name} · {cache_label}")),
                )
                .child(
                    Button::new("mcbe-pg-tags-refresh")
                        .icon(Icon::new(Ico::RefreshCw))
                        .label(tr("Check Updates"))
                        .xsmall()
                        .ghost()
                        .loading(m.refreshing_tags)
                        .on_click(cx.listener(|this, _, _, cx| this.mcbe_refresh_dist_tags(cx))),
                ),
        );

        let used_pkgs: std::collections::HashSet<&str> = ws
            .map(|w| {
                w.packs
                    .iter()
                    .flat_map(|p| p.module_deps.iter().map(|d| d.module_name.as_str()))
                    .collect()
            })
            .unwrap_or_default();

        let mut list = v_flex()
            .w_full()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .overflow_hidden();

        for (ix, pkg) in mcbe_types::catalog().into_iter().enumerate() {
            let npm_ver = pkg.npm_for(ch).to_string();
            let man_ver = pkg.manifest_for(ch).to_string();
            let cached = mcbe_types::is_cached(pkg.name, &npm_ver);
            let in_manifest = used_pkgs.contains(pkg.name);
            let busy = m.busy_pkg.as_deref() == Some(pkg.name);
            let pkg_name = pkg.name;
            let man_v_click = man_ver.clone();
            let npm_v_click = npm_ver.clone();

            list = list.child(
                h_flex()
                    .id(("mcbe-pg-pkg", ix))
                    .px_4()
                    .py_3()
                    .gap_3()
                    .items_center()
                    .when(ix > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(c(theme::FG_STRONG))
                                            .child(pkg.name),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(c(theme::FG_DIM))
                                            .child(format_api_version(&man_ver, &npm_ver)),
                                    )
                                    .when(in_manifest, |h| {
                                        h.child(
                                            div()
                                                .flex_none()
                                                .px_1p5()
                                                .py(px(1.))
                                                .rounded(px(4.))
                                                .bg(c(theme::ACCENT).opacity(0.14))
                                                .text_xs()
                                                .text_color(c(theme::ACCENT_TEXT))
                                                .child(tr("In manifest")),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(pkg_friendly_summary(pkg.name, pkg.summary)),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1p5()
                            .items_center()
                            .child(
                                Button::new(("mcbe-pg-pkg-add", ix))
                                    .label(if in_manifest {
                                        tr("Update Manifest")
                                    } else {
                                        tr("+ Manifest")
                                    })
                                    .xsmall()
                                    .outline()
                                    .tooltip(tr(
                                        "Add or update this module in your Behavior Pack manifest.json",
                                    ))
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        this.mcbe_add_script_dep(
                                            pkg_name,
                                            man_v_click.clone(),
                                            w,
                                            cx,
                                        )
                                    })),
                            )
                            .child(if cached {
                                h_flex()
                                    .flex_none()
                                    .px_2()
                                    .py(px(3.))
                                    .gap_1()
                                    .items_center()
                                    .rounded(px(4.))
                                    .bg(c(theme::GREEN).opacity(0.12))
                                    .text_xs()
                                    .text_color(c(theme::GREEN))
                                    .child(Icon::new(Ico::Check).size(theme::u(11.)))
                                    .child(tr("Types ready"))
                                    .into_any_element()
                            } else {
                                Button::new(("mcbe-pg-pkg-dts", ix))
                                    .label(tr("Get Types"))
                                    .xsmall()
                                    .ghost()
                                    .loading(busy)
                                    .tooltip(tr(
                                        "Download autocomplete definitions into FluxCode's global cache",
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mcbe_hydrate_pkg(pkg_name, npm_v_click.clone(), cx)
                                    }))
                                    .into_any_element()
                            }),
                    ),
            );
        }

        col.child(list).into_any_element()
    }

    fn render_mcbe_tab_create(
        &self,
        m: &McbeState,
        ch: McbeChannel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.entity().downgrade();
        let gen_mode_seg = crate::ui::SegmentedControl::new(
            "mcbe-pg-gen-mode",
            [
                (GenMode::BothSynced, "mcbe-pgm-both", tr("BP + RP (Linked)")),
                (GenMode::BehaviorOnly, "mcbe-pgm-bp", tr("Behavior Pack")),
                (GenMode::ResourceOnly, "mcbe-pgm-rp", tr("Resource Pack")),
            ]
            .into_iter()
            .map(|(mode, id, lbl)| {
                let w = weak.clone();
                crate::ui::SegItem::new(id, lbl, m.gen_mode == mode, move |_, _, cx| {
                    _ = w.update(cx, |this, cx| {
                        this.mcbe.gen_mode = mode;
                        cx.notify();
                    });
                })
            }),
        );
        let mode_help = match m.gen_mode {
            GenMode::BothSynced => {
                tr("Generates linked Behavior and Resource folders with matching UUIDs.")
            }
            GenMode::BehaviorOnly => {
                tr("Generates a Behavior Pack for scripts, entities, recipes, or loot tables.")
            }
            GenMode::ResourceOnly => {
                tr("Generates a Resource Pack for textures, models, UI, or sounds.")
            }
        };
        let eng = m.engine_for_channel();
        let eng_ver = format!("{}.{}.{}", eng[0], eng[1], eng[2]);
        let ch_name = if ch == McbeChannel::Official {
            tr("Official")
        } else {
            tr("Preview")
        };

        let form = v_flex()
            .w_full()
            .p_4()
            .gap_4()
            .rounded(px(8.))
            .border_1()
            .border_color(c(theme::BORDER))
            .bg(c(theme::BG_ELEVATED))
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(theme::FG_STRONG))
                            .child(tr("Pack name")),
                    )
                    .child(crate::ui::McInput::new(&m.gen_name).small()),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(theme::FG_STRONG))
                            .child(tr("Description (optional)")),
                    )
                    .child(crate::ui::McInput::new(&m.gen_desc).small()),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(c(theme::FG_STRONG))
                            .child(tr("Pack type")),
                    )
                    .child(h_flex().w_full().flex_wrap().child(gen_mode_seg))
                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(mode_help)),
            )
            .when(m.gen_mode != GenMode::ResourceOnly, |v| {
                v.child(
                    v_flex()
                        .gap_2p5()
                        .pt_1()
                        .child(
                            h_flex()
                                .gap_2p5()
                                .items_start()
                                .child(
                                    crate::ui::McToggle::new("mcbe-pg-script")
                                        .small()
                                        .checked(m.gen_script)
                                        .on_click(cx.listener(|this, v: &bool, _, cx| {
                                            this.mcbe.gen_script = *v;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(c(theme::FG_STRONG))
                                                .child(tr("Include Script API (JavaScript)")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(c(theme::FG_DIM))
                                                .child(tr(
                                                    "Adds @minecraft/server and a starter scripts/main.js file.",
                                                )),
                                        ),
                                ),
                        )
                        .when(m.gen_script, |inner| {
                            inner.child(
                                h_flex()
                                    .gap_2p5()
                                    .items_start()
                                    .child(
                                        crate::ui::McToggle::new("mcbe-pg-ui")
                                            .small()
                                            .checked(m.gen_ui_dep)
                                            .on_click(cx.listener(|this, v: &bool, _, cx| {
                                                this.mcbe.gen_ui_dep = *v;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(c(theme::FG_STRONG))
                                                    .child(tr(
                                                        "Include UI Forms (@minecraft/server-ui)",
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(c(theme::FG_DIM))
                                                    .child(tr(
                                                        "Adds ActionFormData and ModalFormData support.",
                                                    )),
                                            ),
                                    ),
                            )
                        }),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .pt_3()
                    .border_t_1()
                    .border_color(c(theme::BORDER))
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(crate::trf!(
                                "Target engine: {ver} ({ch})";
                                ver => eng_ver,
                                ch => ch_name
                            )),
                    )
                    .child(
                        Button::new("mcbe-pg-gen-go")
                            .label(tr("Create Pack"))
                            .small()
                            .primary()
                            .on_click(cx.listener(|this, _, w, cx| this.mcbe_generate(w, cx))),
                    ),
            );

        v_flex()
            .w_full()
            .gap_4()
            .child(page_intro(
                "Create New Pack",
                "Generate clean manifest.json files with fresh UUIDs and the right engine version for your selected Minecraft channel.",
            ))
            .child(form)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_sync_title, format_api_version, short_pack_badge, Kind, McbeTab};

    #[test]
    fn clean_sync_title_and_api_version_are_concise() {
        assert_eq!(
            clean_sync_title("KiwUI [BP] <-> KiwUI [RP]"),
            "KiwUI [BP] ↔ KiwUI [RP]"
        );
        assert_eq!(clean_sync_title("A -> B"), "A → B");
        assert_eq!(format_api_version("2.11.0", "2.11.0"), "v2.11.0");
        assert_eq!(
            format_api_version("2.12.0-beta", "2.12.0-rc.1"),
            "v2.12.0-beta (npm 2.12.0-rc.1)"
        );
        assert_eq!(short_pack_badge(&Kind::Behavior), "BP");
        assert_eq!(short_pack_badge(&Kind::Resource), "RP");
        assert_eq!(McbeTab::default(), McbeTab::Packs);
    }
}
