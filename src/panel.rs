
use crate::i18n::tr;
use crate::proc::{self, python};
use crate::term::{self, LineKind, PyError};
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    h_flex,
    highlighter::{Diagnostic, DiagnosticSeverity},
    input::{
        EditorState, Input, OverviewRulerDecoration, OverviewRulerDecorationCollection, OverviewRulerLane, Position,
        RangeDecoration, RangeDecorationCollection, RangeDecorationStyle, Rope,
    },
    v_flex, Icon, IconName, RopeExt, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn build_mine_diagnostics(mine: &[Problem], rope: &Rope) -> Vec<Diagnostic> {
    if mine.is_empty() {
        return Vec::new();
    }
    let mut rows: std::collections::HashMap<u32, String> = mine.iter().map(|p| (p.line.saturating_sub(1), String::new())).collect();
    let max_row = rows.keys().copied().max().unwrap_or(0);
    for (idx, line) in rope.iter_lines().enumerate() {
        let r = idx as u32;
        if let Some(slot) = rows.get_mut(&r) {
            *slot = line.to_string();
        }
        if r >= max_row {
            break;
        }
    }
    mine.iter()
        .map(|p| {
            let row = p.line.saturating_sub(1);
            let src = rows.get(&row).map(|s| s.trim_end_matches(['\r', '\n'])).unwrap_or("");
            let indent = (src.len() - src.trim_start().len()) as u32;
            let len = src.trim_end().chars().count() as u32;
            let (a, b) = match p.caret {
                Some((s, e)) if (indent + e as u32) <= len.max(1) && e > s => (indent + s as u32, indent + e as u32),
                _ if p.col > 0 => {
                    let c0 = p.col.saturating_sub(1).min(len);
                    (c0, len.max(c0 + 1))
                }
                _ => (indent, len.max(indent + 1)),
            };
            Diagnostic::new(Position::new(row, a)..Position::new(row, b), p.message.clone())
                .with_severity(DiagnosticSeverity::Error)
                .with_source(p.source)
        })
        .collect()
}

fn sync_diagnostic_decorations(
    ed: &Entity<EditorState>,
    ranges_col: Option<&RangeDecorationCollection>,
    ruler_col: Option<&OverviewRulerDecorationCollection>,
    mine: &[Problem],
    code_diags: &[crate::diagnostics::CodeDiag],
    cx: &mut App,
) {
    if ranges_col.is_none() && ruler_col.is_none() {
        return;
    }
    let mut rows: std::collections::BTreeMap<usize, bool> = std::collections::BTreeMap::new();
    for p in mine {
        rows.insert(p.line.saturating_sub(1) as usize, true);
    }
    for d in code_diags {
        let is_err = d.severity == crate::diagnostics::Severity::Error;
        rows.entry(d.row as usize)
            .and_modify(|e| *e |= is_err)
            .or_insert(is_err);
    }
    let rope = ed.read(cx).text().clone();
    let total_lines = rope.lines_len();
    let err_fill = c(theme::DANGER).opacity(0.14);
    let warn_fill = c(theme::WARN).opacity(0.09);
    let err_mark = c(theme::DANGER);
    let warn_mark = c(theme::WARN);
    let mut range_decos = Vec::with_capacity(rows.len());
    let mut ruler_decos = Vec::with_capacity(rows.len());
    for (row, is_err) in rows {
        if row >= total_lines {
            continue;
        }
        let start = rope.line_start_offset(row);
        let end = rope.line_end_offset(row);
        let span = if end > start { start..end } else { start..start.saturating_add(1).min(rope.len()) };
        if !span.is_empty() {
            range_decos.push(
                RangeDecoration::new(span.clone())
                    .with_style(RangeDecorationStyle::Fill)
                    .with_color(if is_err { err_fill } else { warn_fill }),
            );
            ruler_decos.push(
                OverviewRulerDecoration::new(span)
                    .with_lane(OverviewRulerLane::Right)
                    .with_color(if is_err { err_mark } else { warn_mark }),
            );
        }
    }
    if let Some(col) = ranges_col {
        col.set(range_decos, cx);
    }
    if let Some(col) = ruler_col {
        col.set(ruler_decos, cx);
    }
}

fn write_diagnostics(
    ed: &mut EditorState,
    mine_diags: Vec<Diagnostic>,
    code_diags: &[crate::diagnostics::CodeDiag],
    cx: &mut Context<EditorState>,
) {
    let Some(set) = ed.diagnostics_mut() else { return };
    if set.is_empty() && mine_diags.is_empty() && code_diags.is_empty() {
        return;
    }
    set.clear();
    for diag in mine_diags {
        set.push(diag);
    }
    for d in code_diags {
        let sev = match d.severity {
            crate::diagnostics::Severity::Error => DiagnosticSeverity::Error,
            crate::diagnostics::Severity::Warning => DiagnosticSeverity::Warning,
        };
        let end = d.end_col.max(d.col + 1);
        set.push(
            Diagnostic::new(Position::new(d.row, d.col)..Position::new(d.row, end), d.message.clone())
                .with_severity(sev)
                .with_source(d.source),
        );
    }
    cx.notify();
}

pub(crate) struct RunInfo {
    pub started: Instant,
    pub block: usize,
    pub python: String,
    pub cwd: PathBuf,
    pub command: String,
}

pub(crate) struct TabCycle {
    pub items: Vec<String>,
    pub ix: usize,
}

pub(crate) struct TermTab {
    pub lines: Vec<Line>,
    pub cwd: PathBuf,
    pub history: Vec<String>,
    pub hist_ix: Option<usize>,
    pub tab_cycle: Option<TabCycle>,
    pub proc: Option<u32>,
    pub run: Option<RunInfo>,
    pub typed: bool,
    /// Typed but not sent yet, so switching tabs does not eat a half-written command.
    pub draft: String,
    pub wrap: WrapCache,
    pub scroll: UniformListScrollHandle,
}

impl TermTab {
    pub(crate) fn new(cwd: PathBuf) -> Self {
        Self {
            lines: Vec::new(),
            cwd,
            history: Vec::new(),
            hist_ix: None,
            tab_cycle: None,
            proc: None,
            run: None,
            typed: false,
            draft: String::new(),
            wrap: Default::default(),
            scroll: UniformListScrollHandle::new(),
        }
    }
}

/// Which tab is in front after closing `closed` while `active` was; `len` is the list length after the removal.
fn close_index(active: usize, closed: usize, len: usize) -> usize {
    if active > closed {
        active - 1
    } else if active == closed {
        active.min(len.saturating_sub(1))
    } else {
        active
    }
}

pub(crate) struct Row {
    pub line: usize,
    pub text: SharedString,
    pub first: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct PanelSel {
    pub terminal: bool,
    pub anchor: (usize, usize),
    pub head: (usize, usize),
}

impl PanelSel {
    fn span(&self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    fn on_row(&self, i: usize, len: usize) -> Option<(usize, usize)> {
        if self.is_empty() {
            return None;
        }
        let ((r0, c0), (r1, c1)) = self.span();
        if i < r0 || i > r1 {
            return None;
        }
        let a = if i == r0 { c0.min(len) } else { 0 };
        let b = if i == r1 { c1.min(len) } else { len };
        (b > a || i != r1).then_some((a, b.max(a)))
    }
}

fn mono_kind(k: LineKind) -> bool {
    matches!(k, LineKind::Normal | LineKind::Err | LineKind::Warn | LineKind::Dim | LineKind::Success | LineKind::Frame | LineKind::Code | LineKind::Caret | LineKind::Blank)
}

pub(crate) type WrapCache = std::cell::RefCell<((usize, usize, usize, usize, usize), std::rc::Rc<Vec<Row>>)>;

#[derive(Clone)]
pub(crate) struct Problem {
    pub from_terminal: bool,
    pub path: PathBuf,
    pub line: u32,
    pub col: u32,
    pub source: &'static str,
    pub caret: Option<(usize, usize)>,
    pub message: String,
}

pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    let n = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_start_matches(r"\\?\").to_lowercase();
    if cfg!(windows) { n(a) == n(b) } else { a == b }
}

impl Workbench {
    pub(crate) fn on_toggle_panel(&mut self, _: &TogglePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.set_panel_open(!self.panel_open, cx);
    }

    pub(crate) fn on_toggle_terminal(&mut self, _: &ToggleTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel_open && self.panel_tab == PanelTab::Terminal {
            self.set_panel_open(false, cx);
        } else {
            self.panel_tab = PanelTab::Terminal;
            self.set_panel_open(true, cx);
            let h = self.term_input.focus_handle(cx);
            h.focus(window, cx);
        }
    }

    pub(crate) fn on_new_terminal(&mut self, _: &NewTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.new_terminal(window, cx);
    }

    pub(crate) fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ix = self.term_ix;
        self.terms[ix].draft = self.term_input.read(cx).value().to_string();
        let cwd = self.terms[ix].cwd.clone();
        self.terms.push(TermTab::new(cwd));
        self.show_terminal(self.terms.len() - 1, window, cx);
    }

    pub(crate) fn show_terminal(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.terms.len() {
            return;
        }
        if ix != self.term_ix {
            self.terms[self.term_ix].draft = self.term_input.read(cx).value().to_string();
            self.term_ix = ix;
            self.panel_sel = self.panel_sel.filter(|s| !s.terminal);
            let draft = self.terms[ix].draft.clone();
            self.set_term_input(&draft, window, cx);
        }
        self.terms[ix].hist_ix = None;
        self.terms[ix].tab_cycle = None;
        self.panel_tab = PanelTab::Terminal;
        self.set_panel_open(true, cx);
        self.terms[ix].scroll.scroll_to_bottom();
        let h = self.term_input.focus_handle(cx);
        h.focus(window, cx);
        cx.notify();
    }

