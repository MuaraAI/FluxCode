//! Find in files side bar (Ctrl+Shift+F).

use crate::i18n::tr;
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
        if self.root.is_none() {
            return;
        }
        let roots = self.roots();
        let query = self.search_input.read(cx).value().to_string();
        let (case, word) = (self.match_case, self.whole_word);
        self.searching = true;
        cx.notify();
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut all = core::SearchResult::default();
                    for r in &roots {
                        // In a distro the helper searches with ripgrep-speed
                        // readdir instead of walking the 9p share.
                        let remote = wsl.as_ref().and_then(|t| {
                            let distro = t.distro.clone();
                            let linux = t.linux_path(r)?;
                            crate::remote::with_client(&distro, |c| c.search(&linux, &query, case, word).ok())?
                        });
                        if let Some(wire) = remote {
                            all.total += wire.total;
                            all.truncated |= wire.truncated;
                            for f in wire.files {
                                // The editor works in Windows paths, so the
                                // Linux ones come back as UNC.
                                all.files.push(core::FileHits {
                                    path: t_unc(&wsl, &f.path),
                                    hits: f.hits.into_iter().map(|h| core::Hit { line: h.line, text: h.text }).collect(),
                                });
                            }
                            continue;
                        }
                        let one = core::search(r, &query, case, word);
                        all.total += one.total;
                        all.truncated |= one.truncated;
                        all.files.extend(one.files);
                    }
                    all
                })
                .await;
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
            tr("Searching…").to_string()
        } else if self.search.total == 0 {
            String::new()
        } else {
            crate::trf!("{0}{1} results in {2} files"; 0 => self.search.total, 1 => if self.search.truncated { "+" } else { "" }, 2 => self.search.files.len())
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
                            .child(toggle("s-case", "Aa", self.match_case, tr("Match Case")).on_click(cx.listener(|this, _, _, cx| {
                                this.match_case = !this.match_case;
                                this.run_search(cx);
                            })))
                            .child(toggle("s-word", "ab", self.whole_word, tr("Match Whole Word")).on_click(cx.listener(|this, _, _, cx| {
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
                                        let rel = fh.path.parent().map(|p| this.rel_display(&p.to_string_lossy())).unwrap_or_default();
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
                                                this.open_location(path.clone(), line, 0, window, cx);
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

/// A Linux path from the helper as the Windows path the editor uses.
fn t_unc(wsl: &Option<crate::remote::WslTarget>, linux: &str) -> std::path::PathBuf {
    match wsl {
        Some(t) => t.unc_path(linux),
        None => std::path::PathBuf::from(linux),
    }
}

#[cfg(test)]
mod tests {
    use super::t_unc;
    use crate::remote::WslTarget;
    use std::path::PathBuf;

    #[test]
    fn a_linux_hit_becomes_a_windows_path() {
        let t = WslTarget::new("Ubuntu", "/home/me");
        assert_eq!(
            t_unc(&Some(t), "/home/me/src/a.rs"),
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\src\a.rs")
        );
        assert_eq!(t_unc(&None, "/tmp/a.rs"), PathBuf::from("/tmp/a.rs"));
    }
}
