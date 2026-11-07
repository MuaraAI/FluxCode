//! Bottom panel: Terminal (one command at a time, like the Tauri app's Flux
//! Terminal) and Output (Run File).

use crate::proc::{self, python};
use crate::theme::{self, c};
use crate::workbench::*;
use gpui_kit::component::{
    h_flex,
    input::Input,
    v_flex, IconName, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::{Path, PathBuf};

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

    /// Enter in the terminal prompt: run a command, or feed stdin of the running one.
    pub(crate) fn terminal_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = self.term_input.read(cx).value().to_string();
        self.term_input.update(cx, |s, cx| s.set_value("", window, cx));
        if let Some(id) = self.term_proc.filter(|id| self.procs.is_running(*id)) {
            push_text(&mut self.term_lines, &format!("{line}\n"), LineKind::Cmd);
            self.procs.write_stdin(id, &format!("{line}\n"));
            cx.notify();
            return;
        }
        let trimmed = line.trim();
        push_line(&mut self.term_lines, &format!("{}> {trimmed}", self.term_cwd.display()), LineKind::Cmd);
        if !trimmed.is_empty() && self.term_history.last().map(String::as_str) != Some(trimmed) {
            self.term_history.push(trimmed.to_string());
        }
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.is_empty() {
        } else if lower == "cls" || lower == "clear" {
            self.term_lines.clear();
        } else if lower == "cd" || lower.starts_with("cd ") || lower.starts_with("cd\\") || lower.starts_with("cd..") || lower.starts_with("chdir") {
            let arg = trimmed.trim_start_matches(|c: char| c.is_ascii_alphabetic()).trim();
            let arg = arg.strip_prefix("/d").or_else(|| arg.strip_prefix("/D")).unwrap_or(arg).trim().trim_matches('"');
            if arg.is_empty() {
                push_line(&mut self.term_lines, &self.term_cwd.display().to_string(), LineKind::Normal);
            } else {
                self.change_dir(arg);
            }
        } else if trimmed.len() == 2 && trimmed.ends_with(':') {
            self.change_dir(&format!("{trimmed}\\"));
        } else {
            let mut cmd = proc::shell_command(trimmed);
            crate::github::apply_env(&mut cmd);
            match self.procs.spawn(cmd, Some(&self.term_cwd.clone())) {
                Ok(id) => self.term_proc = Some(id),
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
            _ => push_line(&mut self.term_lines, "The system cannot find the path specified.", LineKind::Err),
        }
    }

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
        self.out_lines.clear();
        push_line(&mut self.out_lines, &format!("[Running] {program} {}", args.join(" ")), LineKind::Info);
        let mut cmd = proc::command(&program);
        cmd.args(&args);
        let cwd = path.parent().map(Path::to_path_buf);
        match self.procs.spawn(cmd, cwd.as_deref()) {
            Ok(id) => self.out_proc = Some(id),
            Err(e) => push_line(&mut self.out_lines, &format!("Failed to start '{program}': {e}"), LineKind::Err),
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

    pub(crate) fn render_panel(&self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, which: PanelTab, this: &Self, cx: &mut Context<Self>| {
            let on = this.panel_tab == which;
            div()
                .id(id)
                .px_2()
                .h_full()
                .flex()
                .items_center()
                .text_xs()
                .border_b_1()
                .border_color(if on { c(theme::FG_STRONG) } else { transparent_black() })
                .text_color(if on { c(theme::FG_STRONG) } else { c(theme::FG_MUTED) })
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
            .h(px(32.))
            .px_3()
            .gap_3()
            .child(tab("p-term", "TERMINAL", PanelTab::Terminal, self, cx))
            .child(tab("p-out", "OUTPUT", PanelTab::Output, self, cx))
            .child(div().flex_1())
            .when(running, |h| {
                h.child(tool_button("p-stop", IconName::Square, "Stop (Shift+F5)", cx, |this, w, cx| this.on_stop_run(&StopRun, w, cx)))
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

        let (count, scroll, which) = match self.panel_tab {
            PanelTab::Terminal => (self.term_lines.len(), self.term_scroll.clone(), PanelTab::Terminal),
            PanelTab::Output => (self.out_lines.len(), self.out_scroll.clone(), PanelTab::Output),
        };
        let list = uniform_list(
            "panel-lines",
            count,
            cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
                let lines = if which == PanelTab::Terminal { &this.term_lines } else { &this.out_lines };
                range
                    .map(|i| {
                        let l = &lines[i];
                        let col = match l.kind {
                            LineKind::Normal => theme::FG,
                            LineKind::Err => 0xff7b72,
                            LineKind::Cmd => 0x82bfff,
                            LineKind::Info => theme::FG_DIM,
                        };
                        div().h(px(18.)).px_3().whitespace_nowrap().text_color(c(col)).child(l.text.clone())
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&scroll)
        .flex_1()
        .font_family(self.settings.font_family.clone())
        .text_size(px(13.));

        v_flex()
            .size_full()
            .bg(c(theme::BG_EDITOR))
            .border_t_1()
            .border_color(c(theme::BORDER))
            .child(header)
            .child(list)
            .when(self.panel_tab == PanelTab::Terminal, |v| {
                v.child(
                    h_flex()
                        .px_3()
                        .py_1()
                        .gap_2()
                        .font_family(self.settings.font_family.clone())
                        .text_size(px(13.))
                        .child(
                            div()
                                .text_color(c(0x82bfff))
                                .child(if self.term_proc.is_some() { "stdin>".to_string() } else { format!("{}>", self.term_cwd.display()) }),
                        )
                        .child(div().flex_1().child(Input::new(&self.term_input).appearance(false).small())),
                )
            })
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