    /// The last tab stays: there is always one terminal to type in.
    pub(crate) fn close_terminal(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.terms.len() < 2 || ix >= self.terms.len() {
            return;
        }
        if let Some(id) = self.terms[ix].proc.take() {
            self.procs.kill(id);
        }
        let was = self.term_ix;
        self.terms.remove(ix);
        self.term_ix = close_index(was, ix, self.terms.len());
        if ix == was {
            self.panel_sel = self.panel_sel.filter(|s| !s.terminal);
            self.terms[self.term_ix].hist_ix = None;
            let draft = self.terms[self.term_ix].draft.clone();
            self.set_term_input(&draft, window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_show_output(&mut self, _: &ShowOutput, _: &mut Window, cx: &mut Context<Self>) {
        self.panel_tab = PanelTab::Output;
        self.set_panel_open(true, cx);
    }

    pub(crate) fn on_show_problems(&mut self, _: &ShowProblems, _: &mut Window, cx: &mut Context<Self>) {
        if self.panel_open && self.panel_tab == PanelTab::Problems {
            self.set_panel_open(false, cx);
        } else {
            self.panel_tab = PanelTab::Problems;
            self.set_panel_open(true, cx);
        }
    }

    fn term_running(&self) -> bool {
        self.terms[self.term_ix].proc.is_some_and(|id| self.procs.is_running(id))
    }

    fn set_term_input(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let chars = text.chars().count() as u32;
        self.terms[self.term_ix].typed = !text.is_empty();
        self.term_input.update(cx, |s, cx| {
            s.set_value(text.to_string(), window, cx);
            s.set_cursor_position(Position::new(0, chars), window, cx);
        });
    }

    pub(crate) fn terminal_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = self.term_input.read(cx).value().to_string();
        self.term_input.update(cx, |s, cx| s.set_value("", window, cx));
        self.terms[self.term_ix].typed = false;
        self.terms[self.term_ix].hist_ix = None;
        self.terms[self.term_ix].tab_cycle = None;
        if let Some(id) = self.terms[self.term_ix].proc.filter(|id| self.procs.is_running(*id)) {
            push_text(&mut self.terms[self.term_ix].lines, &format!("{line}\n"), false);
            self.procs.write_stdin(id, &format!("{line}\n"));
            self.terms[self.term_ix].scroll.scroll_to_bottom();
            cx.notify();
            return;
        }
        let typed = line.trim().to_string();
        if typed.is_empty() {
            return;
        }
        if typed.eq_ignore_ascii_case("exit") {
            if self.terms.len() > 1 {
                self.close_terminal(self.term_ix, window, cx);
            } else {
                self.terms[self.term_ix].lines.clear();
                self.set_panel_open(false, cx);
                cx.notify();
            }
            return;
        }
        if self.terms[self.term_ix].history.last() != Some(&typed) {
            self.terms[self.term_ix].history.push(typed.clone());
        }
        self.run_in_terminal(&typed, cx);
    }

    pub(crate) fn run_in_terminal(&mut self, typed: &str, cx: &mut Context<Self>) {
        if !self.terms[self.term_ix].lines.is_empty() {
            push_line(&mut self.terms[self.term_ix].lines, "", LineKind::Blank);
        }
        let mut prompt = Line::new(typed, LineKind::Cmd);
        prompt.meta = self.term_meta().into();
        close_line(&mut self.terms[self.term_ix].lines);
        self.terms[self.term_ix].lines.push(prompt);

        let cmdline = match term::autocorrect(typed, &self.terms[self.term_ix].cwd) {
            Some(fix) => {
                let what = fix.fixes.iter().map(|(a, b)| format!("{a} → {b}")).collect::<Vec<_>>().join(",  ");
                let mut l = Line::new(&what, LineKind::Fixed);
                l.meta = fix.line.clone().into();
                self.terms[self.term_ix].lines.push(l);
                fix.line
            }
            None => typed.to_string(),
        };

        let lower = cmdline.to_ascii_lowercase();
        if lower == "cls" || lower == "clear" {
            self.terms[self.term_ix].lines.clear();
        } else if lower == "help" || lower == "?" {
            push_line(
                &mut self.terms[self.term_ix].lines,
                "cd, pwd, clear, exit  ·  Up/Down history  ·  Tab complete  ·  Ctrl+L clear  ·  Ctrl+C stop",
                LineKind::Dim,
            );
        } else if lower == "pwd" {
            let meta = self.term_full();
            push_line(&mut self.terms[self.term_ix].lines, &meta, LineKind::Normal);
        } else if lower == "cd" || lower.starts_with("cd ") || lower.starts_with("cd\\") || lower.starts_with("cd..") || lower.starts_with("chdir") {
            let arg = cmdline.trim_start_matches(|c: char| c.is_ascii_alphabetic()).trim();
            let arg = arg.strip_prefix("/d").or_else(|| arg.strip_prefix("/D")).unwrap_or(arg).trim().trim_matches('"');
            if arg.is_empty() {
                let dir = self.terms[self.term_ix].cwd.display().to_string();
                push_line(&mut self.terms[self.term_ix].lines, &dir, LineKind::Normal);
            } else {
                self.change_dir(arg);
            }
        } else if cmdline.len() == 2 && cmdline.ends_with(':') {
            self.change_dir(&format!("{cmdline}\\"));
        } else {
            let first = cmdline.split_whitespace().next().unwrap_or("");
            let py = if term::is_python(first) {
                first.trim_matches('"').to_string()
            } else if self.wsl.is_some() {
                "python3".to_string()
            } else {
                python(self.root.as_deref())
            };
            self.problems.retain(|p| !p.from_terminal);
            self.apply_problems(cx);
            let mut cmd = match &self.wsl {
                Some(t) => {
                    let cwd = t.linux_path(&self.terms[self.term_ix].cwd).unwrap_or_else(|| t.linux_root.clone());
                    crate::remote::shell_command(t, &cwd, &cmdline)
                }
                None => proc::shell_command(&cmdline),
            };
            crate::github::apply_env(&mut cmd);
            let local_cwd = self.wsl.is_none().then(|| self.terms[self.term_ix].cwd.clone());
            match self.procs.spawn(cmd, local_cwd.as_deref()) {
                Ok(id) => {
                    self.terms[self.term_ix].proc = Some(id);
                    self.terms[self.term_ix].run = Some(RunInfo {
                        started: Instant::now(),
                        block: self.terms[self.term_ix].lines.len(),
                        python: py,
                        cwd: self.terms[self.term_ix].cwd.clone(),
                        command: cmdline.clone(),
                    });
                }
                Err(e) => push_line(&mut self.terms[self.term_ix].lines, &e, LineKind::Err),
            }
        }
        self.terms[self.term_ix].scroll.scroll_to_bottom();
        cx.notify();
    }

    fn term_meta(&self) -> String {
        match &self.wsl {
            Some(t) => t.linux_path(&self.terms[self.term_ix].cwd).unwrap_or_else(|| t.linux_root.clone()),
            None => term::short_cwd(&self.terms[self.term_ix].cwd, self.root.as_deref()),
        }
    }

    fn term_full(&self) -> String {
        match &self.wsl {
            Some(t) => t.linux_path(&self.terms[self.term_ix].cwd).unwrap_or_else(|| t.linux_root.clone()),
            None => self.terms[self.term_ix].cwd.display().to_string(),
        }
    }

    fn change_dir(&mut self, arg: &str) {
        if let Some(t) = self.wsl.clone() {
            let current = t.linux_path(&self.terms[self.term_ix].cwd).unwrap_or_else(|| t.linux_root.clone());
            let linux = if arg == "~" {
                crate::remote::home_dir(&t.distro).unwrap_or_else(|| "/".to_string())
            } else if arg.starts_with('/') {
                arg.to_string()
            } else if arg == ".." {
                let mut p = current.trim_end_matches('/').to_string();
                if let Some(i) = p.rfind('/') {
                    p.truncate(i.max(1));
                }
                p
            } else {
                format!("{}/{}", current.trim_end_matches('/'), arg)
            };
            let exists = crate::remote::program_command(&t, "/", "sh", &["-c", r#"test -d "$1""#, "--", &linux])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if exists {
                self.terms[self.term_ix].cwd = t.unc_path(&linux);
            } else {
                push_line(&mut self.terms[self.term_ix].lines, &crate::trf!("No folder named '{arg}' here."; arg => arg), LineKind::Err);
            }
            return;
        }
        let target = if arg == "~" {
            std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default()
        } else {
            self.terms[self.term_ix].cwd.join(arg)
        };
        match target.canonicalize() {
            Ok(p) if p.is_dir() => {
                self.terms[self.term_ix].cwd = PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\"));
            }
            _ => push_line(&mut self.terms[self.term_ix].lines, &crate::trf!("No folder named '{arg}' here."; arg => arg), LineKind::Err),
        }
    }


    pub(crate) fn on_term_history_prev(&mut self, _: &TermHistoryPrev, window: &mut Window, cx: &mut Context<Self>) {
        if self.terms[self.term_ix].history.is_empty() || self.term_running() {
            return;
        }
        self.terms[self.term_ix].tab_cycle = None;
        let ix = match self.terms[self.term_ix].hist_ix {
            None => self.terms[self.term_ix].history.len() - 1,
            Some(i) => i.saturating_sub(1),
        };
        self.terms[self.term_ix].hist_ix = Some(ix);
        let text = self.terms[self.term_ix].history[ix].clone();
        self.set_term_input(&text, window, cx);
    }

    pub(crate) fn on_term_history_next(&mut self, _: &TermHistoryNext, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.terms[self.term_ix].hist_ix else { return };
        self.terms[self.term_ix].tab_cycle = None;
        if i + 1 < self.terms[self.term_ix].history.len() {
            self.terms[self.term_ix].hist_ix = Some(i + 1);
            let text = self.terms[self.term_ix].history[i + 1].clone();
            self.set_term_input(&text, window, cx);
        } else {
            self.terms[self.term_ix].hist_ix = None;
            self.set_term_input("", window, cx);
        }
    }

    pub(crate) fn on_term_complete(&mut self, _: &TermComplete, window: &mut Window, cx: &mut Context<Self>) {
        if self.term_running() {
            return;
        }
        let ix = self.term_ix;
        let value = self.term_input.read(cx).value().to_string();
        if value.trim().is_empty() {
            self.terms[ix].tab_cycle = None;
            return;
        }
        if let Some(cycle) = self.terms[ix].tab_cycle.as_mut() {
            if cycle.items.get(cycle.ix).map(String::as_str) == Some(value.as_str()) && cycle.items.len() > 1 {
                cycle.ix = (cycle.ix + 1) % cycle.items.len();
                let next = cycle.items[cycle.ix].clone();
                let pos = cycle.ix + 1;
                let total = cycle.items.len();
                self.set_term_input(&next, window, cx);
                self.status = format!("Tab {pos}/{total}").into();
                cx.notify();
                return;
            }
        }
        let items = term::complete_candidates(&value, &self.terms[ix].cwd);
        let Some(first) = items.first().cloned() else {
            self.terms[ix].tab_cycle = None;
            return;
        };
        let total = items.len();
        self.terms[ix].tab_cycle = (total > 1).then_some(TabCycle { items, ix: 0 });
        self.set_term_input(&first, window, cx);
        if total > 1 {
            self.status = format!("Tab 1/{total}").into();
            cx.notify();
        }
    }

    pub(crate) fn on_term_interrupt(&mut self, _: &TermInterrupt, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel_sel.is_some_and(|s| s.terminal && !s.is_empty()) {
            self.panel_copy(cx);
            self.panel_sel = None;
            return;
        }
        if !self.term_input.read(cx).selected_range().is_empty() {
            window.dispatch_action(Box::new(gpui_kit::component::input::Copy), cx);
            return;
        }
        if let Some(id) = self.terms[self.term_ix].proc.filter(|id| self.procs.is_running(*id)) {
            push_text(&mut self.terms[self.term_ix].lines, "^C\n", false);
            self.procs.kill(id);
        } else {
            self.set_term_input("", window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_term_escape(&mut self, _: &TermEscape, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_sel = None;
        self.terms[self.term_ix].hist_ix = None;
        self.terms[self.term_ix].tab_cycle = None;
        self.set_term_input("", window, cx);
    }

    pub(crate) fn on_term_clear(&mut self, _: &TermClear, _: &mut Window, cx: &mut Context<Self>) {
        self.panel_sel = None;
        self.terms[self.term_ix].lines.clear();
        cx.notify();
    }


    pub(crate) fn on_run_file(&mut self, _: &RunFile, _: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else {
            self.status = tr("Open a file to run it").into();
            cx.notify();
            return;
        };
        if self.docs[ix].dirty {
            let doc = &mut self.docs[ix];
            if let Some(ed) = doc.editor.as_ref() {
                let text = ed.read(cx).value();
                if crate::core::write_text(&doc.path, &text, doc.crlf).is_ok() {
                    doc.dirty = false;
                }
            }
        }
        let path = self.docs[ix].path.clone();
        let wsl = self.wsl.clone();
        let source = self.docs[ix].text(cx);
        let runtimes = if wsl.is_some() {
            if self.wsl_runtimes == 0 { u32::MAX } else { self.wsl_runtimes }
        } else {
            let mask = local_runtimes_mask();
            if mask == 0 { probe_local_runtimes(self.root.as_deref()) } else { mask }
        };
        if let RunVerdict::NotRunnable(reason) = check_script_runnable(&path, &source, self.root.as_deref(), wsl.is_some(), runtimes) {
            self.status = crate::trf!("{0} can't be run: {1}"; 0 => self.docs[ix].name, 1 => tr(reason)).into();
            cx.notify();
            return;
        }
        let Some((mut program, mut args)) = run_command_for(&path, self.root.as_deref()) else {
            self.status = crate::trf!("Don't know how to run {0}"; 0 => self.docs[ix].name).into();
            cx.notify();
            return;
        };
        if let Some(t) = &wsl {
            if matches!(program.as_str(), "powershell" | "cmd.exe") {
                self.status = crate::trf!("Don't know how to run {0} in WSL"; 0 => self.docs[ix].name).into();
                cx.notify();
                return;
            }
            let linux = t.linux_path(&path).unwrap_or_else(|| path.display().to_string());
            let unc = path.to_string_lossy().into_owned();
            for a in &mut args {
                if *a == unc {
                    *a = linux.clone();
                }
            }
            if program == "python" || program == "py" {
                program = "python3".to_string();
            }
        }
        if let Some(id) = self.out_proc.take() {
            self.procs.kill(id);
        }
        self.problems.retain(|p| p.from_terminal);
        self.apply_problems(cx);
        self.out_lines.clear();
        let shown: Vec<String> = std::iter::once(program.clone())
            .chain(args.iter().map(|a| match Path::new(a).file_name() {
                Some(n) if Path::new(a).is_absolute() => n.to_string_lossy().into_owned(),
                _ => a.clone(),
            }))
            .collect();
        let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut head = Line::new(&shown.join(" "), LineKind::Cmd);
        head.meta = match &wsl {
            Some(t) => t.linux_path(&cwd).unwrap_or_else(|| t.linux_root.clone()),
            None => term::short_cwd(&cwd, self.root.as_deref()),
        }
        .into();
        self.out_lines.push(head);
        let cmd = match &wsl {
            Some(t) => {
                let cwd = t.linux_path(&cwd).unwrap_or_else(|| t.linux_root.clone());
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                crate::remote::program_command(t, &cwd, &program, &refs)
            }
            None => {
                let mut c = proc::command(&program);
                c.args(&args);
                c
            }
        };
        let local_cwd = wsl.is_none().then(|| cwd.clone());
        match self.procs.spawn(cmd, local_cwd.as_deref()) {
            Ok(id) => {
                self.out_proc = Some(id);
                self.out_run = Some(RunInfo { started: Instant::now(), block: self.out_lines.len(), python: program.clone(), cwd, command: shown.join(" ") });
            }
            Err(e) => push_line(&mut self.out_lines, &crate::trf!("Couldn't start '{program}': {e}"; program => program, e => e), LineKind::Err),
        }
        self.panel_open = true;
        self.panel_tab = PanelTab::Output;
        cx.notify();
    }

    pub(crate) fn on_stop_run(&mut self, _: &StopRun, _: &mut Window, cx: &mut Context<Self>) {
        for id in [self.out_proc, self.terms[self.term_ix].proc].into_iter().flatten() {
            self.procs.kill(id);
        }
        cx.notify();
    }

    /// `term` names the terminal tab that finished; `None` is the Output tab.
    pub(crate) fn finish_run(&mut self, term: Option<usize>, code: Option<i32>, cx: &mut Context<Self>) {
        let terminal = term.is_some();
        let run = match term {
            Some(ix) => self.terms[ix].run.take(),
            None => self.out_run.take(),
        };
        let lines = match term {
            Some(ix) => &mut self.terms[ix].lines,
            None => &mut self.out_lines,
        };
        close_line(lines);
        let mut error: Option<PyError> = None;
        let mut reason: Option<String> = None;
        let from = run.as_ref().map_or(lines.len(), |r| r.block.min(lines.len()));
        if let Some(run) = &run {
            if code != Some(0) || lines[from..].iter().any(|l| l.kind == LineKind::Err) {
                let view: Vec<(&str, LineKind)> = lines[from..].iter().map(|l| (l.text.as_ref(), l.kind)).collect();
                error = term::parse_problem(&view, &run.command, &run.python, &run.cwd);
                if error.is_none() && code.is_some_and(|c| c != 0) {
                    reason = term::failure_reason(&view);
                }
            }
        }
        let output = lines[from..].iter().filter(|l| !l.text.trim().is_empty()).count();
        let spent = run.as_ref().map(|r| r.started.elapsed());
        let quiet = terminal && code == Some(0) && error.is_none() && spent.is_some_and(|d| d < std::time::Duration::from_secs(1));
        let inline_shell_err = terminal
            && error
                .as_ref()
                .is_some_and(|e| e.source == "runtime" && e.file.is_none());
        let took = spent.map(term::elapsed).unwrap_or_default();
        if inline_shell_err {
            if let Some(e) = &error {
                let mut idx = from;
                while idx < lines.len() {
                    if term::is_shell_not_found_line(&lines[idx].text) {
                        lines.remove(idx);
                    } else {
                        idx += 1;
                    }
                }
                while lines.len() > from && lines.last().is_some_and(|l| l.text.trim().is_empty()) {
                    lines.pop();
                }
                let mut head = Line::new(&e.message, LineKind::CardHead);
                let exit_tag = code.map(|c| crate::trf!("exit {c}"; c => c)).unwrap_or_default();
                head.meta = match (exit_tag.is_empty(), took.is_empty()) {
                    (false, false) => format!("{exit_tag} · {took}").into(),
                    (false, true) => exit_tag.into(),
                    _ => took.into(),
                };
                lines.push(head);
                if let Some(h) = &e.hint {
                    lines.push(Line::new(h, LineKind::CardHint));
                }
                if let Some(fix) = &e.fix {
                    let mut l = Line::new(fix, LineKind::CardAction);
                    l.run = Some(fix.clone().into());
                    lines.push(l);
                }
            }
        } else {
            match (&error, &run) {
                (Some(e), Some(run)) => push_card(lines, e, &run.cwd),
                _ if !quiet && output >= 2 && lines.last().is_some_and(|l| !l.text.trim().is_empty()) => push_line(lines, "", LineKind::Blank),
                _ => {}
            }
            let (text, ok) = match code {
                Some(0) => (tr("Finished").to_string(), true),
                Some(c) if terminal => (crate::trf!("exit {c}"; c => c), false),
                Some(c) => (crate::trf!("Failed (exit code {c})"; c => c), false),
                None => (tr("Stopped").to_string(), false),
            };
            let mut status = Line::new(&text, LineKind::Status);
            status.err = !ok;
            status.meta = took.into();
            if !quiet {
                lines.push(status);
            }
        }
        if let Some(r) = reason {
            let link = term::parse_path_loc(&r);
            let shown = match &link {
                Some((p, _, _)) => match (r.find(p.as_str()), Path::new(p).file_name()) {
                    (Some(at), Some(name)) => format!("{}{}", name.to_string_lossy(), &r[at + p.len()..]),
                    _ => r.clone(),
                },
                None => r.clone(),
            };
            let mut l = Line::new(&shown, LineKind::Result);
            l.err = true;
            l.link = link.map(|(p, n, col)| (p.into(), n, col));
            lines.push(l);
        }

        if let Some(e) = error {
            if let (Some(path), Some(line)) = (e.file.clone(), e.line) {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let loc = match e.col.filter(|&c| c > 0) {
                    Some(c) => format!("{name}:{line}:{c}"),
                    None => format!("{name}:{line}"),
                };
                self.status = format!("{}: {} ({loc})", e.kind, e.message).into();
                self.problems.retain(|p| p.from_terminal != terminal);
                self.problems.push(Problem {
                    from_terminal: terminal,
                    path,
                    line,
                    col: e.col.unwrap_or(0),
                    source: e.source,
                    caret: e.caret,
                    message: format!("{}: {}", e.kind, e.message),
                });
                self.apply_problems(cx);
            }
        }
        match term {
            Some(ix) => self.terms[ix].scroll.scroll_to_bottom(),
            None => {
                self.out_scroll.scroll_to_bottom();
                self.ext_after_run(code, cx);
            }
        }
    }

    pub(crate) fn apply_problems(&mut self, cx: &mut Context<Self>) {
        for ix in 0..self.docs.len() {
            self.apply_problems_to(ix, cx);
        }
    }

    pub(crate) fn apply_problems_to(&mut self, ix: usize, cx: &mut Context<Self>) {
        const MAX_SYNC_DIAG_BYTES: usize = 128 * 1024;
        const MAX_DIAG_BYTES: usize = 2 * 1024 * 1024;

        let Some(d) = self.docs.get(ix) else { return };
        let Some(editor) = d.editor.clone() else { return };
        let diag_ranges = d.diag_ranges.clone();
        let diag_ruler = d.diag_ruler.clone();
        let mine: Vec<Problem> = self.problems.iter().filter(|p| same_path(&p.path, &d.path)).cloned().collect();
        let lang = (!d.read_only).then_some(d.lang);
        let is_nbt = d.nbt_format.is_some();
        let doc_path = d.path.clone();
        let doc_gen = d.gen;
        let rope_len = editor.read(cx).text().len();

        if rope_len <= MAX_SYNC_DIAG_BYTES {
            let source = editor.read(cx).value();
            let code_diags = if is_nbt {
                crate::nbt::diagnose(&source)
            } else {
                lang.map(|l| crate::diagnostics::analyze(&source, l)).unwrap_or_default()
            };
            self.refresh_doc_runnability_with(ix, &source);
            let err_n = code_diags.iter().filter(|c| c.severity == crate::diagnostics::Severity::Error).count();
            let warn_n = code_diags.len() - err_n;
            editor.update(cx, |ed, cx| {
                let mine_diags = build_mine_diagnostics(&mine, ed.text());
                write_diagnostics(ed, mine_diags, &code_diags, cx);
            });
            sync_diagnostic_decorations(&editor, diag_ranges.as_ref(), diag_ruler.as_ref(), &mine, &code_diags, cx);
            if let Some(d) = self.docs.get_mut(ix) {
                d.diagnostics = code_diags;
                d.error_count = err_n;
                d.warn_count = warn_n;
            }
            return;
        }

        let mut async_rope = None;
        editor.update(cx, |ed, cx| {
            let mine_diags = build_mine_diagnostics(&mine, ed.text());
            if lang.is_some() && rope_len <= MAX_DIAG_BYTES {
                async_rope = Some(ed.text().clone());
            }
            write_diagnostics(ed, mine_diags, &[], cx);
        });
        sync_diagnostic_decorations(&editor, diag_ranges.as_ref(), diag_ruler.as_ref(), &mine, &[], cx);
        if async_rope.is_none() {
            if let Some(d) = self.docs.get_mut(ix) {
                d.diagnostics.clear();
                d.error_count = 0;
                d.warn_count = 0;
            }
            return;
        }

        if let (Some(l), Some(rope)) = (lang, async_rope) {
            cx.spawn(async move |this, cx| {
                let code_diags = cx
                    .background_spawn(async move {
                        let text = rope.to_string();
                        if is_nbt { crate::nbt::diagnose(&text) } else { crate::diagnostics::analyze(&text, l) }
                    })
                    .await;
                _ = this.update(cx, |this, cx| {
                    let Some(ix) = this.docs.iter().position(|d| same_path(&d.path, &doc_path) && d.gen == doc_gen) else {
                        return;
                    };
                    let Some(ed_ent) = this.docs[ix].editor.clone() else { return };
                    let ranges = this.docs[ix].diag_ranges.clone();
                    let ruler = this.docs[ix].diag_ruler.clone();
                    let err_n = code_diags.iter().filter(|c| c.severity == crate::diagnostics::Severity::Error).count();
                    let warn_n = code_diags.len() - err_n;
                    let mine: Vec<Problem> = this.problems.iter().filter(|p| same_path(&p.path, &doc_path)).cloned().collect();
                    ed_ent.update(cx, |ed, cx| {
                        let mine_diags = build_mine_diagnostics(&mine, ed.text());
                        write_diagnostics(ed, mine_diags, &code_diags, cx);
                    });
                    sync_diagnostic_decorations(&ed_ent, ranges.as_ref(), ruler.as_ref(), &mine, &code_diags, cx);
                    this.docs[ix].diagnostics = code_diags;
                    this.docs[ix].error_count = err_n;
                    this.docs[ix].warn_count = warn_n;
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(crate) fn open_location(&mut self, path: PathBuf, line: u32, col: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !path.is_file() {
            self.status = crate::trf!("{0} doesn't exist"; 0 => path.display()).into();
            cx.notify();
            return;
        }
        self.open_file_now(path, window, cx);
        if let Some(ix) = self.active {
            if let Some(ed) = self.ensure_doc_editor(ix, window, cx) {
                ed.update(cx, |ed, cx| {
                    ed.set_cursor_position(Position::new(line.saturating_sub(1), col), window, cx);
                });
                let h = ed.focus_handle(cx);
                h.focus(window, cx);
            }
        }
    }

    pub(crate) fn on_go_to_problem(&mut self, _: &GoToProblem, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.problems.first().cloned() {
            let col = p.caret.map_or(p.col.saturating_sub(1), |c| c.0 as u32);
            self.open_location(p.path, p.line, col, window, cx);
            return;
        }
        if let Some(ix) = self.active {
            if let Some(d) = self.docs.get(ix).and_then(|doc| doc.diagnostics.first().map(|c| (doc.path.clone(), c.row + 1, c.col))) {
                self.open_location(d.0, d.1, d.2, window, cx);
                return;
            }
        }
        if let Some((path, line, col)) = self.docs.iter().find_map(|doc| doc.diagnostics.first().map(|c| (doc.path.clone(), c.row + 1, c.col))) {
            self.open_location(path, line, col, window, cx);
        }
    }

    fn resolve_link_path(&self, path: &str, terminal: bool) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() && p.is_file() {
            return p;
        }
        let base = if terminal {
            self.terms[self.term_ix].run.as_ref().map(|r| r.cwd.clone()).unwrap_or_else(|| self.terms[self.term_ix].cwd.clone())
        } else {
            self.out_run.as_ref().map(|r| r.cwd.clone()).or_else(|| self.ext.last_cwd.clone()).or_else(|| self.root.clone()).unwrap_or_default()
        };
        let joined = base.join(&p);
        if joined.is_file() {
            return joined;
        }
        if let Some(root) = &self.root {
            let from_root = root.join(&p);
            if from_root.is_file() {
                return from_root;
            }
            let needle = path.replace('\\', "/").trim_start_matches("./").to_ascii_lowercase();
            let suffix = format!("/{needle}");
            if let Some(found) = self.all_files.iter().find(|f| {
                let fp = Path::new(f.as_str());
                let rel = fp.strip_prefix(root).unwrap_or(fp).to_string_lossy().replace('\\', "/").to_ascii_lowercase();
                rel == needle || rel.ends_with(&suffix)
            }) {
                return PathBuf::from(found);
            }
        }
        if p.is_absolute() { p } else { joined }
    }

    fn open_link(&mut self, path: &str, line: u32, col: u32, terminal: bool, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.panel_sel = None;
        let full = self.resolve_link_path(path, terminal);
        self.open_location(full, line, col.saturating_sub(1), window, cx);
    }

    fn run_fix(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_open = true;
        self.panel_tab = PanelTab::Terminal;
        if self.term_running() {
            self.set_term_input(command, window, cx);
            self.status = tr("The terminal is busy: the fix is in the prompt, press Enter when it's free").into();
            cx.notify();
            return;
        }
        if self.terms[self.term_ix].history.last().map(String::as_str) != Some(command) {
            self.terms[self.term_ix].history.push(command.to_string());
        }
        self.run_in_terminal(command, cx);
    }


    #[allow(clippy::too_many_arguments)]
    fn render_line(&self, i: usize, ix: usize, l: &Line, shown: &SharedString, first: bool, terminal: bool, m: Metrics, cx: &mut Context<Self>) -> AnyElement {
        let mono = mono_kind(l.kind);
        let text_left = theme::u(12.) + if first { px(0.) } else { px(m.em * 2.) };
        let span = self.panel_sel.filter(|s| s.terminal == terminal).and_then(|s| s.on_row(i, shown.chars().count()));
        let highlight = span.filter(|_| mono).map(|(a, b)| div().absolute().top_0().bottom_0().bg(c(theme::SELECT)).left(text_left + px(a as f32 * m.em)).w(px((b - a).max(1) as f32 * m.em)));
        let over = span.filter(|_| !mono).map(|_| div().absolute().top_0().bottom_0().left(theme::u(8.)).right(theme::u(8.)).rounded(px(3.)).bg(c(theme::SELECT).opacity(0.6)));
        let base = h_flex()
            .id(("pl", i))
            .relative()
            .h(m.row)
            .w_full()
            .px_3()
            .gap_2()
            .whitespace_nowrap()
            .overflow_hidden()
            .children(highlight)
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let col = if mono { this.panel_col(ev.position.x, text_left, m) } else { 0 };
                this.panel_sel_press(terminal, i, col, mono, ev.click_count, ev.modifiers.shift, cx);
            }))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if ev.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let Some(sel) = this.panel_sel.filter(|s| s.terminal == terminal) else { return };
                let col = if mono { this.panel_col(ev.position.x, text_left, m) } else if i >= sel.anchor.0 { usize::MAX } else { 0 };
                if sel.head != (i, col) {
                    this.panel_sel = Some(PanelSel { head: (i, col), ..sel });
                    cx.notify();
                }
            }));
        let text = |s: &SharedString, col: theme::Tok| div().text_color(c(col)).child(s.clone());
        let ui = |s: &SharedString, col: theme::Tok| div().font_family(UI_FONT).text_color(c(col)).child(s.clone());
        let hang = theme::u(12.) + px(m.em * 2.);
        let card = |row: Stateful<Div>, inner: Div| {
            row.px_0().child(
                inner
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .px_3()
                    .gap_2()
                    .border_l_2()
                    .border_color(c(theme::DANGER).opacity(0.75))
                    .bg(c(theme::DANGER).opacity(0.045)),
            )
            .into_any_element()
        };
        let link = |label: SharedString, path: SharedString, line: u32, col: u32, cx: &mut Context<Self>| {
            div()
                .id(("lk", i))
                .text_color(c(theme::BLUE))
                .cursor_pointer()
                .hover(|s| s.underline())
                .child(label)
                .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, col, terminal, w, cx)))
        };
        let el = match l.kind {
            LineKind::Blank => base.into_any_element(),
            LineKind::Cmd if terminal => {
                let place: SharedString = prompt_place(&l.meta, self.panel_size.get().0).into();
                let full = l.meta.clone();
                base.child(
                    div()
                        .id(("wd", i))
                        .flex_none()
                        .font_family(UI_FONT)
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .child(place)
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx)),
                )
                .child(text(&"❯".into(), theme::ACCENT_TEXT).flex_none())
                .child(text(&l.text, theme::FG_STRONG).min_w_0().truncate())
                .into_any_element()
            }
            LineKind::Cmd => {
                let lines = &self.out_lines;
                let (status, last) = block_status(lines, ix);
                let live = self.out_run.is_some();
                let badge = match status {
                    Some(s) => {
                        let stopped = s.err && s.text.as_ref() == tr("Stopped");
                        let (icon, col) = match (s.err, stopped) {
                            (_, true) => (IconName::Square, theme::FG_DIM),
                            (true, _) => (IconName::CircleX, theme::DANGER),
                            _ => (IconName::CircleCheck, theme::GREEN),
                        };
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .child(Icon::new(icon).size(theme::u(12.)).text_color(c(col)))
                            .when(!s.meta.is_empty(), |h| h.child(ui(&s.meta, theme::FG_DIM)))
                            .into_any_element()
                    }
                    None if last && live => ui(&tr("Running").into(), theme::BLUE).flex_none().into_any_element(),
                    None => div().into_any_element(),
                };
                base.child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .h(m.row - px(2.))
                        .px_2()
                        .gap_2()
                        .rounded(px(4.))
                        .bg(c(theme::BG_ELEVATED))
                        .child(text(&"❯".into(), theme::FG_DIM).flex_none())
                        .child(text(&l.text, theme::FG_STRONG).flex_none().max_w(relative(0.6)).truncate())
                        .child(div().flex_1())
                        .when(!l.meta.is_empty(), |h| h.child(ui(&l.meta, theme::FG_DIM).min_w_0().truncate()))
                        .child(badge),
                )
                .into_any_element()
            }
            LineKind::Fixed => base
                .child(ui(&tr("↳ auto-corrected").into(), theme::BLUE).flex_none())
                .child(text(&l.text, theme::FG_MUTED).min_w_0().truncate())
                .into_any_element(),
            LineKind::Status => {
                let stopped = l.err && l.text.as_ref() == tr("Stopped");
                let (icon, col) = match (l.err, stopped) {
                    (_, true) => (IconName::Square, theme::FG_MUTED),
                    (true, _) => (IconName::CircleX, theme::DANGER),
                    _ => (IconName::CircleCheck, theme::GREEN),
                };
                let took: SharedString = match (l.meta.is_empty(), l.err) {
                    (true, _) => "".into(),
                    (false, false) => crate::trf!("in {0}"; 0 => l.meta).into(),
                    (false, true) => format!("· {}", l.meta).into(),
                };
                let quiet = terminal && !l.err;
                base.font_family(UI_FONT)
                    .child(Icon::new(icon).size(theme::u(if quiet { 12. } else { 14. })).text_color(c(col)))
                    .child(
                        div()
                            .when(!quiet, |d| d.font_weight(FontWeight::MEDIUM))
                            .text_color(c(if l.err && !stopped {
                                theme::DANGER
                            } else if quiet {
                                theme::FG_MUTED
                            } else {
                                theme::FG_STRONG
                            }))
                            .child(l.text.clone()),
                    )
                    .when(!took.is_empty(), |h| h.child(text(&took, theme::FG_MUTED)))
                    .into_any_element()
            }
            LineKind::Result if l.err => {
                let row = base.pl(theme::u(34.)).font_family(UI_FONT);
                match l.link.clone() {
                    Some((path, line, col)) => row.child(
                        div()
                            .id(("lk", i))
                            .min_w_0()
                            .text_color(c(theme::FG_STRONG))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::BLUE)).underline())
                            .child(shown.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, col, terminal, w, cx))),
                    ),
                    None => row.child(text(shown, theme::FG_STRONG).min_w_0()),
                }
                .into_any_element()
            }
            LineKind::Result => {
                let parts: Vec<&str> = l.text.split(" · ").collect();
                let row = base.pl(theme::u(34.)).font_family(UI_FONT);
                match parts.as_slice() {
                    [name, mid @ .., path] => {
                        let mid: SharedString = mid.join(" · ").into();
                        row.child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(c(theme::FG_STRONG)).child(name.to_string()))
                            .when(!mid.is_empty(), |h| h.child(text(&mid, theme::FG_MUTED).min_w_0().truncate()))
                            .child(text(&path.to_string().into(), theme::FG_DIM).flex_1().min_w_0().truncate())
                            .into_any_element()
                    }
                    _ => row.child(text(&l.text, theme::FG_STRONG).min_w_0().truncate()).into_any_element(),
                }
            }
            LineKind::Frame | LineKind::Warn | LineKind::Err | LineKind::Normal | LineKind::Dim | LineKind::Code if l.link.is_some() => {
                let (path, line, line_col) = l.link.clone().unwrap_or_default();
                let col = match l.kind {
                    LineKind::Warn => theme::WARN,
                    LineKind::Err => theme::DANGER,
                    LineKind::Frame => theme::FG_MUTED,
                    LineKind::Dim => theme::FG_DIM,
                    LineKind::Code => theme::FG_STRONG,
                    _ => theme::FG,
                };
                let tip: SharedString = if line_col > 0 {
                    format!("{path}:{line}:{line_col}").into()
                } else {
                    format!("{path}:{line}").into()
                };
                base.when(!first, |b| b.pl(hang))
                    .child(
                        div()
                            .id(("lk", i))
                            .text_color(c(col))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::BLUE)).underline())
                            .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                            .child(shown.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, line_col, terminal, w, cx))),
                    )
                    .into_any_element()
            }
            LineKind::CardHead => {
                let mut inner = h_flex()
                    .font_family(UI_FONT)
                    .child(Icon::new(IconName::CircleX).size(theme::u(14.)).text_color(c(theme::DANGER)))
                    .child(div().flex_shrink(1.).min_w_0().truncate().text_color(c(theme::DANGER)).font_weight(FontWeight::SEMIBOLD).child(l.text.clone()));
                if let Some((path, line, col)) = &l.link {
                    inner = inner.child(text(&"in".into(), theme::FG_DIM)).child(link(l.meta.clone(), path.clone(), *line, *col, cx).min_w_0().truncate());
                } else if !l.meta.is_empty() {
                    inner = inner.child(div().flex_none().text_xs().text_color(c(theme::FG_DIM)).child(format!("· {}", l.meta)));
                }
                card(base, inner)
            }
            LineKind::CardMsg => card(base, h_flex().child(ui(shown, theme::FG_STRONG).pl(theme::u(22.)))),
            LineKind::CardCode => {
                let mut inner = h_flex()
                    .child(div().flex_none().pl(theme::u(22.)).text_color(c(theme::FG_DIM)).child(l.meta.clone()))
                    .child(div().flex_none().text_color(c(theme::BORDER_STRONG)).child("│"));
                if let Some((path, line, col)) = l.link.clone() {
                    inner = inner.child(
                        div()
                            .id(("lk", i))
                            .min_w_0()
                            .truncate()
                            .text_color(c(theme::FG))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::BLUE)).underline())
                            .child(l.text.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, col, terminal, w, cx))),
                    );
                } else {
                    inner = inner.child(text(&l.text, theme::FG).min_w_0().truncate());
                }
                card(base, inner)
            }
            LineKind::CardHint => card(
                base,
                h_flex()
                    .when(first, |h| h.child(Icon::new(IconName::Info).size(theme::u(14.)).text_color(c(theme::WARN))))
                    .child(ui(shown, theme::FG_MUTED).when(!first, |d| d.pl(theme::u(22.)))),
            ),
            LineKind::CardAction => {
                let cmd = l.run.clone().unwrap_or_default();
                card(
                    base,
                    h_flex()
                        .child(
                            h_flex()
                                .id(("fix", i))
                                .flex_none()
                                .ml(theme::u(22.))
                                .px_2()
                                .gap_1()
                                .rounded(px(3.))
                                .font_family(UI_FONT)
                                .bg(c(theme::ACCENT))
                                .text_color(c(theme::ON_ACCENT))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.88))
                                .child(Icon::new(IconName::Play).size(theme::u(12.)))
                                .child(tr("Fix"))
                                .on_click(cx.listener(move |this, _, w, cx| this.run_fix(&cmd, w, cx))),
                        )
                        .child(text(&l.text, theme::FG).min_w_0().truncate()),
                )
            }
            k => {
                let col = match k {
                    LineKind::Err | LineKind::Caret => theme::DANGER,
                    LineKind::Warn => theme::WARN,
                    LineKind::Dim => theme::FG_DIM,
                    LineKind::Success => theme::GREEN,
                    LineKind::Code => theme::FG_STRONG,
                    LineKind::Frame => theme::FG_MUTED,
                    _ => theme::FG,
                };
                base.when(!first, |b| b.pl(hang)).child(div().text_color(c(col)).child(shown.clone())).into_any_element()
            }
        };
        match over {
            Some(o) => div().relative().h(m.row).w_full().child(el).child(o).into_any_element(),
            None => el,
        }
    }

    fn panel_col(&self, x: Pixels, text_left: Pixels, m: Metrics) -> usize {
        let from = f32::from(x) - self.panel_left.get() - f32::from(text_left);
        (from / m.em).round().max(0.) as usize
    }

    #[allow(clippy::too_many_arguments)]
    fn panel_sel_press(&mut self, terminal: bool, i: usize, col: usize, mono: bool, clicks: usize, shift: bool, cx: &mut Context<Self>) {
        let old = self.panel_sel.filter(|s| s.terminal == terminal);
        let rows = self.panel_rows(terminal);
        let text: Vec<char> = rows.get(i).map(|r| r.text.chars().collect()).unwrap_or_default();
        self.panel_sel = Some(match (shift, old, clicks) {
            (true, Some(s), _) => PanelSel { head: (i, col), ..s },
            (_, _, 2) if mono => {
                let at = col.min(text.len().saturating_sub(1));
                let word = |c: &char| !c.is_whitespace();
                if text.get(at).is_some_and(word) {
                    let a = (0..=at).rev().take_while(|&k| word(&text[k])).last().unwrap_or(at);
                    let b = (at..text.len()).take_while(|&k| word(&text[k])).last().map_or(at, |k| k + 1);
                    PanelSel { terminal, anchor: (i, a), head: (i, b) }
                } else {
                    PanelSel { terminal, anchor: (i, col), head: (i, col) }
                }
            }
            (_, _, n) if n >= 2 => PanelSel { terminal, anchor: (i, 0), head: (i, usize::MAX) },
            _ => PanelSel { terminal, anchor: (i, col), head: (i, col) },
        });
        cx.notify();
    }

    fn panel_rows(&self, terminal: bool) -> std::rc::Rc<Vec<Row>> {
        (if terminal { &self.terms[self.term_ix].wrap } else { &self.out_wrap }).borrow().1.clone()
    }

    pub(crate) fn panel_sel_text(&self) -> Option<String> {
        let sel = self.panel_sel.filter(|s| !s.is_empty())?;
        let rows = self.panel_rows(sel.terminal);
        let lines = if sel.terminal { &self.terms[self.term_ix].lines } else { &self.out_lines };
        let ((r0, _), (r1, _)) = sel.span();
        let last = r1.min(rows.len().checked_sub(1)?);
        let mut out: Vec<String> = Vec::new();
        let mut i = r0;
        while i <= last {
            let ix = rows[i].line;
            let mut j = i;
            while j < last && rows[j + 1].line == ix {
                j += 1;
            }
            let Some(l) = lines.get(ix) else { break };
            let whole = rows[i].first
                && (j + 1 >= rows.len() || rows[j + 1].line != ix)
                && (i..=j).all(|k| {
                    let n = rows[k].text.chars().count();
                    sel.on_row(k, n) == Some((0, n))
                });
            let piece = if whole || !mono_kind(l.kind) {
                match (l.kind, l.meta.is_empty()) {
                    (LineKind::Status, false) => format!("{} {}", l.text, l.meta),
                    _ => l.text.to_string(),
                }
            } else {
                (i..=j)
                    .filter_map(|k| {
                        let t = &rows[k].text;
                        let (a, b) = sel.on_row(k, t.chars().count())?;
                        Some(t.chars().skip(a).take(b - a).collect::<String>())
                    })
                    .collect()
            };
            out.push(piece);
            i = j + 1;
        }
        Some(out.join("\n"))
    }

    pub(crate) fn panel_copy(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.panel_sel_text() else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.status = tr("Copied").into();
        cx.notify();
        true
    }

    fn wrapped_rows(&self, terminal: bool, cols: Cols) -> std::rc::Rc<Vec<Row>> {
        let (lines, cache) = if terminal { (&self.terms[self.term_ix].lines, &self.terms[self.term_ix].wrap) } else { (&self.out_lines, &self.out_wrap) };
        let key = (
            lines.len(),
            lines.first().map_or(0, |l| l.text.as_ptr() as usize),
            lines.last().map_or(0, |l| l.text.as_ptr() as usize),
            lines.last().map_or(0, |l| l.text.len()),
            cols.mono | cols.card << 20 | cols.result << 40,
        );
        let mut cache = cache.borrow_mut();
        if cache.0 == key {
            return cache.1.clone();
        }
        let mut rows = Vec::with_capacity(lines.len() + 8);
        for (ix, l) in lines.iter().enumerate() {
            let marked = l.kind == LineKind::Code && lines.get(ix + 1).is_some_and(|n| n.kind == LineKind::Caret);
            let (width, hang) = match l.kind {
                LineKind::Normal | LineKind::Err | LineKind::Warn | LineKind::Dim | LineKind::Success | LineKind::Frame => (cols.mono, 2),
                LineKind::Code if !marked => (cols.mono, 2),
                LineKind::CardMsg | LineKind::CardHint => (cols.card, 0),
                LineKind::Result if l.err => (cols.result, 0),
                _ => (0, 0),
            };
            if width == 0 {
                rows.push(Row { line: ix, text: l.text.clone(), first: true });
                continue;
            }
            let shown = term::display_text(&l.text);
            let pieces = term::wrap_ranges(&shown, width, hang);
            if pieces.len() == 1 {
                let text: SharedString = match shown {
                    std::borrow::Cow::Borrowed(_) => l.text.clone(),
                    std::borrow::Cow::Owned(s) => s.into(),
                };
                rows.push(Row { line: ix, text, first: true });
            } else {
                for (n, (a, b)) in pieces.into_iter().enumerate() {
                    let piece = if n == 0 { &shown[a..b] } else { shown[a..b].trim_start() };
                    rows.push(Row { line: ix, text: piece.to_string().into(), first: n == 0 });
                }
            }
        }
        let rows = std::rc::Rc::new(rows);
        *cache = (key, rows.clone());
        rows
    }

    fn ran_python(lines: &[Line]) -> bool {
        lines.iter().rev().find(|l| l.kind == LineKind::Cmd).is_some_and(|l| {
            let mut words = l.text.split_whitespace();
            let first = words.next().unwrap_or("").trim_matches('"');
            term::is_python(first)
                || ["pip", "pip3", "pip.exe"].iter().any(|p| first.eq_ignore_ascii_case(p))
                || l.text.split_whitespace().any(|w| {
                    let w = w.trim_matches('"').to_ascii_lowercase();
                    w.ends_with(".py") || w.ends_with(".pyw")
                })
        })
    }

    fn collect_problems_rows(&self) -> Vec<ProblemRow> {
        let mut rows = Vec::new();
        for p in &self.problems {
            let file = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            rows.push(ProblemRow {
                is_error: true,
                path: p.path.clone(),
                file: file.into(),
                line: p.line.max(1),
                col: p.caret.map_or(p.col.saturating_sub(1), |c| c.0 as u32),
                message: p.message.clone().into(),
                source: p.source,
            });
        }
        // Show active file's diagnostics first, then other open files.
        let mut order: Vec<usize> = Vec::with_capacity(self.docs.len());
        if let Some(a) = self.active.filter(|&a| a < self.docs.len()) {
            order.push(a);
        }
        for i in 0..self.docs.len() {
            if Some(i) != self.active {
                order.push(i);
            }
        }
        for ix in order {
            let d = &self.docs[ix];
            for c in &d.diagnostics {
                rows.push(ProblemRow {
                    is_error: c.severity == crate::diagnostics::Severity::Error,
                    path: d.path.clone(),
                    file: d.name.clone(),
                    line: c.row + 1,
                    col: c.col,
                    message: c.message.clone().into(),
                    source: c.source,
                });
            }
        }
        rows
    }

    fn render_problems_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = std::rc::Rc::new(self.collect_problems_rows());
        if rows.is_empty() {
            return v_flex()
                .id("problems-empty")
                .size_full()
                .px_3()
                .py_3()
                .gap_1()
                .text_xs()
                .text_color(c(theme::FG_DIM))
                .child(div().text_sm().text_color(c(theme::FG_MUTED)).child(tr("No problems detected")))
                .child(tr("Syntax and writing errors in open files appear here as you type."))
                .into_any_element();
        }
        let count = rows.len();
        let row_h = theme::u(26.);
        let list = uniform_list(
            "problems-list",
            count,
            cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|i| {
                        let r = &rows[i];
                        let (icon, col) = if r.is_error {
                            (IconName::CircleX, theme::DANGER)
                        } else {
                            (IconName::TriangleAlert, theme::WARN)
                        };
                        let path = r.path.clone();
                        let (line, col_ix) = (r.line, r.col);
                        let loc: SharedString = format!("{}  Ln {}, Col {}", r.file, r.line, r.col + 1).into();
                        h_flex()
                            .id(("prob-row", i))
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .h(row_h)
                            .px_3()
                            .gap_2()
                            .items_center()
                            .cursor_pointer()
                            .text_xs()
                            .hover(|s| s.bg(c(theme::BG_HOVER)))
                            .child(Icon::new(icon).flex_none().size(theme::u(13.)).text_color(c(col)))
                            .child(div().flex_1().min_w_0().truncate().text_color(c(theme::FG_STRONG)).child(r.message.clone()))
                            .child(
                                div()
                                    .flex_none()
                                    .px_1p5()
                                    .rounded(px(3.))
                                    .bg(c(theme::BG_ELEVATED))
                                    .text_color(c(theme::FG_DIM))
                                    .child(r.source),
                            )
                            .child(
                                div()
                                    .flex_shrink(1.)
                                    .min_w_0()
                                    .max_w(relative(0.45))
                                    .truncate()
                                    .text_color(c(theme::FG_MUTED))
                                    .child(loc),
                            )
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.open_location(path.clone(), line, col_ix, w, cx);
                            }))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.problems_scroll)
        .size_full();

        div()
            .id("problems-body")
            .relative()
            .flex_1()
            .min_h_0()
            .size_full()
            .child(list)
            .into_any_element()
    }

    pub(crate) fn render_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prob_errs: usize = self.problems.len() + self.docs.iter().map(|d| d.error_count).sum::<usize>();
        let prob_warns: usize = self.docs.iter().map(|d| d.warn_count).sum();
        let prob_total = prob_errs + prob_warns;

        let is_problems = self.panel_tab == PanelTab::Problems;
        let terminal = self.panel_tab == PanelTab::Terminal;
        let motion = !cx.reduce_motion();
        let tab_h = f32::from(theme::u(26.));
        let tab_specs = [
            ("p-term", tr("Terminal"), None, PanelTab::Terminal),
            ("p-out", tr("Output"), None, PanelTab::Output),
            (
                "p-prob",
                tr("Problems"),
                (prob_total > 0).then_some((prob_total, if prob_errs > 0 { theme::DANGER } else { theme::WARN })),
                PanelTab::Problems,
            ),
        ];
        let active_tab_ix = tab_specs.iter().position(|(_, _, _, w)| *w == self.panel_tab);
        let tab_liq = crate::ui::liquid::keyed_state("panel-tab-liq", window, cx);
        let (tab_frame, has_tab_liq) = crate::ui::liquid::keyed_sync_and_sample(&tab_liq, false, tab_specs.len(), active_tab_ix, tab_h, 190, motion);
        let panel_view_id = window.current_view();
        let tab_origin = crate::ui::liquid::keyed_origin_probe(tab_liq.clone(), panel_view_id);
        let tab_liq_layer = tab_frame.map(|fr| {
            let st = crate::ui::liquid::LiquidStyle {
                inset_a: 1.0,
                inset_b: 1.0,
                cross_base: tab_h,
                max_squeeze: 2.0,
                radius: 6.0,
                capillary: 20.0,
                active_bg: c(theme::ACCENT).opacity(0.18),
                active_border: c(theme::ACCENT).opacity(0.55),
                hover_bg: c(theme::BG_HOVER).opacity(0.75),
                hover_border: c(theme::BORDER_STRONG).opacity(0.78),
                union_border: c(theme::ACCENT).opacity(0.58),
                bridge_bg: c(theme::ACCENT).opacity(0.18),
            };
            crate::ui::liquid::render_coalesced_layer("panel-tab-liq-l", false, st, fr)
        });
        let tab_bar = h_flex()
            .id("panel-tab-bar")
            .relative()
            .h(theme::u(26.))
            .items_center()
            .gap_1()
            .on_hover({
                let h = tab_liq.clone();
                move |entered: &bool, _, cx| {
                    if !*entered && motion {
                        crate::ui::liquid::keyed_hover_leave(&h, panel_view_id, cx);
                    }
                }
            })
            .child(tab_origin)
            .children(tab_liq_layer)
            .children(tab_specs.into_iter().enumerate().map(|(i, (id, label, badge, which))| {
                let on = self.panel_tab == which;
                let probe = crate::ui::liquid::keyed_item_probe(tab_liq.clone(), i, panel_view_id);
                let h_hov = tab_liq.clone();
                h_flex()
                    .id(id)
                    .relative()
                    .flex_none()
                    .whitespace_nowrap()
                    .px_2p5()
                    .h_full()
                    .gap_1p5()
                    .items_center()
                    .rounded(px(6.))
                    .text_sm()
                    .cursor_pointer()
                    .when(on && !has_tab_liq, |d| d.bg(c(theme::ACCENT).opacity(0.18)))
                    .text_color(if on { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                    .when(!on && !motion, |d| d.hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG))))
                    .when(!on && motion, |d| d.hover(|s| s.text_color(c(theme::FG))))
                    .on_hover(move |entered: &bool, _, cx| {
                        if *entered && motion {
                            crate::ui::liquid::keyed_hover_enter(&h_hov, i, false, tab_h, 150, panel_view_id, cx);
                        }
                    })
                    .child(probe)
                    .child(label)
                    .when_some(badge.filter(|(n, _)| *n > 0), |h, (n, col)| {
                        h.child(
                            div()
                                .px_1()
                                .min_w(theme::u(16.))
                                .h(theme::u(16.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(c(col).opacity(0.18))
                                .text_color(c(col))
                                .text_size(theme::u(10.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(if n > 99 { "99+".to_string() } else { n.to_string() }),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.panel_tab = which;
                        cx.notify();
                    }))
            }));

        let running = if terminal { self.terms[self.term_ix].proc.is_some() } else if is_problems { false } else { self.out_proc.is_some() };
        let shown_lines = if terminal { &self.terms[self.term_ix].lines } else { &self.out_lines };
        let python = (!is_problems)
            .then(|| self.python_version.clone().filter(|_| Self::ran_python(shown_lines)))
            .flatten();
        let header = h_flex()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .h(theme::u(35.))
            .px_3()
            .gap_2()
            .items_center()
            .flex_none()
            .child(tab_bar)
            .when(terminal, |h| {
                h.child(div().flex_none().w(px(1.)).h(theme::u(16.)).bg(c(theme::BORDER)))
                    .child(
                        h_flex()
                            .min_w_0()
                            .overflow_hidden()
                            .gap_1()
                            .items_center()
                            .children((0..self.terms.len()).map(|ix| {
                                let on = ix == self.term_ix;
                                let name: SharedString = crate::trf!("Terminal {0}"; 0 => ix + 1).into();
                                h_flex()
                                    .id(("p-tt", ix))
                                    .flex_none()
                                    .h(theme::u(22.))
                                    .px_1p5()
                                    .gap_1()
                                    .items_center()
                                    .rounded(px(4.))
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(if on { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                                    .when(on, |d| d.bg(c(theme::BG_ELEVATED)))
                                    .when(!on, |d| d.hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG))))
                                    .child(name)
                                    .when(self.terms.len() > 1, |d| {
                                        d.child(
                                            div()
                                                .id(("p-ttx", ix))
                                                .flex_none()
                                                .size(theme::u(14.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(3.))
                                                .text_color(c(theme::FG_DIM))
                                                .hover(|s| s.bg(c(theme::BG_HOVER)).text_color(c(theme::FG_STRONG)))
                                                .child("×")
                                                .on_click(cx.listener(move |this, _, w, cx| {
                                                    cx.stop_propagation();
                                                    this.close_terminal(ix, w, cx);
                                                })),
                                        )
                                    })
                                    .on_click(cx.listener(move |this, _, w, cx| this.show_terminal(ix, w, cx)))
                                    .into_any_element()
                            })),
                    )
                    .child(tool_button("p-newterm", IconName::Plus, tr("New terminal"), cx, |this, w, cx| {
                        this.on_new_terminal(&NewTerminal, w, cx)
                    }))
            })
            .child(div().flex_1())
            .when_some(python, |h, v| {
                h.child(
                    h_flex()
                        .min_w_0()
                        .gap_1()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(div().flex_none().size(px(6.)).rounded_full().bg(c(theme::GREEN)))
                        .child(div().min_w_0().truncate().child(v)),
                )
            })
            .when(running, |h| {
                h.child(tool_button("p-stop", IconName::Square, "Stop (Shift+F5 or Ctrl+C)", cx, |this, w, cx| this.on_stop_run(&StopRun, w, cx)))
            })
            .when(!is_problems && self.panel_sel.is_some_and(|s| s.terminal == terminal && !s.is_empty()), |h| {
                h.child(tool_button("p-copy", IconName::Copy, "Copy the selection (Ctrl+C)", cx, |this, _, cx| {
                    this.panel_copy(cx);
                }))
            })
            .when(!is_problems || !self.problems.is_empty(), |h| {
                h.child(tool_button("p-clear", IconName::Delete, "Clear", cx, |this, _, cx| {
                    this.panel_sel = None;
                    match this.panel_tab {
                        PanelTab::Terminal => this.terms[this.term_ix].lines.clear(),
                        PanelTab::Output => this.out_lines.clear(),
                        PanelTab::Problems => {
                            this.problems.clear();
                            this.apply_problems(cx);
                        }
                    }
                    cx.notify();
                }))
            })
            .child(tool_button("p-close", IconName::Close, "Close Panel", cx, |this, _, cx| {
                this.set_panel_open(false, cx);
            }));

        let (prev_tab, mut tab_gen) = self.panel_tab_anim.get();
        if prev_tab != self.panel_tab {
            tab_gen += 1;
            self.panel_tab_anim.set((self.panel_tab, tab_gen));
        }
        let reduce_motion = cx.reduce_motion();
        let wrap_tab = move |el: AnyElement| -> AnyElement {
            if tab_gen == 0 || reduce_motion {
                return el;
            }
            div()
                .flex_1()
                .min_h_0()
                .size_full()
                .flex()
                .flex_col()
                .child(el)
                .with_animation(
                    SharedString::from(format!("panel-tab-{tab_gen}")),
                    Animation::new(std::time::Duration::from_millis(140)).with_easing(gpui_kit::ease_out_quint()),
                    |d, t| d.opacity(0.35 + 0.65 * t),
                )
                .into_any_element()
        };

        if is_problems {
            return v_flex()
                .size_full()
                .bg(c(theme::BG_EDITOR))
                .border_t_1()
                .border_color(c(theme::BORDER))
                .child(header)
                .child(wrap_tab(self.render_problems_body(cx)));
        }

        let count = shown_lines.len();
        let scroll = if terminal { self.terms[self.term_ix].scroll.clone() } else { self.out_scroll.clone() };
        let font_size = px((self.settings.font_size - 2.).clamp(10., 28.));
        let mono_font = theme::resolve_mono_font(&self.settings.font_family, cx);
        let (m, avg) = panel_font_metrics(window, &mono_font, font_size);
        let (width, height) = self.panel_size.get();
        let cols = if width <= 0. {
            Cols { mono: 160, card: 160, result: 160 }
        } else {
            let fit = |indent: f32, per: f32| (((width - indent - f32::from(theme::u(12.)) - 6.) / per).floor() as usize).max(16);
            Cols { mono: fit(f32::from(theme::u(12.)), m.em).max(24), card: fit(f32::from(theme::u(46.)) + 2., avg), result: fit(f32::from(theme::u(34.)), avg) }
        };
        let rows = self.wrapped_rows(terminal, cols);
        let gap = 4.;
        let row = f32::from(m.row);
        let measured = height > 0.;
        let air = terminal && count > 0 && height > gap + row * 3.;
        let prompt_rows = if !terminal { 0. } else if air { 2. } else { 1. };
        let room = height - gap - prompt_rows * row;
        let fit = if room > 0. { (room / row).floor() * row } else { 0. };
        let n_rows = rows.len();
        let list = uniform_list(
            "panel-lines",
            rows.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|i| {
                        let r = &rows[i];
                        let lines = if terminal { &this.terms[this.term_ix].lines } else { &this.out_lines };
                        match lines.get(r.line) {
                            Some(l) => this.render_line(i, r.line, l, &r.text, r.first, terminal, m, cx),
                            None => div().id(("pl", i)).h(m.row).into_any_element(),
                        }
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&scroll)
        .w_full()
        .map(|l| {
            if terminal {
                let all = n_rows as f32 * row;
                l.flex_none().h(px(if measured { fit.min(all) } else { all.min(8. * row) }))
            } else if fit > 0. {
                l.h(px(fit))
            } else {
                l.h_full()
            }
        })
        .font_family(mono_font.clone())
        .font_weight(FontWeight::NORMAL)
        .text_size(font_size);
        let probe = {
            let cell = self.panel_size.clone();
            let id = cx.entity_id();
            let bottom = scroll.clone();
            let left = self.panel_left.clone();
            canvas(
                move |bounds, _, cx| {
                    left.set(f32::from(bounds.origin.x));
                    let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let old = cell.get();
                    if (old.0 - size.0).abs() > 0.5 || (old.1 - size.1).abs() > 0.5 {
                        cell.set(size);
                        let bottom = bottom.clone();
                        cx.defer(move |cx| {
                            bottom.scroll_to_bottom();
                            cx.notify(id);
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
        };
        let narrow = width > 0. && width < 480.;
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .pt(px(gap))
            .child(probe)
            .child(list)
            .when(terminal, |d| {
                d.when(air, |d| d.child(div().h(m.row))).child(self.render_prompt(mono_font, font_size, narrow, m, window, cx))
            });

        let empty = count == 0;
        let content = v_flex()
            .id("panel-body")
            .flex_1()
            .min_h_0()
            .when(empty && !terminal, |v| {
                v.child(
                    div()
                        .flex_shrink(1.)
                        .min_h_0()
                        .overflow_hidden()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(tr("No output yet. Press F5 to run.")),
                )
            })
            .child(body)
            .when(terminal, |v| {
                v.cursor_text().on_click(cx.listener(|this, _, window, cx| {
                    let h = this.term_input.focus_handle(cx);
                    h.focus(window, cx);
                }))
            })
            .into_any_element();
        v_flex()
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .border_t_1()
            .border_color(c(theme::BORDER))
            .child(header)
            .child(wrap_tab(content))
    }

    fn render_prompt(
        &self,
        mono_font: SharedString,
        font_size: Pixels,
        narrow: bool,
        m: Metrics,
        window: &Window,
        cx: &App,
    ) -> impl IntoElement {
        let busy = self.term_running();
        let short = term::short_cwd(&self.terms[self.term_ix].cwd, self.root.as_deref());
        let place: SharedString = prompt_place(&short, self.panel_size.get().0).into();
        let full: SharedString = self.terms[self.term_ix].cwd.display().to_string().into();
        let placeholder = match (busy, narrow) {
            (false, false) => tr("Run command…"),
            (false, true) => tr("Type a command"),
            (true, false) => tr("Running… (Ctrl+C to stop)"),
            (true, true) => tr("Running"),
        };
        let focused = self.term_input.focus_handle(cx).is_focused(window);
        let dark = theme::live_dark();
        let hint = !self.terms[self.term_ix].typed && (busy || self.terms[self.term_ix].lines.is_empty());
        let box_h = m.row.max(theme::u(26.));
        h_flex()
            .h(box_h + px(6.))
            .w_full()
            .flex_none()
            .px_3()
            .pb_1()
            .gap_2()
            .items_center()
            .whitespace_nowrap()
            .font_family(mono_font)
            .text_size(font_size)
            .when(!busy, |h| {
                h.child(
                    div()
                        .id("t-cwd")
                        .flex_none()
                        .font_family(UI_FONT)
                        .text_xs()
                        .text_color(c(theme::FG_MUTED))
                        .child(place)
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(full.clone()).build(w, cx)),
                )
                .child(div().flex_none().text_color(c(theme::ACCENT_TEXT)).child("❯"))
            })
            .child(
                crate::ui::mc_box_frame(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h(box_h)
                        .px_2()
                        .flex()
                        .items_center()
                        .key_context("Terminal"),
                    focused,
                    dark,
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .ml(px(-8.))
                        .child(Input::new(&self.term_input).appearance(false).focus_bordered(false).with_size(px(f32::from(font_size) / 0.875))),
                )
                .when(hint, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .pl(px(8.))
                            .flex()
                            .items_center()
                            .font_family(UI_FONT)
                            .text_xs()
                            .text_color(c(theme::FG_DIM))
                            .overflow_hidden()
                            .child(placeholder),
                    )
                }),
            )
    }
}

struct ProblemRow {
    is_error: bool,
    path: PathBuf,
    file: SharedString,
    line: u32,
    col: u32,
    message: SharedString,
    source: &'static str,
}

#[derive(Clone, Copy)]
struct Cols {
    mono: usize,
    card: usize,
    result: usize,
}

#[derive(Clone, Copy)]
struct Metrics {
    row: Pixels,
    em: f32,
}

fn panel_font_metrics(window: &Window, mono_font: &SharedString, font_size: Pixels) -> (Metrics, f32) {
    type Cached = (SharedString, u32, u32, Metrics, f32);
    thread_local! {
        static CACHE: std::cell::RefCell<Option<Cached>> = const { std::cell::RefCell::new(None) };
    }
    let fs_bits = f32::from(font_size).to_bits();
    let sc_bits = theme::scale().to_bits();
    CACHE.with(|cell| {
        if let Some((f, fb, sb, m, avg)) = cell.borrow().as_ref() {
            if f == mono_font && *fb == fs_bits && *sb == sc_bits {
                return (*m, *avg);
            }
        }
        let ts = window.text_system();
        let id = ts.resolve_font(&font(mono_font.clone()));
        let em = ts.em_advance(id, font_size).map(f32::from).unwrap_or(f32::from(font_size) * 0.6).max(4.);
        let m = Metrics { row: px((f32::from(font_size) * 1.6).round()), em };
        let ui_id = ts.resolve_font(&font(UI_FONT));
        let sample = "Python doesn't know this name, check the spelling: Main.java:12";
        let w: f32 = sample.chars().map(|ch| ts.advance(ui_id, font_size, ch).map(|s| f32::from(s.width)).unwrap_or(em)).sum();
        let avg = (w / sample.chars().count() as f32).max(3.) * 1.08;
        *cell.borrow_mut() = Some((mono_font.clone(), fs_bits, sc_bits, m, avg));
        (m, avg)
    })
}

fn prompt_place(short: &str, width: f32) -> String {
    if width <= 0. {
        return term::fit_path_left(short, 40);
    }
    let narrow = width < 480.;
    let budget = ((width * if narrow { 0.4 } else { 0.34 }) / (f32::from(theme::u(12.)) * 0.5)) as usize;
    if narrow { term::fit_path_left(term::last_folder(short), budget.max(6)) } else { term::fit_path_left(short, budget.max(10)) }
}

const UI_FONT: &str = ".SystemUIFont";

fn block_status(lines: &[Line], cmd: usize) -> (Option<&Line>, bool) {
    for l in lines.iter().skip(cmd + 1) {
        match l.kind {
            LineKind::Cmd => return (None, false),
            LineKind::Status => return (Some(l), false),
            _ => {}
        }
    }
    (None, true)
}

fn push_card(lines: &mut Vec<Line>, e: &PyError, cwd: &Path) {
    if !lines.is_empty() && !matches!(lines.last().map(|l| l.kind), Some(LineKind::Cmd | LineKind::Blank)) {
        push_line(lines, "", LineKind::Blank);
    }
    let compact_msg = !e.message.is_empty() && e.kind.len() + e.message.len() <= 84;
    let title = if compact_msg {
        format!("{}: {}", e.kind, e.message)
    } else {
        e.kind.clone()
    };
    let mut head = Line::new(&title, LineKind::CardHead);
    let col = e.col.unwrap_or(0);
    let link = match (&e.file, e.line) {
        (Some(file), Some(line)) => {
            let shown = file.strip_prefix(cwd).map(|p| p.display().to_string()).unwrap_or_else(|_| {
                file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            });
            head.meta = if col > 0 {
                format!("{shown}:{line}:{col}").into()
            } else {
                format!("{shown}:{line}").into()
            };
            let lk = (SharedString::from(file.display().to_string()), line, col);
            head.link = Some(lk.clone());
            Some(lk)
        }
        _ => None,
    };
    lines.push(head);
    if !e.message.is_empty() && !compact_msg {
        lines.push(Line::new(&e.message, LineKind::CardMsg));
    }
    if let (Some(code), Some(line)) = (&e.code, e.line) {
        let mut l = Line::new(code, LineKind::CardCode);
        l.meta = line.to_string().into();
        l.link = link;
        lines.push(l);
    }
    if let Some(h) = &e.hint {
        lines.push(Line::new(h, LineKind::CardHint));
    }
    if let Some(fix) = &e.fix {
        let mut l = Line::new(fix, LineKind::CardAction);
        l.run = Some(fix.clone().into());
        lines.push(l);
    }
}

pub(crate) const RT_PYTHON: u32 = 1 << 0;
pub(crate) const RT_NODE: u32 = 1 << 1;
pub(crate) const RT_NPX: u32 = 1 << 2;
pub(crate) const RT_POWERSHELL: u32 = 1 << 3;
pub(crate) const RT_CMD: u32 = 1 << 4;
pub(crate) const RT_BASH: u32 = 1 << 5;
pub(crate) const RT_CARGO: u32 = 1 << 6;
pub(crate) const RT_GO: u32 = 1 << 7;
pub(crate) const RT_JAVA: u32 = 1 << 8;
pub(crate) const RT_DART: u32 = 1 << 9;
pub(crate) const RT_ZIG: u32 = 1 << 10;
pub(crate) const RT_SWIFT: u32 = 1 << 11;
pub(crate) const RT_PHP: u32 = 1 << 12;
pub(crate) const RT_LUA: u32 = 1 << 13;
pub(crate) const RT_RUBY: u32 = 1 << 14;
pub(crate) const RT_PERL: u32 = 1 << 15;
pub(crate) const RT_ELIXIR: u32 = 1 << 16;

static LOCAL_RUNTIMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn local_runtimes_mask() -> u32 {
    LOCAL_RUNTIMES.load(std::sync::atomic::Ordering::Relaxed)
}

fn runtime_bit_for_stem(stem_lower: &str) -> u32 {
    match stem_lower {
        "py" | "python" | "python3" => RT_PYTHON,
        "node" => RT_NODE,
        "npx" => RT_NPX,
        "powershell" | "pwsh" => RT_POWERSHELL,
        "cmd" => RT_CMD,
        "bash" => RT_BASH,
        "cargo" => RT_CARGO,
        "go" => RT_GO,
        "java" => RT_JAVA,
        "dart" => RT_DART,
        "zig" => RT_ZIG,
        "swift" => RT_SWIFT,
        "php" => RT_PHP,
        "lua" => RT_LUA,
        "ruby" => RT_RUBY,
        "perl" => RT_PERL,
        "elixir" => RT_ELIXIR,
        _ => 0,
    }
}

/// Scans PATH once per workspace off the UI thread so render() reads an atomic bitmask in O(1).
pub(crate) fn probe_local_runtimes(workspace: Option<&Path>) -> u32 {
    let mut mask = 0u32;
    if let Some(ws) = workspace {
        for venv in [".venv", "venv", "env"] {
            let p = if cfg!(windows) {
                ws.join(venv).join("Scripts").join("python.exe")
            } else {
                ws.join(venv).join("bin").join("python")
            };
            if p.is_file() {
                mask |= RT_PYTHON;
                break;
            }
        }
    }
    if let Some(path_env) = std::env::var_os("PATH") {
        let exts: Vec<String> = if cfg!(windows) {
            std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                .split(';')
                .filter(|e| !e.is_empty())
                .map(|e| e.to_ascii_lowercase())
                .collect()
        } else {
            Vec::new()
        };
        for dir in std::env::split_paths(&path_env) {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                let lower = s.to_ascii_lowercase();
                let (stem, valid_ext) = if cfg!(windows) {
                    if let Some((st, ex)) = lower.rsplit_once('.') {
                        let dot_ext = format!(".{ex}");
                        (st, exts.iter().any(|allowed| allowed == &dot_ext))
                    } else {
                        ("", false)
                    }
                } else {
                    (lower.as_str(), true)
                };
                if !valid_ext {
                    continue;
                }
                let bit = runtime_bit_for_stem(stem);
                if bit != 0 && (mask & bit) == 0 && entry.file_type().map(|t| !t.is_dir()).unwrap_or(false) {
                    mask |= bit;
                }
            }
        }
    }
    LOCAL_RUNTIMES.store(mask, std::sync::atomic::Ordering::Relaxed);
    mask
}

pub(crate) fn probe_wsl_runtimes(target: &crate::remote::WslTarget) -> u32 {
    let script = concat!(
        "for c in python3 node npx bash cargo go java dart zig swift php lua ruby perl elixir; do ",
        "command -v \"$c\" >/dev/null 2>&1 && printf '%s\\n' \"$c\"; ",
        "done"
    );
    let Ok(out) = crate::remote::program_command(target, &target.linux_root, "sh", &["-c", script]).output() else {
        return 0;
    };
    let mut mask = 0u32;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        mask |= runtime_bit_for_stem(line.trim());
    }
    mask
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunVerdict {
    Runnable(&'static str),
    NotRunnable(&'static str),
}

fn runtime_spec_for_ext(ext: &str) -> Option<(u32, &'static str)> {
    Some(match ext {
        "py" | "pyw" => (RT_PYTHON, "Python"),
        "js" | "jsx" | "mjs" | "cjs" => (RT_NODE, "Node.js"),
        "ts" | "tsx" | "mts" | "cts" => (RT_NPX, "TypeScript"),
        "ps1" => (RT_POWERSHELL, "PowerShell"),
        "bat" | "cmd" => (RT_CMD, "Batch"),
        "sh" => (RT_BASH, "Bash"),
        "rs" => (RT_CARGO, "Cargo"),
        "go" => (RT_GO, "Go"),
        "java" => (RT_JAVA, "Java"),
        "dart" => (RT_DART, "Dart"),
        "zig" => (RT_ZIG, "Zig"),
        "swift" => (RT_SWIFT, "Swift"),
        "php" => (RT_PHP, "PHP"),
        "lua" => (RT_LUA, "Lua"),
        "rb" => (RT_RUBY, "Ruby"),
        "pl" => (RT_PERL, "Perl"),
        "exs" => (RT_ELIXIR, "Elixir"),
        _ => return None,
    })
}

fn find_ancestor_file(path: &Path, filename: &str, root: Option<&Path>, max_depth: usize) -> Option<PathBuf> {
    let mut dir = path.parent()?;
    for _ in 0..max_depth {
        let cand = dir.join(filename);
        if cand.is_file() {
            return Some(cand);
        }
        if root.is_some_and(|r| dir == r) {
            break;
        }
        dir = dir.parent()?;
    }
    None
}

fn is_bedrock_pack_script(path: &Path, head: &str, root: Option<&Path>) -> bool {
    for comp in path.components() {
        let s = comp.as_os_str().to_string_lossy();
        if s.eq_ignore_ascii_case("behavior_packs")
            || s.eq_ignore_ascii_case("development_behavior_packs")
            || s.eq_ignore_ascii_case("resource_packs")
            || s.eq_ignore_ascii_case("development_resource_packs")
        {
            return true;
        }
    }
    if head.contains("@minecraft/") || head.contains("mojang-minecraft") {
        return true;
    }
    if let Some(manifest) = find_ancestor_file(path, "manifest.json", root, 6) {
        if let Ok(raw) = std::fs::read_to_string(manifest) {
            if raw.contains("\"format_version\"")
                && raw.contains("\"modules\"")
                && (raw.contains("\"script\"") || raw.contains("\"data\"") || raw.contains("\"resources\""))
            {
                return true;
            }
        }
    }
    false
}

fn has_non_empty_code(source: &str, ext: &str) -> bool {
    let mut in_block = false;
    for raw in source.lines().take(512) {
        let mut s = raw.trim();
        if in_block {
            if let Some((_, after)) = s.split_once("*/") {
                in_block = false;
                s = after.trim();
            } else {
                continue;
            }
        }
        if s.is_empty() {
            continue;
        }
        match ext {
            "py" | "pyw" | "sh" | "rb" | "pl" | "exs" => {
                if s.starts_with('#') {
                    continue;
                }
            }
            "bat" | "cmd" => {
                let low = s.to_ascii_lowercase();
                if low.starts_with("rem ") || low == "rem" || low.starts_with("::") || low == "@echo off" {
                    continue;
                }
            }
            "ps1" => {
                if s.starts_with('#') {
                    continue;
                }
            }
            "lua" => {
                if s.starts_with("--") {
                    continue;
                }
            }
            _ => {
                if s.starts_with("//") {
                    continue;
                }
                if s.starts_with("/*") {
                    if let Some((_, after)) = s.split_once("*/") {
                        if after.trim().is_empty() {
                            continue;
                        }
                    } else {
                        in_block = true;
                        continue;
                    }
                }
            }
        }
        return true;
    }
    false
}

fn is_host_framework_import(head: &str) -> bool {
    const HOSTS: &[&str] = &[
        "'react'", "\"react\"", "'react-dom", "\"react-dom", "'vue'", "\"vue\"",
        "'svelte'", "\"svelte\"", "'next/", "\"next/", "'@angular/", "\"@angular/",
        "'electron'", "\"electron\"", "'vscode'", "\"vscode\"", "'obsidian'", "\"obsidian\"",
        "'three'", "\"three\"",
    ];
    HOSTS.iter().any(|p| head.contains(p))
}

fn starts_with_kw(stmt: &str, kw: &str) -> bool {
    stmt.strip_prefix(kw).is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace() || c == '{' || c == '*' || c == '('))
}

fn js_has_top_level_execution(source: &str) -> bool {
    let mut depth = 0i32;
    let mut in_block = false;
    let mut stmt = String::new();
    let mut saw_export = false;

    let check_stmt = |raw: &str, saw_export: &mut bool| -> bool {
        let s = raw.trim().trim_start_matches(';').trim();
        if s.is_empty() || s == "'use strict'" || s == "\"use strict\"" {
            return false;
        }
        if starts_with_kw(s, "export") {
            *saw_export = true;
            return false;
        }
        if s.starts_with("module.exports") || s.starts_with("exports.") {
            *saw_export = true;
            return false;
        }
        !(starts_with_kw(s, "import")
            || starts_with_kw(s, "function")
            || starts_with_kw(s, "async function")
            || starts_with_kw(s, "class")
            || starts_with_kw(s, "abstract")
            || starts_with_kw(s, "interface")
            || starts_with_kw(s, "type")
            || starts_with_kw(s, "enum")
            || starts_with_kw(s, "declare")
            || starts_with_kw(s, "namespace")
            || starts_with_kw(s, "const")
            || starts_with_kw(s, "let")
            || starts_with_kw(s, "var"))
    };

    for raw_line in source.lines().take(2048) {
        let mut line = raw_line;
        loop {
            if in_block {
                if let Some((_, after)) = line.split_once("*/") {
                    in_block = false;
                    line = after;
                } else {
                    line = "";
                    break;
                }
            }
            if let Some(pos) = line.find("/*") {
                let before = &line[..pos];
                let rest = &line[pos + 2..];
                if !before.is_empty() && depth == 0 {
                    stmt.push_str(before);
                    stmt.push(' ');
                }
                for ch in before.chars() {
                    match ch {
                        '{' | '(' | '[' => depth += 1,
                        '}' | ')' | ']' => depth = (depth - 1).max(0),
                        ';' if depth == 0 => {
                            if check_stmt(&stmt, &mut saw_export) {
                                return true;
                            }
                            stmt.clear();
                        }
                        _ => {}
                    }
                }
                if let Some((_, after)) = rest.split_once("*/") {
                    line = after;
                } else {
                    in_block = true;
                    line = "";
                    break;
                }
            } else {
                break;
            }
        }
        let code = line.split("//").next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        if depth == 0 && !stmt.is_empty() && !stmt.ends_with(' ') {
            stmt.push(' ');
        }
        for ch in code.chars() {
            if depth == 0 {
                stmt.push(ch);
            }
            match ch {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => {
                    depth = (depth - 1).max(0);
                    if depth == 0 && ch == '}' {
                        if check_stmt(&stmt, &mut saw_export) {
                            return true;
                        }
                        stmt.clear();
                    }
                }
                ';' if depth == 0 => {
                    if check_stmt(&stmt, &mut saw_export) {
                        return true;
                    }
                    stmt.clear();
                }
                _ => {}
            }
        }
        if depth == 0 && !stmt.trim().is_empty() && !stmt.trim_end().ends_with(['=', ',', '+', '-', '*', '/', ':', '?', '&', '|']) {
            // Handle ASI single-line calls like `main()` without trailing semicolon.
            let candidate = stmt.trim();
            if !starts_with_kw(candidate, "import") && !starts_with_kw(candidate, "export") {
                if check_stmt(candidate, &mut saw_export) {
                    return true;
                }
                stmt.clear();
            }
        }
    }
    if !stmt.trim().is_empty() && check_stmt(&stmt, &mut saw_export) {
        return true;
    }
    !saw_export && (source.contains("console.") || source.contains("process."))
}

pub(crate) fn check_script_runnable(
    path: &Path,
    source: &str,
    root: Option<&Path>,
    wsl: bool,
    runtimes: u32,
) -> RunVerdict {
    let Some(ext_raw) = path.extension().and_then(|e| e.to_str()) else {
        return RunVerdict::NotRunnable("Not a runnable script");
    };
    let ext = ext_raw.to_ascii_lowercase();
    let Some((bit, label)) = runtime_spec_for_ext(&ext) else {
        return RunVerdict::NotRunnable("Not a runnable script");
    };
    if (wsl || !cfg!(windows)) && matches!(ext.as_str(), "ps1" | "bat" | "cmd") {
        return RunVerdict::NotRunnable("Windows script not runnable here");
    }
    if (runtimes & bit) == 0 {
        return RunVerdict::NotRunnable("Runtime not installed");
    }
    let head = if source.len() > 16_384 {
        let mut end = 16_384;
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        &source[..end]
    } else {
        source
    };
    if !has_non_empty_code(head, &ext) {
        return RunVerdict::NotRunnable("File has no executable code");
    }
    let name_lower = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => {
            if matches!(ext.as_str(), "jsx" | "tsx") {
                return RunVerdict::NotRunnable("UI component module is not directly runnable");
            }
            if name_lower.ends_with(".d.ts")
                || name_lower.ends_with(".d.mts")
                || name_lower.ends_with(".d.cts")
                || name_lower.ends_with(".min.js")
                || name_lower.contains(".config.")
                || name_lower.contains(".test.")
                || name_lower.contains(".spec.")
            {
                return RunVerdict::NotRunnable("Module, config, or type declaration is not standalone");
            }
            if is_bedrock_pack_script(path, head, root) {
                return RunVerdict::NotRunnable("Minecraft Bedrock script requires the game runtime");
            }
            if is_host_framework_import(head) {
                return RunVerdict::NotRunnable("Browser or host-app module cannot run standalone");
            }
            if !js_has_top_level_execution(source) {
                return RunVerdict::NotRunnable("Library module has no top-level entrypoint");
            }
        }
        "py" | "pyw" => {
            if name_lower == "__init__.py" && !source.contains("__main__") {
                return RunVerdict::NotRunnable("Package initializer is not a standalone script");
            }
        }
        "rs" => {
            if !source.contains("fn main(") {
                return RunVerdict::NotRunnable("Rust file has no fn main()");
            }
            if find_ancestor_file(path, "Cargo.toml", root, 6).is_none() {
                return RunVerdict::NotRunnable("No Cargo.toml found for this Rust file");
            }
        }
        "go" => {
            if name_lower.ends_with("_test.go") {
                return RunVerdict::NotRunnable("Go test files require go test");
            }
            if !head.contains("package main") || !source.contains("func main(") {
                return RunVerdict::NotRunnable("Go file has no package main / func main()");
            }
        }
        "java" => {
            if !source.contains("void main(") {
                return RunVerdict::NotRunnable("Java file has no main method");
            }
        }
        "dart" => {
            if !source.contains("main(") {
                return RunVerdict::NotRunnable("Dart file has no main()");
            }
        }
        "zig" => {
            if !source.contains("pub fn main(") {
                return RunVerdict::NotRunnable("Zig file has no pub fn main()");
            }
        }
        "lua" if head.contains("game:GetService") || head.contains("RegisterNetEvent") || head.contains("minetest.") => {
            return RunVerdict::NotRunnable("Embedded engine Lua script cannot run standalone");
        }
        _ => {}
    }
    RunVerdict::Runnable(label)
}

#[cfg(test)]
fn can_run(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| runtime_spec_for_ext(&e.to_ascii_lowercase()).is_some())
}

fn run_command_for(path: &Path, root: Option<&Path>) -> Option<(String, Vec<String>)> {
    let p = path.to_string_lossy().into_owned();
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "py" | "pyw" => (python(root), vec!["-u".into(), p]),
        "js" | "jsx" | "mjs" | "cjs" => ("node".into(), vec![p]),
        "ts" | "tsx" | "mts" | "cts" => ("npx".into(), vec!["--yes".into(), "tsx".into(), p]),
        "ps1" => ("powershell".into(), vec!["-NoProfile".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-File".into(), p]),
        "bat" | "cmd" => ("cmd.exe".into(), vec!["/d".into(), "/c".into(), p]),
        "sh" => ("bash".into(), vec![p]),
        "rs" => {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
            let parent = path.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_ascii_lowercase());
            match (parent.as_deref(), stem) {
                (Some("bin"), Some(s)) => ("cargo".into(), vec!["run".into(), "--bin".into(), s]),
                (Some("examples"), Some(s)) => ("cargo".into(), vec!["run".into(), "--example".into(), s]),
                _ => ("cargo".into(), vec!["run".into()]),
            }
        }
        "go" => ("go".into(), vec!["run".into(), p]),
        "java" => ("java".into(), vec![p]),
        "dart" => ("dart".into(), vec!["run".into(), p]),
        "zig" => ("zig".into(), vec!["run".into(), p]),
        "swift" => ("swift".into(), vec![p]),
        "php" => ("php".into(), vec![p]),
        "lua" => ("lua".into(), vec![p]),
        "rb" => ("ruby".into(), vec![p]),
        "pl" => ("perl".into(), vec![p]),
        "exs" => ("elixir".into(), vec![p]),
        _ => return None,
    })
}

