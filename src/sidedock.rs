use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::{SideView, Workbench};
use gpui_kit::component::{h_flex, menu::PopupMenu, menu::PopupMenuItem, Icon};
use gpui_kit::{prelude::FluentBuilder, *};

impl SideView {
    pub(crate) const ALL: [SideView; 7] = [
        SideView::Explorer,
        SideView::Search,
        SideView::Scm,
        SideView::Run,
        SideView::Extensions,
        SideView::Agent,
        SideView::Mcbe,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            SideView::Explorer => "explorer",
            SideView::Search => "search",
            SideView::Scm => "scm",
            SideView::Run => "run",
            SideView::Extensions => "extensions",
            SideView::Agent => "agent",
            SideView::Mcbe => "mcbe",
            SideView::Ext(_) => "ext",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        use gpui_kit::assets::IconName as A;
        use gpui_kit::component::IconName as C;
        match self {
            SideView::Explorer => Icon::new(C::FolderOpen),
            SideView::Search => Icon::new(C::Search),
            SideView::Scm => Icon::new(A::GitBranch),
            SideView::Run => Icon::new(C::Play),
            SideView::Extensions => Icon::new(A::Blocks),
            SideView::Agent => Icon::new(A::MessageSquareCode),
            SideView::Mcbe => Icon::new(crate::ext::mcbe_view::MCBE_ICON),
            SideView::Ext(_) => Icon::new(A::Blocks),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            SideView::Explorer => "Explorer",
            SideView::Search => "Search",
            SideView::Scm => "Source Control",
            SideView::Run => "Run and Debug",
            SideView::Extensions => "Extensions",
            SideView::Agent => "Agent",
            SideView::Mcbe => "Minecraft Bedrock",
            SideView::Ext(_) => "Extension",
        }
    }
}

impl Workbench {
    pub(crate) fn view_id_key(&self, view: SideView) -> String {
        match view {
            SideView::Ext(ix) => self
                .ext_containers
                .get(ix as usize)
                .map(|id| format!("ext:{id}"))
                .unwrap_or_else(|| "ext".into()),
            v => v.id().to_string(),
        }
    }

    pub(crate) fn all_side_views(&self) -> Vec<SideView> {
        let mut out = SideView::ALL.to_vec();
        for i in 0..self.ext_containers.len() {
            out.push(SideView::Ext(i as u16));
        }
        out
    }

    pub(crate) fn view_display_label(&self, view: SideView) -> String {
        match view {
            SideView::Ext(ix) => self
                .ext_containers
                .get(ix as usize)
                .and_then(|id| crate::ext::vsc_registry::container_by_id(id))
                .map(|c| c.title)
                .unwrap_or_else(|| tr("Extension").to_string()),
            v => tr(v.label()).to_string(),
        }
    }

    pub(crate) fn on_right(&self, view: SideView) -> bool {
        match view {
            SideView::Agent => self.settings.agent_right,
            v => {
                let key = self.view_id_key(v);
                self.settings.right_views.iter().any(|id| id == &key)
            }
        }
    }

    pub(crate) fn pop_closed_outside(&mut self, key: &'static str) {
        self.pop_shut = Some((key, std::time::Instant::now()));
    }

    pub(crate) fn pop_may_open(&mut self, key: &'static str) -> bool {
        !matches!(self.pop_shut.take(), Some((k, at)) if k == key && at.elapsed().as_millis() < 400)
    }

    pub(crate) fn right_dock_on(&self) -> bool {
        self.agent_open && self.on_right(self.right_view)
    }

    pub(crate) fn agent_docked_visible(&self) -> bool {
        self.right_dock_on() && self.right_view == SideView::Agent
    }

    pub(crate) fn view_shown(&self, view: SideView) -> bool {
        !self.settings_open
            && if self.on_right(view) {
                self.right_dock_on() && self.right_view == view
            } else {
                self.sidebar_open && self.view == view
            }
    }

    pub(crate) fn reveal_view(&mut self, view: SideView, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.settings_theme_open = false;
        self.settings_icon_open = false;
        self.settings_lang_open = false;
        self.settings_font_open = false;
        if self.on_right(view) {
            self.right_view = view;
            self.set_right_open(true, cx);
        } else {
            self.view = view;
            self.set_sidebar_open(true, cx);
        }
    }

