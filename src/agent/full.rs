use super::{ago, when_label, Item, HISTORY_FILE};
use crate::i18n::tr;
use crate::theme::{self, c, GlassExt};
use crate::workbench::{tool_button, Workbench};
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState},
    menu::DropdownMenu,
    v_flex, Icon, IconName, Selectable, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

const WIDE: f32 = 1180.;
const MEDIUM: f32 = 860.;

pub struct FullState {
    pub left: bool,
    pub right: bool,
    pub rename: Option<Rename>,
    pub drag: Option<(f32, f32)>,
}

pub const LEFT_W: f32 = 260.;
pub const RIGHT_W: f32 = 300.;
pub const MIN_W: f32 = 200.;
pub const MAX_W: f32 = 520.;

pub struct Rename {
    pub id: u64,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

impl Default for FullState {
    fn default() -> Self {
        FullState {
            left: true,
            right: true,
            rename: None,
            drag: None,
        }
    }
}

impl Workbench {
    pub(crate) fn full_left_w(&self) -> f32 {
        let w = self.settings.full_left_w;
        if w > 0. {
            w.clamp(MIN_W, MAX_W)
        } else {
            f32::from(theme::u(LEFT_W))
        }
    }

    pub(crate) fn full_right_w(&self) -> f32 {
        let w = self.settings.full_right_w;
        if w > 0. {
            w.clamp(MIN_W, MAX_W)
        } else {
            f32::from(theme::u(RIGHT_W))
        }
    }