pub(crate) fn detect_python(root: Option<&Path>, target: Option<&crate::remote::WslTarget>) -> Option<String> {
    if let Some(t) = target {
        let out = crate::remote::program_command(t, &t.linux_root, "python3", &["--version"]).output().ok()?;
        let text = String::from_utf8_lossy(if out.stdout.is_empty() { &out.stderr } else { &out.stdout }).trim().to_string();
        return text.starts_with("Python").then_some(text);
    }
    let exe = python(root);
    let out = proc::command(&exe).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(if out.stdout.is_empty() { &out.stderr } else { &out.stdout }).trim().to_string();
    if !text.starts_with("Python") {
        return None;
    }
    let venv = Path::new(&exe).parent().and_then(Path::parent).and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
    Some(match venv {
        Some(v) if exe.contains(['\\', '/']) => format!("{text} · {v}"),
        _ => text,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        can_run, check_script_runnable, close_index, run_command_for, PanelSel, RunVerdict,
        RT_BASH, RT_CARGO, RT_CMD, RT_GO, RT_JAVA, RT_NODE, RT_NPX, RT_POWERSHELL, RT_PYTHON,
    };
    use std::path::Path;

    #[test]
    fn close_index_lands_on_a_tab_that_still_exists() {
        assert_eq!(close_index(0, 1, 2), 0, "closing a tab to the right keeps the active one");
        assert_eq!(close_index(2, 0, 2), 1, "closing a tab to the left shifts the active one down");
        assert_eq!(close_index(1, 0, 2), 0);
        assert_eq!(close_index(0, 0, 1), 0, "the last tab stays");
        assert_eq!(close_index(1, 1, 1), 0, "closing the active last tab steps back");
    }

    #[test]
    fn a_selection_covers_its_rows_in_either_direction() {
        let s = PanelSel { terminal: true, anchor: (3, 10), head: (1, 4) };
        assert_eq!(s.on_row(0, 20), None);
        assert_eq!(s.on_row(1, 20), Some((4, 20)));
        assert_eq!(s.on_row(2, 0), Some((0, 0)), "an empty row in between is selected");
        assert_eq!(s.on_row(3, 20), Some((0, 10)));
        assert_eq!(s.on_row(4, 20), None);
        assert_eq!(PanelSel { terminal: true, anchor: (0, 2), head: (0, usize::MAX) }.on_row(0, 5), Some((2, 5)));
        assert!(PanelSel { terminal: true, anchor: (2, 3), head: (2, 3) }.on_row(2, 9).is_none());
    }

    #[test]
    fn can_run_matches_run_command_for() {
        for name in [
            "a.py", "A.PY", "b.pyw", "c.js", "c2.jsx", "d.mjs", "e.cjs", "f.ts", "f2.tsx", "g.mts", "g2.cts",
            "h.ps1", "i.bat", "j.cmd", "k.sh", "l.rs", "m.go", "m2.java", "m3.dart", "m4.zig", "m5.swift",
            "n.php", "o.lua", "p.rb", "p2.pl", "p3.exs", "warps.yml", "x.txt", "y.json", "noext", "z.html",
        ] {
            let p = Path::new(name);
            assert_eq!(can_run(p), run_command_for(p, None).is_some(), "{name}");
        }
    }

    #[test]
    fn check_script_runnable_rejects_bedrock_libraries_and_missing_runtimes() {
        let all = RT_PYTHON | RT_NODE | RT_NPX | RT_POWERSHELL | RT_CMD | RT_BASH | RT_CARGO | RT_GO | RT_JAVA;
        let bedrock = Path::new("behavior_packs/KiwEssentialsVIP_BP/scripts/menu_member/functions/shop/index.js");
        assert!(matches!(
            check_script_runnable(bedrock, "import { world } from '@minecraft/server';\nworld.afterEvents.worldInitialize.subscribe(() => {});", None, false, all),
            RunVerdict::NotRunnable(_)
        ));
        assert!(matches!(
            check_script_runnable(Path::new("src/shop/index.js"), "import { world } from '@minecraft/server';\nconsole.log(world);", None, false, all),
            RunVerdict::NotRunnable(_)
        ));
        let pure_lib = "export function add(a, b) {\n  return a + b;\n}\nexport const VERSION = '1.0';";
        assert!(matches!(check_script_runnable(Path::new("src/math.js"), pure_lib, None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("types.d.ts"), "export type Id = string;", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("App.tsx"), "export function App() { return null; }", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("vite.config.ts"), "export default { base: '/' };", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("math.test.js"), "test('ok', () => {});", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("Component.js"), "import React from 'react';\nconsole.log(React);", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("pkg/__init__.py"), "VERSION = '1.0'", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("empty.py"), "# just a comment\n   \n", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("README.md"), "# Hello", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("data.json"), "{\"ok\":true}", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("main_test.go"), "package main\nfunc TestX(t *testing.T) {}", None, false, all), RunVerdict::NotRunnable(_)));
        assert!(matches!(check_script_runnable(Path::new("Helper.java"), "class Helper { int x() { return 1; } }", None, false, all), RunVerdict::NotRunnable(_)));

        // Standalone runnable scripts pass when runtime is present and fail when missing.
        assert_eq!(check_script_runnable(Path::new("hello.py"), "print('hi')", None, false, all), RunVerdict::Runnable("Python"));
        assert!(matches!(check_script_runnable(Path::new("hello.py"), "print('hi')", None, false, 0), RunVerdict::NotRunnable(_)));
        assert_eq!(
            check_script_runnable(Path::new("index.js"), "import { x } from './x.js';\nconsole.log(x);", None, false, all),
            RunVerdict::Runnable("Node.js")
        );
        assert_eq!(check_script_runnable(Path::new("cli.ts"), "async function main() { console.log(1); }\nmain();", None, false, all), RunVerdict::Runnable("TypeScript"));
        assert_eq!(check_script_runnable(Path::new("main.go"), "package main\nimport \"fmt\"\nfunc main() { fmt.Println(1) }", None, false, all), RunVerdict::Runnable("Go"));
        assert_eq!(check_script_runnable(Path::new("Main.java"), "class Main { public static void main(String[] a) {} }", None, false, all), RunVerdict::Runnable("Java"));
        assert_eq!(check_script_runnable(Path::new("deploy.sh"), "echo ok", None, true, all), RunVerdict::Runnable("Bash"));
        assert!(matches!(check_script_runnable(Path::new("build.ps1"), "Write-Host 'ok'", None, true, all), RunVerdict::NotRunnable(_)));

        // Temp workspace tests: Bedrock manifest.json vs Cargo.toml main.rs.
        let tmp = std::env::temp_dir().join(format!("fluxcode_panel_test_{}", std::process::id()));
        let pack_scripts = tmp.join("my_pack").join("scripts");
        let _ = std::fs::create_dir_all(&pack_scripts);
        let _ = std::fs::write(tmp.join("my_pack").join("manifest.json"), "{\"format_version\":2,\"modules\":[{\"type\":\"script\"}]}");
        let pack_js = pack_scripts.join("main.js");
        assert!(matches!(check_script_runnable(&pack_js, "console.log('tick');", Some(&tmp), false, all), RunVerdict::NotRunnable(_)));

        let crate_src = tmp.join("rust_app").join("src");
        let _ = std::fs::create_dir_all(&crate_src);
        let _ = std::fs::write(tmp.join("rust_app").join("Cargo.toml"), "[package]\nname=\"demo\"\nversion=\"0.1.0\"");
        assert_eq!(check_script_runnable(&crate_src.join("main.rs"), "fn main() { println!(\"hi\"); }", Some(&tmp), false, all), RunVerdict::Runnable("Cargo"));
        assert!(matches!(check_script_runnable(&crate_src.join("lib.rs"), "pub fn helper() -> i32 { 1 }", Some(&tmp), false, all), RunVerdict::NotRunnable(_)));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