    pub(crate) fn toggle_view(&mut self, view: SideView, cx: &mut Context<Self>) {
        if self.view_shown(view) {
            if self.on_right(view) {
                self.set_right_open(false, cx);
            } else {
                self.set_sidebar_open(false, cx);
            }
            return;
        }
        self.reveal_view(view, cx);
    }

    pub(crate) fn move_view(&mut self, view: SideView, right: bool, cx: &mut Context<Self>) {
        if self.on_right(view) == right {
            return self.reveal_view(view, cx);
        }
        let key = self.view_id_key(view);
        match view {
            SideView::Agent => self.settings.agent_right = right,
            _ if right => self.settings.right_views.push(key),
            _ => self.settings.right_views.retain(|id| id != &key),
        }
        self.settings.save();
        let all = self.all_side_views();
        if right && self.view == view {
            match all.iter().copied().find(|v| !self.on_right(*v)) {
                Some(v) => self.view = v,
                None => self.set_sidebar_open(false, cx),
            }
        }
        if !right && self.right_view == view {
            self.right_view = all
                .into_iter()
                .find(|v| self.on_right(*v))
                .unwrap_or(SideView::Agent);
            if !self.on_right(self.right_view) {
                self.set_right_open(false, cx);
            }
        }
        self.agent_full &= self.right_view == SideView::Agent;
        self.reveal_view(view, cx);
    }

    pub(crate) fn view_side_menu(
        &self,
        view: SideView,
        menu: PopupMenu,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        let weak = cx.entity().downgrade();
        let right = self.on_right(view);
        let label = if right {
            tr("Move to Left Side Bar")
        } else {
            tr("Move to Right Side Bar")
        };
        let mut menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
            _ = weak.update(cx, |this, cx| this.move_view(view, !right, cx));
        }));
        if let SideView::Ext(ix) = view {
            if let Some(cid) = self.ext_containers.get(ix as usize).cloned() {
                if let Some(desc) = crate::ext::vsc_registry::container_by_id(&cid) {
                    let ext_id = desc.ext_id;
                    let w2 = cx.entity().downgrade();
                    menu = menu.item(PopupMenuItem::new(tr("Open Extension Page")).on_click(
                        move |_, window, cx| {
                            _ = w2.update(cx, |this, cx| {
                                if let Some(inst) = this.ext.market.installed(&ext_id).cloned() {
                                    this.market_open(inst.item(), window, cx);
                                }
                            });
                        },
                    ));
                }
            }
        }
        menu
    }

    pub(crate) fn render_right_dock(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.right_view == SideView::Agent {
            return self.render_agent_dock(window, cx);
        }
        self.render_view_panel(self.right_view, true, window, cx)
    }

    pub(crate) fn render_right_tabs(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mcbe_on = self.has_mcbe_workspace();
        let views: Vec<SideView> = self
            .all_side_views()
            .into_iter()
            .filter(|v| self.on_right(*v) && (*v != SideView::Mcbe || mcbe_on))
            .collect();
        if views.len() < 2 {
            return None;
        }
        let mut row = h_flex().gap_px().flex_none();
        for (tab_ix, v) in views.into_iter().enumerate() {
            let on = self.right_view == v;
            let tip = self.view_display_label(v);
            let icon_el: AnyElement = match v {
                SideView::Ext(ix) => self
                    .ext_containers
                    .get(ix as usize)
                    .and_then(|id| crate::ext::vsc_registry::container_by_id(id))
                    .map(|c| crate::ext::vsc_view::render_ext_icon(&c.icon, theme::u(14.)))
                    .unwrap_or_else(|| v.icon().size(theme::u(14.)).into_any_element()),
                _ => v.icon().size(theme::u(14.)).into_any_element(),
            };
            row = row.child(
                div()
                    .id(("rt-tab", tab_ix))
                    .size(theme::u(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(c(if on { theme::FG_STRONG } else { theme::FG_DIM }))
                    .when(on, |d| d.bg(c(theme::BG_HOVER)))
                    .hover(|s| s.text_color(c(theme::FG)))
                    .tooltip(move |w, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.right_view = v;
                        if let Some(cid) = this.ext_container_id(v).map(str::to_string) {
                            this.activate_ext_container(&cid, cx);
                        }
                        cx.notify();
                    }))
                    .child(icon_el),
            );
        }
        Some(row.into_any_element())
    }

    pub(crate) fn right_close_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        crate::workbench::tool_button(
            "rd-close",
            gpui_kit::component::IconName::Close,
            tr("Hide"),
            cx,
            |this, _, cx| {
                this.set_right_open(false, cx);
            },
        )
    }
}