    pub(crate) fn render_agent_full(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let w = f32::from(window.viewport_size().width);
        let left = self.agent.full.left && w >= MEDIUM;
        let right = self.agent.full.right && w >= WIDE;
        let gap = theme::u(10.);
        let edge = |id: &'static str, which: u8, this: &Self, cx: &mut Context<Self>| {
            let on = this.resizing == which;
            div()
                .id(id)
                .flex_none()
                .w(gap)
                .h_full()
                .flex()
                .justify_center()
                .cursor_col_resize()
                .group("agf-edge")
                .child(
                    div()
                        .w(px(2.))
                        .h(relative(0.12))
                        .my_auto()
                        .rounded_full()
                        .bg(c(theme::ACCENT))
                        .opacity(if on { 1. } else { 0. })
                        .group_hover("agf-edge", |s| s.opacity(0.7)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        let w = if which == 3 {
                            this.full_left_w()
                        } else {
                            this.full_right_w()
                        };
                        this.agent.full.drag = Some((f32::from(e.position.x), w));
                        this.resizing = which;
                        cx.notify();
                    }),
                )
        };
        h_flex()
            .id("ag-full")
            .size_full()
            .p(gap)
            .when(left, |d| {
                d.child(self.render_full_chats(window, cx))
                    .child(edge("agf-edge-l", 3, self, cx))
            })
            .when(!left, |d| d.child(div().w(gap).flex_none()))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .glass()
                    .overflow_hidden()
                    .child(self.render_full_top(w, cx))
                    .children(self.render_worktree_strip(cx))
                    .child(
                        h_flex().flex_1().min_h_0().justify_center().child(
                            div()
                                .h_full()
                                .w_full()
                                .max_w(theme::u(900.))
                                .child(self.render_agent(window, cx)),
                        ),
                    ),
            )
            .when(right, |d| {
                d.child(edge("agf-edge-r", 4, self, cx))
                    .child(self.render_full_changes(cx))
            })
            .when(!right, |d| d.child(div().w(gap).flex_none()))
            .with_animation(
                "ag-full-in",
                Animation::new(std::time::Duration::from_millis(180)).with_easing(ease_out_quint()),
                |d, t| d.opacity(t),
            )
            .into_any_element()
    }

    fn render_full_top(&self, w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = |id: &'static str, icon: IconName, on: bool, can: bool, tip: &'static str| {
            Button::new(id)
                .icon(Icon::new(icon))
                .xsmall()
                .ghost()
                .selected(on && can)
                .tooltip(tip)
        };
        h_flex()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .h(theme::u(42.))
            .flex_none()
            .px_3()
            .gap_1()
            .border_b_1()
            .border_color(theme::glass_edge())
            .child(
                toggle(
                    "agf-left",
                    IconName::PanelLeft,
                    self.agent.full.left,
                    w >= MEDIUM,
                    tr("Chats"),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.agent.full.left = !this.agent.full.left;
                    cx.notify();
                })),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .overflow_hidden()
                    .child(self.render_agent_picker(cx)),
            )
            .child(div().flex_none().child(self.render_agent_switch(cx)))
            .when(w >= 540., |h| h.children(self.render_agent_opt_badge(cx)))
            .when(w >= 660., |h| {
                h.children(self.render_agent_profile_pill(cx))
            })
            .child(div().flex_1())
            .child(div().flex_none().child(self.render_agent_tools(cx)))
            .child(div().flex_none().child(self.render_agent_more(cx)))
            .child({
                let focused = !self.agent.full.left && !self.agent.full.right;
                Button::new("agf-focus")
                    .icon(Icon::new(Ico::Focus))
                    .xsmall()
                    .ghost()
                    .selected(focused)
                    .tooltip(if focused {
                        tr("Show chats and changes")
                    } else {
                        tr("Only the chat")
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.agent.full.left = focused;
                        this.agent.full.right = focused;
                        cx.notify();
                    }))
            })
            .child(
                toggle(
                    "agf-right",
                    IconName::PanelRight,
                    self.agent.full.right,
                    w >= WIDE,
                    tr("Changes"),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.agent.full.right = !this.agent.full.right;
                    cx.notify();
                })),
            )
    }

    fn agent_rename_start(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(title) = self
            .agent
            .history
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.title.clone())
        else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Chat name")));
        input.update(cx, |s, cx| {
            s.set_value(title, window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut Workbench, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.agent_rename_done(true, cx);
                }
            },
        );
        input.focus_handle(cx).focus(window, cx);
        self.agent.full.rename = Some(Rename {
            id,
            input,
            _sub: sub,
        });
        cx.notify();
    }

    fn agent_rename_done(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(r) = self.agent.full.rename.take() else {
            return;
        };
        let text: String = r.input.read(cx).value().trim().chars().take(120).collect();
        if keep {
            if let Some(chat) = self.agent.history.iter_mut().find(|c| c.id == r.id) {
                if text.is_empty() {
                    chat.renamed = false;
                    chat.title = chat
                        .messages
                        .iter()
                        .find(|m| m.0 == "user")
                        .map(|m| m.1.lines().next().unwrap_or("").chars().take(70).collect())
                        .unwrap_or_default();
                } else {
                    chat.renamed = true;
                    chat.title = text;
                }
                crate::settings::save_json(HISTORY_FILE, &self.agent.history);
            }
        }
        cx.notify();
    }

    fn render_full_chats(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use super::chats::{self, ChatView};
        let a = &self.agent;
        let busy = a.run.working();
        let view = ChatView::from_id(&self.settings.chat_view);
        let motion = !cx.reduce_motion();
        let view_id = window.current_view();
        let liq = crate::ui::liquid::keyed_state("agf-chats-liq", window, cx);
        let row_cross = 240.0f32;
        let mut chat_ix = 0usize;
        let mut active_chat_ix = None;
        let mut rows_box = v_flex().w_full().gap_px();
        let fresh = a.chat_id.is_none() && a.items.iter().any(|i| matches!(i, Item::User(_)));
        if fresh && view != ChatView::Archived {
            let title = a
                .items
                .iter()
                .find_map(|i| {
                    if let Item::User(t) = i {
                        Some(t.lines().next().unwrap_or("").to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            active_chat_ix = Some(chat_ix);
            let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), chat_ix, view_id);
            let liq_h = liq.clone();
            let ix_now = chat_ix;
            chat_ix += 1;
            rows_box = rows_box.child(
                chat_row(
                    "agf-now",
                    title,
                    tr("Now").into(),
                    Flags {
                        current: true,
                        working: busy,
                        archived: false,
                        pinned: false,
                        mark: Some(c(theme::GREEN)),
                        motion,
                    },
                )
                .on_hover(move |entered: &bool, _, cx| {
                    if *entered && motion {
                        crate::ui::liquid::keyed_hover_enter(
                            &liq_h, ix_now, true, row_cross, 145, view_id, cx,
                        );
                    }
                })
                .child(probe),
            );
        }
        let busy_ids = a.working_ids();
        let now = super::now_ms();
        let today_start = {
            use chrono::{Local, TimeZone};
            let midnight = Local::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default();
            Local
                .from_local_datetime(&midnight)
                .single()
                .map(|t| t.timestamp_millis() as u64)
                .unwrap_or(now)
        };
        let all = chats::groups(&a.history, view, &busy_ids, now, today_start);
        let foldable = !matches!(view, ChatView::Archived | ChatView::Working);
        let show_all = self.settings.chat_show_all || !foldable;
        let (groups, hidden) = if show_all {
            (all, 0)
        } else {
            chats::compact(all, &a.history, &busy_ids, a.chat_id)
        };
        let archived_n = a.history.iter().filter(|c| c.archived).count();
        for (gi, (heading, idx)) in groups.iter().enumerate() {
            let folded =
                !heading.is_empty() && self.settings.chat_collapsed.iter().any(|h| h == heading);
            if !heading.is_empty() {
                let key = heading.clone();
                let label = tr_dyn(heading);
                let pill = chats::Recency::of_heading(heading)
                    .and_then(recency_color)
                    .or(match heading.as_str() {
                        "Working now" => Some(c(theme::ACCENT_TEXT)),
                        "Pinned" => Some(c(theme::FG_MUTED)),
                        _ => None,
                    });
                rows_box = rows_box.child(
                    h_flex()
                        .id(("agf-g", gi))
                        .mx_1()
                        .px_1()
                        .when(gi > 0 || fresh, |d| d.mt_2())
                        .mb_0p5()
                        .h(theme::u(22.))
                        .gap_1p5()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_size(theme::u(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_DIM))
                        .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_MUTED)))
                        .child(
                            Icon::new(if folded {
                                Ico::ChevronRight
                            } else {
                                Ico::ChevronDown
                            })
                            .size(theme::u(11.)),
                        )
                        .children(pill.map(|col| {
                            div()
                                .flex_none()
                                .size(px(6.))
                                .rounded_full()
                                .bg(col.opacity(0.85))
                        }))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(c(theme::FG_MUTED))
                                .child(label),
                        )
                        .child(
                            div()
                                .text_color(c(theme::FG_DIM))
                                .font_weight(FontWeight::NORMAL)
                                .child(idx.len().to_string()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let list = &mut this.settings.chat_collapsed;
                            if let Some(p) = list.iter().position(|h| *h == key) {
                                list.remove(p);
                            } else {
                                list.push(key.clone());
                            }
                            this.settings.save();
                            cx.notify();
                        })),
                );
            }
            if folded {
                continue;
            }
            for &i in idx {
                let chat = &a.history[i];
                let id = chat.id;
                let current = a.chat_id == Some(id);
                if let Some(r) = a.full.rename.as_ref().filter(|r| r.id == id) {
                    rows_box = rows_box.child(
                        div()
                            .id(("agf-rn", i))
                            .w_full()
                            .px_1()
                            .py_1()
                            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                                if e.keystroke.key == "escape" {
                                    cx.stop_propagation();
                                    this.agent_rename_done(false, cx);
                                }
                            }))
                            .child(crate::ui::McInput::new(&r.input).small()),
                    );
                    continue;
                }
                let r_ix = chat_ix;
                chat_ix += 1;
                if current {
                    active_chat_ix = Some(r_ix);
                }
                let probe = crate::ui::liquid::keyed_item_probe(liq.clone(), r_ix, view_id);
                let liq_h = liq.clone();
                let clock = when_label(chat.updated);
                let mut when = if clock.contains(' ') {
                    clock
                } else {
                    format!("{clock} · {}", ago(chat.updated))
                };
                if view != ChatView::Agent {
                    if let Some(agent) = &chat.agent {
                        when = format!("{when} · {}", crate::acp::display_name(agent));
                    }
                }
                let working = busy_ids.contains(&id);
                let archived = chat.archived;
                let pinned = chat.pinned;
                let mark = recency_color(chats::recency(chat.updated, today_start));
                rows_box = rows_box.child(
                    chat_row(
                        SharedString::from(format!("agf-c-{i}")),
                        chat.title.clone(),
                        when.into(),
                        Flags {
                            current,
                            working,
                            archived,
                            pinned,
                            mark,
                            motion,
                        },
                    )
                    .on_hover(move |entered: &bool, _, cx| {
                        if *entered && motion {
                            crate::ui::liquid::keyed_hover_enter(
                                &liq_h, r_ix, true, row_cross, 145, view_id, cx,
                            );
                        }
                    })
                    .child(probe)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, w, cx| {
                            if e.click_count >= 2 {
                                cx.stop_propagation();
                                this.agent_rename_start(id, w, cx);
                            }
                        }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.agent_open_chat(id, cx)))
                    .child(
                        row_action(
                            ("agf-pin", i),
                            if pinned { Ico::PinOff } else { Ico::Pin },
                            if pinned {
                                tr("Unpin")
                            } else {
                                tr("Pin to the top")
                            },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.agent_pin_chat(id, !pinned, cx)
                        })),
                    )
                    .child(
                        row_action(
                            ("agf-rename", i),
                            Ico::Pencil,
                            tr("Rename (or double-click)"),
                        )
                        .on_click(cx.listener(move |this, _, w, cx| {
                            cx.stop_propagation();
                            this.agent_rename_start(id, w, cx)
                        })),
                    )
                    .when(!working, |r| {
                        r.child(
                            row_action(
                                ("agf-arch", i),
                                if archived {
                                    Ico::ArchiveRestore
                                } else {
                                    Ico::Archive
                                },
                                if archived {
                                    tr("Take out of the archive")
                                } else {
                                    tr("Archive")
                                },
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.agent_archive_chat(id, !archived, cx)
                                },
                            )),
                        )
                    }),
                );
            }
        }
        if foldable && (hidden > 0 || self.settings.chat_show_all) {
            let more = hidden > 0;
            let label: SharedString = if more {
                crate::trf!("Show {n} more chats"; n => hidden).into()
            } else {
                tr("Show only active and pinned").into()
            };
            rows_box = rows_box.child(
                h_flex()
                    .id("agf-more")
                    .mx_1()
                    .mt_1()
                    .px_2()
                    .h(theme::u(28.))
                    .gap_1p5()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(c(theme::FG_MUTED))
                    .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                    .child(
                        Icon::new(if more {
                            Ico::ChevronDown
                        } else {
                            Ico::ChevronUp
                        })
                        .size(theme::u(12.)),
                    )
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.chat_show_all = more;
                        this.settings.save();
                        cx.notify();
                    })),
            );
        }
        let empty = groups.is_empty() && !fresh && hidden == 0;
        if empty {
            let text = match view {
                ChatView::Archived => tr("Nothing archived. Chats untouched for a week move here on their own; the archive button on a chat moves it now."),
                ChatView::Working => tr("No chat is working right now."),
                _ if a.history.is_empty() => tr("No chats yet. Each answer saves the chat here."),
                _ => tr("Every chat is in the archive."),
            };
            rows_box = rows_box.child(
                div()
                    .px_2()
                    .py_2()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(text),
            );
        }
        let (frame, _) = crate::ui::liquid::keyed_sync_and_sample(
            &liq,
            true,
            chat_ix,
            active_chat_ix,
            row_cross,
            175,
            motion,
        );
        let layer = frame.map(|fr| {
            crate::ui::liquid::render_coalesced_layer(
                "agf-chats-liq-l",
                true,
                crate::ui::liquid::agent_liquid_style(row_cross, 5.0),
                fr,
            )
        });
        let list_inner = v_flex()
            .id("agf-chats-inner")
            .relative()
            .w_full()
            .on_hover({
                let h = liq.clone();
                move |entered: &bool, _, cx| {
                    if !*entered && motion {
                        crate::ui::liquid::keyed_hover_leave(&h, view_id, cx);
                    }
                }
            })
            .child(crate::ui::liquid::keyed_origin_probe(liq, view_id))
            .children(layer)
            .child(rows_box);
        let list = v_flex()
            .id("agf-chats")
            .flex_1()
            .min_h_0()
            .px_2()
            .pb_2()
            .overflow_y_scroll()
            .child(list_inner);
        let weak = cx.entity().downgrade();
        let view_menu = Button::new("agf-view")
            .icon(Icon::new(Ico::ListFilter))
            .label(tr(view.label()))
            .xsmall()
            .ghost()
            .dropdown_menu(
                move |mut menu: gpui_kit::component::menu::PopupMenu, _, _| {
                    for v in ChatView::ALL {
                        let w = weak.clone();
                        let label: SharedString = if v == ChatView::Archived && archived_n > 0 {
                            format!("{} ({archived_n})", tr(v.label())).into()
                        } else {
                            tr(v.label()).into()
                        };
                        if v == ChatView::Working {
                            menu = menu.separator();
                        }
                        menu = menu.item(
                            gpui_kit::component::menu::PopupMenuItem::new(label)
                                .checked(v == view)
                                .on_click(move |_, _, cx| {
                                    _ = w.update(cx, |this, cx| {
                                        this.settings.chat_view = v.id().to_string();
                                        this.settings.save();
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                    menu
                },
            );
        v_flex()
            .w(px(self.full_left_w()))
            .flex_none()
            .h_full()
            .glass()
            .overflow_hidden()
            .child(
                h_flex()
                    .p_2()
                    .gap_1()
                    .child(
                        h_flex()
                            .id("agf-new")
                            .flex_1()
                            .min_w_0()
                            .h(theme::u(34.))
                            .px_2()
                            .gap_2()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_sm()
                            .text_color(c(theme::FG_STRONG))
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| this.agent_new_chat(cx)))
                            .child(Icon::new(IconName::Plus).flex_none().size(theme::u(15.)))
                            .child(div().min_w_0().truncate().child(tr("New chat"))),
                    )
                    .child(view_menu)
                    .child(tool_button(
                        "agf-left-hide",
                        IconName::PanelLeftClose,
                        tr("Hide chats"),
                        cx,
                        |this, _, cx| {
                            this.agent.full.left = false;
                            cx.notify();
                        },
                    )),
            )
            .child(list)
    }

    fn render_full_changes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.agent;
        let (add, del) = a
            .changes
            .iter()
            .fold((0, 0), |t, ch| (t.0 + ch.added, t.1 + ch.removed));
        let mut list = v_flex()
            .id("agf-changes")
            .flex_1()
            .min_h_0()
            .px_2()
            .gap_px()
            .overflow_y_scroll();
        for (i, ch) in a.changes.iter().enumerate() {
            let name = ch
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let dir = ch
                .path
                .parent()
                .map(|p| self.rel_display(&p.to_string_lossy()))
                .unwrap_or_default();
            list = list.child(
                h_flex()
                    .id(("agf-ch", i))
                    .px_2()
                    .py_1()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_sm()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.agent.user_nav = Some(std::time::Instant::now());
                        this.agent_show_diff(i, w, cx)
                    }))
                    .child(crate::files::file_icon_el(&name, theme::u(14.)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_color(c(theme::FG_STRONG)).child(name))
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(c(theme::FG_DIM))
                                    .child(dir),
                            ),
                    )
                    .child(if ch.counted {
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .text_xs()
                            .child(
                                div()
                                    .text_color(c(theme::GREEN))
                                    .child(format!("+{}", ch.added)),
                            )
                            .child(
                                div()
                                    .text_color(c(theme::DANGER))
                                    .child(format!("−{}", ch.removed)),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child("…")
                            .into_any_element()
                    })
                    .child(
                        div()
                            .id(("agf-undo", i))
                            .flex_none()
                            .p_1()
                            .rounded(px(6.))
                            .text_color(c(theme::FG_DIM))
                            .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
                            .tooltip(|w, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(tr("Undo this file"))
                                    .build(w, cx)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.agent_undo(i, cx)
                            }))
                            .child(
                                Icon::new(gpui_kit::assets::IconName::Undo2).size(theme::u(13.)),
                            ),
                    ),
            );
        }
        if a.changes.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_2()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(tr(
                        "Files the agent edits show up here, with a diff a click away.",
                    )),
            );
        }
        v_flex()
            .w(px(self.full_right_w()))
            .flex_none()
            .h_full()
            .glass()
            .overflow_hidden()
            .child(
                h_flex()
                    .h(theme::u(42.))
                    .flex_none()
                    .px_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme::glass_edge())
                    .child(
                        Icon::new(gpui_kit::assets::IconName::FileDiff)
                            .size(theme::u(14.))
                            .text_color(c(theme::FG_MUTED)),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(theme::FG_STRONG))
                            .child(tr("Changes")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(a.changes.len().to_string()),
                    )
                    .child(div().flex_1())
                    .when(!a.changes.is_empty(), |h| {
                        h.child(
                            div()
                                .text_xs()
                                .text_color(c(theme::GREEN))
                                .child(format!("+{add}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(c(theme::DANGER))
                                .child(format!("−{del}")),
                        )
                    })
                    .child(tool_button(
                        "agf-right-hide",
                        IconName::PanelRightClose,
                        tr("Hide changes"),
                        cx,
                        |this, _, cx| {
                            this.agent.full.right = false;
                            cx.notify();
                        },
                    ))
                    .child(tool_button(
                        "agf-exit",
                        IconName::Minimize,
                        tr("Back beside the editor"),
                        cx,
                        |this, _, cx| {
                            this.agent_full = false;
                            cx.notify();
                        },
                    )),
            )
            .child(list.pt_1())
            .child(self.render_limits_card(cx))
    }
}

