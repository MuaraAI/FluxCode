//! Bottom panel: Terminal (one command at a time, like the Tauri app's Flux
//! Terminal) and Output (Run File).
//!
//! Every command gets a block: the prompt line, its output (colored by what each
//! line means, see `term::classify`), and a status line with exit code and time.
//! A Python error ends the block with a card: what went wrong, at which file and
//! line (click to jump there; the line is also underlined in the editor), a hint
//! in plain words and, when there is an obvious fix, a button that runs it.

use crate::i18n::tr;
use crate::proc::{self, python};
use crate::term::{self, LineKind, PyError};
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    h_flex,
    highlighter::{Diagnostic, DiagnosticSeverity},
    input::{Input, Position},
    v_flex, Icon, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// One running (or last run) command of a panel, for its status line and error card.
pub(crate) struct RunInfo {
    pub started: Instant,
    /// Index in the panel's lines where this command's output starts.
    pub block: usize,
    /// The Python that ran it (for the `-m pip install` fix).
    pub python: String,
    pub cwd: PathBuf,
    /// The command line that ran (to build a corrected one for the fix button).
    pub command: String,
}

/// One visual row of a panel: a whole line, or one wrapped piece of it.
pub(crate) struct Row {
    pub line: usize,
    pub text: SharedString,
    pub first: bool,
}

/// (lines fingerprint, rows) for one panel; see `wrapped_rows`.
pub(crate) type WrapCache = std::cell::RefCell<((usize, usize, usize, usize, usize), std::rc::Rc<Vec<Row>>)>;

/// A Python error to show in the editor (wavy underline + hover message).
#[derive(Clone)]
pub(crate) struct Problem {
    pub from_terminal: bool,
    pub path: PathBuf,
    pub line: u32,
    pub caret: Option<(usize, usize)>,
    pub message: String,
}

