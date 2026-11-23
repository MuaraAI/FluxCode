
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
                _ => (indent, len.max(indent + 1)),
            };
            Diagnostic::new(Position::new(row, a)..Position::new(row, b), p.message.clone())
                .with_severity(DiagnosticSeverity::Error)
                .with_source("python")
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
    matches!(k, LineKind::Normal | LineKind::Err | LineKind::Warn | LineKind::Dim | LineKind::Success | LineKind::Frame | LineKind::Hunk | LineKind::Code | LineKind::Caret | LineKind::Blank)
}

pub(crate) type WrapCache = std::cell::RefCell<((usize, usize, usize, usize, usize), std::rc::Rc<Vec<Row>>)>;

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
        self.term_proc.is_some_and(|id| self.procs.is_running(id))
    }

    fn set_term_input(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let chars = text.chars().count() as u32;
        self.term_typed = !text.is_empty();
        self.term_input.update(cx, |s, cx| {
            s.set_value(text.to_string(), window, cx);
            s.set_cursor_position(Position::new(0, chars), window, cx);
        });
    }

    pub(crate) fn terminal_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = self.term_input.read(cx).value().to_string();
        self.term_input.update(cx, |s, cx| s.set_value("", window, cx));
        self.term_typed = false;
        self.term_hist_ix = None;
        if let Some(id) = self.term_proc.filter(|id| self.procs.is_running(*id)) {
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

    pub(crate) fn run_in_terminal(&mut self, typed: &str, cx: &mut Context<Self>) {
        if !self.term_lines.is_empty() {
            push_line(&mut self.term_lines, "", LineKind::Blank);
        }
        let mut prompt = Line::new(typed, LineKind::Cmd);
        prompt.meta = self.term_meta().into();
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
            let meta = self.term_full();
            push_line(&mut self.term_lines, &meta, LineKind::Normal);
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
                    let cwd = t.linux_path(&self.term_cwd).unwrap_or_else(|| t.linux_root.clone());
                    crate::remote::shell_command(t, &cwd, &cmdline)
                }
                None => proc::shell_command(&cmdline),
            };
            crate::github::apply_env(&mut cmd);
            let local_cwd = self.wsl.is_none().then(|| self.term_cwd.clone());
            match self.procs.spawn(cmd, local_cwd.as_deref()) {
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

    fn term_meta(&self) -> String {
        match &self.wsl {
            Some(t) => t.linux_path(&self.term_cwd).unwrap_or_else(|| t.linux_root.clone()),
            None => term::short_cwd(&self.term_cwd, self.root.as_deref()),
        }
    }

    fn term_full(&self) -> String {
        match &self.wsl {
            Some(t) => t.linux_path(&self.term_cwd).unwrap_or_else(|| t.linux_root.clone()),
            None => self.term_cwd.display().to_string(),
        }
    }

    fn change_dir(&mut self, arg: &str) {
        if let Some(t) = self.wsl.clone() {
            let current = t.linux_path(&self.term_cwd).unwrap_or_else(|| t.linux_root.clone());
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
                self.term_cwd = t.unc_path(&linux);
            } else {
                push_line(&mut self.term_lines, &crate::trf!("No folder named '{arg}' here."; arg => arg), LineKind::Err);
            }
            return;
        }
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
        if let Some(id) = self.term_proc.filter(|id| self.procs.is_running(*id)) {
            push_text(&mut self.term_lines, "^C\n", false);
            self.procs.kill(id);
        } else {
            self.set_term_input("", window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_term_escape(&mut self, _: &TermEscape, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_sel = None;
        self.term_hist_ix = None;
        self.set_term_input("", window, cx);
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
        for id in [self.out_proc, self.term_proc].into_iter().flatten() {
            self.procs.kill(id);
        }
        cx.notify();
    }

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
        let spent = run.as_ref().map(|r| r.started.elapsed());
        let quiet = terminal && code == Some(0) && error.is_none() && spent.is_some_and(|d| d < std::time::Duration::from_secs(1));
        match (&error, &run) {
            (Some(e), Some(run)) => push_card(lines, e, &run.cwd),
            _ if !quiet && output >= 2 && lines.last().is_some_and(|l| !l.text.trim().is_empty()) => push_line(lines, "", LineKind::Blank),
            _ => {}
        }
        let took = spent.map(term::elapsed).unwrap_or_default();
        let (text, ok) = match code {
            Some(0) => (tr("Finished").to_string(), true),
            Some(c) => (crate::trf!("Failed (exit code {c})"; c => c), false),
            None => (tr("Stopped").to_string(), false),
        };
        let mut status = Line::new(&text, LineKind::Status);
        status.err = !ok;
        status.meta = took.into();
        if !quiet {
            lines.push(status);
        }
        if let Some(r) = reason {
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
        let doc_path = d.path.clone();
        let doc_gen = d.gen;
        let rope_len = editor.read(cx).text().len();

        if rope_len <= MAX_SYNC_DIAG_BYTES {
            let code_diags = lang
                .map(|l| crate::diagnostics::analyze(&editor.read(cx).value(), l))
                .unwrap_or_default();
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
                        crate::diagnostics::analyze(&text, l)
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
            }
        }
    }

    pub(crate) fn on_go_to_problem(&mut self, _: &GoToProblem, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.problems.first().cloned() {
            let col = p.caret.map_or(0, |c| c.0 as u32);
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

    fn open_link(&mut self, path: &str, line: u32, terminal: bool, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let p = PathBuf::from(path);
        let base = if terminal {
            self.term_run.as_ref().map(|r| r.cwd.clone()).unwrap_or_else(|| self.term_cwd.clone())
        } else {
            self.out_run.as_ref().map(|r| r.cwd.clone()).or_else(|| self.ext.last_cwd.clone()).or_else(|| self.root.clone()).unwrap_or_default()
        };
        let full = if p.is_absolute() { p } else { base.join(p) };
        self.open_location(full, line, 0, window, cx);
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
        if self.term_history.last().map(String::as_str) != Some(command) {
            self.term_history.push(command.to_string());
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
                    .bg(c(theme::DANGER).opacity(0.08)),
            )
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
        (if terminal { &self.term_wrap } else { &self.out_wrap }).borrow().1.clone()
    }

    pub(crate) fn panel_sel_text(&self) -> Option<String> {
        let sel = self.panel_sel.filter(|s| !s.is_empty())?;
        let rows = self.panel_rows(sel.terminal);
        let lines = if sel.terminal { &self.term_lines } else { &self.out_lines };
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
            let marked = l.kind == LineKind::Code && lines.get(ix + 1).is_some_and(|n| n.kind == LineKind::Caret);
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
                col: p.caret.map_or(0, |c| c.0 as u32),
                message: p.message.clone().into(),
                source: "runtime",
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
                            .child(div().flex_none().text_color(c(theme::FG_MUTED)).child(loc))
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

        let tab = |id: &'static str, label: &'static str, badge: Option<(usize, theme::Tok)>, which: PanelTab, this: &Self, cx: &mut Context<Self>| {
            let on = this.panel_tab == which;
            h_flex()
                .id(id)
                .px_1()
                .h_full()
                .gap_1p5()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .border_b_1()
                .border_color(if on { c(theme::ACCENT) } else { transparent_black() })
                .text_color(if on { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
                .when(!on, |d| d.hover(|s| s.text_color(c(theme::FG))))
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
        };
        let is_problems = self.panel_tab == PanelTab::Problems;
        let terminal = self.panel_tab == PanelTab::Terminal;
        let running = if terminal { self.term_proc.is_some() } else if is_problems { false } else { self.out_proc.is_some() };
        let shown_lines = if terminal { &self.term_lines } else { &self.out_lines };
        let python = (!is_problems)
            .then(|| self.python_version.clone().filter(|_| Self::ran_python(shown_lines)))
            .flatten();
        let prob_badge = (prob_total > 0).then_some((prob_total, if prob_errs > 0 { theme::DANGER } else { theme::WARN }));
        let header = h_flex()
            .h(theme::u(35.))
            .px_3()
            .gap_4()
            .flex_none()
            .child(tab("p-term", tr("Terminal"), None, PanelTab::Terminal, self, cx))
            .child(tab("p-out", tr("Output"), None, PanelTab::Output, self, cx))
            .child(tab("p-prob", tr("Problems"), prob_badge, PanelTab::Problems, self, cx))
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
                        PanelTab::Terminal => this.term_lines.clear(),
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
        let scroll = if terminal { self.term_scroll.clone() } else { self.out_scroll.clone() };
        let font_size = px((self.settings.font_size - 2.).clamp(10., 28.));
        let mono_font = theme::resolve_mono_font(&self.settings.font_family, cx);
        let m = {
            let ts = window.text_system();
            let id = ts.resolve_font(&font(mono_font.clone()));
            let em = ts.em_advance(id, font_size).map(f32::from).unwrap_or(f32::from(font_size) * 0.6).max(4.);
            Metrics { row: px((f32::from(font_size) * 1.6).round()), em }
        };
        let (width, height) = self.panel_size.get();
        let cols = if width <= 0. {
            Cols { mono: 160, card: 160, result: 160 }
        } else {
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
            .when(terminal, |d| d.when(air, |d| d.child(div().h(m.row))).child(self.render_prompt(mono_font, font_size, narrow, m)));

        let empty = count == 0;
        let hint = |line: &'static str| div().min_w_0().child(line);
        let content = v_flex()
            .id("panel-body")
            .flex_1()
            .min_h_0()
            .when(empty, |v| {
                v.child(
                    v_flex()
                        .flex_shrink(1.)
                        .min_h_0()
                        .overflow_hidden()
                        .px_3()
                        .py_2()
                        .gap_1()
                        .text_xs()
                        .text_color(c(theme::FG_DIM))
                        .when(terminal, |v| {
                            v.child(div().min_w_0().text_sm().text_color(c(theme::FG_MUTED)).child(tr("Type a command after the ❯ and press Enter to run it.")))
                                .child(hint(tr("For example:  dir  lists the files here,  cd folder  goes into a folder,  py app.py  runs a Python file.")))
                                .when(!narrow, |v| v.child(hint(tr("↑ ↓ history  ·  Tab completes names  ·  Ctrl+C stops  ·  typos like  pip i  are fixed for you"))))
                        })
                        .when(!terminal, |v| v.child(hint(tr("Nothing has run yet. Press F5 to run the open file, or pick a task under Run.")))),
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

    fn render_prompt(&self, mono_font: SharedString, font_size: Pixels, narrow: bool, m: Metrics) -> impl IntoElement {
        let busy = self.term_running();
        let short = term::short_cwd(&self.term_cwd, self.root.as_deref());
        let place: SharedString = prompt_place(&short, self.panel_size.get().0).into();
        let full: SharedString = self.term_cwd.display().to_string().into();
        let placeholder = match (busy, narrow) {
            (false, false) => tr("Type a command, then press Enter"),
            (false, true) => tr("Type a command"),
            (true, false) => tr("Running. Type here if the program asks for something"),
            (true, true) => tr("Running"),
        };
        let hint = !self.term_typed && (busy || self.term_lines.is_empty());
        h_flex()
            .h(m.row)
            .w_full()
            .flex_none()
            .px_3()
            .gap_2()
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
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h(m.row)
                    .flex()
                    .items_center()
                    .ml(px(-8.))
                    .key_context("Terminal")
                    .child(Input::new(&self.term_input).appearance(false).with_size(px(f32::from(font_size) / 0.875)))
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
                                .text_sm()
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

const RUNNABLE: &[&str] = &["py", "pyw", "js", "mjs", "cjs", "ts", "mts", "ps1", "bat", "cmd", "sh", "rs", "go", "php", "lua", "rb"];

pub(crate) fn can_run(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| RUNNABLE.contains(&e.to_ascii_lowercase().as_str()))
}

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
    use super::{can_run, run_command_for, PanelSel};
    use std::path::Path;

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
        for name in ["a.py", "A.PY", "b.pyw", "c.js", "d.mjs", "e.cjs", "f.ts", "g.mts", "h.ps1", "i.bat", "j.cmd", "k.sh", "l.rs", "m.go", "n.php", "o.lua", "p.rb", "warps.yml", "x.txt", "y.json", "noext", "z.html"] {
            let p = Path::new(name);
            assert_eq!(can_run(p), run_command_for(p, None).is_some(), "{name}");
        }
    }
}
