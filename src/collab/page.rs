//! The Live share page: it opens in the editor area (the people icon in the activity
//! bar). Before a session it offers two choices, host this folder or join with a code,
//! side by side on a wide window and one under the other on a narrow one. While a
//! session runs, the thing people need most comes first: the invite code for a host,
//! the connection for a guest. Then who is in, how guests get in, and the edits.

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
    tooltip::Tooltip,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

/// Narrower than this, a card takes the whole row.
const CARD_MIN: f32 = 260.;
const MONO: &str = "Cascadia Code";

impl Workbench {
    pub(crate) fn render_collab_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let mut col = v_flex().w_full().min_w_0().max_w(theme::u(960.)).px_5().pt_6().pb_10().gap_5().child(self.page_header(cx));
        if let Some(e) = &co.error {
            col = col.child(error_banner(e.clone(), cx));
        }
        col = match &co.role {
            None => col
                .child(row([self.host_card(cx), self.join_card(cx)], true))
                .child(div().px_1().text_xs().text_color(c(theme::FG_DIM)).child(tr("Edits go straight between your PC and each guest, encrypted with the invite code. No account or server needed."))),
            Some(Role::Host { server, .. }) => col.child(self.invite_card(cx)).child(row([self.people_card(), self.ways_card(server.port, cx)], false)).child(self.activity_card(cx)),
            Some(Role::Guest { addr, .. }) => col.child(self.connection_card(addr)).child(row([self.files_card(cx), self.people_card()], false)).child(self.activity_card(cx)),
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

    /// The title, where the session stands, and how to end it.
    fn page_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let state: Option<SharedString> = match &co.role {
            None => None,
            Some(Role::Host { .. }) => Some(crate::trf!("Hosting {folder}"; folder => co.folder).into()),
            Some(Role::Guest { .. }) => {
                let gh = co.peers.iter().any(|p| p.host && p.github);
                let host = if gh { format!("@{}", co.host_name) } else { co.host_name.clone() };
                Some(crate::trf!("In {host}'s {folder}"; host => host, folder => co.folder).into())
            }
        };
        let action = match &co.role {
            None => None,
            Some(Role::Host { .. }) => Some(Button::new("co-end").label(tr("End session")).small().outline().tooltip(tr("End the session for everyone")).on_click(cx.listener(|this, _, _, cx| this.collab_stop("The host ended the session.", cx)))),
            Some(Role::Guest { .. }) => Some(Button::new("co-leave").label(tr("Leave")).small().outline().on_click(cx.listener(|this, _, _, cx| this.collab_stop("", cx)))),
        };
        h_flex()
            .w_full()
            .gap_x_4()
            .gap_y_2()
            .flex_wrap()
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w(theme::u(160.))
                    .gap_1()
                    .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(tr("Live share")))
                    .child(match state {
                        None => div().text_color(c(theme::FG_MUTED)).child(tr("Code together in real time, from your own PC.")).into_any_element(),
                        Some(s) => h_flex()
                            .gap_2()
                            .min_w_0()
                            .child(div().flex_none().size(px(7.)).rounded_full().bg(c(theme::GREEN)))
                            .child(div().min_w_0().truncate().text_color(c(theme::FG)).child(s))
                            .into_any_element(),
                    }),
            )
            .children(action)
            .into_any_element()
    }

    /// Before a session: share the open folder.
    fn host_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let folder = self.root.as_ref().map(|r| (r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), r.display().to_string()));
        let on = co.internet;
        // A typed invite code says what this person came to do: Join is then the one primary button.
        let joining = !co.join_input.read(cx).value().trim().is_empty();
        let Some((name, path)) = folder else {
            return card(IconName::Users, tr("Host this folder"), None, None)
                .child(body(tr("Open a folder first: the session shares it.").into()))
                .child(h_flex().child(Button::new("co-open-folder").label(tr("Open folder")).icon(Icon::new(IconName::FolderOpen)).outline().on_click(|_, w, cx| w.dispatch_action(Box::new(crate::workbench::OpenFolder), cx))))
                .into_any_element();
        };
        let start = Button::new("co-host")
            .label(tr("Start hosting"))
            .icon(Icon::new(IconName::Play))
            .when(!joining, |b| b.primary())
            .when(joining, |b| b.outline())
            .on_click(cx.listener(|this, _, w, cx| this.collab_host(w, cx)));
        card(IconName::Users, tr("Host this folder"), None, None)
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        h_flex()
                            .id("co-folder")
                            .flex_1()
                            .min_w(theme::u(160.))
                            .h(theme::u(34.))
                            .gap_2()
                            .px_3()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_EDITOR))
                            .tooltip(move |w, cx| Tooltip::new(path.clone()).build(w, cx))
                            .child(Icon::new(IconName::FolderOpen).size(theme::u(15.)).text_color(c(theme::FOLDER)))
                            .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(name)),
                    )
                    .child(start),
            )
            .child(
                h_flex()
                    .id("co-net")
                    .gap_2p5()
                    .items_start()
                    .cursor_pointer()
                    .child(checkbox(on))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_color(c(theme::FG)).child(tr("Allow joining over the internet")))
                            .child(meta(if on { tr("Through your router, or an encrypted relay.") } else { tr("Only people on your network can join.") }.into())),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.collab.internet = !this.collab.internet;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// Before a session: join someone else's with the code they sent.
    fn join_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let into_open = co.into_open_folder;
        let pick = |id: &'static str, label: &'static str, open: bool| {
            let on = into_open == open;
            div()
                .id(id)
                .flex_1()
                .min_w_0()
                .px_1()
                .py_1()
                .rounded(px(5.))
                .text_center()
                .truncate()
                .cursor_pointer()
                .when(on, |d| d.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                .when(!on, |d| d.text_color(c(theme::FG_MUTED)).hover(|s| s.text_color(c(theme::FG))))
                .child(label)
                // The whole label, when a narrow window cuts it short.
                .tooltip(move |w, cx| Tooltip::new(label).build(w, cx))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.collab.into_open_folder = open;
                    cx.notify();
                }))
        };
        let has_code = !co.join_input.read(cx).value().trim().is_empty();
        let (me, github) = my_name();
        card(IconName::LogIn, tr("Join with a code"), None, None)
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(div().flex_1().min_w(theme::u(160.)).child(Input::new(&co.join_input)))
                    .child(
                        Button::new("co-join")
                            .label(if co.connecting { tr("Connecting…") } else { tr("Join") })
                            .when(has_code, |b| b.primary())
                            .when(!has_code, |b| b.outline())
                            .loading(co.connecting)
                            .on_click(cx.listener(|this, _, w, cx| this.collab_join(w, cx))),
                    ),
            )
            .child(
                h_flex()
                    .gap_x_3()
                    .gap_y_1p5()
                    .flex_wrap()
                    .child(div().flex_none().text_color(c(theme::FG_MUTED)).child(tr("Files go to")))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(theme::u(170.))
                            .p_0p5()
                            .gap_0p5()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(theme::BORDER))
                            .bg(c(theme::BG_EDITOR))
                            .child(pick("co-into-new", tr("New folder"), false))
                            .child(pick("co-into-open", tr("My open folder"), true)),
                    ),
            )
            .when(into_open, |v| v.child(meta(tr("Use your clone of the same repository. The host's version of each file you open replaces yours.").into())))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(meta(if github { crate::trf!("You appear as @{name}."; name => me).into() } else { crate::trf!("You appear as {name}."; name => me).into() }))
                    .when(!github, |h| {
                        h.child(Button::new("co-gh").label(tr("Use my GitHub name")).xsmall().ghost().tooltip(tr("Sign in to GitHub in Settings")).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::workbench::ShowSettings), cx)))
                    }),
            )
            .into_any_element()
    }

    /// Host: the code to send, front and centre, and who it lets in.
    fn invite_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let code = co.invite.as_ref().map(|i| i.code()).unwrap_or_default();
        let reach = co.reach.clone().unwrap_or_else(|| reach::Reach::local_only(None));
        let copied = co.copied.is_some_and(|t| t.elapsed().as_millis() < 1800);
        let stale = co.copied_code.as_ref().is_some_and(|c| *c != code);
        let (sum_icon, sum_text): (AnyElement, SharedString) = if reach.internet() {
            (Icon::new(IconName::Globe).size(theme::u(14.)).text_color(c(theme::GREEN)).into_any_element(), tr("Guests can join from anywhere.").into())
        } else if reach.checking() {
            (Spinner::new().icon(IconName::LoaderCircle).small().color(c(theme::FG_MUTED)).into_any_element(), tr("Opening ways in from the internet. The code updates as they open.").into())
        } else if !co.internet {
            (Icon::new(IconName::House).size(theme::u(14.)).text_color(c(theme::FG_MUTED)).into_any_element(), tr("Only people on your network can join.").into())
        } else if matches!(reach.ipv6, reach::State::Ready(_)) {
            (Icon::new(IconName::Globe).size(theme::u(14.)).text_color(c(theme::WARN)).into_any_element(), tr("Guests with IPv6 may join over the internet; everyone else only from your network.").into())
        } else {
            (Icon::new(IconName::TriangleAlert).size(theme::u(14.)).text_color(c(theme::WARN)).into_any_element(), tr("Only people on your network can join. No way in from the internet opened (see Ways in).").into())
        };
        card(IconName::Link, tr("Invite code"), None, None)
            .child(body(crate::trf!("Anyone with this code can edit {folder} with you. Send it only to people you trust."; folder => co.folder).into()))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        h_flex()
                            .id("co-code")
                            .flex_1()
                            .min_w(theme::u(160.))
                            .h(theme::u(36.))
                            .px_3()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(theme::BORDER_STRONG))
                            .bg(c(theme::BG_EDITOR))
                            .cursor_pointer()
                            .hover(|s| s.border_color(c(theme::ACCENT)))
                            .tooltip(|w, cx| Tooltip::new(tr("Click to copy")).build(w, cx))
                            .child(div().flex_1().min_w_0().truncate().font_family(MONO).text_color(c(theme::FG_STRONG)).child(code))
                            .on_click(cx.listener(|this, _, _, cx| this.collab_copy_invite(cx))),
                    )
                    .child(
                        Button::new("co-copy")
                            .label(if copied { tr("Copied") } else { tr("Copy") })
                            .icon(Icon::new(if copied { IconName::Check } else { IconName::Copy }))
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.collab_copy_invite(cx))),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(div().flex_none().pt(px(2.)).child(sum_icon))
                    .child(div().flex_1().min_w_0().text_color(c(theme::FG_MUTED)).child(sum_text)),
            )
            .when(stale, |v| {
                v.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .bg(c(theme::BG_HOVER))
                        .child(div().flex_none().pt(px(2.)).child(Icon::new(IconName::RefreshCw).size(theme::u(14.)).text_color(c(theme::WARN))))
                        .child(div().flex_1().min_w_0().text_color(c(theme::FG)).child(tr("The code changed after you copied it. Copy it again before you send it."))),
                )
            })
            .into_any_element()
    }

    /// Host: each way a guest can get in, with a mark; the addresses and reasons fold out.
    fn ways_card(&self, port: u16, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let reach = co.reach.clone().unwrap_or_else(|| reach::Reach::local_only(None));
        let lan = match &reach.lan {
            Some(ip) => reach::State::Ready(format!("{ip}:{port}")),
            None => reach::State::No(tr("No network connection.").into()),
        };
        let open = [&lan, &reach.upnp, &reach.ipv6, &reach.relay].iter().filter(|s| matches!(s, reach::State::Ready(_))).count();
        let full = co.ways_open;
        let toggle = Button::new("co-ways-more")
            .label(if full { tr("Less") } else { tr("Details") })
            .icon(Icon::new(if full { IconName::ChevronDown } else { IconName::ChevronRight }))
            .xsmall()
            .ghost()
            .on_click(cx.listener(|this, _, _, cx| {
                this.collab.ways_open = !this.collab.ways_open;
                cx.notify();
            }));
        // Same network only: the three internet ways were never tried, so one line says so.
        let local = [&reach.upnp, &reach.ipv6, &reach.relay].iter().all(|s| matches!(s, reach::State::Off));
        let count = (!local).then(|| format!("{open}/4"));
        card(IconName::Network, tr("Ways in"), count, Some(toggle.into_any_element()))
            .child(
                v_flex()
                    .gap(theme::u(if full { 10. } else { 6. }))
                    .child(way_row(0, tr("Same network"), &lan, None, full))
                    .when(!local, |v| {
                        v.child(way_row(1, tr("Router port (UPnP)"), &reach.upnp, None, full))
                            .child(way_row(2, tr("Public IPv6"), &reach.ipv6, Some(tr("For guests with IPv6, when your router lets connections in.")), full))
                            .child(way_row(3, tr("Relay (bore.pub)"), &reach.relay, Some(tr("Works from any network.")), full))
                    })
                    .when(local, |v| {
                        v.child(
                            h_flex()
                                .gap_2p5()
                                .items_start()
                                .child(div().flex_none().pt(px(2.)).child(Icon::new(IconName::Minus).size(theme::u(15.)).text_color(c(theme::FG_DIM))))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(div().text_color(c(theme::FG_MUTED)).child(tr("From the internet")))
                                        .child(meta(tr("Off for this session. To allow it, end the session and host again with internet joining on.").into())),
                                ),
                        )
                    }),
            )
            .when(full, |v| {
                v.child(meta(tr("Every way in that opened is in the code; a guest's FluxCode tries them all and keeps the first that answers. The relay only passes bytes encrypted with the code's secret. If someone can't connect, allow FluxCode through the firewall.").into()))
            })
            .into_any_element()
    }

    /// Guest: how this PC reaches the host.
    fn connection_card(&self, addr: &str) -> AnyElement {
        let (icon, via): (IconName, SharedString) = match super::path_kind(addr) {
            super::Via::Lan => (IconName::House, tr("Straight to the host over your network, encrypted.").into()),
            super::Via::Relay => (IconName::RadioTower, tr("Through the bore.pub relay, encrypted end to end: the relay can't read it.").into()),
            super::Via::Internet => (IconName::Globe, tr("Straight to the host over the internet, encrypted.").into()),
        };
        let root = self.collab.root.as_ref().map(|r| r.display().to_string()).unwrap_or_default();
        card(IconName::Link, tr("Connection"), None, None)
            .child(
                h_flex()
                    .gap_2p5()
                    .items_start()
                    .child(div().flex_none().pt(px(2.)).child(Icon::new(icon).size(theme::u(15.)).text_color(c(theme::GREEN))))
                    .child(div().flex_1().min_w_0().text_color(c(theme::FG)).child(via)),
            )
            .child(v_flex().gap_1().child(info_line(tr("Address"), addr.to_string(), true)).child(info_line(tr("Files on this PC"), root, false)))
            .into_any_element()
    }

    /// Who is in the session, and the file and line each one is on.
    fn people_card(&self) -> AnyElement {
        let co = &self.collab;
        let mut list = v_flex().gap_0p5();
        for p in &co.peers {
            let initial: String = p.name.chars().next().unwrap_or('?').to_uppercase().collect();
            let place = if p.file.is_empty() { tr("No file open").to_string() } else { format!("{}:{}", p.file, p.line) };
            let tag = if p.id == co.me { Some(tr("you")) } else if p.host { Some(tr("host")) } else { None };
            list = list.child(
                h_flex()
                    .gap_2p5()
                    .py_1()
                    .child(
                        div()
                            .flex_none()
                            .size(theme::u(24.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(c(if p.host { theme::ACCENT } else { theme::BG_HOVER }))
                            .text_xs()
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
                                    .min_w_0()
                                    .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(if p.github { format!("@{}", p.name) } else { p.name.clone() }))
                                    .when_some(tag, |h, t| h.child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(t))),
                            )
                            .child(div().min_w_0().truncate().text_xs().text_color(c(if p.file.is_empty() { theme::FG_DIM } else { theme::FG_MUTED })).child(place)),
                    ),
            );
        }
        card(IconName::Users, tr("People"), Some(co.peers.len().to_string()), None)
            .child(list)
            .when(co.peers.len() <= 1, |v| v.child(meta(if co.is_host() { tr("Nobody else yet. They show here as soon as they join.").into() } else { tr("Only you so far.").into() })))
            .into_any_element()
    }

    /// Guest: the host's files.
    fn files_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let q = co.filter_input.read(cx).value().to_lowercase();
        let hits: Vec<&String> = co.files.iter().filter(|f| q.is_empty() || f.to_lowercase().contains(&q)).take(200).collect();
        let empty = hits.is_empty();
        let mut list = v_flex().id("co-files").max_h(theme::u(320.)).overflow_y_scroll();
        for (i, f) in hits.into_iter().enumerate() {
            let rel = f.clone();
            let (dir, name) = f.rsplit_once('/').unwrap_or(("", f.as_str()));
            let (icon, tint) = crate::files::icon_for(name);
            let open = co.docs.contains_key(f);
            list = list.child(
                h_flex()
                    .id(("co-file", i))
                    .flex_none()
                    .h(theme::u(26.))
                    .gap_2()
                    .px_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(c(theme::BG_HOVER)))
                    .child(Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                    // The name first: a long folder path must not hide which file it is.
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(div().flex_none().max_w(relative(0.7)).truncate().text_color(c(if open { theme::FG_STRONG } else { theme::FG })).child(name.to_string()))
                            .when(!dir.is_empty(), |h| h.child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG_DIM)).child(dir.to_string()))),
                    )
                    .when(open, |h| h.child(div().flex_none().text_xs().text_color(c(theme::GREEN)).child(tr("open"))))
                    .on_click(cx.listener(move |this, _, w, cx| this.collab_open(rel.clone(), w, cx))),
            );
        }
        card(IconName::FolderOpen, tr("Shared files"), Some(co.files.len().to_string()), None)
            .child(meta(tr("Open a file to edit it with the others. It comes from the host and stays in sync.").into()))
            .child(Input::new(&co.filter_input).small())
            .child(list)
            .when(empty && !q.is_empty(), |v| v.child(meta(tr("No shared file matches.").into())))
            .into_any_element()
    }

    /// The edits everyone made, newest first; each opens as a diff.
    fn activity_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.collab;
        let mut list = v_flex().id("co-feed-list").max_h(theme::u(380.)).overflow_y_scroll();
        for (i, f) in co.feed.iter().enumerate() {
            let secs = f.at.elapsed().as_secs();
            let ago = if secs < 60 { tr("just now").to_string() } else { crate::trf!("{m} min ago"; m => secs / 60) };
            list = list.child(
                v_flex()
                    .id(("co-feed", i))
                    .flex_none()
                    .px_2()
                    .py_1p5()
                    .rounded(px(5.))
                    .when(i > 0, |d| d.border_t_1().border_color(c(theme::BORDER)))
                    .child(
                        h_flex()
                            .gap_2()
                            .min_w_0()
                            .child(div().flex_none().max_w(relative(0.4)).truncate().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(f.who.clone()))
                            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_MUTED)).child(f.rel.clone()))
                            .child(div().flex_none().text_xs().font_family(MONO).text_color(c(theme::GREEN)).child(format!("+{}", f.lines.0)))
                            .child(div().flex_none().text_xs().font_family(MONO).text_color(c(theme::DANGER)).child(format!("-{}", f.lines.1)))
                            .child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(ago))
                            .child(Button::new(("co-diff", i)).label(tr("Diff")).xsmall().ghost().on_click(cx.listener(move |this, _, w, cx| this.collab_show_diff(i, w, cx)))),
                    )
                    .when_some(f.ai.as_ref(), |v, ai| {
                        v.child(
                            h_flex()
                                .gap_1p5()
                                .min_w_0()
                                .child(div().flex_none().px_1().rounded(px(3.)).border_1().border_color(c(theme::ACCENT)).text_size(theme::u(10.)).font_weight(FontWeight::BOLD).text_color(c(theme::ACCENT_TEXT)).child("AI"))
                                .child(div().min_w_0().truncate().text_xs().text_color(c(theme::FG_MUTED)).child(ai.line())),
                        )
                    }),
            );
        }
        card(IconName::ScrollText, tr("Activity"), (!co.feed.is_empty()).then(|| co.feed.len().to_string()), None)
            .when(co.feed.is_empty(), |v| v.child(meta(tr("Edits show here as people make them. An edit made by an AI agent says which agent, model and effort.").into())))
            .child(list)
            .into_any_element()
    }
}

