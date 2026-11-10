//! The Live share page: it opens in the editor area (the people icon in the activity
//! bar), where there is room for it. Cards sit two to a row on a wide window and one
//! under the other on a narrow one (each row wraps once a card would get narrower than
//! `CARD_MIN`).

use super::{my_name, reach, Role};
use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    spinner::Spinner,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

/// Narrower than this, a card takes the whole row.
const CARD_MIN: f32 = 300.;

impl Workbench {
    pub(crate) fn render_collab_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let mut col = v_flex().w_full().max_w(theme::u(1080.)).px_8().pt_8().pb_10().gap_6().child(self.page_header(cx));
        if let Some(e) = &co.error {
            col = col.child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .px_4()
                    .py_3()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(theme::DANGER))
                    .child(div().flex_none().pt(px(1.)).child(Icon::new(IconName::CircleAlert).size(theme::u(16.)).text_color(c(theme::DANGER))))
                    .child(div().flex_1().min_w_0().text_sm().text_color(c(theme::FG)).child(e.clone()))
                    .child(Button::new("co-err-x").icon(Icon::new(IconName::Close)).xsmall().ghost().tooltip(tr("Dismiss")).on_click(cx.listener(|this, _, _, cx| {
                        this.collab.error = None;
                        cx.notify();
                    }))),
            );
        }
        col = match &co.role {
            None => col.child(row([self.host_card(cx), self.join_card(cx)], true)).child(self.how_it_works()),
            Some(Role::Host { server, .. }) => col.child(row([self.invite_card(cx), self.ways_card(server.port)], false)).child(row([self.people_card(), self.activity_card(cx)], false)),
            Some(Role::Guest { addr, .. }) => col.child(row([self.connection_card(addr), self.people_card()], false)).child(row([self.files_card(cx), self.activity_card(cx)], false)),
        };
        div()
            .id("collab-page")
            .size_full()
            .min_w_0()
            // A long path must not widen the page past the window.
            .overflow_x_hidden()
            .overflow_y_scroll()
            .bg(theme::editor_wash())
            .text_sm()
            .child(h_flex().w_full().justify_center().child(col))
            .into_any_element()
    }

    /// Title, what the page is for, where the session stands, and how to leave it.
    fn page_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let (dot, state): (theme::Tok, SharedString) = match &co.role {
            None => (theme::FG_DIM, tr("Not in a session").into()),
            Some(Role::Host { .. }) => (theme::GREEN, crate::trf!("Live · hosting {folder}"; folder => co.folder).into()),
            Some(Role::Guest { .. }) => {
                let gh = co.peers.iter().any(|p| p.host && p.github);
                let host = if gh { format!("@{}", co.host_name) } else { co.host_name.clone() };
                (theme::GREEN, crate::trf!("Connected · {host}'s {folder}"; host => host, folder => co.folder).into())
            }
        };
        let action = match &co.role {
            None => None,
            Some(Role::Host { .. }) => Some(Button::new("co-end").label(tr("End session")).small().outline().tooltip(tr("End the session for everyone")).on_click(cx.listener(|this, _, _, cx| this.collab_stop("The host ended the session.", cx)))),
            Some(Role::Guest { .. }) => Some(Button::new("co-leave").label(tr("Leave")).small().outline().on_click(cx.listener(|this, _, _, cx| this.collab_stop("", cx)))),
        };
        h_flex()
            .w_full()
            .gap_4()
            .flex_wrap()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .size(theme::u(44.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .bg(c(theme::BG_ELEVATED))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Users).size(theme::u(22.)).text_color(c(theme::ACCENT_TEXT))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(theme::u(260.))
                    .gap_0p5()
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Live share")))
                    .child(div().text_color(c(theme::FG_MUTED)).child(tr("Code together in real time. Your PC is the server: no account, no VPN, no server to rent."))),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .px_3()
                            .py_1p5()
                            .rounded_full()
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_ELEVATED))
                            .child(div().size(px(8.)).rounded_full().bg(c(dot)))
                            .child(div().max_w(theme::u(320.)).truncate().text_color(c(if co.active() { theme::FG_STRONG } else { theme::FG_MUTED })).child(state)),
                    )
                    .children(action),
            )
            .into_any_element()
    }

    fn host_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let folder = self.root.as_ref().map(|r| (r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), r.display().to_string()));
        let on = co.internet;
        card(IconName::Users, tr("Host a session"), None)
            .child(body(match &folder {
                Some((f, _)) => crate::trf!("Share {folder} with the people you invite. Edits go straight between your PC and each guest, encrypted with the invite code."; folder => f).into(),
                None => tr("Open a folder first: the session shares it.").into(),
            }))
            .when_some(folder.as_ref(), |v, (_, path)| {
                v.child(
                    h_flex()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .bg(c(theme::BG_EDITOR))
                        .border_1()
                        .border_color(c(theme::BORDER))
                        .child(Icon::new(IconName::FolderOpen).size(theme::u(14.)).text_color(c(theme::FOLDER)))
                        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).child(path.clone())),
                )
            })
            .child(
                h_flex()
                    .id("co-net")
                    .gap_3()
                    .items_start()
                    .p_3()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(if on { theme::ACCENT } else { theme::BORDER }))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(
                        div()
                            .flex_none()
                            .mt(px(2.))
                            .size(theme::u(16.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(if on { theme::ACCENT } else { theme::BORDER_STRONG }))
                            .when(on, |d| d.bg(c(theme::ACCENT)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(on, |d| d.child(Icon::new(IconName::Check).size(theme::u(12.)).text_color(c(theme::ON_ACCENT)))),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(tr("Let people join over the internet")))
                            .child(meta(tr("Through your router, or a free relay that passes only encrypted bytes. No VPN or server.").into())),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.collab.internet = !this.collab.internet;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(
                h_flex().child(
                    Button::new("co-host")
                        .label(tr("Start hosting"))
                        .icon(Icon::new(IconName::Play))
                        .when(folder.is_some(), |b| b.primary())
                        .when(folder.is_none(), |b| b.outline())
                        .on_click(cx.listener(|this, _, w, cx| this.collab_host(w, cx))),
                ),
            )
            .into_any_element()
    }

    fn join_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let into_open = co.into_open_folder;
        let pick = |id: &'static str, label: &'static str, open: bool| {
            let on = into_open == open;
            div()
                .id(id)
                .flex_1()
                .min_w_0()
                .py_1p5()
                .rounded(px(5.))
                .text_center()
                .truncate()
                .cursor_pointer()
                .when(on, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .when(!on, |d| d.text_color(c(theme::FG_MUTED)).hover(|s| s.text_color(c(theme::FG))))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.collab.into_open_folder = open;
                    cx.notify();
                }))
        };
        let has_code = !co.join_input.read(cx).value().trim().is_empty();
        let (me, github) = my_name();
        card(IconName::LogIn, tr("Join a session"), None)
            .child(body(tr("Paste the code the host sent you.").into()))
            .child(Input::new(&co.join_input))
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_color(c(theme::FG_MUTED)).child(tr("Files go to")))
                    .child(
                        h_flex()
                            .p_0p5()
                            .gap_0p5()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_EDITOR))
                            .child(pick("co-into-new", tr("A new folder"), false))
                            .child(pick("co-into-open", tr("My open folder"), true)),
                    )
                    .child(meta(if into_open {
                        tr("Your open folder should be your clone of the same repository; the host's version of each file you open replaces yours.").into()
                    } else {
                        tr("Files arrive from the host as you open them, into a folder of their own.").into()
                    })),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        Button::new("co-join")
                            .label(if co.connecting { tr("Connecting…") } else { tr("Join") })
                            .icon(Icon::new(IconName::LogIn))
                            .when(has_code, |b| b.primary())
                            .when(!has_code, |b| b.outline())
                            .loading(co.connecting)
                            .on_click(cx.listener(|this, _, w, cx| this.collab_join(w, cx))),
                    )
                    .child(div().flex_1().min_w(theme::u(160.)).child(meta(if github {
                        crate::trf!("Others see you as @{name} (GitHub)."; name => me).into()
                    } else {
                        crate::trf!("Others see you as {name}. Sign in to GitHub in Settings so they see your GitHub name."; name => me).into()
                    }))),
            )
            .into_any_element()
    }

    /// Three steps, under the two cards before a session.
    fn how_it_works(&self) -> AnyElement {
        let step = |n: &'static str, title: &'static str, text: &'static str| {
            h_flex()
                .flex_1()
                .min_w(theme::u(220.))
                .gap_3()
                .items_start()
                .child(
                    div()
                        .flex_none()
                        .size(theme::u(24.))
                        .rounded_full()
                        .border_1()
                        .border_color(c(theme::BORDER_STRONG))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(theme::FG_MUTED))
                        .child(n),
                )
                .child(v_flex().flex_1().min_w_0().gap_0p5().child(div().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(tr(title))).child(meta(tr(text).into())))
        };
        h_flex()
            .w_full()
            .flex_wrap()
            .items_start()
            .gap_6()
            .px_1()
            .child(step("1", "Host", "Start hosting and copy the invite code."))
            .child(step("2", "Send the code", "In any chat or e-mail. It works on the same network and over the internet."))
            .child(step("3", "Code together", "Edits merge live. An edit an AI agent made says which agent and model."))
            .into_any_element()
    }

    fn invite_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let code = co.invite.as_ref().map(|i| i.code()).unwrap_or_default();
        let reach = co.reach.clone().unwrap_or_else(|| reach::Reach::local_only(None));
        let copied = co.copied.is_some_and(|t| t.elapsed().as_millis() < 1800);
        let stale = co.copied_code.as_ref().is_some_and(|c| *c != code);
        let (sum_icon, sum_tint, sum_text): (AnyElement, theme::Tok, SharedString) = if reach.internet() {
            (Icon::new(IconName::Globe).size(theme::u(15.)).text_color(c(theme::GREEN)).into_any_element(), theme::FG, tr("Anyone with this code can join, from anywhere.").into())
        } else if reach.checking() {
            (Spinner::new().icon(IconName::LoaderCircle).small().color(c(theme::FG_MUTED)).into_any_element(), theme::FG_MUTED, tr("Opening ways in from the internet. The code updates as each one opens.").into())
        } else if !co.internet {
            (Icon::new(IconName::House).size(theme::u(15.)).text_color(c(theme::FG_MUTED)).into_any_element(), theme::FG_MUTED, tr("Only people on your network can join.").into())
        } else if matches!(reach.ipv6, reach::State::Ready(_)) {
            (Icon::new(IconName::Globe).size(theme::u(15.)).text_color(c(theme::WARN)).into_any_element(), theme::FG, tr("Guests with IPv6 may join over the internet; everyone else only from your network.").into())
        } else {
            (Icon::new(IconName::TriangleAlert).size(theme::u(15.)).text_color(c(theme::WARN)).into_any_element(), theme::FG, tr("Only people on your network can join: no way in from the internet worked (see below).").into())
        };
        card(IconName::Link, tr("Invite code"), None)
            .child(body(crate::trf!("Anyone with this code can join {folder} and edit it with you. Send it only to the people you invite."; folder => co.folder).into()))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(h_flex()
                    .id("co-code")
                    .flex_1()
                    .min_w(theme::u(220.))
                    .gap_3()
                    .px_3()
                    .py_2p5()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER_STRONG))
                    .bg(c(theme::BG_EDITOR))
                    .cursor_pointer()
                    .hover(|s| s.border_color(c(theme::ACCENT)))
                    .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new(tr("Click to copy")).build(w, cx))
                    .child(div().flex_1().min_w_0().truncate().font_family("Cascadia Code").text_color(c(theme::FG)).child(code))
                    .child(Icon::new(if copied { IconName::Check } else { IconName::Copy }).size(theme::u(15.)).text_color(c(if copied { theme::GREEN } else { theme::FG_MUTED })))
                    .on_click(cx.listener(|this, _, _, cx| this.collab_copy_invite(cx))))
                    .child(Button::new("co-copy").label(if copied { tr("Copied") } else { tr("Copy invite code") }).icon(Icon::new(if copied { IconName::Check } else { IconName::Copy })).primary().on_click(cx.listener(|this, _, _, cx| this.collab_copy_invite(cx)))),
            )
            .child(h_flex().gap_3().items_center().child(sum_icon).child(div().flex_1().min_w_0().text_color(c(sum_tint)).child(sum_text)))
            .when(stale, |v| {
                v.child(
                    h_flex()
                        .gap_3()
                        .items_start()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .bg(c(theme::BG_HOVER))
                        .child(div().flex_none().pt(px(2.)).child(Icon::new(IconName::RefreshCw).size(theme::u(14.)).text_color(c(theme::WARN))))
                        .child(div().flex_1().min_w_0().text_color(c(theme::FG)).child(tr("The code changed after you copied it (a new way in opened). Copy it again before you send it."))),
                )
            })
            .into_any_element()
    }

    fn ways_card(&self, port: u16) -> AnyElement {
        let reach = self.collab.reach.clone().unwrap_or_else(|| reach::Reach::local_only(None));
        let lan = match &reach.lan {
            Some(ip) => reach::State::Ready(format!("{ip}:{port}")),
            None => reach::State::No(tr("No network connection.").into()),
        };
        let open = [&lan, &reach.upnp, &reach.ipv6, &reach.relay].iter().filter(|s| matches!(s, reach::State::Ready(_))).count();
        card(IconName::Network, tr("Ways in"), Some(format!("{open}/4")))
            .child(body(tr("Every way in that opened is in the code. A guest's FluxCode tries them all at once and keeps the first that answers.").into()))
            .child(
                v_flex()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .child(way_row(IconName::House, tr("Same network"), &lan, None, false))
                    .child(way_row(IconName::Router, tr("Router port (UPnP)"), &reach.upnp, None, true))
                    .child(way_row(IconName::Globe, tr("Public IPv6"), &reach.ipv6, Some(tr("For guests with IPv6, when your router lets connections in.")), true))
                    .child(way_row(IconName::RadioTower, tr("Relay (bore.pub)"), &reach.relay, Some(tr("Works from any network.")), true)),
            )
            .child(meta(tr("The relay only passes bytes encrypted with the invite code's secret. If someone can't connect straight to you, allow FluxCode through the firewall.").into()))
            .into_any_element()
    }

    fn connection_card(&self, addr: &str) -> AnyElement {
        let (icon, via): (IconName, SharedString) = match super::path_kind(addr) {
            super::Via::Lan => (IconName::House, tr("Straight to the host over your network, encrypted.").into()),
            super::Via::Relay => (IconName::RadioTower, tr("Through the bore.pub relay, encrypted end to end: the relay can't read it.").into()),
            super::Via::Internet => (IconName::Globe, tr("Straight to the host over the internet, encrypted.").into()),
        };
        let root = self.collab.root.as_ref().map(|r| r.display().to_string()).unwrap_or_default();
        card(IconName::Link, tr("Connection"), None)
            .child(h_flex().gap_3().items_start().child(div().flex_none().pt(px(2.)).child(Icon::new(icon).size(theme::u(16.)).text_color(c(theme::FG_MUTED)))).child(div().flex_1().min_w_0().text_color(c(theme::FG)).child(via)))
            .child(info_line(tr("Address"), addr.to_string(), true))
            .child(info_line(tr("Files on this PC"), root, false))
            .into_any_element()
    }

    fn people_card(&self) -> AnyElement {
        let co = &self.collab;
        let mut list = v_flex().gap_1();
        for p in &co.peers {
            let initial: String = p.name.chars().next().unwrap_or('?').to_uppercase().collect();
            let place = if p.file.is_empty() { tr("No file open").to_string() } else { format!("{}:{}", p.file, p.line) };
            let pill = |t: &'static str| div().flex_none().px_1p5().rounded(px(4.)).border_1().border_color(c(theme::BORDER_STRONG)).text_xs().text_color(c(theme::FG_MUTED)).child(t);
            list = list.child(
                h_flex()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(px(6.))
                    .bg(c(theme::BG_EDITOR))
                    .child(
                        div()
                            .flex_none()
                            .size(theme::u(30.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(c(if p.host { theme::ACCENT } else { theme::BG_HOVER }))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(if p.host { theme::ON_ACCENT } else { theme::FG_STRONG }))
                            .child(initial),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(if p.github { format!("@{}", p.name) } else { p.name.clone() }))
                                    .when(p.host, |h| h.child(pill(tr("host"))))
                                    .when(p.id == co.me, |h| h.child(pill(tr("you")))),
                            )
                            .child(div().truncate().text_xs().text_color(c(theme::FG_DIM)).child(place)),
                    ),
            );
        }
        card(IconName::Users, tr("People"), Some(co.peers.len().to_string()))
            .when(co.peers.len() <= 1, |v| v.child(body(if self.collab.is_host() { tr("Nobody else yet. They show here as soon as they join.").into() } else { tr("Only you so far.").into() })))
            .child(list)
            .into_any_element()
    }

    /// Guest: the host's files.
    fn files_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let q = co.filter_input.read(cx).value().to_lowercase();
        let hits: Vec<&String> = co.files.iter().filter(|f| q.is_empty() || f.to_lowercase().contains(&q)).take(200).collect();
        let mut list = v_flex().id("co-files").max_h(theme::u(360.)).overflow_y_scroll();
        for (i, f) in hits.into_iter().enumerate() {
            let rel = f.clone();
            let (icon, tint) = crate::files::icon_for(f.rsplit('/').next().unwrap_or(f));
            let open = co.docs.contains_key(f);
            list = list.child(
                h_flex()
                    .id(("co-file", i))
                    .flex_none()
                    .h(theme::u(28.))
                    .gap_2()
                    .px_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                    .child(div().flex_1().min_w_0().truncate().text_color(c(if open { theme::FG_STRONG } else { theme::FG })).child(f.clone()))
                    .when(open, |h| h.child(div().flex_none().text_xs().text_color(c(theme::GREEN)).child(tr("open"))))
                    .on_click(cx.listener(move |this, _, w, cx| this.collab_open(rel.clone(), w, cx))),
            );
        }
        card(IconName::FolderOpen, tr("Shared files"), Some(co.files.len().to_string()))
            .child(body(tr("Open a file to edit it with the others. It comes from the host and stays in sync.").into()))
            .child(Input::new(&co.filter_input).small())
            .child(list)
            .into_any_element()
    }

    fn activity_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let mut list = v_flex().id("co-feed-list").max_h(theme::u(420.)).overflow_y_scroll().gap_1();
        for (i, f) in co.feed.iter().enumerate() {
            let secs = f.at.elapsed().as_secs();
            let ago = if secs < 60 { tr("just now").to_string() } else { crate::trf!("{m} min ago"; m => secs / 60) };
            list = list.child(
                v_flex()
                    .id(("co-feed", i))
                    .flex_none()
                    .px_3()
                    .py_2()
                    .gap_1()
                    .rounded(px(6.))
                    .bg(c(theme::BG_EDITOR))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(f.who.clone()))
                            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_MUTED)).child(f.rel.clone()))
                            .child(div().flex_none().text_xs().text_color(c(theme::GREEN)).child(format!("+{}", f.lines.0)))
                            .child(div().flex_none().text_xs().text_color(c(theme::DANGER)).child(format!("-{}", f.lines.1))),
                    )
                    .when_some(f.ai.as_ref(), |v, ai| {
                        v.child(
                            h_flex()
                                .gap_1p5()
                                .child(div().flex_none().px_1().rounded(px(3.)).bg(c(theme::ACCENT)).text_size(theme::u(10.)).font_weight(FontWeight::BOLD).text_color(c(theme::ON_ACCENT)).child("AI"))
                                .child(div().min_w_0().truncate().text_xs().text_color(c(theme::ACCENT_TEXT)).child(ai.line())),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().text_xs().text_color(c(theme::FG_DIM)).child(ago))
                            .child(Button::new(("co-diff", i)).label(tr("Diff")).xsmall().ghost().on_click(cx.listener(move |this, _, w, cx| this.collab_show_diff(i, w, cx)))),
                    ),
            );
        }
        card(IconName::ScrollText, tr("Activity"), (!co.feed.is_empty()).then(|| co.feed.len().to_string()))
            .when(co.feed.is_empty(), |v| v.child(body(tr("Edits show here as people make them. An edit made by an AI agent says which agent, model and effort.").into())))
            .child(list)
            .into_any_element()
    }
}

