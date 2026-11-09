//! Find in files side bar (Ctrl+Shift+F).

use crate::core;
use crate::files;
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    v_flex, Selectable, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};

impl Workbench {
    pub(crate) fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let query = self.search_input.read(cx).value().to_string();
        let (case, word) = (self.match_case, self.whole_word);
        self.searching = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { core::search(&root, &query, case, word) }).await;
            _ = this.update(cx, |this, cx| {
                this.search_rows = result
                    .files
                    .iter()
                    .enumerate()
                    .flat_map(|(f, fh)| std::iter::once((f, None)).chain((0..fh.hits.len()).map(move |h| (f, Some(h)))))
                    .collect();
                this.search = result;
                this.searching = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn render_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let summary = if self.searching {
            "Searching…".to_string()
        } else if self.search.total == 0 {
            String::new()
        } else {
            format!(
                "{}{} results in {} files",
                self.search.total,
                if self.search.truncated { "+" } else { "" },
                self.search.files.len()
            )
        };
        let toggle = |id: &'static str, label: &'static str, on: bool, tip: &'static str| {
            Button::new(id).label(label).xsmall().ghost().selected(on).tooltip(tip)
        };
        let rows = self.search_rows.len();
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .px_3()
                    .pb_2()
                    .gap_1()
                    .child(Input::new(&self.search_input).small())
                    .child(
                        h_flex()
                            .gap_1()
                            .child(toggle("s-case", "Aa", self.match_case, "Match Case").on_click(cx.listener(|this, _, _, cx| {
                                this.match_case = !this.match_case;
                                this.run_search(cx);
                            })))
                            .child(toggle("s-word", "ab", self.whole_word, "Match Whole Word").on_click(cx.listener(|this, _, _, cx| {
                                this.whole_word = !this.whole_word;
                                this.run_search(cx);
                            })))
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(summary)),
                    ),
            )
            .child(
                uniform_list(
                    "search-results",
                    rows,
                    cx.processor(|this, range: std::ops::Range<usize>, _window, cx| {
                        range
                            .map(|i| {
                                let (f, h) = this.search_rows[i];
                                let fh = &this.search.files[f];
                                let row = h_flex().id(("sr", i)).w_full().h(theme::u(20.)).px_2().gap_2().text_size(theme::u(11.5)).hover(|s| s.bg(c(theme::BG_HOVER)));
                                match h {
                                    None => {
                                        let name = fh.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                                        let rel = this
                                            .root
                                            .as_deref()
                                            .and_then(|r| fh.path.parent()?.strip_prefix(r).ok())
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_default();
                                        let (icon, tint) = files::icon_for(&name);
                                        row.child(gpui_kit::component::Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                                            .child(div().text_color(c(theme::FG_STRONG)).child(name))
                                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(rel))
                                            .child(div().flex_1())
                                            .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(fh.hits.len().to_string()))
                                    }
                                    Some(h) => {
                                        let hit = &fh.hits[h];
                                        let (path, line) = (fh.path.clone(), hit.line);
                                        row.pl(px(28.))
                                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(hit.line.to_string()))
                                            .child(div().text_color(c(theme::FG)).truncate().child(hit.text.clone()))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.open_file(path.clone(), window, cx);
                                                this.go_to_line(line, window, cx);
                                            }))
                                    }
                                }
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .when(rows == 0, |l| l.h(px(0.))),
            )
            .into_any_element()
    }
}