// ---------------------------------------------------------------- pieces

/// What went wrong, in the words of whoever found it (they say the cause and what to do).
fn error_banner(e: String, cx: &mut Context<Workbench>) -> AnyElement {
    h_flex()
        .gap_2p5()
        .items_start()
        .pl_3()
        .pr_1()
        .py_1()
        .rounded(px(8.))
        .border_1()
        .border_color(c(theme::DANGER).opacity(0.6))
        .bg(c(theme::DANGER).opacity(0.08))
        .child(div().flex_none().pt(theme::u(7.)).child(Icon::new(IconName::CircleAlert).size(theme::u(15.)).text_color(c(theme::DANGER))))
        .child(div().flex_1().min_w_0().py(theme::u(5.)).text_color(c(theme::FG_STRONG)).child(e))
        .child(Button::new("co-err-x").icon(Icon::new(IconName::Close)).xsmall().ghost().tooltip(tr("Dismiss")).on_click(cx.listener(|this, _, _, cx| {
            this.collab.error = None;
            cx.notify();
        })))
        .into_any_element()
}

/// A row of cards that wraps to one per row on a narrow window. `even`: both as tall
/// as the taller one (their buttons line up at the bottom).
fn row(cards: [AnyElement; 2], even: bool) -> Div {
    h_flex()
        .w_full()
        .flex_wrap()
        .gap_4()
        .when(even, |h| h.items_stretch())
        .when(!even, |h| h.items_start())
        // The basis makes them wrap; shrinking lets one card fit a column narrower than that.
        .children(cards.into_iter().map(|c| v_flex().flex_grow(1.).flex_shrink(1.).flex_basis(theme::u(CARD_MIN)).min_w_0().overflow_hidden().child(c)))
}