struct Flags {
    current: bool,
    working: bool,
    archived: bool,
    pinned: bool,
    mark: Option<Hsla>,
    motion: bool,
}

fn recency_color(r: super::chats::Recency) -> Option<Hsla> {
    use super::chats::Recency;
    match r {
        Recency::Today => Some(c(theme::GREEN)),
        Recency::Yesterday => Some(c(theme::BLUE)),
        Recency::ThisWeek => Some(c(theme::WARN)),
        Recency::Earlier => None,
    }
}

fn chat_row(
    id: impl Into<ElementId>,
    title: String,
    when: SharedString,
    f: Flags,
) -> Stateful<Div> {
    let Flags {
        current,
        working,
        archived,
        pinned,
        mark,
        motion,
    } = f;
    h_flex()
        .id(id)
        .group("agf-row")
        .w_full()
        .px_2()
        .py_1p5()
        .gap_1()
        .relative()
        .rounded(px(5.))
        .cursor_pointer()
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .when(current && !motion, |d| {
            d.bg(c(theme::ACCENT).opacity(0.10))
                .border_color(c(theme::ACCENT).opacity(0.28))
        })
        .when(current, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(-1.))
                    .w(px(3.))
                    .rounded_full()
                    .bg(c(theme::ACCENT))
                    .with_animation(
                        SharedString::from(format!("agf-cur-{title}")),
                        Animation::new(std::time::Duration::from_millis(220))
                            .with_easing(gpui_kit::ease_out_quint()),
                        |d, t| {
                            d.top(relative(0.5 - 0.3 * t))
                                .bottom(relative(0.5 - 0.3 * t))
                        },
                    ),
            )
        })
        .when(!current && !motion, |d| {
            d.hover(|s| s.bg(c(theme::BG_HOVER)))
        })
        .when(working, |d| {
            d.child(
                div().flex_none().pr_1().child(
                    gpui_kit::component::spinner::Spinner::new()
                        .icon(Ico::LoaderCircle)
                        .xsmall()
                        .color(c(theme::ACCENT_TEXT)),
                ),
            )
        })
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .when(archived, |d| d.opacity(0.7))
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_1()
                        .when(pinned, |h| {
                            h.child(
                                Icon::new(Ico::Pin)
                                    .flex_none()
                                    .size(theme::u(11.))
                                    .text_color(c(theme::FG_MUTED)),
                            )
                        })
                        .child(
                            div()
                                .min_w_0()
                                .text_sm()
                                .truncate()
                                .text_color(c(if current { theme::FG_STRONG } else { theme::FG }))
                                .child(if title.is_empty() {
                                    tr("Untitled chat").to_string()
                                } else {
                                    title
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .min_w_0()
                        .text_xs()
                        .when(working, |h| {
                            h.child(
                                div()
                                    .flex_none()
                                    .text_color(c(theme::ACCENT_TEXT))
                                    .child(tr("Working")),
                            )
                            .child(div().flex_none().text_color(c(theme::FG_DIM)).child("·"))
                        })
                        .when_some(mark.filter(|_| !working), |h, col| {
                            h.child(div().flex_none().size(px(6.)).rounded_full().bg(col))
                        })
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(c(theme::FG_DIM))
                                .child(when),
                        ),
                ),
        )
}

fn row_action(id: impl Into<ElementId>, icon: Ico, tip: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .p_1()
        .rounded(px(6.))
        .invisible()
        .group_hover("agf-row", |s| s.visible())
        .text_color(c(theme::FG_DIM))
        .hover(|s| s.bg(c(theme::BG_ELEVATED)).text_color(c(theme::FG_STRONG)))
        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
        .child(Icon::new(icon).size(theme::u(13.)))
}

fn tr_dyn(s: &str) -> String {
    match s {
        "Working now" => tr("Working now").into(),
        "Pinned" => tr("Pinned").into(),
        "Today" => tr("Today").into(),
        "Yesterday" => tr("Yesterday").into(),
        "This week" => tr("This week").into(),
        "Earlier" => tr("Earlier").into(),
        "No folder" => tr("No folder").into(),
        "Unknown agent" => tr("Unknown agent").into(),
        other => other.to_string(),
    }
}