// ---------------------------------------------------------------- pieces

/// A row of cards that wraps to one per row on a narrow window. `even`: both as tall
/// as the taller one (their buttons line up at the bottom).
fn row(cards: [AnyElement; 2], even: bool) -> Div {
    h_flex()
        .w_full()
        .flex_wrap()
        .gap_5()
        .when(even, |h| h.items_stretch())
        .when(!even, |h| h.items_start())
        .children(cards.into_iter().map(|c| v_flex().flex_1().min_w(theme::u(CARD_MIN)).overflow_hidden().child(c)))
}

fn card(icon: IconName, title: &'static str, count: Option<String>) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .flex_1()
        .gap_4()
        .p_5()
        .rounded(px(10.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .child(
            h_flex()
                .gap_2p5()
                .child(Icon::new(icon).size(theme::u(17.)).text_color(c(theme::FG_MUTED)))
                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
                .when_some(count, |h, n| h.child(div().px_1p5().rounded(px(4.)).bg(c(theme::BG_HOVER)).text_xs().text_color(c(theme::FG_MUTED)).child(n))),
        )
}

fn body(t: SharedString) -> Div {
    div().text_color(c(theme::FG_MUTED)).child(t)
}

fn meta(t: SharedString) -> Div {
    div().text_xs().text_color(c(theme::FG_DIM)).child(t)
}

fn info_line(label: &'static str, value: String, mono: bool) -> Div {
    h_flex()
        .gap_3()
        .child(div().flex_none().w(theme::u(120.)).text_color(c(theme::FG_DIM)).child(label))
        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).when(mono, |d| d.font_family("Cascadia Code")).child(value))
}