/// A card: a small titled box. `aside` sits at the right of the title.
fn card(icon: IconName, title: &'static str, count: Option<String>, aside: Option<AnyElement>) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .flex_1()
        .gap_3()
        .px_4()
        .pt_3p5()
        .pb_4()
        .rounded(px(8.))
        .border_1()
        .border_color(c(theme::BORDER))
        .bg(c(theme::BG_ELEVATED))
        .child(
            h_flex()
                .gap_2()
                .min_h(theme::u(22.))
                .child(Icon::new(icon).size(theme::u(15.)).text_color(c(theme::FG_MUTED)))
                .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(title))
                .when_some(count, |h, n| h.child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(n)))
                .children(aside),
        )
}

fn body(t: SharedString) -> Div {
    div().text_color(c(theme::FG_MUTED)).child(t)
}

fn meta(t: SharedString) -> Div {
    div().text_xs().text_color(c(theme::FG_DIM)).child(t)
}

fn checkbox(on: bool) -> Div {
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
        .when(on, |d| d.child(Icon::new(IconName::Check).size(theme::u(12.)).text_color(c(theme::ON_ACCENT))))
}

fn info_line(label: &'static str, value: String, mono: bool) -> Div {
    h_flex()
        .gap_3()
        .min_w_0()
        .child(div().flex_none().w(theme::u(110.)).text_color(c(theme::FG_DIM)).child(label))
        .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG)).when(mono, |d| d.font_family(MONO)).child(value))
}

