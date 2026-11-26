use super::bedrock::{
    self, analyze_workspace, deploy_packs, generate_manifests,
    regenerate_duplicate_uuids, sync_pair, sync_world_packs, upsert_script_dep, validate, GenMode,
    Kind, McbeChannel, McbeWorkspace, SyncKind,
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

pub struct McbeState {
    pub page: bool,
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
            workspace: None,
            installed,
            channel,
            gen_open: false,
            gen_mode: GenMode::BothSynced,
            gen_script: true,
            gen_ui_dep: false,
            gen_name: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Pack name"))),
            gen_desc: cx.new(|cx| InputState::new(window, cx).placeholder(tr("Description (optional)"))),
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
        .when(active, |d| d.bg(c(theme::ACCENT).opacity(0.18)).text_color(c(theme::FG_STRONG)))
        .when(!active, |d| d.text_color(c(theme::FG_MUTED)).hover(|h| h.bg(c(theme::BG_HOVER)).text_color(c(theme::FG))))
        .text_xs()
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
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
        .when(active, |d| d.bg(c(theme::ACCENT).opacity(0.15)).text_color(c(theme::FG_STRONG)))
        .when(!active, |d| d.text_color(c(theme::FG_MUTED)).hover(|h| h.bg(c(theme::BG_HOVER))))
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
        .child(Icon::new(ico).size(theme::u(sz * 0.52)).text_color(c(col_k)))
}

fn mcbe_card(title: SharedString, right: Option<AnyElement>, body: impl IntoElement) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .rounded(px(8.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .overflow_hidden()
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .px_4()
                .py_2p5()
                .gap_2()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_SIDE).opacity(0.5))
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(title))
                .children(right),
        )
        .child(body)
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
                    versions.entry(dep.module_name.clone()).or_insert_with(|| dep.version.clone());
                }
            }
        }
        mcbe_types::set_workspace_context(self.has_mcbe_workspace(), self.mcbe.channel, versions);
    }

    pub(crate) fn mcbe_refresh(&mut self, cx: &mut Context<Self>) {
        let roots = self.roots();
        cx.spawn(async move |this, cx| {
            let ws = cx.background_spawn(async move { analyze_workspace(&roots) }).await;
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
        let Some(ws) = self.mcbe.workspace.as_ref() else { return };
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
        let Some(ws) = self.mcbe.workspace.as_ref() else { return };
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
                    .map(|(d, m)| format!("{}: {m}", d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
        });
    }

    fn mcbe_pack_all(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = self.mcbe.workspace.as_ref() else { return };
        let proj = super::Project {
            ext: "bedrock".into(),
            dir: ws.root.clone(),
            name: ws
                .root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "addon".into()),
        };
        self.ext_background(proj.name.clone(), tr("Pack the add-on").into(), cx, move || {
            bedrock::pack(&proj)
        });
    }

    fn mcbe_fix_sync(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(ws) = self.mcbe.workspace.as_ref() else { return };
        let Some(item) = ws.sync_items.get(ix).cloned() else { return };
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
                let bp = item.bp_dir.or_else(|| packs.iter().find(|p| p.pack.kind == Kind::Behavior).map(|p| p.pack.dir.clone()));
                let rp = item.rp_dir.or_else(|| packs.iter().find(|p| p.pack.kind == Kind::Resource).map(|p| p.pack.dir.clone()));
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

    fn mcbe_add_script_dep(&mut self, pkg: &'static str, ver: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bp) = self
            .mcbe
            .workspace
            .as_ref()
            .and_then(|w| w.packs.iter().find(|p| p.pack.kind == Kind::Behavior))
            .map(|p| p.pack.dir.clone())
        else {
            self.mcbe.note = Some(tr("Need a Behavior Pack first (generate one below)").to_string());
            self.mcbe.gen_open = true;
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
        let Some(root) = self.mcbe.workspace.as_ref().map(|w| w.root.clone()).or_else(|| self.root.clone()) else {
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
                self.mcbe.gen_name.update(cx, |s, cx| s.set_value("", window, cx));
                self.mcbe.gen_desc.update(cx, |s, cx| s.set_value("", window, cx));
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
                                img(crate::fluxicon::MCBE_COLOR.0)
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
                    ),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        channel_pill("mcbe-ch-off", "Official", ch == McbeChannel::Official)
                            .on_click(cx.listener(|this, _, _, cx| this.mcbe_set_channel(McbeChannel::Official, cx))),
                    )
                    .child(
                        channel_pill("mcbe-ch-prev", "Preview", ch == McbeChannel::Preview)
                            .on_click(cx.listener(|this, _, _, cx| this.mcbe_set_channel(McbeChannel::Preview, cx))),
                    ),
            );
        col = col.child(header_card);

        // 2. Quick Action Strip (Deploy / Check / Pack)
        if ws.is_some_and(|w| !w.packs.is_empty()) {
            let action_btn = |id: &'static str, ico: Ico, label: &'static str, tip: &'static str, primary: bool| {
                let b = Button::new(id)
                    .icon(Icon::new(ico))
                    .label(tr(label))
                    .xsmall()
                    .flex_1()
                    .tooltip(tr(tip));
                if primary { b.primary() } else { b.outline() }
            };
            col = col.child(
                h_flex()
                    .w_full()
                    .gap_1p5()
                    .child(action_btn("mcbe-deploy", Ico::Upload, "Deploy", "Deploy to active channel's com.mojang folder", true).on_click(cx.listener(|this, _, _, cx| this.mcbe_deploy_all(cx))))
                    .child(action_btn("mcbe-check", Ico::CircleCheck, "Check", "Validate all manifests and UUIDs", false).on_click(cx.listener(|this, _, _, cx| this.mcbe_check_all(cx))))
                    .child(action_btn("mcbe-pack", Ico::Package, "Pack", "Bundle into .mcaddon / .mcpack", false).on_click(cx.listener(|this, _, _, cx| this.mcbe_pack_all(cx)))),
            );
        }

        if let Some(note) = &m.note {
            col = col.child(
                div()
                    .px_2p5()
                    .py_1p5()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::ACCENT).opacity(0.4))
                    .bg(c(theme::ACCENT).opacity(0.1))
                    .text_xs()
                    .text_color(c(theme::FG_STRONG))
                    .child(note.clone()),
            );
        }

        // 3. Packs & Sync Diagnostics
        let mut packs_sec = v_flex().w_full().gap_1p5();
        let issues = ws.map(|w| w.sync_items.iter().filter(|s| s.kind != SyncKind::Synced).count()).unwrap_or(0);
        let badge_txt: SharedString = if issues == 0 {
            tr("All synced").into()
        } else if issues == 1 {
            crate::trf!("{n} issue"; n => issues).into()
        } else {
            crate::trf!("{n} issues"; n => issues).into()
        };
        let badge_col = if issues == 0 { theme::GREEN } else { theme::WARN };
        packs_sec = packs_sec.child(
            h_flex()
                .px_0p5()
                .items_center()
                .justify_between()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr("PACKS & SYNC")))
                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(badge_col)).child(badge_txt)),
        );

        if let Some(w) = ws {
            for (ix, item) in w.sync_items.iter().enumerate() {
                let (tint, ico) = match item.kind {
                    SyncKind::Synced => (theme::GREEN, Ico::Check),
                    SyncKind::MissingCounterpart | SyncKind::DuplicateUuid => (theme::DANGER, Ico::CircleAlert),
                    SyncKind::VersionMismatch | SyncKind::UnlinkedPair | SyncKind::WorldPackMismatch => (theme::WARN, Ico::TriangleAlert),
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
                        .border_color(c(if item.kind == SyncKind::Synced { theme::BORDER } else { tint }).opacity(0.45))
                        .bg(c(theme::BG_ELEVATED))
                        .child(Icon::new(ico).flex_none().size(theme::u(13.)).text_color(c(tint)))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(item.title.clone()))
                                .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(item.detail.clone())),
                        )
                        .when(item.fixable, |h| {
                            h.child(
                                Button::new(("mcbe-fix", ix))
                                    .label(btn_lbl)
                                    .xsmall()
                                    .outline()
                                    .on_click(cx.listener(move |this, _, _, cx| this.mcbe_fix_sync(ix, cx))),
                            )
                        }),
                );
            }

            if w.packs.is_empty() {
                packs_sec = packs_sec.child(div().px_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("No packs detected yet. Generate BP/RP below.")));
            } else {
                let mut list = v_flex().rounded(px(6.)).border_1().border_color(c(theme::BORDER)).overflow_hidden();
                for (ix, info) in w.packs.iter().enumerate() {
                    let (_, kind_label, _) = pack_icon_spec(&info.pack.kind);
                    let manifest_path: PathBuf = info.pack.dir.join("manifest.json");
                    let ver = format!("v{}.{}.{}", info.version[0], info.version[1], info.version[2]);
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
                                    .child(div().truncate().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(info.pack.name.clone()))
                                    .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(format!("{} · {sub_line}", tr(kind_label)))),
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
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr("SCRIPT API & NPM TYPES")))
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(div().text_xs().text_color(c(theme::FG_DIM)).child(cache_label))
                        .child(
                            Button::new("mcbe-tags-refresh")
                                .icon(Icon::new(Ico::RefreshCw))
                                .xsmall()
                                .ghost()
                                .loading(m.refreshing_tags)
                                .tooltip(tr("Refresh"))
                                .on_click(cx.listener(|this, _, _, cx| this.mcbe_refresh_dist_tags(cx))),
                        ),
                ),
        );
        let mut api_list = v_flex().rounded(px(6.)).border_1().border_color(c(theme::BORDER)).overflow_hidden();
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
                            .child(div().truncate().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(pkg.name))
                            .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(format!("{man_ver} · npm {npm_ver}"))),
                    )
                    .child(
                        Button::new(("mcbe-pkg-add", ix))
                            .label(tr("+ Manifest"))
                            .xsmall()
                            .outline()
                            .tooltip(pkg.summary)
                            .on_click(cx.listener(move |this, _, w, cx| this.mcbe_add_script_dep(pkg_name, man_v_click.clone(), w, cx))),
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
                            .on_click(cx.listener(move |this, _, _, cx| this.mcbe_hydrate_pkg(pkg_name, npm_v_click.clone(), cx)))
                            .into_any_element()
                    }),
            );
        }
        api_sec = api_sec.child(api_list);
        col = col.child(api_sec);

        // 5. BP / RP Manifest Generator
        let gen_toggle_lbl = if m.gen_open { tr("Hide") } else { tr("New Pack / Manifest") };
        let mut gen_sec = v_flex().w_full().gap_1p5().child(
            h_flex()
                .px_0p5()
                .items_center()
                .justify_between()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_MUTED)).child(tr("MANIFEST GENERATOR")))
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
                                option_chip("mcbe-gm-both", tr("BP + RP").into(), m.gen_mode == GenMode::BothSynced)
                                    .on_click(cx.listener(|this, _, _, cx| { this.mcbe.gen_mode = GenMode::BothSynced; cx.notify(); })),
                            )
                            .child(
                                option_chip("mcbe-gm-bp", tr("BP only").into(), m.gen_mode == GenMode::BehaviorOnly)
                                    .on_click(cx.listener(|this, _, _, cx| { this.mcbe.gen_mode = GenMode::BehaviorOnly; cx.notify(); })),
                            )
                            .child(
                                option_chip("mcbe-gm-rp", tr("RP only").into(), m.gen_mode == GenMode::ResourceOnly)
                                    .on_click(cx.listener(|this, _, _, cx| { this.mcbe.gen_mode = GenMode::ResourceOnly; cx.notify(); })),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                option_chip("mcbe-g-script", tr("Script API").into(), m.gen_script)
                                    .on_click(cx.listener(|this, _, _, cx| { this.mcbe.gen_script = !this.mcbe.gen_script; cx.notify(); })),
                            )
                            .when(m.gen_script, |h| {
                                h.child(
                                    option_chip("mcbe-g-ui", tr("Add @minecraft/server-ui").into(), m.gen_ui_dep)
                                        .on_click(cx.listener(|this, _, _, cx| { this.mcbe.gen_ui_dep = !this.mcbe.gen_ui_dep; cx.notify(); })),
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

    pub(crate) fn render_mcbe_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.mcbe;
        let ws = m.workspace.as_ref();
        let ch = m.channel;
        let vp_w = f32::from(window.viewport_size().width);
        let two_col = vp_w >= 920.;
        let weak = cx.entity().downgrade();

        let base_installed = match (ch, &m.installed.official, &m.installed.preview) {
            (McbeChannel::Official, Some(v), _) => crate::trf!("Minecraft Bedrock {v} detected"; v => v),
            (McbeChannel::Preview, _, Some(v)) => crate::trf!("Minecraft Bedrock Preview {v} detected"; v => v),
            _ => tr("Minecraft Bedrock not detected on PC (using default engine versions)").to_string(),
        };
        let installed_line: SharedString = if ws.is_some_and(|w| w.is_world_folder) {
            format!("{base_installed} · World Folder").into()
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

        let has_packs = ws.is_some_and(|w| !w.packs.is_empty());
        let header = h_flex()
            .w_full()
            .flex_wrap()
            .gap_3()
            .pb_4()
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
                            .size(theme::u(40.))
                            .rounded(px(9.))
                            .bg(c(theme::BG_ELEVATED))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(img(crate::fluxicon::MCBE_COLOR.0).flex_none().size(theme::u(28.)).object_fit(ObjectFit::Contain)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Minecraft Bedrock Studio")))
                            .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(installed_line)),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(ch_seg)
                    .when(has_packs, |h| {
                        h.child(
                            Button::new("mcbe-pg-deploy")
                                .icon(Icon::new(Ico::Upload))
                                .label(tr("Deploy"))
                                .small()
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| this.mcbe_deploy_all(cx))),
                        )
                        .child(
                            Button::new("mcbe-pg-check")
                                .icon(Icon::new(Ico::CircleCheck))
                                .label(tr("Check"))
                                .small()
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| this.mcbe_check_all(cx))),
                        )
                        .child(
                            Button::new("mcbe-pg-pack")
                                .icon(Icon::new(Ico::Package))
                                .label(tr("Pack"))
                                .small()
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| this.mcbe_pack_all(cx))),
                        )
                    })
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

        // Packs & Sync Card
        let issues = ws.map(|w| w.sync_items.iter().filter(|s| s.kind != SyncKind::Synced).count()).unwrap_or(0);
        let sync_badge_txt: SharedString = if issues == 0 {
            tr("All synced").into()
        } else if issues == 1 {
            crate::trf!("{n} issue"; n => issues).into()
        } else {
            crate::trf!("{n} issues"; n => issues).into()
        };
        let sync_badge = div()
            .px_2()
            .py(px(2.))
            .rounded(px(4.))
            .bg(c(if issues == 0 { theme::GREEN } else { theme::WARN }).opacity(0.14))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(if issues == 0 { theme::GREEN } else { theme::WARN }))
            .child(sync_badge_txt)
            .into_any_element();

        let mut packs_body = v_flex().w_full();
        if let Some(w) = ws {
            if !w.sync_items.is_empty() {
                let mut sync_list = v_flex().w_full().p_3().gap_2();
                for (ix, item) in w.sync_items.iter().enumerate() {
                    let (tint, ico) = match item.kind {
                        SyncKind::Synced => (theme::GREEN, Ico::Check),
                        SyncKind::MissingCounterpart | SyncKind::DuplicateUuid => (theme::DANGER, Ico::CircleAlert),
                        SyncKind::VersionMismatch | SyncKind::UnlinkedPair | SyncKind::WorldPackMismatch => (theme::WARN, Ico::TriangleAlert),
                    };
                    let btn_lbl = match item.kind {
                        SyncKind::UnlinkedPair | SyncKind::MissingCounterpart => tr("Link"),
                        SyncKind::DuplicateUuid => tr("Fix"),
                        _ => tr("Sync"),
                    };
                    sync_list = sync_list.child(
                        h_flex()
                            .id(("mcbe-pg-sync", ix))
                            .p_2p5()
                            .gap_2p5()
                            .items_center()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(if item.kind == SyncKind::Synced { theme::BORDER } else { tint }).opacity(0.45))
                            .bg(c(theme::BG_SIDE).opacity(0.6))
                            .child(Icon::new(ico).flex_none().size(theme::u(14.)).text_color(c(tint)))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(item.title.clone()))
                                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(item.detail.clone())),
                            )
                            .when(item.fixable, |h| {
                                h.child(
                                    Button::new(("mcbe-pg-fix", ix))
                                        .label(btn_lbl)
                                        .xsmall()
                                        .outline()
                                        .on_click(cx.listener(move |this, _, _, cx| this.mcbe_fix_sync(ix, cx))),
                                )
                            }),
                    );
                }
                packs_body = packs_body.child(sync_list);
            }

            if w.packs.is_empty() {
                packs_body = packs_body.child(
                    div()
                        .p_4()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(tr("No packs detected yet. Use the Manifest Generator below to scaffold a BP/RP pair.")),
                );
            } else {
                for (ix, info) in w.packs.iter().enumerate() {
                    let (_, kind_label, _) = pack_icon_spec(&info.pack.kind);
                    let manifest_path: PathBuf = info.pack.dir.join("manifest.json");
                    let ver = format!("v{}.{}.{}", info.version[0], info.version[1], info.version[2]);
                    let meta_line = if let Some(entry) = &info.script_entry {
                        format!("{} · {} · {ver} · {entry}", tr(kind_label), info.rel_dir)
                    } else {
                        format!("{} · {} · {ver}", tr(kind_label), info.rel_dir)
                    };
                    packs_body = packs_body.child(
                        h_flex()
                            .id(("mcbe-pg-pack", ix))
                            .px_4()
                            .py_2p5()
                            .gap_3()
                            .items_center()
                            .cursor_pointer()
                            .border_t_1()
                            .border_color(c(theme::BORDER))
                            .hover(|h| h.bg(c(theme::BG_HOVER)))
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.open_file(manifest_path.clone(), w, cx);
                            }))
                            .child(pack_icon_tile(&info.pack.kind, 30.))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(info.pack.name.clone()))
                                    .child(div().text_xs().text_color(c(theme::FG_DIM)).child(meta_line)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child("manifest.json"),
                            ),
                    );
                }
            }
        } else {
            packs_body = packs_body.child(
                div()
                    .p_4()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(tr("Open a folder or world to inspect Behavior & Resource Packs.")),
            );
        }
        let packs_card = mcbe_card(tr("PACKS & SYNC").into(), Some(sync_badge), packs_body);

        // Manifest Generator Card
        let gen_mode_seg = crate::ui::SegmentedControl::new(
            "mcbe-pg-gen-mode",
            [
                (GenMode::BothSynced, "mcbe-pgm-both", tr("BP + RP (Synced)")),
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
        let gen_body = v_flex()
            .p_4()
            .gap_3()
            .child(crate::ui::McInput::new(&m.gen_name).small())
            .child(crate::ui::McInput::new(&m.gen_desc).small())
            .child(h_flex().w_full().flex_wrap().gap_2().items_center().child(gen_mode_seg))
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap_4()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                crate::ui::McToggle::new("mcbe-pg-script")
                                    .small()
                                    .checked(m.gen_script)
                                    .on_click(cx.listener(|this, v: &bool, _, cx| {
                                        this.mcbe.gen_script = *v;
                                        cx.notify();
                                    })),
                            )
                            .child(div().text_xs().text_color(c(theme::FG)).child(tr("Include Script API"))),
                    )
                    .when(m.gen_script, |h| {
                        h.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    crate::ui::McToggle::new("mcbe-pg-ui")
                                        .small()
                                        .checked(m.gen_ui_dep)
                                        .on_click(cx.listener(|this, v: &bool, _, cx| {
                                            this.mcbe.gen_ui_dep = *v;
                                            cx.notify();
                                        })),
                                )
                                .child(div().text_xs().text_color(c(theme::FG)).child(tr("Add @minecraft/server-ui"))),
                        )
                    }),
            )
            .child(
                h_flex().justify_end().child(
                    Button::new("mcbe-pg-gen-go")
                        .label(tr("Generate Manifests"))
                        .small()
                        .primary()
                        .on_click(cx.listener(|this, _, w, cx| this.mcbe_generate(w, cx))),
                ),
            );
        let gen_card = mcbe_card(tr("MANIFEST GENERATOR").into(), None, gen_body);

        // Script API & NPM Types Card
        let cache_kb = mcbe_types::cache_total_bytes() / 1024;
        let cache_label: SharedString = if cache_kb == 0 {
            tr("Zero node_modules").into()
        } else {
            crate::trf!("Cache: {kb} KB"; kb => cache_kb).into()
        };
        let api_right = h_flex()
            .flex_none()
            .gap_2()
            .items_center()
            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(cache_label))
            .child(
                Button::new("mcbe-pg-tags-refresh")
                    .icon(Icon::new(Ico::RefreshCw))
                    .xsmall()
                    .ghost()
                    .loading(m.refreshing_tags)
                    .tooltip(tr("Refresh"))
                    .on_click(cx.listener(|this, _, _, cx| this.mcbe_refresh_dist_tags(cx))),
            )
            .into_any_element();

        let mut api_body = v_flex().w_full();
        for (ix, pkg) in mcbe_types::catalog().into_iter().enumerate() {
            let npm_ver = pkg.npm_for(ch).to_string();
            let man_ver = pkg.manifest_for(ch).to_string();
            let cached = mcbe_types::is_cached(pkg.name, &npm_ver);
            let busy = m.busy_pkg.as_deref() == Some(pkg.name);
            let pkg_name = pkg.name;
            let man_v_click = man_ver.clone();
            let npm_v_click = npm_ver.clone();
            api_body = api_body.child(
                h_flex()
                    .id(("mcbe-pg-pkg", ix))
                    .px_4()
                    .py_2p5()
                    .gap_3()
                    .items_center()
                    .when(ix > 0, |h| h.border_t_1().border_color(c(theme::BORDER)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(pkg.name))
                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(format!("manifest {man_ver} · npm {npm_ver} · {}", pkg.summary))),
                    )
                    .child(
                        Button::new(("mcbe-pg-pkg-add", ix))
                            .label(tr("+ Manifest"))
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(move |this, _, w, cx| this.mcbe_add_script_dep(pkg_name, man_v_click.clone(), w, cx))),
                    )
                    .child(if cached {
                        div()
                            .flex_none()
                            .px_2()
                            .py(px(3.))
                            .rounded(px(4.))
                            .bg(c(theme::GREEN).opacity(0.14))
                            .text_xs()
                            .text_color(c(theme::GREEN))
                            .child(tr("Cached"))
                            .into_any_element()
                    } else {
                        Button::new(("mcbe-pg-pkg-dts", ix))
                            .label(tr("Get types"))
                            .xsmall()
                            .ghost()
                            .loading(busy)
                            .on_click(cx.listener(move |this, _, _, cx| this.mcbe_hydrate_pkg(pkg_name, npm_v_click.clone(), cx)))
                            .into_any_element()
                    }),
            );
        }
        let api_card = mcbe_card(tr("SCRIPT API & NPM TYPES").into(), Some(api_right), api_body);

        let columns = if two_col {
            h_flex()
                .w_full()
                .gap_5()
                .items_start()
                .child(v_flex().flex_1().min_w_0().gap_5().child(packs_card).child(gen_card))
                .child(v_flex().flex_1().min_w_0().gap_5().child(api_card))
                .into_any_element()
        } else {
            v_flex()
                .w_full()
                .gap_5()
                .child(packs_card)
                .child(api_card)
                .child(gen_card)
                .into_any_element()
        };

        let container = v_flex()
            .w_full()
            .max_w(theme::u(1040.))
            .mx_auto()
            .gap_5()
            .child(header)
            .when_some(m.note.clone(), |v, note| {
                v.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(c(theme::ACCENT).opacity(0.4))
                        .bg(c(theme::ACCENT).opacity(0.1))
                        .text_xs()
                        .text_color(c(theme::FG_STRONG))
                        .child(note),
                )
            })
            .child(columns);

        v_flex()
            .id("mcbe-page")
            .key_context("McbePage")
            .size_full()
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .px_6()
            .py_6()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.mcbe.page = false;
                    cx.notify();
                }
            }))
            .child(container)
            .into_any_element()
    }
}
