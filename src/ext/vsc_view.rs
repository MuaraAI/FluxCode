use super::host::{ExtStatus, HostAction, HostEvent, TreeNode};
use super::native_webview::{NativeWebviewManager, NativeWebviewState, PhysicalRect};
use super::vsc_registry::{self, codicon_to_icon, ExtIcon, ViewType, VscSvgIcon, WhenContext};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::{SideView, Workbench};
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    spinner::Spinner,
    tooltip::Tooltip,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::Value;
use std::collections::HashMap;

pub fn render_ext_icon(icon: &ExtIcon, sz: Pixels) -> AnyElement {
    match icon {
        ExtIcon::SvgMonochrome(path) => Icon::new(VscSvgIcon(path.clone()))
            .size(sz)
            .into_any_element(),
        ExtIcon::SvgColor(path) => img(path.clone())
            .flex_none()
            .size(sz)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        ExtIcon::Raster(path) => img(path.clone())
            .flex_none()
            .size(sz)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        ExtIcon::Codicon(name) => Icon::new(codicon_to_icon(name)).size(sz).into_any_element(),
    }
}

impl Workbench {
    pub(crate) fn reconcile_ext_views(&mut self) {
        let containers = vsc_registry::activitybar_containers();
        let next_ids: Vec<String> = containers.into_iter().map(|c| c.id).collect();
        let prev_left = match self.view {
            SideView::Ext(ix) => self.ext_containers.get(ix as usize).cloned(),
            _ => None,
        };
        let prev_right = match self.right_view {
            SideView::Ext(ix) => self.ext_containers.get(ix as usize).cloned(),
            _ => None,
        };
        self.ext_containers = next_ids;
        if let Some(id) = prev_left {
            self.view = self
                .ext_containers
                .iter()
                .position(|c| c == &id)
                .map(|i| SideView::Ext(i as u16))
                .unwrap_or(SideView::Explorer);
        }
        if let Some(id) = prev_right {
            self.right_view = self
                .ext_containers
                .iter()
                .position(|c| c == &id)
                .map(|i| SideView::Ext(i as u16))
                .unwrap_or(SideView::Agent);
        }
    }

    pub(crate) fn ext_container_id(&self, view: SideView) -> Option<&str> {
        match view {
            SideView::Ext(ix) => self.ext_containers.get(ix as usize).map(String::as_str),
            _ => None,
        }
    }