/// One way in: what it is, its address or why it isn't open, and a mark.
fn way_row(icon: IconName, name: &'static str, st: &reach::State, hint: Option<&'static str>, line: bool) -> Div {
    let (detail, mono, mark): (SharedString, bool, AnyElement) = match st {
        reach::State::Trying => (tr("Checking…").into(), false, Spinner::new().icon(IconName::LoaderCircle).small().color(c(theme::FG_MUTED)).into_any_element()),
        reach::State::Ready(a) => (a.clone().into(), true, Icon::new(IconName::CircleCheck).size(theme::u(16.)).text_color(c(theme::GREEN)).into_any_element()),
        reach::State::No(why) => (why.clone().into(), false, Icon::new(IconName::Minus).size(theme::u(16.)).text_color(c(theme::FG_DIM)).into_any_element()),
        reach::State::Off => (tr("Off: you chose same network only.").into(), false, Icon::new(IconName::Minus).size(theme::u(16.)).text_color(c(theme::FG_DIM)).into_any_element()),
    };
    let ready = matches!(st, reach::State::Ready(_));
    h_flex()
        .gap_3()
        .items_start()
        .px_3()
        .py_2p5()
        .when(line, |d| d.border_t_1().border_color(c(theme::BORDER)))
        .child(div().flex_none().pt(px(1.)).child(Icon::new(icon).size(theme::u(16.)).text_color(c(if ready { theme::FG_STRONG } else { theme::FG_DIM }))))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().font_weight(FontWeight::MEDIUM).text_color(c(if ready { theme::FG_STRONG } else { theme::FG_MUTED })).child(name))
                .child(div().text_xs().text_color(c(if ready { theme::FG } else { theme::FG_DIM })).when(mono, |d| d.truncate().font_family("Cascadia Code")).child(detail))
                .when_some(hint.filter(|_| ready), |v, h| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(h))),
        )
        .child(div().flex_none().pt(px(1.)).child(mark))
}