fn same_path(a: &Path, b: &Path) -> bool {
    let n = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_start_matches(r"\\?\").to_lowercase();
    if cfg!(windows) { n(a) == n(b) } else { a == b }
}

impl Workbench {
    pub(crate) fn on_toggle_panel(&mut self, _: &TogglePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.panel_open = !self.panel_open;
        cx.notify();
    }

    pub(crate) fn on_toggle_terminal(&mut self, _: &ToggleTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel_open && self.panel_tab == PanelTab::Terminal {
            self.panel_open = false;
        } else {
            self.panel_open = true;
            self.panel_tab = PanelTab::Terminal;
            let h = self.term_input.focus_handle(cx);
            h.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_show_output(&mut self, _: &ShowOutput, _: &mut Window, cx: &mut Context<Self>) {
        self.panel_open = true;
        self.panel_tab = PanelTab::Output;
        cx.notify();
    }

    fn term_running(&self) -> bool {
        self.term_proc.is_some_and(|id| self.procs.is_running(id))
    }

    fn set_term_input(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let chars = text.chars().count() as u32;
        self.term_input.update(cx, |s, cx| {
            s.set_value(text.to_string(), window, cx);
            s.set_cursor_position(Position::new(0, chars), window, cx);
        });
    }

    /// Enter in the terminal prompt: run a command, or feed stdin of the running one.
    pub(crate) fn terminal_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = self.term_input.read(cx).value().to_string();
        self.term_input.update(cx, |s, cx| s.set_value("", window, cx));
        self.term_hist_ix = None;
        if let Some(id) = self.term_proc.filter(|id| self.procs.is_running(*id)) {
            // Echo into the program's own prompt line ("Name: " + what was typed).
            push_text(&mut self.term_lines, &format!("{line}\n"), false);
            self.procs.write_stdin(id, &format!("{line}\n"));
            self.term_scroll.scroll_to_bottom();
            cx.notify();
            return;
        }
        let typed = line.trim().to_string();
        if typed.is_empty() {
            return;
        }
        if self.term_history.last() != Some(&typed) {
            self.term_history.push(typed.clone());
        }
        self.run_in_terminal(&typed, cx);
    }

    /// Run one command line in the terminal panel (typed, or a fix from an error card).
    pub(crate) fn run_in_terminal(&mut self, typed: &str, cx: &mut Context<Self>) {
        if !self.term_lines.is_empty() {
            push_line(&mut self.term_lines, "", LineKind::Blank);
        }
        let mut prompt = Line::new(typed, LineKind::Cmd);
        prompt.meta = term::short_cwd(&self.term_cwd, self.root.as_deref()).into();
        close_line(&mut self.term_lines);
        self.term_lines.push(prompt);

        let cmdline = match term::autocorrect(typed, &self.term_cwd) {
            Some(fix) => {
                let what = fix.fixes.iter().map(|(a, b)| format!("{a} → {b}")).collect::<Vec<_>>().join(",  ");
                let mut l = Line::new(&what, LineKind::Fixed);
                l.meta = fix.line.clone().into();
                self.term_lines.push(l);
                fix.line
            }
            None => typed.to_string(),
        };

        let lower = cmdline.to_ascii_lowercase();
        if lower == "cls" || lower == "clear" {
            self.term_lines.clear();
        } else if lower == "pwd" {
            push_line(&mut self.term_lines, &self.term_cwd.display().to_string(), LineKind::Normal);
        } else if lower == "cd" || lower.starts_with("cd ") || lower.starts_with("cd\\") || lower.starts_with("cd..") || lower.starts_with("chdir") {
            let arg = cmdline.trim_start_matches(|c: char| c.is_ascii_alphabetic()).trim();
            let arg = arg.strip_prefix("/d").or_else(|| arg.strip_prefix("/D")).unwrap_or(arg).trim().trim_matches('"');
            if arg.is_empty() {
                push_line(&mut self.term_lines, &self.term_cwd.display().to_string(), LineKind::Normal);
            } else {
                self.change_dir(arg);
            }
        } else if cmdline.len() == 2 && cmdline.ends_with(':') {
            self.change_dir(&format!("{cmdline}\\"));
        } else {
            let first = cmdline.split_whitespace().next().unwrap_or("");
            let py = if term::is_python(first) { first.trim_matches('"').to_string() } else { python(self.root.as_deref()) };
            self.problems.retain(|p| !p.from_terminal);
            self.apply_problems(cx);
            let mut cmd = proc::shell_command(&cmdline);
            crate::github::apply_env(&mut cmd);
            match self.procs.spawn(cmd, Some(&self.term_cwd.clone())) {
                Ok(id) => {
                    self.term_proc = Some(id);
                    self.term_run = Some(RunInfo {
                        started: Instant::now(),
                        block: self.term_lines.len(),
                        python: py,
                        cwd: self.term_cwd.clone(),
                        command: cmdline.clone(),
                    });
                }
                Err(e) => push_line(&mut self.term_lines, &e, LineKind::Err),
            }
        }
        self.term_scroll.scroll_to_bottom();
        cx.notify();
    }

    fn change_dir(&mut self, arg: &str) {
        let target = if arg == "~" {
            std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default()
        } else {
            self.term_cwd.join(arg)
        };
        match target.canonicalize() {
            Ok(p) if p.is_dir() => {
                self.term_cwd = PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\"));
            }
            _ => push_line(&mut self.term_lines, &crate::trf!("No folder named '{arg}' here."; arg => arg), LineKind::Err),
        }
    }

    // ------------------------------------------------------------ prompt keys

    pub(crate) fn on_term_history_prev(&mut self, _: &TermHistoryPrev, window: &mut Window, cx: &mut Context<Self>) {
        if self.term_history.is_empty() || self.term_running() {
            return;
        }
        let ix = match self.term_hist_ix {
            None => self.term_history.len() - 1,
            Some(i) => i.saturating_sub(1),
        };
        self.term_hist_ix = Some(ix);
        let text = self.term_history[ix].clone();
        self.set_term_input(&text, window, cx);
    }

    pub(crate) fn on_term_history_next(&mut self, _: &TermHistoryNext, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.term_hist_ix else { return };
        if i + 1 < self.term_history.len() {
            self.term_hist_ix = Some(i + 1);
            let text = self.term_history[i + 1].clone();
            self.set_term_input(&text, window, cx);
        } else {
            self.term_hist_ix = None;
            self.set_term_input("", window, cx);
        }
    }

    pub(crate) fn on_term_complete(&mut self, _: &TermComplete, window: &mut Window, cx: &mut Context<Self>) {
        if self.term_running() {
            return;
        }
        let value = self.term_input.read(cx).value().to_string();
        let (new, choices) = term::complete(&value, &self.term_cwd);
        if let Some(new) = new {
            self.set_term_input(&new, window, cx);
        }
        if choices.len() > 1 {
            let shown: Vec<String> = choices.iter().take(40).cloned().collect();
            let more = choices.len().saturating_sub(40);
            let text = if more > 0 { crate::trf!("{0}   … {more} more"; 0 => shown.join("   "), more => more) } else { shown.join("   ") };
            push_line(&mut self.term_lines, &text, LineKind::Dim);
            self.term_scroll.scroll_to_bottom();
            cx.notify();
        }
    }

    /// Ctrl+C in the prompt: copy when text is selected, else stop the running command.
    pub(crate) fn on_term_interrupt(&mut self, _: &TermInterrupt, window: &mut Window, cx: &mut Context<Self>) {
        if !self.term_input.read(cx).selected_range().is_empty() {
            window.dispatch_action(Box::new(gpui_kit::component::input::Copy), cx);
            return;
        }
        if let Some(id) = self.term_proc.filter(|id| self.procs.is_running(*id)) {
            push_text(&mut self.term_lines, "^C\n", false);
            self.procs.kill(id);
        } else {
            self.set_term_input("", window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_term_escape(&mut self, _: &TermEscape, window: &mut Window, cx: &mut Context<Self>) {
        self.term_hist_ix = None;
        self.set_term_input("", window, cx);
    }

    // ------------------------------------------------------------ run file

    /// F5: run the active file and stream its output to the Output panel.
    pub(crate) fn on_run_file(&mut self, _: &RunFile, _: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else {
            self.status = tr("Open a file to run it").into();
            cx.notify();
            return;
        };
        if self.docs[ix].dirty {
            // Run what is on screen, like VS Code's "save before run".
            let doc = &mut self.docs[ix];
            let text = doc.editor.read(cx).value();
            if crate::core::write_text(&doc.path, &text, doc.crlf).is_ok() {
                doc.dirty = false;
            }
        }
        let path = self.docs[ix].path.clone();
        let Some((program, args)) = run_command_for(&path, self.root.as_deref()) else {
            self.status = crate::trf!("Don't know how to run {0}"; 0 => self.docs[ix].name).into();
            cx.notify();
            return;
        };
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
        head.meta = term::short_cwd(&cwd, self.root.as_deref()).into();
        self.out_lines.push(head);
        let mut cmd = proc::command(&program);
        cmd.args(&args);
        match self.procs.spawn(cmd, Some(&cwd)) {
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
        for id in [self.out_proc, self.term_proc].into_iter().flatten() {
            self.procs.kill(id);
        }
        cx.notify();
    }

    /// A terminal command or a Run File process ended: status line, then the
    /// error card if Python reported an error.
    pub(crate) fn finish_run(&mut self, terminal: bool, code: Option<i32>, cx: &mut Context<Self>) {
        let (lines, run) = if terminal { (&mut self.term_lines, self.term_run.take()) } else { (&mut self.out_lines, self.out_run.take()) };
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
        match (&error, &run) {
            (Some(e), Some(run)) => push_card(lines, e, &run.cwd),
            // A little air between a long output and its outcome.
            _ if output >= 2 && lines.last().is_some_and(|l| !l.text.trim().is_empty()) => push_line(lines, "", LineKind::Blank),
            _ => {}
        }
        let took = run.as_ref().map(|r| term::elapsed(r.started.elapsed())).unwrap_or_default();
        let (text, ok) = match code {
            Some(0) => (tr("Finished").to_string(), true),
            Some(c) => (crate::trf!("Failed (exit code {c})"; c => c), false),
            None => (tr("Stopped").to_string(), false),
        };
        let mut status = Line::new(&text, LineKind::Status);
        status.err = !ok;
        status.meta = took.into();
        lines.push(status);
        if let Some(r) = reason {
            // "C:\long\path\Main.java:12: error: ..." reads as "Main.java:12: error: ...";
            // the link keeps the full path.
            let link = term::parse_path_line(&r);
            let shown = match &link {
                Some((p, _)) => match (r.find(p.as_str()), Path::new(p).file_name()) {
                    (Some(at), Some(name)) => format!("{}{}", name.to_string_lossy(), &r[at + p.len()..]),
                    _ => r.clone(),
                },
                None => r.clone(),
            };
            let mut l = Line::new(&shown, LineKind::Result);
            l.err = true;
            l.link = link.map(|(p, n)| (p.into(), n));
            lines.push(l);
        }

        if let Some(e) = error {
            if let (Some(path), Some(line)) = (e.file.clone(), e.line) {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.status = format!("{}: {} ({name}:{line})", e.kind, e.message).into();
                self.problems.retain(|p| p.from_terminal != terminal);
                self.problems.push(Problem {
                    from_terminal: terminal,
                    path,
                    line,
                    caret: e.caret,
                    message: format!("{}: {}", e.kind, e.message),
                });
                self.apply_problems(cx);
            }
        }
        if terminal {
            self.term_scroll.scroll_to_bottom();
        } else {
            self.out_scroll.scroll_to_bottom();
            self.ext_after_run(code, cx);
        }
    }

    /// Show the current problems as underlines in the open editors.
    pub(crate) fn apply_problems(&mut self, cx: &mut Context<Self>) {
        for d in &self.docs {
            let mine: Vec<Problem> = self.problems.iter().filter(|p| same_path(&p.path, &d.path)).cloned().collect();
            d.editor.update(cx, |ed, cx| {
                let text = ed.value();
                let Some(set) = ed.diagnostics_mut() else { return };
                if set.is_empty() && mine.is_empty() {
                    return;
                }
                set.clear();
                for p in &mine {
                    let row = p.line.saturating_sub(1);
                    let src = text.split('\n').nth(row as usize).unwrap_or("").trim_end_matches('\r');
                    let indent = (src.len() - src.trim_start().len()) as u32;
                    let len = src.trim_end().chars().count() as u32;
                    let (a, b) = match p.caret {
                        Some((s, e)) if (indent + e as u32) <= len.max(1) && e > s => (indent + s as u32, indent + e as u32),
                        _ => (indent, len.max(indent + 1)),
                    };
                    set.push(
                        Diagnostic::new(Position::new(row, a)..Position::new(row, b), p.message.clone())
                            .with_severity(DiagnosticSeverity::Error)
                            .with_source("python"),
                    );
                }
                cx.notify();
            });
        }
    }

    /// Open `path` at `line` (from a traceback link or the status bar).
    pub(crate) fn open_location(&mut self, path: PathBuf, line: u32, col: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !path.is_file() {
            self.status = crate::trf!("{0} doesn't exist"; 0 => path.display()).into();
            cx.notify();
            return;
        }
        self.open_file(path, window, cx);
        if let Some(ix) = self.active {
            self.docs[ix].editor.update(cx, |ed, cx| {
                ed.set_cursor_position(Position::new(line.saturating_sub(1), col), window, cx);
            });
        }
    }

    pub(crate) fn on_go_to_problem(&mut self, _: &GoToProblem, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.problems.first().cloned() {
            let col = p.caret.map_or(0, |c| c.0 as u32);
            self.open_location(p.path, p.line, col, window, cx);
        }
    }

    /// Click on a traceback frame or a card's file:line.
    fn open_link(&mut self, path: &str, line: u32, terminal: bool, window: &mut Window, cx: &mut Context<Self>) {
        let p = PathBuf::from(path);
        let base = if terminal {
            self.term_run.as_ref().map(|r| r.cwd.clone()).unwrap_or_else(|| self.term_cwd.clone())
        } else {
            self.out_run.as_ref().map(|r| r.cwd.clone()).or_else(|| self.ext.last_cwd.clone()).or_else(|| self.root.clone()).unwrap_or_default()
        };
        let full = if p.is_absolute() { p } else { base.join(p) };
        self.open_location(full, line, 0, window, cx);
    }

    /// The fix button on an error card: run it in the terminal.
    fn run_fix(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_open = true;
        self.panel_tab = PanelTab::Terminal;
        if self.term_running() {
            self.set_term_input(command, window, cx);
            self.status = tr("The terminal is busy: the fix is in the prompt, press Enter when it's free").into();
            cx.notify();
            return;
        }
        if self.term_history.last().map(String::as_str) != Some(command) {
            self.term_history.push(command.to_string());
        }
        self.run_in_terminal(command, cx);
    }

    // ------------------------------------------------------------ render

    /// One visual row. `ix` is the line's index, `shown` the row's text (a wrapped piece
    /// of the line, already made readable by `term::display_text`); `first` is false
    /// on continuation rows.
    #[allow(clippy::too_many_arguments)]
    fn render_line(&self, i: usize, ix: usize, l: &Line, shown: &SharedString, first: bool, terminal: bool, m: Metrics, cx: &mut Context<Self>) -> AnyElement {
        let base = h_flex().id(("pl", i)).h(m.row).w_full().px_3().gap_2().whitespace_nowrap().overflow_hidden();
        // Program output keeps the mono font; what FluxCode itself says uses the UI font.
        let text = |s: &SharedString, col: theme::Tok| div().text_color(c(col)).child(s.clone());
        let ui = |s: &SharedString, col: theme::Tok| div().font_family(UI_FONT).text_color(c(col)).child(s.clone());
        // Wrapped rows hang two characters in, so a continued line reads as one.
        let hang = theme::u(12.) + px(m.em * 2.);
        // One callout across several fixed-height rows: a tinted full-width band with a red edge.
        let card = |row: Stateful<Div>, inner: Div| {
            row.child(inner.flex_1().min_w_0().h_full().items_center().px_3().gap_2().bg(c(theme::BG_ELEVATED)).border_l_2().border_color(c(theme::DANGER)))
                .into_any_element()
        };
        let link = |label: SharedString, path: SharedString, line: u32, cx: &mut Context<Self>| {
            div()
                .id(("lk", i))
                .text_color(c(theme::BLUE))
                .cursor_pointer()
                .hover(|s| s.underline())
                .child(label)
                .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, terminal, w, cx)))
        };
        match l.kind {
            LineKind::Blank => base.into_any_element(),
            // The block header: what ran, where, and how it ended.
            LineKind::Cmd => {
                let lines = if terminal { &self.term_lines } else { &self.out_lines };
                let (status, last) = block_status(lines, ix);
                let live = if terminal { self.term_run.is_some() } else { self.out_run.is_some() };
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
                        // The command keeps its room; the place gives way first.
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
            // The outcome: a clear mark, the word, the time.
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
                base.font_family(UI_FONT)
                    .child(Icon::new(icon).size(theme::u(14.)).text_color(c(col)))
                    .child(div().font_weight(FontWeight::MEDIUM).text_color(c(if l.err && !stopped { theme::DANGER } else { theme::FG_STRONG })).child(l.text.clone()))
                    .when(!took.is_empty(), |h| h.child(text(&took, theme::FG_MUTED)))
                    .into_any_element()
            }
            // Under the status: why it failed (a link when it names a file:line)...
            LineKind::Result if l.err => {
                let row = base.pl(theme::u(34.)).font_family(UI_FONT);
                match l.link.clone() {
                    Some((path, line)) => row.child(
                        div()
                            .id(("lk", i))
                            .min_w_0()
                            .text_color(c(theme::FG_STRONG))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::BLUE)).underline())
                            .child(shown.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, terminal, w, cx))),
                    ),
                    None => row.child(text(shown, theme::FG_STRONG).min_w_0()),
                }
                .into_any_element()
            }
            // ...or what was built: "KiwEssentials 0.1.0 · 609 KB · 2.9s · build\libs\x.jar".
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
            // Traceback frames and `file.py:12:` warnings: the whole line opens the file there.
            LineKind::Frame | LineKind::Warn | LineKind::Err if l.link.is_some() => {
                let (path, line) = l.link.clone().unwrap_or_default();
                let col = match l.kind {
                    LineKind::Warn => theme::WARN,
                    LineKind::Err => theme::DANGER,
                    _ => theme::FG_MUTED,
                };
                base.when(!first, |b| b.pl(hang))
                    .child(
                        div()
                            .id(("lk", i))
                            .text_color(c(col))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(theme::BLUE)).underline())
                            .child(shown.clone())
                            .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, terminal, w, cx))),
                    )
                    .into_any_element()
            }
            LineKind::CardHead => {
                let mut inner = h_flex()
                    .font_family(UI_FONT)
                    .child(Icon::new(IconName::CircleX).size(theme::u(14.)).text_color(c(theme::DANGER)))
                    .child(div().flex_none().text_color(c(theme::DANGER)).font_weight(FontWeight::SEMIBOLD).child(l.text.clone()));
                if let Some((path, line)) = &l.link {
                    inner = inner.child(text(&"in".into(), theme::FG_DIM)).child(link(l.meta.clone(), path.clone(), *line, cx).min_w_0().truncate());
                }
                card(base, inner)
            }
            LineKind::CardMsg => card(base, h_flex().child(ui(shown, theme::FG_STRONG).pl(theme::u(22.)))),
            LineKind::CardCode => card(
                base,
                h_flex()
                    .child(div().flex_none().pl(theme::u(22.)).text_color(c(theme::FG_DIM)).child(l.meta.clone()))
                    .child(div().flex_none().text_color(c(theme::BORDER_STRONG)).child("│"))
                    .child(text(&l.text, theme::FG).min_w_0().truncate()),
            ),
            // Wrapped hint rows: the icon only on the first one.
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
                    LineKind::Hunk => theme::BLUE,
                    LineKind::Dim => theme::FG_DIM,
                    LineKind::Success => theme::GREEN,
                    LineKind::Code => theme::FG_STRONG,
                    LineKind::Frame => theme::FG_MUTED,
                    _ => theme::FG,
                };
                base.when(!first, |b| b.pl(hang)).child(div().text_color(c(col)).child(shown.clone())).into_any_element()
            }
        }
    }

    /// The panel's lines as visual rows (wrapped to `cols`), cached per panel.
    fn wrapped_rows(&self, terminal: bool, cols: Cols) -> std::rc::Rc<Vec<Row>> {
        let (lines, cache) = if terminal { (&self.term_lines, &self.term_wrap) } else { (&self.out_lines, &self.out_wrap) };
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
            // A code line with a ^^^ line under it stays one row, so the marks line up.
            let marked = l.kind == LineKind::Code && lines.get(ix + 1).is_some_and(|n| n.kind == LineKind::Caret);
            // Columns left after the row's own indent (card edge, status icon), and how
            // many fewer the continuation rows get (log text hangs two characters in).
            let (width, hang) = match l.kind {
                LineKind::Normal | LineKind::Err | LineKind::Warn | LineKind::Dim | LineKind::Success | LineKind::Frame | LineKind::Hunk => (cols.mono, 2),
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

    /// Whether the newest command in these lines ran Python (or pip), so the
    /// header's Python version is about what is on screen.
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

    pub(crate) fn render_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, which: PanelTab, this: &Self, cx: &mut Context<Self>| {
            let on = this.panel_tab == which;
            // A 1px underline on the open tab, sentence case like the rest of the app.
            div()
                .id(id)
                .px_1()
                .h_full()
                .flex()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .border_b_1()
                .border_color(if on { c(theme::ACCENT) } else { transparent_black() })
                .text_color(if on { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                .when(!on, |d| d.hover(|s| s.text_color(c(theme::FG))))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.panel_tab = which;
                    cx.notify();
                }))
        };
        let terminal = self.panel_tab == PanelTab::Terminal;
        let running = if terminal { self.term_proc.is_some() } else { self.out_proc.is_some() };
        let shown_lines = if terminal { &self.term_lines } else { &self.out_lines };
        let python = self.python_version.clone().filter(|_| Self::ran_python(shown_lines));
        let header = h_flex()
            .h(theme::u(35.))
            .px_3()
            .gap_4()
            .flex_none()
            .child(tab("p-term", tr("Terminal"), PanelTab::Terminal, self, cx))
            .child(tab("p-out", tr("Output"), PanelTab::Output, self, cx))
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
            .child(tool_button("p-clear", IconName::Delete, "Clear", cx, |this, _, cx| {
                match this.panel_tab {
                    PanelTab::Terminal => this.term_lines.clear(),
                    PanelTab::Output => this.out_lines.clear(),
                }
                cx.notify();
            }))
            .child(tool_button("p-close", IconName::Close, "Close Panel", cx, |this, _, cx| {
                this.panel_open = false;
                cx.notify();
            }));

        let count = shown_lines.len();
        let scroll = if terminal { self.term_scroll.clone() } else { self.out_scroll.clone() };
        // Log text: the editor's font, a little smaller, normal weight, roomy rows.
        let font_size = px((self.settings.font_size - 2.).clamp(10., 28.));
        let m = {
            let ts = window.text_system();
            let id = ts.resolve_font(&font(self.settings.font_family.clone()));
            let em = ts.em_advance(id, font_size).map(f32::from).unwrap_or(f32::from(font_size) * 0.6).max(4.);
            Metrics { row: px((f32::from(font_size) * 1.6).round()), em }
        };
        // Long lines wrap to the panel's width instead of being cut off. The size comes
        // from the probe below (last frame); the rows are cached until lines or width change.
        let (width, height) = self.panel_size.get();
        let cols = if width <= 0. {
            Cols { mono: 160, card: 160, result: 160 }
        } else {
            // Interface-font rows (card text, failure reason) by the font's average letter,
            // with a little slack so a run of wide letters still fits.
            let avg = {
                let ts = window.text_system();
                let id = ts.resolve_font(&font(UI_FONT));
                let sample = "Python doesn't know this name, check the spelling: Main.java:12";
                let w: f32 = sample.chars().map(|ch| ts.advance(id, font_size, ch).map(|s| f32::from(s.width)).unwrap_or(m.em)).sum();
                (w / sample.chars().count() as f32).max(3.) * 1.08
            };
            let fit = |indent: f32, per: f32| (((width - indent - f32::from(theme::u(12.)) - 6.) / per).floor() as usize).max(16);
            Cols { mono: fit(f32::from(theme::u(12.)), m.em).max(24), card: fit(f32::from(theme::u(46.)) + 2., avg), result: fit(f32::from(theme::u(34.)), avg) }
        };
        let rows = self.wrapped_rows(terminal, cols);
        // The list is a whole number of rows tall, so the top row is never cut in half.
        let gap = 4.;
        let row = f32::from(m.row);
        let fit = if height > gap + row { ((height - gap) / row).floor() * row } else { 0. };
        let list = uniform_list(
            "panel-lines",
            rows.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|i| {
                        let r = &rows[i];
                        let lines = if terminal { &this.term_lines } else { &this.out_lines };
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
        .map(|l| if fit > 0. { l.h(px(fit)) } else { l.h_full() })
        .font_family(self.settings.font_family.clone())
        .font_weight(FontWeight::NORMAL)
        .text_size(font_size);
        let probe = {
            let cell = self.panel_size.clone();
            let id = cx.entity_id();
            canvas(
                move |bounds, _, cx| {
                    let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let old = cell.get();
                    if (old.0 - size.0).abs() > 0.5 || (old.1 - size.1).abs() > 0.5 {
                        cell.set(size);
                        cx.defer(move |cx| cx.notify(id));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
        };
        let list = div().relative().flex_1().min_h_0().pt(px(gap)).child(probe).child(list);

        let empty = count == 0;
        let hint = |line: &'static str| div().min_w_0().child(line);
        v_flex()
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .border_t_1()
            .border_color(c(theme::BORDER))
            .child(header)
            .when(empty, |v| {
                v.child(
                    v_flex()
                        .px_3()
                        .py_1()
                        .gap_1()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .when(terminal, |v| {
                            v.child(hint(tr("Run any command, for example  py app.py  or  pip install -r requirements.txt")))
                                .child(hint(tr("↑ ↓ history  ·  Tab completes names  ·  Ctrl+C stops  ·  typos like  pip i  are fixed for you")))
                        })
                        .when(!terminal, |v| v.child(hint(tr("Nothing has run yet. Press F5 to run the open file, or pick a task under Run.")))),
                )
            })
            .child(list)
            .when(terminal, |v| {
                let busy = self.term_running();
                let venv = self.root.as_deref().and_then(|r| {
                    [".venv", "venv", "env"].into_iter().find(|v| r.join(v).join(if cfg!(windows) { "Scripts" } else { "bin" }).is_dir())
                });
                v.child(
                    h_flex()
                        .px_3()
                        .h(theme::u(30.))
                        .flex_none()
                        .gap_2()
                        .border_t_1()
                        .border_color(c(theme::BORDER))
                        .font_family(self.settings.font_family.clone())
                        .text_size(font_size)
                        .when(busy, |h| {
                            h.child(div().flex_none().size(px(6.)).rounded_full().bg(c(theme::BLUE)))
                                .child(div().flex_none().font_family(UI_FONT).text_color(c(theme::BLUE)).child(tr("running · input")))
                        })
                        .when(!busy, |h| {
                            h.when_some(venv, |h, v| {
                                h.child(div().flex_none().px_1().rounded(px(3.)).bg(c(theme::BG_ELEVATED)).text_xs().text_color(c(theme::GREEN)).child(format!("({v})")))
                            })
                            .child(div().min_w_0().max_w(relative(0.4)).truncate().text_color(c(theme::FG_DIM)).child(term::short_cwd(&self.term_cwd, self.root.as_deref())))
                        })
                        .child(div().flex_none().text_color(c(theme::ACCENT_TEXT)).font_weight(FontWeight::BOLD).child("❯"))
                        .child(div().flex_1().min_w_0().key_context("Terminal").child(Input::new(&self.term_input).appearance(false).small())),
                )
            })
    }
}

/// How many characters fit on a row: mono log text, error-card text, the failure reason.
#[derive(Clone, Copy)]
struct Cols {
    mono: usize,
    card: usize,
    result: usize,
}

/// Row height and character width of the panel's log, for one frame.
#[derive(Clone, Copy)]
struct Metrics {
    row: Pixels,
    em: f32,
}

/// What GPUI calls the platform's interface font (Segoe UI on Windows).
const UI_FONT: &str = ".SystemUIFont";

/// The status line that ends the block starting at `cmd`, and whether that block
/// is the last one (still running when it has no status yet).
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

/// The error card's rows (fixed-height rows; the message and hint wrap to the panel).
fn push_card(lines: &mut Vec<Line>, e: &PyError, cwd: &Path) {
    push_line(lines, "", LineKind::Blank);
    let mut head = Line::new(&e.kind, LineKind::CardHead);
    if let (Some(file), Some(line)) = (&e.file, e.line) {
        let shown = file.strip_prefix(cwd).map(|p| p.display().to_string()).unwrap_or_else(|_| {
            file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        });
        head.meta = crate::trf!("{shown}, line {line}"; shown => shown, line => line).into();
        head.link = Some((file.display().to_string().into(), line));
    }
    lines.push(head);
    if !e.message.is_empty() {
        lines.push(Line::new(&e.message, LineKind::CardMsg));
    }
    if let (Some(code), Some(line)) = (&e.code, e.line) {
        let mut l = Line::new(code, LineKind::CardCode);
        l.meta = line.to_string().into();
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

/// How to run a file, by extension (same choices as the Tauri app's Run).
fn run_command_for(path: &Path, root: Option<&Path>) -> Option<(String, Vec<String>)> {
    let p = path.to_string_lossy().into_owned();
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "py" | "pyw" => (python(root), vec!["-u".into(), p]),
        "js" | "mjs" | "cjs" => ("node".into(), vec![p]),
        "ts" | "mts" => ("npx".into(), vec!["--yes".into(), "tsx".into(), p]),
        "ps1" => ("powershell".into(), vec!["-NoProfile".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-File".into(), p]),
        "bat" | "cmd" => ("cmd.exe".into(), vec!["/d".into(), "/c".into(), p]),
        "sh" => ("bash".into(), vec![p]),
        "rs" => ("cargo".into(), vec!["run".into()]),
        "go" => ("go".into(), vec!["run".into(), p]),
        "php" => ("php".into(), vec![p]),
        "lua" => ("lua".into(), vec![p]),
        "rb" => ("ruby".into(), vec![p]),
        _ => return None,
    })
}

/// "Python 3.15.0" (+ " · .venv"), for the panel header. Runs a process: call off the UI thread.
pub(crate) fn detect_python(root: Option<&Path>) -> Option<String> {
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
