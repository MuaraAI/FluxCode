//! Left and right side bars: any view (Explorer, Search, Source Control, Run, Extensions, the agent) can live on either side, as in VS Code's secondary side bar.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::{SideView, Workbench};
use gpui_kit::component::{h_flex, menu::PopupMenu, menu::PopupMenuItem, Icon};
use gpui_kit::{prelude::FluentBuilder, *};

impl SideView {
    pub(crate) const ALL: [SideView; 6] = [SideView::Explorer, SideView::Search, SideView::Scm, SideView::Run, SideView::Extensions, SideView::Agent];

    pub(crate) fn id(self) -> &'static str {
        match self {
            SideView::Explorer => "explorer",
            SideView::Search => "search",
            SideView::Scm => "scm",
            SideView::Run => "run",
            SideView::Extensions => "extensions",
            SideView::Agent => "agent",
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
        }
    }
}

impl Workbench {
    /// The view lives in the right side bar.
    pub(crate) fn on_right(&self, view: SideView) -> bool {
        match view {
            SideView::Agent => self.settings.agent_right,
            v => self.settings.right_views.iter().any(|id| id == v.id()),
        }
    }

    /// A popover closed by a click outside it: remember which, for `pop_reopens`.
    pub(crate) fn pop_closed_outside(&mut self, key: &'static str) {
        self.pop_shut = Some((key, std::time::Instant::now()));
    }

    /// The popover's button was clicked: false when that same press just closed it from outside (mouse down closes it, the click would open it again).
    pub(crate) fn pop_may_open(&mut self, key: &'static str) -> bool {
        !matches!(self.pop_shut.take(), Some((k, at)) if k == key && at.elapsed().as_millis() < 400)
    }

    /// The right dock is on screen (whatever view it shows).
    pub(crate) fn right_dock_on(&self) -> bool {
        self.agent_open && self.on_right(self.right_view)
    }

    /// The agent chat is the view the right dock is showing.
    pub(crate) fn agent_docked_visible(&self) -> bool {
        self.right_dock_on() && self.right_view == SideView::Agent
    }

    /// The view's icon is lit: it shows on its side and settings is not over it.
    pub(crate) fn view_shown(&self, view: SideView) -> bool {
        !self.settings_open
            && if self.on_right(view) { self.right_dock_on() && self.right_view == view } else { self.sidebar_open && self.view == view }
    }

    /// Show a view on its side (without toggling it off).
    pub(crate) fn reveal_view(&mut self, view: SideView, cx: &mut Context<Self>) {
        self.settings_open = false;
        if self.on_right(view) {
            self.right_view = view;
            self.agent_open = true;
        } else {
            self.view = view;
            self.sidebar_open = true;
        }
        cx.notify();
    }

    /// An activity bar click: show the view, or hide its side bar when it already shows.
    pub(crate) fn toggle_view(&mut self, view: SideView, cx: &mut Context<Self>) {
        if self.view_shown(view) {
            if self.on_right(view) {
                self.agent_open = false;
            } else {
                self.sidebar_open = false;
            }
            return cx.notify();
        }
        self.reveal_view(view, cx);
    }

    /// Move a view to the other side bar and show it there.
    pub(crate) fn move_view(&mut self, view: SideView, right: bool, cx: &mut Context<Self>) {
        if self.on_right(view) == right {
            return self.reveal_view(view, cx);
        }
        match view {
            SideView::Agent => self.settings.agent_right = right,
            v if right => self.settings.right_views.push(v.id().into()),
            v => self.settings.right_views.retain(|id| id != v.id()),
        }
        self.settings.save();
        if right && self.view == view {
            // The left bar shows the first view still on the left, or closes.
            match SideView::ALL.into_iter().find(|v| !self.on_right(*v)) {
                Some(v) => self.view = v,
                None => self.sidebar_open = false,
            }
        }
        if !right && self.right_view == view {
            self.right_view = SideView::ALL.into_iter().find(|v| self.on_right(*v)).unwrap_or(SideView::Agent);
            if !self.on_right(self.right_view) {
                self.agent_open = false;
            }
        }
        self.agent_full &= self.right_view == SideView::Agent;
        self.reveal_view(view, cx);
    }

    /// Right-click menu of an activity bar icon.
    pub(crate) fn view_side_menu(&self, view: SideView, menu: PopupMenu, cx: &mut Context<Self>) -> PopupMenu {
        let weak = cx.entity().downgrade();
        let right = self.on_right(view);
        let label = if right { tr("Move to Left Side Bar") } else { tr("Move to Right Side Bar") };
        menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
            _ = weak.update(cx, |this, cx| this.move_view(view, !right, cx));
        }))
    }

    /// The right side bar: the agent's own dock, or any other view with a tab strip of the views that live on the right.
    pub(crate) fn render_right_dock(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.right_view == SideView::Agent {
            return self.render_agent_dock(window, cx);
        }
        self.render_view_panel(self.right_view, true, window, cx)
    }

    /// Icon tabs for the views on the right, shown in the right dock's header when more than one view lives there.
    pub(crate) fn render_right_tabs(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let views: Vec<SideView> = SideView::ALL.into_iter().filter(|v| self.on_right(*v)).collect();
        if views.len() < 2 {
            return None;
        }
        let mut row = h_flex().gap_px().flex_none();
        for v in views {
            let on = self.right_view == v;
            row = row.child(
                div()
                    .id(SharedString::from(format!("rt-{}", v.id())))
                    .size(theme::u(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(c(if on { theme::FG_STRONG } else { theme::FG_DIM }))
                    .when(on, |d| d.bg(c(theme::BG_HOVER)))
                    .hover(|s| s.text_color(c(theme::FG)))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tr(v.label())).build(w, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.right_view = v;
                        cx.notify();
                    }))
                    .child(v.icon().size(theme::u(14.))),
            );
        }
        Some(row.into_any_element())
    }

    /// Hide-the-right-dock button for its header.
    pub(crate) fn right_close_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        crate::workbench::tool_button("rd-close", gpui_kit::component::IconName::Close, tr("Hide"), cx, |this, _, cx| {
            this.agent_open = false;
            cx.notify();
        })
    }
}