/// One way in: a mark, its name, and its address or why it isn't open. Folded, that
/// fits one line; unfolded, the reason wraps and a hint follows.
fn way_row(ix: usize, name: &'static str, st: &reach::State, hint: Option<&'static str>, full: bool) -> AnyElement {
    let (detail, mono, mark): (SharedString, bool, AnyElement) = match st {
        reach::State::Trying => (tr("Checking…").into(), false, Spinner::new().icon(IconName::LoaderCircle).small().color(c(theme::FG_MUTED)).into_any_element()),
        reach::State::Ready(a) => (a.clone().into(), true, Icon::new(IconName::CircleCheck).size(theme::u(15.)).text_color(c(theme::GREEN)).into_any_element()),
        reach::State::No(why) => (why.clone().into(), false, Icon::new(IconName::CircleX).size(theme::u(15.)).text_color(c(theme::FG_DIM)).into_any_element()),
        reach::State::Off => (tr("Off: you chose same network only.").into(), false, Icon::new(IconName::Minus).size(theme::u(15.)).text_color(c(theme::FG_DIM)).into_any_element()),
    };
    let ready = matches!(st, reach::State::Ready(_));
    let name_el = div().flex_none().text_color(c(if ready { theme::FG_STRONG } else { theme::FG_MUTED })).child(name);
    let detail_el = div().min_w_0().text_xs().text_color(c(if ready { theme::FG } else { theme::FG_DIM })).when(mono, |d| d.font_family(MONO));
    let row = h_flex().id(("co-way", ix)).gap_2p5().items_start().child(div().flex_none().pt(px(2.)).child(mark));
    if full {
        row.child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(name_el)
                .child(detail_el.when(mono, |d| d.truncate()).child(detail))
                .when_some(hint.filter(|_| ready), |v, h| v.child(div().text_xs().text_color(c(theme::FG_DIM)).child(h))),
        )
        .into_any_element()
    } else {
        let tip = detail.clone();
        row.child(h_flex().flex_1().min_w_0().gap_3().items_center().child(name_el).child(detail_el.flex_1().truncate().text_right().child(detail)))
            .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
            .into_any_element()
    }
}