    pub(crate) fn build_when_context<'a>(
        &'a self,
        view_id: Option<&'a str>,
        empty_cfg: &'a HashMap<String, Value>,
    ) -> WhenContext<'a> {
        let active_doc = self.active.and_then(|i| self.docs.get(i));
        let ext_name = active_doc
            .and_then(|d| d.path.extension())
            .and_then(|e| e.to_str());
        WhenContext {
            view: view_id,
            focused_view: view_id,
            editor_open: active_doc.is_some(),
            editor_focus: false,
            editor_has_selection: false,
            editor_readonly: active_doc.is_some_and(|d| d.read_only),
            terminal_focus: false,
            resource_scheme: active_doc.map(|_| "file"),
            resource_ext: ext_name,
            dynamic: &self.ext_host.context_keys,
            config: empty_cfg,
        }
    }

    pub(crate) fn activate_ext_container(&mut self, container_id: &str, cx: &mut Context<Self>) {
        self.ext_host
            .activated_containers
            .insert(container_id.to_string());
        let views = vsc_registry::views_for_container(container_id);
        let roots = self.roots();
        let dark = theme::palette().dark;
        for v in views {
            let status = self.ext_host.status_for(&v.ext_id);
            if status != ExtStatus::Active {
                if let Err(e) = self.ext_host.host.activate_extension(
                    &v.ext_id,
                    &format!("onView:{}", v.id),
                    &roots,
                    dark,
                ) {
                    self.ext_host
                        .ext_status
                        .insert(v.ext_id.clone(), ExtStatus::Error(e));
                    cx.notify();
                    continue;
                }
                self.ext_host
                    .ext_status
                    .entry(v.ext_id.clone())
                    .or_insert(ExtStatus::Activating);
            }
            if v.kind == ViewType::Webview {
                let already_resolved =
                    status == ExtStatus::Active && self.ext_host.webviews.contains_key(&v.id);
                if already_resolved {
                    continue;
                }
                match self
                    .ext_host
                    .host
                    .resolve_webview_view(&v.ext_id, &v.id, &roots, dark)
                {
                    Ok(url) => {
                        self.ext_host
                            .webviews
                            .entry(v.id.clone())
                            .or_insert_with(|| super::host::WebviewInfo {
                                handle: String::new(),
                                url,
                                title: v.name.clone(),
                                has_html: false,
                                html_rev: 0,
                            });
                    }
                    Err(e) => {
                        self.ext_host
                            .ext_status
                            .insert(v.ext_id.clone(), ExtStatus::Error(e));
                    }
                }
            } else {
                let st = self.ext_host.trees.entry(v.id.clone()).or_default();
                if st.roots.is_empty() || status != ExtStatus::Active {
                    st.loading = true;
                    self.ext_host.host.request_tree_children(&v.id, None);
                }
            }
        }
        self.ext_host
            .host
            .update_theme_css(super::webview_bridge::theme_css(theme::palette()));
        cx.notify();
    }

    pub(crate) fn reload_ext_container(&mut self, container_id: &str, cx: &mut Context<Self>) {
        let views = vsc_registry::views_for_container(container_id);
        for v in views {
            if v.kind == ViewType::Webview {
                if let Some(info) = self.ext_host.webviews.get(&v.id) {
                    if !info.handle.is_empty() {
                        self.native_webviews.reload(&info.handle);
                    }
                } else {
                    self.activate_ext_container(container_id, cx);
                    return;
                }
            } else {
                self.ext_host.host.request_tree_children(&v.id, None);
            }
        }
        cx.notify();
    }

    pub(crate) fn open_ext_container_by_id(&mut self, container_id: &str, cx: &mut Context<Self>) {
        let Some(ix) = self.ext_containers.iter().position(|c| c == container_id) else {
            return;
        };
        let view = SideView::Ext(ix as u16);
        self.reveal_view(view, cx);
        self.activate_ext_container(container_id, cx);
    }

    pub(crate) fn open_webview_url(&mut self, url: &str, title: &str, cx: &mut Context<Self>) {
        if let Some(browser) = crate::browser::chosen("auto") {
            let mut cmd = crate::proc::command(&browser.exe.to_string_lossy());
            cmd.arg(format!("--app={url}"));
            if cmd.spawn().is_ok() {
                self.status = crate::trf!("Opened {name}"; name => title).into();
                cx.notify();
                return;
            }
        }
        cx.open_url(url);
        self.status = crate::trf!("Opened {name}"; name => title).into();
        cx.notify();
    }

    pub(crate) fn execute_ext_command(
        &mut self,
        cmd_id: &str,
        args: Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(cid) = cmd_id.strip_prefix("workbench.view.extension.") {
            self.open_ext_container_by_id(cid, cx);
            return;
        }
        if let Some(vid) = cmd_id.strip_suffix(".focus") {
            for cid in &self.ext_containers.clone() {
                if vsc_registry::views_for_container(cid)
                    .iter()
                    .any(|v| v.id == vid)
                {
                    self.open_ext_container_by_id(cid, cx);
                    return;
                }
            }
        }
        let ext_id = vsc_registry::ext_for_command(cmd_id);
        let roots = self.roots();
        let dark = theme::palette().dark;
        if let Err(e) =
            self.ext_host
                .host
                .execute_command(ext_id.as_deref(), cmd_id, args, &roots, dark)
        {
            self.status = e.into();
        } else {
            let label = vsc_registry::command_by_id(cmd_id)
                .map(|c| c.display_label())
                .unwrap_or_else(|| cmd_id.to_string());
            self.status = crate::trf!("Ran {name}"; name => label).into();
        }
        let _ = window;
        cx.notify();
    }

    pub(crate) fn on_ext_host_event(
        &mut self,
        ev: HostEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(action) = self.ext_host.apply_event(ev) {
            match action {
                HostAction::OpenExternalUrl(url) => cx.open_url(&url),
                HostAction::OpenPanelUrl(url) => {
                    self.open_webview_url(&url, "Extension Panel", cx);
                }
                HostAction::OpenFile(path) => self.open_file(path, window, cx),
                HostAction::RunTerminal(cmd) => {
                    self.panel_tab = crate::workbench::PanelTab::Terminal;
                    self.run_in_terminal(&cmd, cx);
                }
                HostAction::FocusContainer(cid) => self.open_ext_container_by_id(&cid, cx),
                HostAction::WebviewHtmlUpdated { handle, rev } => {
                    if rev > 0 && !self.ext_host.host.has_sse_client(&handle) {
                        self.native_webviews.reload(&handle);
                    }
                }
            }
        }
        if let Some(note) = self.ext_host.host_notice.take() {
            self.status = note.into();
        }
        cx.notify();
    }

    pub(crate) fn render_vsc_header_actions(
        &self,
        container_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let views = vsc_registry::views_for_container(container_id);
        let empty_cfg = HashMap::new();
        let mut row = h_flex().flex_none().gap_0p5();
        let mut has_webview = false;
        let mut ext_browser: Option<(String, String)> = None;
        for v in &views {
            if v.kind == ViewType::Webview {
                has_webview = true;
                if ext_browser.is_none() {
                    if let Some(info) = self.ext_host.webviews.get(&v.id) {
                        ext_browser = Some((info.url.clone(), v.name.clone()));
                    }
                }
            }
            let ctx = self.build_when_context(Some(&v.id), &empty_cfg);
            for (idx, item) in vsc_registry::view_title_menus(&v.id, &ctx)
                .into_iter()
                .enumerate()
            {
                if item.is_submenu {
                    let sub_id = item.command_or_submenu.clone();
                    let sub_meta = vsc_registry::submenu(&sub_id);
                    let label = sub_meta
                        .as_ref()
                        .map(|s| s.label.clone())
                        .unwrap_or_else(|| tr("More Actions").to_string());
                    let sub_icon = match sub_meta.and_then(|s| s.icon) {
                        Some(ExtIcon::SvgMonochrome(p)) => Icon::new(VscSvgIcon(p)),
                        Some(ExtIcon::Codicon(n)) => Icon::new(codicon_to_icon(&n)),
                        _ => Icon::new(Ico::Ellipsis),
                    };
                    let cid_for_sub = container_id.to_string();
                    let open_here = self.ext_submenu_open.as_ref()
                        == Some(&(cid_for_sub.clone(), sub_id.clone()));
                    row = row.child(
                        div()
                            .id(("vsc-sub", idx))
                            .size(theme::u(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_color(c(if open_here {
                                theme::FG_STRONG
                            } else {
                                theme::FG_MUTED
                            }))
                            .when(open_here, |d| d.bg(c(theme::BG_HOVER)))
                            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                            .tooltip(move |w, cx| Tooltip::new(label.clone()).build(w, cx))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let key = (cid_for_sub.clone(), sub_id.clone());
                                if this.ext_submenu_open.as_ref() == Some(&key) {
                                    this.ext_submenu_open = None;
                                } else {
                                    this.ext_submenu_open = Some(key);
                                }
                                cx.notify();
                            }))
                            .child(sub_icon.size(theme::u(14.))),
                    );
                } else if let Some(cmd) = vsc_registry::command_by_id(&item.command_or_submenu) {
                    let icon_el = cmd
                        .icon
                        .as_ref()
                        .map(|ic| render_ext_icon(ic, theme::u(14.)))
                        .unwrap_or_else(|| {
                            Icon::new(Ico::Play).size(theme::u(13.)).into_any_element()
                        });
                    let cmd_id = cmd.id.clone();
                    let tip = cmd.display_label();
                    row = row.child(
                        div()
                            .id(("vsc-vt", idx))
                            .size(theme::u(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_color(c(theme::FG_MUTED))
                            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                            .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.execute_ext_command(&cmd_id, Vec::new(), window, cx);
                            }))
                            .child(icon_el),
                    );
                }
            }
        }
        if has_webview {
            let cid_rel = container_id.to_string();
            let reload_tip = tr("Reload View").to_string();
            row = row.child(
                div()
                    .id("vsc-wv-reload")
                    .size(theme::u(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(c(theme::FG_MUTED))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .tooltip(move |w, cx| Tooltip::new(reload_tip.clone()).build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.reload_ext_container(&cid_rel, cx);
                    }))
                    .child(Icon::new(gpui_kit::component::IconName::RefreshCw).size(theme::u(13.))),
            );
        }
        if let Some((url_tab, title_tab)) = ext_browser {
            let pop_tip = tr("Open in Browser").to_string();
            row = row.child(
                div()
                    .id("vsc-wv-popout")
                    .size(theme::u(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(c(theme::FG_MUTED))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .tooltip(move |w, cx| Tooltip::new(pop_tip.clone()).build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.open_url(&url_tab);
                        this.status = crate::trf!("Opened {name}"; name => title_tab).into();
                        cx.notify();
                    }))
                    .child(Icon::new(Ico::ExternalLink).size(theme::u(13.))),
            );
        }
        row.into_any_element()
    }

    fn submenu_entries(&self, sub_id: &str, view_id: &str) -> Vec<(String, String)> {
        let empty_cfg = HashMap::new();
        let ctx = self.build_when_context(Some(view_id), &empty_cfg);
        vsc_registry::submenu_items(sub_id, &ctx)
            .into_iter()
            .filter(|i| !i.is_submenu)
            .filter_map(|i| {
                vsc_registry::command_by_id(&i.command_or_submenu)
                    .map(|c| (c.id.clone(), c.display_label()))
            })
            .collect()
    }

    pub(crate) fn render_vsc_sidebar_view(
        &self,
        ix: u16,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(container_id) = self.ext_containers.get(ix as usize).cloned() else {
            return div()
                .p_3()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(tr("Extension view unavailable."))
                .into_any_element();
        };
        let Some(container) = vsc_registry::container_by_id(&container_id) else {
            return div().into_any_element();
        };
        let views = vsc_registry::views_for_container(&container_id);
        let empty_cfg = HashMap::new();
        let base_ctx = self.build_when_context(views.first().map(|v| v.id.as_str()), &empty_cfg);

        let ext_status = self.ext_host.status_for(&container.ext_id);
        let node_ok = super::host::ExtHost::node_available();
        let native_ok = NativeWebviewManager::is_available();
        let visible_views: Vec<_> = views
            .iter()
            .filter(|v| v.when.eval(&base_ctx))
            .collect();
        let has_webview = visible_views.iter().any(|v| v.kind == ViewType::Webview);

        let submenu_bar: Option<AnyElement> =
            if let Some((sub_cid, sub_id)) = &self.ext_submenu_open {
                if sub_cid == &container_id {
                    let entries = visible_views
                        .first()
                        .map(|v| self.submenu_entries(sub_id, &v.id))
                        .unwrap_or_default();
                    if entries.is_empty() {
                        None
                    } else {
                        let mut bar = h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_1()
                            .px_2p5()
                            .py_1p5()
                            .border_b_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_ELEVATED));
                        for (sidx, (cmd_id, title)) in entries.into_iter().enumerate() {
                            bar = bar.child(
                                Button::new(("vsc-sub-item", sidx))
                                    .label(title)
                                    .xsmall()
                                    .outline()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.ext_submenu_open = None;
                                        this.execute_ext_command(&cmd_id, Vec::new(), window, cx);
                                    })),
                            );
                        }
                        Some(bar.into_any_element())
                    }
                } else {
                    None
                }
            } else {
                None
            };

        if has_webview && native_ok {
            let show_status = !node_ok || ext_status != ExtStatus::Active;
            let status_banner = show_status.then(|| {
                div()
                    .w_full()
                    .px_2p5()
                    .py_1p5()
                    .border_b_1()
                    .border_color(c(theme::BORDER))
                    .child(self.render_vsc_status_row(
                        &container.ext_id,
                        &container_id,
                        &ext_status,
                        node_ok,
                        cx,
                    ))
            });

            let mut root = v_flex()
                .id(("vsc-panel", ix as usize))
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .children(submenu_bar)
                .children(status_banner);

            let tree_views: Vec<_> = visible_views
                .iter()
                .copied()
                .filter(|v| v.kind == ViewType::Tree)
                .collect();
            if !tree_views.is_empty() {
                let mut tree_col = v_flex()
                    .id(("vsc-tree-strip", ix as usize))
                    .w_full()
                    .max_h(theme::u(180.))
                    .overflow_y_scroll()
                    .px_2()
                    .py_1p5()
                    .gap_1()
                    .border_b_1()
                    .border_color(c(theme::BORDER));
                for tv in tree_views {
                    if let Some(tree) = self.ext_host.trees.get(&tv.id) {
                        if !tree.roots.is_empty() {
                            tree_col = tree_col.child(self.render_vsc_tree(&tv.id, tree, cx));
                        }
                    }
                }
                root = root.child(tree_col);
            }

            if let Some(wv_view) = visible_views.iter().find(|v| v.kind == ViewType::Webview) {
                let info_opt = self.ext_host.webviews.get(&wv_view.id);
                let frame_seq = self.wv_frame_seq;
                let wv_mgr = self.native_webviews.clone();
                let wv_state = info_opt
                    .map(|info| wv_mgr.state(&info.handle))
                    .unwrap_or(NativeWebviewState::Loading);

                let viewport = div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .when_some(info_opt, |box_el, info| {
                        let handle = info.handle.clone();
                        let url = info.url.clone();
                        let title = wv_view.name.clone();
                        let url_fallback = url.clone();
                        let wv_mgr_c = wv_mgr.clone();
                        let overlay_hint = if wv_state == NativeWebviewState::Failed {
                            Some(
                                v_flex()
                                    .size_full()
                                    .items_center()
                                    .justify_center()
                                    .p_4()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(c(theme::DANGER))
                                            .child(tr("Extension view failed to load.")),
                                    )
                                    .child(
                                        Button::new("vsc-native-fallback")
                                            .label(tr("Open in Browser"))
                                            .small()
                                            .primary()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.open_url(&url_fallback);
                                                this.status = crate::trf!(
                                                    "Opened {name}";
                                                    name => title
                                                )
                                                .into();
                                                cx.notify();
                                            })),
                                    )
                                    .into_any_element(),
                            )
                        } else if wv_state == NativeWebviewState::Loading || !info.has_html {
                            Some(
                                h_flex()
                                    .size_full()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(Spinner::new().icon(Ico::LoaderCircle).xsmall())
                                    .child(tr("Loading extension UI..."))
                                    .into_any_element(),
                            )
                        } else {
                            None
                        };
                        let canvas_el = canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let scale = window.scale_factor();
                                let rect = PhysicalRect::from_logical(
                                    f32::from(bounds.origin.x),
                                    f32::from(bounds.origin.y),
                                    f32::from(bounds.size.width),
                                    f32::from(bounds.size.height),
                                    scale,
                                );
                                wv_mgr_c.present_slot(frame_seq, &handle, &url, rect);
                            },
                        )
                        .size_full();
                        box_el
                            .children(overlay_hint)
                            .when(wv_state != NativeWebviewState::Failed, |b| {
                                b.child(canvas_el.absolute().inset_0())
                            })
                    })
                    .when(info_opt.is_none(), |box_el| {
                        box_el.child(
                            h_flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(Spinner::new().icon(Ico::LoaderCircle).xsmall())
                                .child(tr("Starting extension host...")),
                        )
                    });
                root = root.child(viewport);
            }

            let cmds = vsc_registry::commands_for_ext(&container.ext_id, &base_ctx);
            let logs_len = self
                .ext_host
                .output_logs
                .get(&container.ext_id)
                .map(|l| l.len())
                .unwrap_or(0);
            if !cmds.is_empty() || logs_len > 0 {
                let drawer_open = self.ext_drawer_open.contains(&container_id);
                let cid_drawer = container_id.clone();
                let mut footer = v_flex()
                    .flex_none()
                    .w_full()
                    .border_t_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_SIDE))
                    .child(
                        h_flex()
                            .id("vsc-drawer-toggle")
                            .w_full()
                            .h(theme::u(28.))
                            .px_2p5()
                            .gap_1p5()
                            .items_center()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(theme::FG_MUTED))
                            .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                            .child(
                                Icon::new(if drawer_open {
                                    Ico::ChevronDown
                                } else {
                                    Ico::ChevronRight
                                })
                                .size(theme::u(12.)),
                            )
                            .child(div().flex_1().child(tr("Commands & Output")))
                            .when(logs_len > 0, |h| {
                                h.child(
                                    div()
                                        .px_1p5()
                                        .rounded(px(4.))
                                        .bg(c(theme::BG_ELEVATED))
                                        .text_xs()
                                        .text_color(c(theme::FG_DIM))
                                        .child(logs_len.to_string()),
                                )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.ext_drawer_open.remove(&cid_drawer) {
                                    this.ext_drawer_open.insert(cid_drawer.clone());
                                }
                                cx.notify();
                            })),
                    );
                if drawer_open {
                    let mut body = v_flex()
                        .id(("vsc-drawer-body", ix as usize))
                        .w_full()
                        .max_h(theme::u(180.))
                        .overflow_y_scroll()
                        .px_2p5()
                        .pb_2()
                        .gap_1p5();
                    for (cidx, cmd) in cmds.into_iter().take(12).enumerate() {
                        let cmd_id = cmd.id.clone();
                        let key_hint = vsc_registry::keybinding_for_command(&cmd.id);
                        body = body.child(
                            h_flex()
                                .id(("vsc-dcmd", cidx))
                                .w_full()
                                .min_w_0()
                                .px_1p5()
                                .py_0p5()
                                .gap_2()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(|s| s.bg(c(theme::BG_HOVER)))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .text_color(c(theme::FG))
                                        .child(cmd.display_label()),
                                )
                                .when_some(key_hint, |h, k| {
                                    h.child(
                                        div()
                                            .flex_none()
                                            .text_xs()
                                            .text_color(c(theme::FG_DIM))
                                            .child(k),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.execute_ext_command(&cmd_id, Vec::new(), window, cx);
                                })),
                        );
                    }
                    if let Some(logs) = self.ext_host.output_logs.get(&container.ext_id) {
                        if !logs.is_empty() {
                            let mut pre = v_flex()
                                .w_full()
                                .p_1p5()
                                .gap_0p5()
                                .rounded(px(4.))
                                .bg(c(theme::BG_EDITOR))
                                .border_1()
                                .border_color(c(theme::BORDER));
                            for line in logs.iter().rev().take(20).rev() {
                                pre = pre.child(
                                    div()
                                        .text_xs()
                                        .font_family("Cascadia Code")
                                        .text_color(c(theme::FG_DIM))
                                        .child(line.clone()),
                                );
                            }
                            body = body.child(pre);
                        }
                    }
                    footer = footer.child(body);
                }
                root = root.child(footer);
            }

            return root.into_any_element();
        }

        let status_row =
            self.render_vsc_status_row(&container.ext_id, &container_id, &ext_status, node_ok, cx);

        let mut col = v_flex()
            .id(("vsc-panel", ix as usize))
            .size_full()
            .overflow_y_scroll()
            .px_3()
            .py_2()
            .gap_3()
            .children(submenu_bar)
            .child(status_row);

        for (vidx, view) in views.iter().enumerate() {
            if !view.when.eval(&base_ctx) {
                continue;
            }
            let view_ctx = self.build_when_context(Some(&view.id), &empty_cfg);
            let welcomes = vsc_registry::welcome_for_view(&view.id, &view_ctx);

            let mut card = v_flex()
                .w_full()
                .gap_2()
                .p_2p5()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::BORDER))
                .bg(c(theme::BG_ELEVATED));

            if views.len() > 1 {
                card = card.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_MUTED))
                        .child(view.name.clone()),
                );
            }

            match view.kind {
                ViewType::Webview => {
                    let wv = self.ext_host.webviews.get(&view.id);
                    if let Some(info) = wv {
                        let url_app = info.url.clone();
                        let url_tab = info.url.clone();
                        let title = view.name.clone();
                        let title2 = title.clone();
                        card = card
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(if info.has_html {
                                        tr("Interactive extension panel is ready via FluxCode's local bridge.")
                                    } else {
                                        tr("Starting interactive extension UI...")
                                    }),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_1p5()
                                    .child(
                                        Button::new(("vsc-open-app", vidx))
                                            .label(tr("Open Interactive Panel"))
                                            .small()
                                            .primary()
                                            .flex_1()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.open_webview_url(&url_app, &title, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new(("vsc-open-browser", vidx))
                                            .icon(Icon::new(Ico::ExternalLink))
                                            .small()
                                            .outline()
                                            .tooltip(tr("Open in Browser"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.open_url(&url_tab);
                                                this.status = crate::trf!("Opened {name}"; name => title2).into();
                                                cx.notify();
                                            })),
                                    ),
                            );
                    } else if matches!(ext_status, ExtStatus::Activating) {
                        card = card.child(
                            h_flex()
                                .gap_2()
                                .text_xs()
                                .text_color(c(theme::FG_DIM))
                                .child(Spinner::new().icon(Ico::LoaderCircle).xsmall())
                                .child(tr("Starting extension host...")),
                        );
                    }
                }
                ViewType::Tree => {
                    if let Some(tree) = self.ext_host.trees.get(&view.id) {
                        if tree.loading && tree.roots.is_empty() {
                            card = card.child(
                                h_flex()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(Spinner::new().icon(Ico::LoaderCircle).xsmall())
                                    .child(tr("Loading view...")),
                            );
                        } else if !tree.roots.is_empty() {
                            card = card.child(self.render_vsc_tree(&view.id, tree, cx));
                        }
                    }
                }
            }

            for w in welcomes {
                for line in w.text_lines {
                    card = card.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(line));
                }
                for (lidx, link) in w.links.into_iter().enumerate() {
                    let cmd_id = link.command;
                    card = card.child(
                        Button::new(("vsc-welcome-btn", vidx * 32 + lidx))
                            .label(link.label)
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.execute_ext_command(&cmd_id, Vec::new(), window, cx);
                            })),
                    );
                }
            }

            col = col.child(card);
        }

        // Extension commands section
        let cmds = vsc_registry::commands_for_ext(&container.ext_id, &base_ctx);
        if !cmds.is_empty() {
            let mut cmd_box = v_flex().w_full().gap_1().child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_MUTED))
                    .child(tr("COMMANDS")),
            );
            for (cidx, cmd) in cmds.into_iter().take(12).enumerate() {
                let cmd_id = cmd.id.clone();
                let key_hint = vsc_registry::keybinding_for_command(&cmd.id);
                cmd_box = cmd_box.child(
                    h_flex()
                        .id(("vsc-cmd-row", cidx))
                        .w_full()
                        .min_w_0()
                        .px_2()
                        .py_1()
                        .gap_2()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(c(theme::BG_HOVER)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(c(theme::FG))
                                .child(cmd.display_label()),
                        )
                        .when_some(key_hint, |h, k| {
                            h.child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(k),
                            )
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.execute_ext_command(&cmd_id, Vec::new(), window, cx);
                        })),
                );
            }
            col = col.child(cmd_box);
        }

        // Collapsible output drawer
        if let Some(logs) = self.ext_host.output_logs.get(&container.ext_id) {
            if !logs.is_empty() {
                let open = self.ext_host.expanded_logs.contains(&container.ext_id);
                let ext_key = container.ext_id.clone();
                let mut log_box = v_flex()
                    .w_full()
                    .gap_1()
                    .pt_1()
                    .border_t_1()
                    .border_color(c(theme::BORDER))
                    .child(
                        h_flex()
                            .id("vsc-log-toggle")
                            .w_full()
                            .gap_1p5()
                            .py_1()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(theme::FG_MUTED))
                            .hover(|s| s.text_color(c(theme::FG_STRONG)))
                            .child(
                                Icon::new(if open {
                                    Ico::ChevronDown
                                } else {
                                    Ico::ChevronRight
                                })
                                .size(theme::u(12.)),
                            )
                            .child(format!("{} ({})", tr("Output"), logs.len()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.ext_host.expanded_logs.remove(&ext_key) {
                                    this.ext_host.expanded_logs.insert(ext_key.clone());
                                }
                                cx.notify();
                            })),
                    );
                if open {
                    let mut pre = v_flex()
                        .w_full()
                        .p_2()
                        .gap_0p5()
                        .rounded(px(4.))
                        .bg(c(theme::BG_EDITOR))
                        .border_1()
                        .border_color(c(theme::BORDER));
                    for line in logs.iter().rev().take(30).rev() {
                        pre = pre.child(
                            div()
                                .text_xs()
                                .font_family("Cascadia Code")
                                .text_color(c(theme::FG_DIM))
                                .child(line.clone()),
                        );
                    }
                    log_box = log_box.child(pre);
                }
                col = col.child(log_box);
            }
        }

        col.into_any_element()
    }

    fn render_vsc_status_row(
        &self,
        ext_id: &str,
        container_id: &str,
        status: &ExtStatus,
        node_ok: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cid = container_id.to_string();
        if !node_ok {
            return h_flex()
                .w_full()
                .p_2()
                .gap_2()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::WARN))
                .text_xs()
                .text_color(c(theme::WARN))
                .child(Icon::new(Ico::TriangleAlert).size(theme::u(13.)))
                .child(
                    div()
                        .flex_1()
                        .child(tr("Node.js not found on PATH; extension host disabled.")),
                )
                .into_any_element();
        }
        match status {
            ExtStatus::Idle => h_flex()
                .w_full()
                .gap_2()
                .justify_between()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(tr("Extension host idle"))
                .child(
                    Button::new("vsc-start-host")
                        .label(tr("Activate"))
                        .xsmall()
                        .outline()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.activate_ext_container(&cid, cx);
                        })),
                )
                .into_any_element(),
            ExtStatus::Activating => h_flex()
                .w_full()
                .gap_2()
                .text_xs()
                .text_color(c(theme::FG_MUTED))
                .child(Spinner::new().icon(Ico::LoaderCircle).xsmall())
                .child(tr("Activating extension..."))
                .into_any_element(),
            ExtStatus::Active => {
                let _ = ext_id;
                h_flex()
                    .w_full()
                    .gap_1p5()
                    .text_xs()
                    .text_color(c(theme::GREEN))
                    .child(Icon::new(Ico::Check).size(theme::u(12.)))
                    .child(tr("Extension active"))
                    .into_any_element()
            }
            ExtStatus::Error(err) => v_flex()
                .w_full()
                .p_2()
                .gap_1p5()
                .rounded(px(6.))
                .border_1()
                .border_color(c(theme::DANGER))
                .child(
                    div()
                        .text_xs()
                        .text_color(c(theme::DANGER))
                        .child(err.clone()),
                )
                .child(
                    Button::new("vsc-retry-host")
                        .label(tr("Retry"))
                        .xsmall()
                        .outline()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.activate_ext_container(&cid, cx);
                        })),
                )
                .into_any_element(),
        }
    }

    fn render_vsc_tree(
        &self,
        view_id: &str,
        tree: &super::host::TreeState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = v_flex().w_full().gap_0p5();
        for (i, node) in tree.roots.iter().enumerate() {
            col = col.child(self.render_vsc_tree_node(view_id, tree, node, 0, i, cx));
        }
        col.into_any_element()
    }

    fn render_vsc_tree_node(
        &self,
        view_id: &str,
        tree: &super::host::TreeState,
        node: &TreeNode,
        depth: usize,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = tree.expanded.contains(&node.id);
        let vid = view_id.to_string();
        let nid = node.id.clone();
        let cmd = node.command.clone();
        let args = node.command_args.clone();
        let collapsible = node.collapsible;
        let row = h_flex()
            .id(("vsc-tree-node", idx + depth * 1000))
            .w_full()
            .min_w_0()
            .pl(px(6. + (depth as f32) * 12.))
            .pr_2()
            .py_1()
            .gap_1p5()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(|s| s.bg(c(theme::BG_HOVER)))
            .when(collapsible, |h| {
                h.child(
                    Icon::new(if expanded {
                        Ico::ChevronDown
                    } else {
                        Ico::ChevronRight
                    })
                    .flex_none()
                    .size(theme::u(12.))
                    .text_color(c(theme::FG_DIM)),
                )
            })
            .when_some(node.icon.as_deref(), |h, ic| {
                h.child(
                    Icon::new(codicon_to_icon(ic))
                        .flex_none()
                        .size(theme::u(13.))
                        .text_color(c(theme::FG_MUTED)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(c(theme::FG))
                    .child(node.label.clone()),
            )
            .when(!node.description.is_empty(), |h| {
                h.child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(node.description.clone()),
                )
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if collapsible {
                    let st = this.ext_host.trees.entry(vid.clone()).or_default();
                    if st.expanded.remove(&nid) {
                        cx.notify();
                    } else {
                        st.expanded.insert(nid.clone());
                        if !st.children.contains_key(&nid) {
                            this.ext_host.host.request_tree_children(&vid, Some(&nid));
                        }
                        cx.notify();
                    }
                } else if let Some(cmd_id) = &cmd {
                    this.execute_ext_command(cmd_id, args.clone(), window, cx);
                }
            }));
        let mut wrap = v_flex().w_full().child(row);
        if collapsible && expanded {
            if let Some(kids) = tree.children.get(&node.id) {
                for (cidx, kid) in kids.iter().enumerate() {
                    wrap = wrap.child(self.render_vsc_tree_node(
                        view_id,
                        tree,
                        kid,
                        depth + 1,
                        cidx,
                        cx,
                    ));
                }
            }
        }
        wrap.into_any_element()
    }
}
