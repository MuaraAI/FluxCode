//! Bottom panel: Terminal (one command at a time, like the Tauri app's Flux
//! Terminal) and Output (Run File).
//!
//! Every command gets a block: the prompt line, its output (colored by what each
//! line means, see `term::classify`), and a status line with exit code and time.
//! A Python error ends the block with a card: what went wrong, at which file and
//! line (click to jump there; the line is also underlined in the editor), a hint
//! in plain words and, when there is an obvious fix, a button that runs it.

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
            _ => push_line(&mut self.term_lines, &format!("No folder named '{arg}' here."), LineKind::Err),
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
            let text = if more > 0 { format!("{}   … {more} more", shown.join("   ")) } else { shown.join("   ") };
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
            self.status = "Open a file to run it".into();
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
            self.status = format!("Don't know how to run {}", self.docs[ix].name).into();
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
        let mut head = Line::new(&shown.join(" "), LineKind::Cmd);
        head.meta = "Run".into();
        self.out_lines.push(head);
        let mut cmd = proc::command(&program);
        cmd.args(&args);
        let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
        match self.procs.spawn(cmd, Some(&cwd)) {
            Ok(id) => {
                self.out_proc = Some(id);
                self.out_run = Some(RunInfo { started: Instant::now(), block: self.out_lines.len(), python: program.clone(), cwd, command: shown.join(" ") });
            }
            Err(e) => push_line(&mut self.out_lines, &format!("Couldn't start '{program}': {e}"), LineKind::Err),
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
        if let Some(run) = &run {
            let from = run.block.min(lines.len());
            if code != Some(0) || lines[from..].iter().any(|l| l.kind == LineKind::Err) {
                let view: Vec<(&str, LineKind)> = lines[from..].iter().map(|l| (l.text.as_ref(), l.kind)).collect();
                error = term::parse_problem(&view, &run.command, &run.python, &run.cwd);
            }
            if let Some(e) = &error {
                push_card(lines, e, &run.cwd);
            }
        }
        let took = run.as_ref().map(|r| term::elapsed(r.started.elapsed())).unwrap_or_default();
        let (text, ok) = match code {
            Some(0) => ("done".to_string(), true),
            Some(c) => (format!("exit code {c}"), false),
            None => ("stopped".to_string(), false),
        };
        let mut status = Line::new(&text, LineKind::Status);
        status.err = !ok;
        status.meta = took.into();
        lines.push(status);

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
            self.status = format!("{} doesn't exist", path.display()).into();
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
            self.out_run.as_ref().map(|r| r.cwd.clone()).or_else(|| self.root.clone()).unwrap_or_default()
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
            self.status = "The terminal is busy: the fix is in the prompt, press Enter when it's free".into();
            cx.notify();
            return;
        }
        if self.term_history.last().map(String::as_str) != Some(command) {
            self.term_history.push(command.to_string());
        }
        self.run_in_terminal(command, cx);
    }

    // ------------------------------------------------------------ render

    fn render_line(&self, i: usize, l: &Line, terminal: bool, cx: &mut Context<Self>) -> AnyElement {
        let base = h_flex().id(("pl", i)).h(theme::u(20.)).w_full().px_3().gap_2().whitespace_nowrap().overflow_hidden();
        let text = |s: &SharedString, col: theme::Tok| div().text_color(c(col)).child(s.clone());
        // One callout across several fixed-height rows: a tinted full-width band with a red edge.
        let card = |row: Stateful<Div>, inner: Div| {
            row.child(inner.flex_1().h_full().items_center().px_3().gap_2().bg(c(theme::BG_ELEVATED)).border_l_2().border_color(c(theme::DANGER)))
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
            LineKind::Cmd => base
                .child(text(&l.meta, theme::FG_DIM))
                .child(div().text_color(c(theme::ACCENT_TEXT)).font_weight(FontWeight::BOLD).child("❯"))
                .child(div().text_color(c(theme::FG_STRONG)).font_weight(FontWeight::MEDIUM).child(l.text.clone()))
                .into_any_element(),
            LineKind::Fixed => base
                .child(div().text_color(c(theme::BLUE)).child("↳ auto-corrected"))
                .child(text(&l.text, theme::FG_MUTED))
                .into_any_element(),
            LineKind::Status => {
                let (mark, col) = if l.err { ("✕", theme::DANGER) } else { ("✓", theme::GREEN) };
                base.child(div().text_color(c(col)).child(mark))
                    .child(text(&l.text, if l.err { theme::DANGER } else { theme::FG_DIM }))
                    .when(!l.meta.is_empty(), |h| h.child(text(&format!("· {}", l.meta).into(), theme::FG_DIM)))
                    .into_any_element()
            }
            // Traceback frames and `file.py:12:` warnings: the whole line opens the file there.
            LineKind::Frame | LineKind::Warn if l.link.is_some() => {
                let (path, line) = l.link.clone().unwrap_or_default();
                let col = if l.kind == LineKind::Warn { theme::WARN } else { theme::FG_MUTED };
                base.child(
                    div()
                        .id(("lk", i))
                        .text_color(c(col))
                        .cursor_pointer()
                        .hover(|s| s.text_color(c(theme::BLUE)).underline())
                        .child(l.text.clone())
                        .on_click(cx.listener(move |this, _, w, cx| this.open_link(&path, line, terminal, w, cx))),
                )
                .into_any_element()
            }
            LineKind::Frame => base.child(text(&l.text, theme::FG_MUTED)).into_any_element(),
            LineKind::CardHead => {
                let mut inner = h_flex()
                    .child(Icon::new(IconName::CircleX).size(theme::u(14.)).text_color(c(theme::DANGER)))
                    .child(div().text_color(c(theme::DANGER)).font_weight(FontWeight::BOLD).child(l.text.clone()));
                if let Some((path, line)) = &l.link {
                    inner = inner.child(text(&"in".into(), theme::FG_DIM)).child(link(l.meta.clone(), path.clone(), *line, cx));
                }
                card(base, inner)
            }
            LineKind::CardMsg => card(base, h_flex().child(text(&l.text, theme::FG_STRONG).pl(theme::u(22.)))),
            LineKind::CardCode => card(
                base,
                h_flex()
                    .child(div().pl(theme::u(22.)).text_color(c(theme::FG_DIM)).child(l.meta.clone()))
                    .child(div().text_color(c(theme::BORDER_STRONG)).child("│"))
                    .child(text(&l.text, theme::FG)),
            ),
            // `err` marks a wrapped continuation row: no second icon.
            LineKind::CardHint => card(
                base,
                h_flex()
                    .when(!l.err, |h| h.child(Icon::new(IconName::Info).size(theme::u(14.)).text_color(c(theme::WARN))))
                    .child(text(&l.text, theme::FG_MUTED).when(l.err, |d| d.pl(theme::u(22.)))),
            ),
            LineKind::CardAction => {
                let cmd = l.run.clone().unwrap_or_default();
                card(
                    base,
                    h_flex()
                    .child(
                        h_flex()
                            .id(("fix", i))
                            .ml(theme::u(22.))
                            .px_2()
                            .gap_1()
                            .rounded(px(3.))
                            .bg(c(theme::ACCENT))
                            .text_color(c(theme::ON_ACCENT))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.88))
                            .child(Icon::new(IconName::Play).size(theme::u(12.)))
                            .child("Fix")
                            .on_click(cx.listener(move |this, _, w, cx| this.run_fix(&cmd, w, cx))),
                    )
                    .child(text(&l.text, theme::FG)),
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
                    _ => theme::FG,
                };
                base.child(div().text_color(c(col)).child(l.text.clone())).into_any_element()
            }
        }
    }

    pub(crate) fn render_panel(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, which: PanelTab, this: &Self, cx: &mut Context<Self>| {
            let on = this.panel_tab == which;
            // VS Code's panel tabs: small caps label, a 1px underline on the open one.
            div()
                .id(id)
                .px_1()
                .h_full()
                .flex()
                .items_center()
                .text_xs()
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
        let running = match self.panel_tab {
            PanelTab::Terminal => self.term_proc.is_some(),
            PanelTab::Output => self.out_proc.is_some(),
        };
        let header = h_flex()
            .h(theme::u(35.))
            .px_3()
            .gap_4()
            .flex_none()
            .child(tab("p-term", "TERMINAL", PanelTab::Terminal, self, cx))
            .child(tab("p-out", "OUTPUT", PanelTab::Output, self, cx))
            .child(div().flex_1())
            .when_some(self.python_version.clone(), |h, v| {
                h.child(
                    h_flex()
                        .gap_1()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .child(div().size(px(6.)).rounded_full().bg(c(theme::GREEN)))
                        .child(v),
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

        let terminal = self.panel_tab == PanelTab::Terminal;
        let (count, scroll) = if terminal { (self.term_lines.len(), self.term_scroll.clone()) } else { (self.out_lines.len(), self.out_scroll.clone()) };
        let list = uniform_list(
            "panel-lines",
            count,
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|i| {
                        let lines = if terminal { &this.term_lines } else { &this.out_lines };
                        this.render_line(i, &lines[i], terminal, cx)
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&scroll)
        .flex_1()
        .py_1()
        .font_family(self.settings.font_family.clone())
        .text_size(theme::u(13.));

        let empty = count == 0 && terminal;
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
                        .child("Run any command, for example  py app.py  or  pip install -r requirements.txt")
                        .child("↑ ↓ history  ·  Tab completes names  ·  Ctrl+C stops  ·  typos like  pip i  are fixed for you"),
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
                        .text_size(theme::u(13.))
                        .when(busy, |h| {
                            h.child(div().size(px(6.)).rounded_full().bg(c(theme::BLUE)))
                                .child(div().text_color(c(theme::BLUE)).child("running · input"))
                        })
                        .when(!busy, |h| {
                            h.when_some(venv, |h, v| {
                                h.child(div().px_1().rounded(px(3.)).bg(c(theme::BG_ELEVATED)).text_xs().text_color(c(theme::GREEN)).child(format!("({v})")))
                            })
                            .child(div().text_color(c(theme::FG_DIM)).child(term::short_cwd(&self.term_cwd, self.root.as_deref())))
                        })
                        .child(div().text_color(c(theme::ACCENT_TEXT)).font_weight(FontWeight::BOLD).child("❯"))
                        .child(div().flex_1().key_context("Terminal").child(Input::new(&self.term_input).appearance(false).small())),
                )
            })
    }
}

/// The error card's rows (fixed-height rows, so the message wraps by hand).
fn push_card(lines: &mut Vec<Line>, e: &PyError, cwd: &Path) {
    push_line(lines, "", LineKind::Blank);
    let mut head = Line::new(&e.kind, LineKind::CardHead);
    if let (Some(file), Some(line)) = (&e.file, e.line) {
        let shown = file.strip_prefix(cwd).map(|p| p.display().to_string()).unwrap_or_else(|_| {
            file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        });
        head.meta = format!("{shown}, line {line}").into();
        head.link = Some((file.display().to_string().into(), line));
    }
    lines.push(head);
    if !e.message.is_empty() {
        for row in term::wrap(&e.message, 110) {
            lines.push(Line::new(&row, LineKind::CardMsg));
        }
    }
    if let (Some(code), Some(line)) = (&e.code, e.line) {
        let mut l = Line::new(code, LineKind::CardCode);
        l.meta = line.to_string().into();
        lines.push(l);
    }
    if let Some(h) = &e.hint {
        for (n, row) in term::wrap(h, 110).into_iter().enumerate() {
            let mut l = Line::new(&row, LineKind::CardHint);
            l.err = n > 0;
            lines.push(l);
        }
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
