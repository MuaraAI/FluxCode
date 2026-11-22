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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Bumped by every search and keystroke: a walk whose number is no longer current stops at the next folder, and its results are dropped.
static GEN: AtomicU64 = AtomicU64::new(0);

/// Pause after the last keystroke before searching: short enough to feel live, long enough that a word typed at speed costs one walk, not one per letter.
const TYPE_PAUSE: Duration = Duration::from_millis(140);

/// Lines shown per file before a "N more" row: a file with hundreds of matches no longer pushes every other file off the list.
const FOLD: usize = 6;
/// The hit index that stands for a file's "N more" row.
const MORE: usize = usize::MAX;

impl Workbench {
    /// The query changed: search once typing pauses (Enter still searches at once).
    pub(crate) fn search_soon(&mut self, cx: &mut Context<Self>) {
        let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
        if self.search_input.read(cx).value().is_empty() {
            self.search = core::SearchResult::default();
            self.search_rows.clear();
            self.searching = false;
            return cx.notify();
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TYPE_PAUSE).await;
            if GEN.load(Ordering::Relaxed) == gen {
                _ = this.update(cx, |this, cx| this.run_search(cx));
            }
        })
        .detach();
    }

    pub(crate) fn run_search(&mut self, cx: &mut Context<Self>) {
        if self.root.is_none() {
            return;
        }
        let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
        let roots = self.roots();
        let query = self.search_input.read(cx).value().to_string();
        let (case, word) = (self.match_case, self.whole_word);
        self.searching = true;
        cx.notify();
        let wsl = self.wsl.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let stale = || GEN.load(Ordering::Relaxed) != gen;
                    let mut all = core::SearchResult::default();
                    for r in &roots {
                        if stale() {
                            break;
                        }
                        // In a distro the helper searches with ripgrep-speed readdir instead of walking the 9p share.
                        let remote = wsl.as_ref().and_then(|t| {
                            let distro = t.distro.clone();
                            let linux = t.linux_path(r)?;
                            crate::remote::with_client(&distro, |c| c.search(&linux, &query, case, word).ok())?
                        });
                        if let Some(wire) = remote {
                            all.total += wire.total;
                            all.truncated |= wire.truncated;
                            for f in wire.files {
                                // The editor works in Windows paths, so the Linux ones come back as UNC.
                                all.files.push(core::FileHits {
                                    path: t_unc(&wsl, &f.path),
                                    hits: f.hits.into_iter().map(|h| core::Hit { line: h.line, text: h.text }).collect(),
                                });
                            }
                            continue;
                        }
                        let one = core::search_until(r, &query, case, word, &stale);
                        all.total += one.total;
                        all.truncated |= one.truncated;
                        all.files.extend(one.files);
                    }
                    all
                })
                .await;
            _ = this.update(cx, |this, cx| {
                if GEN.load(Ordering::Relaxed) != gen {
                    return;
                }
                this.search = result;
                this.search_unfolded.clear();
                this.search_closed.clear();
                this.rebuild_search_rows();
                this.searching = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// The rows the list shows: each file, then its first `FOLD` lines unless the user opened it all, then a "N more" row.
    pub(crate) fn rebuild_search_rows(&mut self) {
        let mut rows = Vec::new();
        for (f, fh) in self.search.files.iter().enumerate() {
            rows.push((f, None));
            if self.search_closed.contains(&f) {
                continue;
            }
            let shown = if self.search_unfolded.contains(&f) { fh.hits.len() } else { fh.hits.len().min(FOLD) };
            rows.extend((0..shown).map(|h| (f, Some(h))));
            if shown < fh.hits.len() {
                rows.push((f, Some(MORE)));
            }
        }
        self.search_rows = rows;
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
                                        let closed = this.search_closed.contains(&f);
                                        row.cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if !this.search_closed.remove(&f) {
                                                    this.search_closed.insert(f);
                                                }
                                                this.rebuild_search_rows();
                                                cx.notify();
                                            }))
                                            .child({
                                                use gpui_kit::component::IconName as I;
                                                gpui_kit::component::Icon::new(if closed { I::ChevronRight } else { I::ChevronDown }).size(theme::u(12.)).text_color(c(theme::FG_DIM))
                                            })
                                            .child(gpui_kit::component::Icon::new(icon).size(theme::u(14.)).text_color(theme::tint(tint)))
                                            .child(div().text_color(c(theme::FG_STRONG)).child(name))
                                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(rel))
                                            .child(div().flex_1())
                                            .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(fh.hits.len().to_string()))
                                    }
                                    Some(MORE) => {
                                        let more = fh.hits.len().saturating_sub(FOLD);
                                        row.pl(px(28.))
                                            .cursor_pointer()
                                            .text_color(c(theme::ACCENT_TEXT))
                                            .child(crate::trf!("{0} more matches"; 0 => more))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.search_unfolded.insert(f);
                                                this.rebuild_search_rows();
                                                cx.notify();
                                            }))
                                    }
                                    Some(h) => {
                                        let hit = &fh.hits[h];
                                        let (path, line) = (fh.path.clone(), hit.line);
                                        let query = this.search_input.read(cx).value();
                                        let marks = match_ranges(&hit.text, &query, this.match_case)
                                            .into_iter()
                                            .map(|r| (r, HighlightStyle { background_color: Some(c(theme::ACCENT).opacity(0.28)), color: Some(c(theme::FG_STRONG)), ..Default::default() }));
                                        let text = StyledText::new(SharedString::from(hit.text.clone())).with_highlights(marks);
                                        row.pl(px(28.))
                                            .child(div().text_xs().text_color(c(theme::FG_DIM)).child(hit.line.to_string()))
                                            .child(div().text_color(c(theme::FG)).truncate().child(text))
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

/// Byte ranges of `query` in `text`, to tint in a result row.
fn match_ranges(text: &str, query: &str, case: bool) -> Vec<std::ops::Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let fold = |s: &str| if case { s.to_string() } else { s.to_ascii_lowercase() };
    let (hay, needle) = (fold(text), fold(query));
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = hay[from..].find(&needle) {
        let start = from + i;
        out.push(start..start + needle.len());
        from = start + needle.len();
        if out.len() >= 32 {
            break;
        }
    }
    out
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
    use super::{match_ranges, t_unc};
    use crate::remote::WslTarget;
    use std::path::PathBuf;

    #[test]
    fn every_match_in_a_row_is_found() {
        assert_eq!(match_ranges("Foo foo FOO", "foo", false), vec![0..3, 4..7, 8..11]);
        assert_eq!(match_ranges("Foo foo FOO", "foo", true), vec![4..7]);
        assert_eq!(match_ranges("é foo", "FOO", false), vec![3..6]);
        assert!(match_ranges("abc", "", false).is_empty());
    }

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
