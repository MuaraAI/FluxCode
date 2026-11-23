
use super::market::{self, Busy, Item, Query, Sort};
use super::view::tile;
use super::Filter;
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::assets::IconName as Ico;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    spinner::Spinner,
    text::TextView,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

pub const PAGE: &str = "vsx:";
const DEBOUNCE_MS: u64 = 150;
const ICON_WORKERS: usize = 6;
const ROW_H: f32 = 66.;
const PREFETCH_ROWS: usize = 20;

fn vsx_icon(path: Option<&PathBuf>, size: f32) -> AnyElement {
    match path {
        Some(p) => img(p.clone())
            .flex_none()
            .size(theme::u(size))
            .rounded(px(size * 0.12))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => tile("", size).into_any_element(),
    }
}

fn chip(id: impl Into<ElementId>, label: SharedString, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px_2()
        .h(theme::u(22.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .text_xs()
        .cursor_pointer()
        .text_color(c(if on {
            theme::FG_STRONG
        } else {
            theme::FG_MUTED
        }))
        .when(on, |d| d.bg(c(theme::BG_HOVER)))
        .hover(|d| d.text_color(c(theme::FG_STRONG)))
        .child(label)
}

impl Workbench {
    fn market_query(&self, cx: &App) -> Query {
        let m = &self.ext.market;
        Query::new(&self.ext.query.read(cx).value(), m.sort, m.category)
    }

    pub(crate) fn market_query_changed(&mut self, cx: &mut Context<Self>) {
        if self.ext.filter == Filter::Market {
            self.market_search(DEBOUNCE_MS, cx);
        }
        cx.notify();
    }

    pub(crate) fn market_search(&mut self, delay: u64, cx: &mut Context<Self>) {
        let q = self.market_query(cx);
        let m = &mut self.ext.market;
        m.kicked.set(true);
        m.generation += 1;
        let generation = m.generation;
        if m.shown.as_ref() == Some(&q) {
            if m.loading {
                m.loading = false;
                cx.notify();
            }
            return;
        }
        if let Some(page) = m.cache.get(&market::cache_key(&q, 1)).cloned() {
            m.show_first(q, page);
            m.scroll.scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
            return;
        }
        m.loading = true;
        m.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            if delay > 0 {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                let current = this
                    .update(cx, |this, _| this.ext.market.generation == generation)
                    .unwrap_or(false);
                if !current {
                    return;
                }
            }
            let asked = q.clone();
            let res = cx
                .background_spawn(async move { market::search(&asked, 1) })
                .await;
            _ = this.update(cx, |this, cx| {
                let m = &mut this.ext.market;
                match res {
                    Ok(page) => {
                        m.remember(market::cache_key(&q, 1), page.clone());
                        if m.generation == generation {
                            m.show_first(q, page);
                            m.scroll.scroll_to_item(0, ScrollStrategy::Top);
                        }
                    }
                    Err(e) if m.generation == generation => {
                        m.loading = false;
                        m.error = Some(e);
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn market_load_more(&mut self, cx: &mut Context<Self>) {
        let m = &mut self.ext.market;
        let Some(q) = m.shown.clone() else { return };
        if m.done || m.loading || m.loading_more || m.more_error.is_some() {
            return;
        }
        let page_no = m.pages + 1;
        let key = market::cache_key(&q, page_no);
        if let Some(page) = m.cache.get(&key).cloned() {
            m.append(page);
            cx.notify();
            return;
        }
        m.loading_more = true;
        let generation = m.generation;
        cx.spawn(async move |this, cx| {
            let asked = q.clone();
            let res = cx
                .background_spawn(async move { market::search(&asked, page_no) })
                .await;
            _ = this.update(cx, |this, cx| {
                let m = &mut this.ext.market;
                let current = m.generation == generation
                    && m.shown.as_ref() == Some(&q)
                    && m.pages + 1 == page_no;
                match res {
                    Ok(page) => {
                        m.remember(key, page.clone());
                        if current {
                            m.append(page);
                        }
                    }
                    Err(e) if current => {
                        m.loading_more = false;
                        m.more_error = Some(e);
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn market_in_view(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let n = self.ext.market.items.len();
        if n > 0 && range.end + PREFETCH_ROWS >= n {
            self.market_load_more(cx);
        }
        let m = &mut self.ext.market;
        let around = range.start.saturating_sub(4)..(range.end + 8).min(m.items.len());
        let mut queue = Vec::new();
        for it in &m.items[around] {
            let id = it.id();
            if m.icons.contains_key(&id) || m.icons_tried.contains(&id) {
                continue;
            }
            let Some(to) = it.icon_cache() else { continue };
            let urls: Vec<String> = [market::ICON_SMALL, market::ICON_DEFAULT]
                .iter()
                .filter_map(|k| it.asset(k))
                .collect();
            queue.push((id, urls, to));
        }
        m.icon_queue = queue;
        self.market_pump_icons(cx);
    }

    fn market_pump_icons(&mut self, cx: &mut Context<Self>) {
        let m = &mut self.ext.market;
        while m.icons_running < ICON_WORKERS && !m.icon_queue.is_empty() {
            let (id, urls, to) = m.icon_queue.remove(0);
            m.icons_tried.insert(id.clone());
            m.icons_running += 1;
            cx.spawn(async move |this, cx| {
                let path = to.clone();
                let ok = cx
                    .background_spawn(async move { market::fetch_icon(&urls, &path) })
                    .await;
                _ = this.update(cx, |this, cx| {
                    let m = &mut this.ext.market;
                    m.icons_running -= 1;
                    if ok {
                        m.icons.insert(id, to);
                    }
                    this.market_pump_icons(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(crate) fn market_install(&mut self, item: Item, cx: &mut Context<Self>) {
        let id = item.id();
        if self.ext.market.busy.contains_key(&id) {
            return;
        }
        self.ext.market.busy.insert(id.clone(), Busy::Installing);
        self.ext.note = None;
        let name = item.display.clone();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    market::install(&item).map(|_| {
                        super::native::load();
                        market::installed_list()
                    })
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.ext.market.busy.remove(&id);
                match res {
                    Ok(list) => {
                        this.ext.market.installed = list;
                        this.vsx_applied_changed(&id, cx);
                    }
                    Err(e) => {
                        this.ext.note =
                            Some(crate::trf!("Couldn't install {name}: {e}"; name => name, e => e))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn market_install_file(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Install from VSIX").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(p) = paths.into_iter().next() else {
                return;
            };
            let res = cx
                .background_spawn(async move {
                    market::install_file(&p).map(|name| {
                        super::native::load();
                        (name, market::installed_list())
                    })
                })
                .await;
            _ = this.update(cx, |this, cx| {
                match res {
                    Ok((name, list)) => {
                        this.ext.market.installed = list;
                        this.ext.note = Some(crate::trf!("Installed {name}"; name => name));
                        this.vsx_applied_changed("", cx);
                    }
                    Err(e) => this.ext.note = Some(crate::trf!("Couldn't install it: {e}"; e => e)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn market_uninstall(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(folder) = self.ext.market.installed(&id).map(|i| i.folder.clone()) else {
            return;
        };
        if self.ext.market.busy.contains_key(&id) {
            return;
        }
        self.ext.market.busy.insert(id.clone(), Busy::Removing);
        self.ext.note = None;
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    market::uninstall(&folder).map(|_| {
                        super::native::load();
                        market::installed_list()
                    })
                })
                .await;
            _ = this.update(cx, |this, cx| {
                this.ext.market.busy.remove(&id);
                match res {
                    Ok(list) => {
                        this.ext.market.installed = list;
                        this.vsx_applied_changed("", cx);
                    }
                    Err(e) => {
                        this.ext.note = Some(crate::trf!("Couldn't uninstall it: {e}"; e => e))
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn vsx_applied_changed(&mut self, installed: &str, cx: &mut Context<Self>) {
        self.settings.apply_backdrop();
        theme::apply(cx);
        cx.refresh_windows();
        let themes = super::native::applied(installed).themes;
        if !installed.is_empty() && !themes.is_empty() {
            let names: Vec<&str> = themes.iter().map(|(_, n)| n.as_str()).collect();
            self.ext.note = Some(crate::trf!("Added to the color themes: {list}. Pick one below or from the theme menu in the status bar."; list => names.join(", ")));
        }
    }

    pub(crate) fn market_open(&mut self, item: Item, cx: &mut Context<Self>) {
        let id = item.id();
        let loading = self.ext.market.open.as_ref().is_some_and(|o| o.id() == id);
        self.ext.page = Some(format!("{PAGE}{id}"));
        self.settings_open = false;
        self.whats_new = None;
        self.collab.page = false;
        if !self.ext.market.readme.contains_key(&id) && !loading {
            let folder = self.ext.market.installed(&id).map(|i| i.folder.clone());
            let it = item.clone();
            cx.spawn(async move |this, cx| {
                let f2 = folder.clone();
                let text = cx
                    .background_spawn(async move { market::readme(&it, f2.as_deref()) })
                    .await;
                let md = text.as_ref().ok().cloned();
                _ = this.update(cx, |this, cx| {
                    this.ext.market.readme.insert(id.clone(), text);
                    cx.notify();
                });
                if let Some(md) = md {
                    let images = cx
                        .background_spawn(async move { market::fetch_readme_images(&md, folder.as_deref()) })
                        .await;
                    _ = this.update(cx, |this, cx| {
                        this.ext.market.readme_images.insert(id, std::sync::Arc::new(images));
                        cx.notify();
                    });
                }
            })
            .detach();
        }
        self.ext.market.open = Some(item);
        cx.notify();
    }

    fn market_button(
        &self,
        id: (&'static str, usize),
        item: &Item,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = item.id();
        match self.ext.market.busy.get(&key) {
            Some(b) => Button::new(id)
                .label(if *b == Busy::Installing {
                    tr("Installing")
                } else {
                    tr("Removing")
                })
                .xsmall()
                .ghost()
                .loading(true)
                .into_any_element(),
            None if self.ext.market.installed(&key).is_some() => {
                let it = item.clone();
                Button::new(id)
                    .label(tr("Installed"))
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.market_open(it.clone(), cx);
                    }))
                    .into_any_element()
            }
            None => {
                let it = item.clone();
                Button::new(id)
                    .label(tr("Install"))
                    .xsmall()
                    .primary()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.market_install(it.clone(), cx);
                    }))
                    .into_any_element()
            }
        }
    }

    fn market_row(
        &self,
        keys: (&'static str, &'static str),
        ix: usize,
        it: &Item,
        icon: Option<&PathBuf>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = it.id();
        let selected =
            self.ext.page.as_deref().and_then(|p| p.strip_prefix(PAGE)) == Some(id.as_str());
        let stats = h_flex()
            .flex_shrink(1.)
            .min_w_0()
            .overflow_hidden()
            .gap_2()
            .when(it.installs > 0, |h| {
                h.child(
                    h_flex()
                        .gap_0p5()
                        .child(Icon::new(Ico::Download).size(theme::u(11.)))
                        .child(market::format_count(it.installs)),
                )
            });
        let open = it.clone();
        h_flex()
            .id((keys.0, ix))
            .w_full()
            .h(theme::u(ROW_H))
            .min_w_0()
            .overflow_hidden()
            .px_3()
            .gap_2p5()
            .cursor_pointer()
            .when(selected, |d| d.bg(c(theme::BG_HOVER)))
            .hover(|d| d.bg(c(theme::BG_HOVER)))
            .child(vsx_icon(icon, 34.))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.))
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(theme::FG_STRONG))
                            .child(it.display.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::FG_MUTED))
                            .child(it.short.clone()),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w(theme::u(48.))
                                    .gap_1()
                                    .when(it.verified, |h| {
                                        h.child(
                                            Icon::new(Ico::BadgeCheck)
                                                .flex_none()
                                                .size(theme::u(12.))
                                                .text_color(c(theme::ACCENT_TEXT)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .child(it.publisher_display.clone()),
                                    ),
                            )
                            .child(stats)
                            .child(div().flex_none().child(self.market_button(
                                (keys.1, ix),
                                it,
                                cx,
                            ))),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.market_open(open.clone(), cx)))
            .into_any_element()
    }

    fn market_more_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.ext.market;
        let row = h_flex()
            .id("vsx-more")
            .w_full()
            .h(theme::u(ROW_H))
            .px_3()
            .gap_2()
            .justify_center()
            .text_xs()
            .text_color(c(theme::FG_DIM));
        match &m.more_error {
            Some(e) => row
                .cursor_pointer()
                .hover(|d| d.text_color(c(theme::FG_STRONG)))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(crate::trf!("Couldn't load more: {e} · Retry"; e => e)),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.ext.market.more_error = None;
                    this.market_load_more(cx);
                }))
                .into_any_element(),
            None => row
                .child(
                    Spinner::new()
                        .icon(Ico::LoaderCircle)
                        .xsmall()
                        .color(c(theme::FG_DIM)),
                )
                .child(tr("Loading more…"))
                .into_any_element(),
        }
    }

    fn market_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let q = self.market_query(cx);
        let current = q.category;
        let sort = q.sort;
        let label: SharedString = match current {
            Some(cat) => format!("{} · {}", tr(sort.label()), tr(cat)).into(),
            None => tr(sort.label()).into(),
        };
        let has_text = !q.text.is_empty();
        let weak = cx.entity().downgrade();
        Button::new("vsx-filter")
            .icon(Icon::new(Ico::ListFilter))
            .label(label)
            .xsmall()
            .ghost()
            .dropdown_menu(move |mut menu: PopupMenu, _, _| {
                menu = menu.min_w(px(210.)).max_h(px(460.)).scrollable(true).label(tr("Sort by"));
                for s in Sort::ALL {
                    if s == Sort::Relevance && !has_text {
                        continue;
                    }
                    let w = weak.clone();
                    menu = menu.item(PopupMenuItem::new(tr(s.label())).checked(s == sort).on_click(move |_, _, cx| {
                        _ = w.update(cx, |this, cx| {
                            this.ext.market.sort = Some(s);
                            this.market_search(0, cx);
                        });
                    }));
                }
                menu = menu.separator().label(tr("Category"));
                for cat in std::iter::once(None).chain(market::CATEGORIES.iter().copied().map(Some)) {
                    let w = weak.clone();
                    menu = menu.item(PopupMenuItem::new(cat.map(tr).unwrap_or(tr("All categories"))).checked(cat == current).on_click(move |_, _, cx| {
                        _ = w.update(cx, |this, cx| {
                            this.ext.market.category = cat;
                            this.market_search(0, cx);
                        });
                    }));
                }
                menu
            })
            .into_any_element()
    }

    pub(crate) fn render_market_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = &self.ext.market;
        let on_market = self.ext.filter == Filter::Market;
        if on_market && !m.kicked.replace(true) {
            cx.spawn(async move |this, cx| {
                _ = this.update(cx, |this, cx| this.market_search(0, cx));
            })
            .detach();
        }
        let tab = |id: &'static str, label: SharedString, f: Filter, cx: &mut Context<Self>| {
            chip(id, label, self.ext.filter == f).on_click(cx.listener(move |this, _, _, cx| {
                this.ext.filter = f;
                if f == Filter::Market {
                    this.market_search(0, cx);
                }
                cx.notify();
            }))
        };
        let tabs = h_flex()
            .px_2()
            .gap_0p5()
            .min_w_0()
            .child(tab(
                "ext-f-market",
                tr("Marketplace").into(),
                Filter::Market,
                cx,
            ))
            .child(tab(
                "ext-f-inst",
                format!("{} {}", tr("Installed"), m.installed.len()).into(),
                Filter::Installed,
                cx,
            ));
        let status: Option<SharedString> = if !on_market {
            None
        } else if let Some(e) = &m.error {
            Some(crate::trf!("Couldn't reach the Marketplace: {e}"; e => e).into())
        } else {
            m.total
                .map(|t| crate::trf!("{n} results"; n => market::format_thousands(t)).into())
        };
        let status_line = h_flex()
            .px_3()
            .pt_1()
            .gap_2()
            .h(theme::u(20.))
            .text_xs()
            .text_color(c(if m.error.is_some() && on_market {
                theme::DANGER
            } else {
                theme::FG_DIM
            }))
            .child(div().flex_1().min_w_0().truncate().children(status))
            .when(on_market && (m.loading || m.loading_more), |h| {
                h.child(
                    Spinner::new()
                        .icon(Ico::LoaderCircle)
                        .xsmall()
                        .color(c(theme::FG_DIM)),
                )
            });

        let list: AnyElement = if on_market {
            if m.items.is_empty() {
                let text: SharedString = if m.loading || m.shown.is_none() {
                    tr("Searching the Marketplace…").into()
                } else if m.error.is_some() {
                    SharedString::default()
                } else {
                    tr("No Marketplace extension matches.").into()
                };
                div()
                    .px_3()
                    .py_4()
                    .text_xs()
                    .text_color(c(theme::FG_DIM))
                    .child(text)
                    .into_any_element()
            } else {
                let extra = usize::from(!m.done);
                uniform_list(
                    "vsx-list",
                    m.items.len() + extra,
                    cx.processor(|this, range: Range<usize>, _, cx| {
                        this.market_in_view(range.clone(), cx);
                        range
                            .map(|i| match this.ext.market.items.get(i) {
                                Some(it) => {
                                    let icon = this.ext.market.icons.get(&it.id()).cloned();
                                    this.market_row(
                                        ("vsx-row", "vsx-btn"),
                                        i,
                                        it,
                                        icon.as_ref(),
                                        cx,
                                    )
                                }
                                None => this.market_more_row(cx),
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&m.scroll)
                .flex_1()
                .min_h_0()
                .w_full()
                .when(m.loading, |l| l.opacity(0.5))
                .into_any_element()
            }
        } else {
            let q = self.ext.query.read(cx).value().to_lowercase();
            let mut col = v_flex()
                .id("vsx-installed")
                .w_full()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .pb_2();
            let mut n = 0;
            for inst in &m.installed {
                let hay =
                    format!("{} {} {}", inst.display, inst.id, inst.description).to_lowercase();
                if !q.split_whitespace().all(|w| hay.contains(w)) {
                    continue;
                }
                let item = m
                    .items
                    .iter()
                    .find(|r| r.id() == inst.id)
                    .cloned()
                    .unwrap_or_else(|| inst.item());
                let icon = inst.icon.clone().or_else(|| m.icons.get(&inst.id).cloned());
                col = col.child(self.market_row(
                    ("vsx-irow", "vsx-ibtn"),
                    n,
                    &item,
                    icon.as_ref(),
                    cx,
                ));
                n += 1;
            }
            if n == 0 {
                col = col.child(
                    div()
                        .px_3()
                        .py_4()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(if m.installed.is_empty() {
                            tr("No VS Code extension installed yet: find one under Marketplace.")
                        } else {
                            tr("No installed extension matches.")
                        }),
                );
            }
            col.into_any_element()
        };

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_2()
                    .pt_1()
                    .pb_2()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.ext.query).small().cleanable(true)),
                    )
                    .child(
                        Button::new("ext-from-file")
                            .icon(Icon::new(Ico::FolderOpen))
                            .ghost()
                            .small()
                            .tooltip(tr("Install from VSIX"))
                            .on_click(cx.listener(|this, _, _, cx| this.market_install_file(cx))),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .pr_2()
                    .gap_1()
                    .child(tabs)
                    .child(div().flex_1())
                    .when(on_market, |h| h.child(self.market_filters(cx))),
            )
            .child(status_line)
            .when_some(self.ext.note.clone(), |v, n| {
                v.child(
                    div()
                        .px_3()
                        .pt_1()
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .child(n),
                )
            })
            .child(div().h(px(1.)).mt_1().bg(c(theme::BORDER)))
            .child(list)
            .into_any_element()
    }

    pub(crate) fn render_market_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(it) = self.ext.market.open.clone() else {
            return div().into_any_element();
        };
        let id = it.id();
        let m = &self.ext.market;
        let inst = m.installed(&id).cloned();
        let busy = m.busy.get(&id).copied();
        let gallery = !it.asset_uri.is_empty();

        let mut actions = h_flex().flex_none().gap_1();
        actions = match (busy, &inst) {
            (Some(b), _) => actions.child(
                Button::new("vsx-busy")
                    .label(if b == Busy::Installing {
                        tr("Installing")
                    } else {
                        tr("Removing")
                    })
                    .small()
                    .loading(true),
            ),
            (None, Some(i)) => {
                let uid = id.clone();
                let a = actions.child(
                    Button::new("vsx-uninstall")
                        .label(tr("Uninstall"))
                        .small()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.market_uninstall(uid.clone(), cx)
                        })),
                );
                if gallery && i.version != it.version {
                    let up = it.clone();
                    a.child(
                        Button::new("vsx-update")
                            .label(crate::trf!("Update to {v}"; v => it.version))
                            .small()
                            .primary()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.market_install(up.clone(), cx)
                            })),
                    )
                } else {
                    a
                }
            }
            (None, None) => {
                let ins = it.clone();
                actions.child(
                    Button::new("vsx-install")
                        .label(tr("Install"))
                        .small()
                        .primary()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.market_install(ins.clone(), cx)),
                        ),
                )
            }
        };
        let url = format!(
            "https://marketplace.visualstudio.com/items?itemName={}.{}",
            it.publisher, it.name
        );
        actions = actions
            .child(
                Button::new("vsx-web")
                    .icon(Icon::new(Ico::ExternalLink))
                    .small()
                    .ghost()
                    .tooltip(tr("Open on the Marketplace website"))
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            )
            .child(
                Button::new("vsx-close")
                    .icon(Icon::new(Ico::Close))
                    .small()
                    .ghost()
                    .tooltip(tr("Close (Esc)"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.ext.page = None;
                        cx.notify();
                    })),
            );

        let title = h_flex()
            .w_full()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .child(
                div()
                    .flex_1()
                    .min_w(theme::u(160.))
                    .truncate()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(theme::FG_STRONG))
                    .child(it.display.clone()),
            )
            .child(actions);
        let meta = h_flex()
            .w_full()
            .flex_wrap()
            .gap_x_3()
            .gap_y_1()
            .text_xs()
            .text_color(c(theme::FG_DIM))
            .child(
                h_flex()
                    .gap_1()
                    .text_color(c(theme::FG_MUTED))
                    .when(it.verified, |h| {
                        h.child(
                            Icon::new(Ico::BadgeCheck)
                                .size(theme::u(12.))
                                .text_color(c(theme::ACCENT_TEXT)),
                        )
                    })
                    .child(it.publisher_display.clone()),
            )
            .when(inst.is_some(), |h| {
                h.child(
                    h_flex()
                        .gap_1()
                        .text_color(c(theme::GREEN))
                        .child(Icon::new(Ico::Check).size(theme::u(12.)))
                        .child(tr("Installed")),
                )
            })
            .when(it.installs > 0, |h| {
                h.child(
                    h_flex()
                        .gap_1()
                        .child(Icon::new(Ico::Download).size(theme::u(12.)))
                        .child(market::format_count(it.installs)),
                )
            })
            .when_some(it.rating, |h, r| {
                h.child(
                    h_flex()
                        .gap_1()
                        .child(
                            Icon::new(Ico::Star)
                                .size(theme::u(12.))
                                .text_color(c(theme::WARN)),
                        )
                        .child(format!("{r:.1} ({})", it.rating_count)),
                )
            });
        let icon = inst
            .as_ref()
            .and_then(|i| i.icon.clone())
            .or_else(|| m.icons.get(&id).cloned());
        let head = h_flex()
            .w_full()
            .gap_4()
            .items_start()
            .child(vsx_icon(icon.as_ref(), 64.))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(title)
                    .child(meta)
                    .when(!it.short.is_empty(), |v| {
                        v.child(
                            div()
                                .pt_1p5()
                                .text_sm()
                                .text_color(c(theme::FG))
                                .child(it.short.clone()),
                        )
                    }),
            );

        let declared: SharedString = match &inst {
            Some(i) if !i.contributes.is_empty() => {
                crate::trf!("It declares {list}."; list => i.contributes.join(", ")).into()
            }
            Some(_) => tr("It declares no themes, languages or snippets.").into(),
            None => tr("Install it to see what it declares (themes, languages, snippets).").into(),
        };
        let applied = inst.as_ref().map(|_| super::native::applied(&id)).unwrap_or_default();
        let current = theme::palette().id;
        let theme_chips = h_flex().flex_wrap().gap_1().children(applied.themes.iter().enumerate().map(|(i, (tid, name))| {
            let tid = tid.clone();
            let on = tid == current;
            chip(("vsx-theme", i), name.clone().into(), on)
                .border_1()
                .border_color(c(if on { theme::ACCENT } else { theme::BORDER }))
                .on_click(cx.listener(move |this, _, w, cx| this.set_theme(&tid, w, cx)))
        }));
        let applied_line: Option<SharedString> = match (&inst, applied.themes.is_empty(), applied.languages.is_empty()) {
            (None, ..) => None,
            (Some(_), true, true) => Some(tr("Nothing it declares is applied: FluxCode applies color themes and the file types of languages.").into()),
            (Some(_), _, false) => Some(crate::trf!("Files open as {list}."; list => applied.languages.join(", ")).into()),
            (Some(_), false, true) => None,
        };
        let here = v_flex()
            .gap_1()
            .child(div().text_sm().text_color(c(theme::FG)).child(declared))
            .when(!applied.themes.is_empty(), |v| {
                v.child(
                    v_flex()
                        .gap_1()
                        .pt_1()
                        .child(div().text_sm().text_color(c(theme::FG)).child(tr("Its color themes are in the theme menu; click one to use it:")))
                        .child(theme_chips),
                )
            })
            .when_some(applied_line, |v, t| v.child(div().text_sm().text_color(c(theme::FG)).child(t)))
            .when(!applied.failed.is_empty(), |v| {
                v.child(div().text_xs().text_color(c(theme::WARN)).child(crate::trf!("Couldn't read: {list}"; list => applied.failed.join("; "))))
            })
            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr("Grammars, snippets and icon themes aren't applied yet.")))
            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(tr(
                "VS Code's JavaScript extension host isn't available in FluxCode, so code that runs inside VS Code (commands, debuggers, language servers started by the extension) doesn't run here.",
            )))
            .when_some(inst.as_ref().map(|i| i.folder.display().to_string()), |v, f| v.child(div().text_xs().font_family("Cascadia Code").text_color(c(theme::FG_DIM)).child(f)));

        let readme: AnyElement = match m.readme.get(&id) {
            None => h_flex()
                .gap_2()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(
                    Spinner::new()
                        .icon(Ico::LoaderCircle)
                        .xsmall()
                        .color(c(theme::FG_DIM)),
                )
                .child(tr("Loading the README…"))
                .into_any_element(),
            Some(Ok(t)) => {
                let images = m.readme_images.get(&id).cloned().unwrap_or_default();
                let gen = images.len();
                div()
                    .text_sm()
                    .text_color(c(theme::FG))
                    .child(
                        TextView::markdown(SharedString::from(format!("vsx-readme-{id}-{gen}")), t.clone()).image_source(move |uri| match images.get(uri.as_ref()) {
                            Some(p) => ImageSource::from(p.clone()),
                            None => ImageSource::from(uri.clone()),
                        }),
                    )
                    .into_any_element()
            }
            Some(Err(e)) => div()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(e.clone())
                .into_any_element(),
        };

        let section = |t: &'static str| {
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(theme::FG_STRONG))
                .child(tr(t))
        };
        let chip = |t: String| div().px_2().py(px(2.)).rounded(px(4.)).bg(c(theme::BG_ELEVATED)).text_xs().text_color(c(theme::FG_MUTED)).child(t);
        let mut chips = h_flex().w_full().flex_wrap().gap_1p5();
        chips = chips.child(chip(format!("v{}", it.version)));
        if !it.updated.is_empty() {
            chips = chips.child(chip(crate::trf!("Updated {d}"; d => it.updated)));
        }
        for cat in &it.categories {
            chips = chips.child(chip(cat.clone()));
        }
        chips = chips.child(chip(format!("{}.{}", it.publisher, it.name)));
        let body = v_flex()
            .w_full()
            .max_w(theme::u(820.))
            .gap(theme::u(22.))
            .child(head)
            .child(chips)
            .when_some(self.ext.note.clone(), |v, n| {
                v.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(n))
            })
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .child(here),
            )
            .child(div().h(px(1.)).bg(c(theme::BORDER)))
            .child(v_flex().gap_2().child(section("README")).child(readme));
        let page_el = v_flex()
            .id("vsx-page")
            .key_context("ExtPage")
            .size_full()
            .items_center()
            .px(theme::u(24.))
            .pt(theme::u(28.))
            .pb(theme::u(40.))
            .overflow_y_scroll()
            .bg(c(theme::BG_EDITOR))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.ext.page = None;
                    cx.notify();
                }
            }))
            .child(body);
        if cx.reduce_motion() {
            page_el.into_any_element()
        } else {
            page_el
                .with_animation(
                    "vsx-page-in",
                    Animation::new(std::time::Duration::from_millis(150)).with_easing(gpui_kit::ease_out_quint()),
                    |el, t| el.opacity(t),
                )
                .into_any_element()
        }
    }
}
