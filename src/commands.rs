use crate::editor_cmds::{self, Edit};
use crate::workbench::*;
use gpui_kit::component::input::Position;
use gpui_kit::*;

pub struct Command {
    pub label: &'static str,
    pub keys: &'static str,
    pub action: fn() -> Box<dyn Action>,
}

macro_rules! cmd {
    ($label:expr, $keys:expr, $action:expr) => {
        Command {
            label: $label,
            keys: $keys,
            action: || Box::new($action),
        }
    };
}

pub fn commands() -> Vec<Command> {
    vec![
        cmd!("File: Go to File", "Ctrl+P", QuickOpenFile),
        cmd!("File: New Text File", "Ctrl+N", NewTextFile),
        cmd!("File: New Window", "Ctrl+Shift+N", NewWindow),
        cmd!("File: Open File", "Ctrl+O", OpenFile),
        cmd!("File: Open Folder", "Ctrl+K Ctrl+O", OpenFolder),
        cmd!("File: Add Folder to Workspace", "", AddFolderToWorkspace),
        cmd!("File: Open Folder in WSL...", "", OpenFolderInWsl),
        cmd!("File: Save", "Ctrl+S", Save),
        cmd!("File: Save As...", "Ctrl+Shift+S", SaveAs),
        cmd!("File: Save All", "Ctrl+K S", SaveAll),
        cmd!("File: Toggle Auto Save", "", ToggleAutoSave),
        cmd!("File: Revert File", "", RevertFile),
        cmd!("File: Close Folder", "Ctrl+K F", CloseFolder),
        cmd!("View: Close Editor", "Ctrl+W", CloseTab),
        cmd!("View: Close Other Editors", "", CloseOtherTabs),
        cmd!("View: Close Saved Editors", "", CloseSavedTabs),
        cmd!("View: Close All Editors", "Ctrl+K Ctrl+W", CloseAllTabs),
        cmd!(
            "View: Reopen Closed Editor",
            "Ctrl+Shift+T",
            ReopenClosedTab
        ),
        cmd!("View: Next Editor", "Ctrl+Tab", NextTab),
        cmd!("View: Previous Editor", "Ctrl+Shift+Tab", PrevTab),
        cmd!("View: Toggle Full Screen", "F11", ToggleFullscreen),
        cmd!("View: Toggle Line Numbers", "", ToggleLineNumbers),
        cmd!("View: Toggle Render Whitespace", "", ToggleRenderWhitespace),
        cmd!("View: Toggle Liquid Glass", "", ToggleGlass),
        cmd!("Go: Back", "Alt+Left", NavBack),
        cmd!("Go: Forward", "Alt+Right", NavForward),
        cmd!("Go to Line", "Ctrl+G", GoToLine),
        cmd!("Toggle Line Comment", "Ctrl+/", ToggleLineComment),
        cmd!("Toggle Block Comment", "Shift+Alt+A", ToggleBlockComment),
        cmd!(
            "Selection: Expand Selection",
            "Shift+Alt+Right",
            ExpandSelection
        ),
        cmd!(
            "Selection: Shrink Selection",
            "Shift+Alt+Left",
            ShrinkSelection
        ),
        cmd!("Selection: Duplicate Selection", "", DuplicateSelection),
        cmd!("Move Line Up", "Alt+Up", MoveLineUp),
        cmd!("Move Line Down", "Alt+Down", MoveLineDown),
        cmd!("Copy Line Up", "Shift+Alt+Up", CopyLineUp),
        cmd!("Copy Line Down", "Shift+Alt+Down", CopyLineDown),
        cmd!("Delete Line", "Ctrl+Shift+K", DeleteLine),
        cmd!("Insert Line Below", "Ctrl+Enter", InsertLineBelow),
        cmd!("Insert Line Above", "Ctrl+Shift+Enter", InsertLineAbove),
        cmd!("View: Toggle Word Wrap", "Alt+Z", ToggleWordWrap),
        cmd!("View: Zoom In (editor font)", "Ctrl+=", ZoomIn),
        cmd!("View: Zoom Out (editor font)", "Ctrl+-", ZoomOut),
        cmd!("View: Reset Zoom", "Ctrl+0", ZoomReset),
        cmd!("View: Toggle Side Bar", "Ctrl+B", ToggleSidebar),
        cmd!("View: Show Explorer", "Ctrl+Shift+E", ShowExplorer),
        cmd!("Search: Find in Files", "Ctrl+Shift+F", FindInFiles),
        cmd!("View: Show Source Control", "Ctrl+Shift+G", ScmShow),
        cmd!("View: Show Run", "Ctrl+Shift+D", ShowRun),
        cmd!("View: Show Extensions", "Ctrl+Shift+X", ShowExtensions),
        cmd!("View: Show Minecraft Bedrock Tools", "", ShowMcbe),
        cmd!("View: Disk Usage (WizTree)", "Ctrl+Shift+Y", ShowWizTree),
        cmd!("View: Toggle Panel", "Ctrl+J", TogglePanel),
        cmd!("View: Toggle Terminal", "Ctrl+`", ToggleTerminal),
        cmd!("Terminal: New Terminal", "Ctrl+Shift+`", NewTerminal),
        cmd!("View: Show Output", "Ctrl+Shift+U", ShowOutput),
        cmd!("View: Show Problems", "Ctrl+Shift+M", ShowProblems),
        cmd!("Run: Run File", "F5", RunFile),
        cmd!("Run: Stop", "Shift+F5", StopRun),
        cmd!(
            "Markdown / HTML: Open Preview to the Side",
            "Ctrl+K V",
            TogglePreview
        ),
        cmd!("Agent: Open or Close the Agent", "Ctrl+L", ToggleAgent),
        cmd!("Agent: Agent Settings", "", AgentShowOptions),
        cmd!("Preferences: Open Settings", "Ctrl+,", ShowSettings),
        cmd!("Preferences: Color Theme", "Ctrl+K Ctrl+T", ChooseTheme),
        cmd!("Preferences: File Icon Theme", "", ChooseIconTheme),
        cmd!("Preferences: Next Color Theme", "", NextTheme),
        cmd!("Preferences: Next File Icon Theme", "", NextIconTheme),
        cmd!("Help: What's New", "", ShowWhatsNew),
        cmd!("Help: Check for Updates", "", CheckUpdates),
        cmd!("Help: About FluxCode", "", About),
    ]
}

impl Workbench {
    fn edit_active(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&str, std::ops::Range<usize>, &str) -> Option<Edit>,
    ) {
        let Some(ix) = self.active else { return };
        let Some(d) = self.docs.get(ix) else { return };
        if d.read_only || d.image.is_some() {
            return;
        }
        let name = d.name.to_string();
        let Some(ed) = self.ensure_doc_editor(ix, window, cx) else {
            return;
        };
        ed.update(cx, |s, cx| {
            let text = s.value().to_string();
            let sel = s.selected_range();
            if let Some(e) = f(&text, sel, &name) {
                s.set_selected_range(e.range.clone(), cx);
                s.replace(e.text.clone(), window, cx);
                s.set_selected_range(e.select.clone(), cx);
            }
        });
    }

    pub(crate) fn on_toggle_line_comment(
        &mut self,
        _: &ToggleLineComment,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, name| {
            editor_cmds::comment_prefix(name).map(|p| editor_cmds::toggle_comment(t, sel, p))
        });
    }
    pub(crate) fn on_toggle_block_comment(
        &mut self,
        _: &ToggleBlockComment,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, name| {
            editor_cmds::block_comment_delimiters(name)
                .map(|(o, c)| editor_cmds::toggle_block_comment(t, sel, o, c))
        });
    }
    pub(crate) fn on_duplicate_selection(
        &mut self,
        _: &DuplicateSelection,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| {
            Some(editor_cmds::duplicate_selection(t, sel))
        });
    }
    pub(crate) fn on_expand_selection(
        &mut self,
        _: &ExpandSelection,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.active else { return };
        let Some(ed) = self.ensure_doc_editor(ix, w, cx) else {
            return;
        };
        let mut pushed = None;
        ed.update(cx, |s, cx| {
            let text = s.value().to_string();
            let sel = s.selected_range();
            if let Some(next) = editor_cmds::expand_selection(&text, sel.clone()) {
                pushed = Some(sel);
                s.set_selected_range(next, cx);
            }
        });
        if let Some(prev) = pushed {
            if self.sel_history.len() >= 32 {
                self.sel_history.remove(0);
            }
            self.sel_history.push(prev);
        }
        ed.focus_handle(cx).focus(w, cx);
    }
    pub(crate) fn on_shrink_selection(
        &mut self,
        _: &ShrinkSelection,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prev) = self.sel_history.pop() else {
            return;
        };
        let Some(ix) = self.active else { return };
        let Some(ed) = self.ensure_doc_editor(ix, w, cx) else {
            return;
        };
        ed.update(cx, |s, cx| {
            let len = s.value().len();
            let clamped = prev.start.min(len)..prev.end.min(len);
            s.set_selected_range(clamped, cx);
        });
        ed.focus_handle(cx).focus(w, cx);
    }
    pub(crate) fn on_new_text_file(
        &mut self,
        _: &NewTextFile,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let base = self.root.clone().unwrap_or_else(std::env::temp_dir);
        let mut n = 1u32;
        let path = loop {
            let cand = base.join(format!("Untitled-{n}.txt"));
            if !cand.exists() && !self.docs.iter().any(|d| d.path == cand) {
                break cand;
            }
            n += 1;
        };
        if std::fs::write(&path, "").is_ok() {
            self.open_file(path, w, cx);
            if self.root.is_some() {
                self.refresh_tree(cx);
            }
        }
    }
    pub(crate) fn on_new_window(&mut self, _: &NewWindow, _: &mut Window, cx: &mut Context<Self>) {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).arg("--new-window").spawn();
        } else {
            self.status = crate::i18n::tr("Could not launch a new window.").into();
            cx.notify();
        }
    }
    pub(crate) fn on_save_as(&mut self, _: &SaveAs, w: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else { return };
        let Some(doc) = self.docs.get(ix) else { return };
        if doc.image.is_some() || doc.read_only {
            return;
        }
        let dir = doc
            .path
            .parent()
            .map(std::path::Path::to_path_buf)
            .or_else(|| self.root.clone())
            .unwrap_or_else(std::env::temp_dir);
        let suggested = doc.name.to_string();
        let text = doc.text(cx);
        let crlf = doc.crlf;
        let nbt = doc.nbt_format;
        let prompt = cx.prompt_for_new_path(&dir, Some(&suggested));
        cx.spawn_in(w, async move |this, cx| {
            let dest = prompt.await.ok()?.ok()??;
            let res = cx
                .background_spawn({
                    let dest = dest.clone();
                    async move { crate::workbench::write_doc_to_disk(None, &dest, &text, crlf, nbt) }
                })
                .await;
            this.update_in(cx, |this, window, cx| match res {
                Ok(()) => {
                    this.open_file(dest, window, cx);
                    if this.root.is_some() {
                        this.refresh_tree(cx);
                    }
                }
                Err(e) => {
                    this.status = e.into();
                    cx.notify();
                }
            })
            .ok()
        })
        .detach();
    }
    pub(crate) fn on_toggle_auto_save(
        &mut self,
        _: &ToggleAutoSave,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.auto_save = !self.settings.auto_save;
        self.settings.save();
        self.status = if self.settings.auto_save {
            crate::i18n::tr("Auto save on")
        } else {
            crate::i18n::tr("Auto save off")
        }
        .into();
        cx.notify();
    }
    pub(crate) fn on_revert_file(
        &mut self,
        _: &RevertFile,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.active else { return };
        let Some(path) = self.docs.get(ix).map(|d| d.path.clone()) else {
            return;
        };
        let Ok(file) = crate::core::read_text(&path) else {
            self.status = crate::i18n::tr("File no longer exists on disk.").into();
            cx.notify();
            return;
        };
        if let Some(ed) = self.docs[ix].editor.clone() {
            ed.update(cx, |s, cx| s.set_value(file.text.clone(), w, cx));
        }
        self.docs[ix].dirty = false;
        self.docs[ix].crlf = file.crlf;
        self.status = crate::i18n::tr("Reverted from disk").into();
        cx.notify();
    }
    pub(crate) fn on_close_folder(
        &mut self,
        _: &CloseFolder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.root = None;
        self.workspace.extra.clear();
        self.wsl = None;
        self.tree_nodes.clear();
        self.expanded.clear();
        self.explorer.rows.clear();
        self.all_files = std::sync::Arc::default();
        self.all_lower = std::sync::Arc::default();
        self.status = crate::i18n::tr("Folder closed").into();
        self.save_session(cx);
        cx.notify();
    }
    pub(crate) fn on_toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        w: &mut Window,
        _: &mut Context<Self>,
    ) {
        w.toggle_fullscreen();
    }
    pub(crate) fn on_toggle_line_numbers(
        &mut self,
        _: &ToggleLineNumbers,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.line_numbers = !self.settings.line_numbers;
        self.apply_settings(w, cx);
        self.status = if self.settings.line_numbers {
            crate::i18n::tr("Line numbers on")
        } else {
            crate::i18n::tr("Line numbers off")
        }
        .into();
        cx.notify();
    }
    pub(crate) fn on_toggle_render_whitespace(
        &mut self,
        _: &ToggleRenderWhitespace,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.show_whitespace = !self.settings.show_whitespace;
        self.apply_settings(w, cx);
        self.status = if self.settings.show_whitespace {
            crate::i18n::tr("Whitespace markers on")
        } else {
            crate::i18n::tr("Whitespace markers off")
        }
        .into();
        cx.notify();
    }
    pub(crate) fn on_toggle_glass(
        &mut self,
        _: &ToggleGlass,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.glass = !self.settings.glass;
        self.apply_appearance(w, cx);
        self.status = if self.settings.glass {
            crate::i18n::tr("Liquid glass on")
        } else {
            crate::i18n::tr("Liquid glass off")
        }
        .into();
        cx.notify();
    }
    pub(crate) fn on_close_other_tabs(
        &mut self,
        _: &CloseOtherTabs,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(cur) = self.active else { return };
        self.close_tabs_where(w, cx, |i, d| i != cur && !d.dirty);
    }
    pub(crate) fn on_close_saved_tabs(
        &mut self,
        _: &CloseSavedTabs,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_tabs_where(w, cx, |_, d| !d.dirty);
    }
    pub(crate) fn on_prev_problem(
        &mut self,
        _: &PrevProblem,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.problems.last().cloned() {
            let col = p.caret.map_or(p.col.saturating_sub(1), |c| c.0 as u32);
            self.open_location(p.path, p.line, col, w, cx);
            return;
        }
        if let Some(ix) = self.active {
            if let Some(d) = self.docs.get(ix).and_then(|doc| {
                doc.diagnostics
                    .last()
                    .map(|c| (doc.path.clone(), c.row + 1, c.col))
            }) {
                self.open_location(d.0, d.1, d.2, w, cx);
                return;
            }
        }
        if let Some((path, line, col)) = self.docs.iter().rev().find_map(|doc| {
            doc.diagnostics
                .last()
                .map(|c| (doc.path.clone(), c.row + 1, c.col))
        }) {
            self.open_location(path, line, col, w, cx);
        }
    }
    pub(crate) fn on_run_selected_text(
        &mut self,
        _: &RunSelectedText,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = self
            .active
            .and_then(|i| self.docs.get(i))
            .and_then(|d| d.editor.as_ref())
            .and_then(|ed| {
                let st = ed.read(cx);
                let r = st.selected_range();
                if r.is_empty() {
                    return None;
                }
                let val = st.value().to_string();
                let s = r.start.min(val.len());
                let e = r.end.min(val.len());
                Some(val[s..e].trim().to_string())
            })
            .filter(|s| !s.is_empty());
        let Some(cmd) = selected else {
            self.status = crate::i18n::tr("No text selected to run").into();
            cx.notify();
            return;
        };
        self.panel_tab = PanelTab::Terminal;
        self.set_panel_open(true, cx);
        self.run_in_terminal(&cmd, cx);
    }
    pub(crate) fn on_move_line_up(
        &mut self,
        _: &MoveLineUp,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| editor_cmds::move_lines(t, sel, true));
    }
    pub(crate) fn on_move_line_down(
        &mut self,
        _: &MoveLineDown,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| editor_cmds::move_lines(t, sel, false));
    }
    pub(crate) fn on_copy_line_up(
        &mut self,
        _: &CopyLineUp,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| {
            Some(editor_cmds::copy_lines(t, sel, true))
        });
    }
    pub(crate) fn on_copy_line_down(
        &mut self,
        _: &CopyLineDown,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| {
            Some(editor_cmds::copy_lines(t, sel, false))
        });
    }
    pub(crate) fn on_delete_line(
        &mut self,
        _: &DeleteLine,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| Some(editor_cmds::delete_lines(t, sel)));
    }
    pub(crate) fn on_insert_line_below(
        &mut self,
        _: &InsertLineBelow,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| {
            Some(editor_cmds::insert_line(t, sel, false))
        });
    }
    pub(crate) fn on_insert_line_above(
        &mut self,
        _: &InsertLineAbove,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_active(w, cx, |t, sel, _| {
            Some(editor_cmds::insert_line(t, sel, true))
        });
    }

    pub(crate) fn on_toggle_word_wrap(
        &mut self,
        _: &ToggleWordWrap,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_wrap(w, cx);
    }

    pub(crate) fn toggle_wrap(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let setting = self.settings.word_wrap;
        let Some(d) = self.active.and_then(|a| self.docs.get_mut(a)) else {
            return;
        };
        let on = !d.wraps(setting);
        d.wrap_override = Some(on);
        if let Some(ed) = &d.editor {
            ed.update(cx, |s, cx| s.set_soft_wrap(on, w, cx));
        }
        self.status = if on {
            crate::i18n::tr("Word wrap on")
        } else {
            crate::i18n::tr("Word wrap off")
        }
        .into();
        cx.notify();
    }

    fn zoom(&mut self, size: f32, cx: &mut Context<Self>) {
        self.settings.font_size = size.clamp(8., 32.);
        self.settings.save();
        self.status = crate::trf!("Editor font: {n} px"; n => self.settings.font_size).into();
        cx.notify();
    }
    pub(crate) fn on_zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self
            .active
            .filter(|&i| self.docs[i].flux.is_some() && !self.docs[i].flux_source)
        {
            return self.flux_zoom_step(ix, 1, cx);
        }
        self.zoom(self.settings.font_size + 1., cx);
    }
    pub(crate) fn on_zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self
            .active
            .filter(|&i| self.docs[i].flux.is_some() && !self.docs[i].flux_source)
        {
            return self.flux_zoom_step(ix, -1, cx);
        }
        self.zoom(self.settings.font_size - 1., cx);
    }
    pub(crate) fn on_zoom_reset(&mut self, _: &ZoomReset, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self
            .active
            .filter(|&i| self.docs[i].flux.is_some() && !self.docs[i].flux_source)
        {
            return self.flux_zoom_step(ix, 0, cx);
        }
        self.zoom(crate::settings::Settings::default().font_size, cx);
    }

    pub(crate) fn on_close_all_tabs(
        &mut self,
        _: &CloseAllTabs,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        while !self.docs.is_empty() {
            let last = self.docs.len() - 1;
            self.close(last, w, cx);
        }
    }

    pub(crate) fn on_reopen_closed_tab(
        &mut self,
        _: &ReopenClosedTab,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        while let Some(p) = self.closed_tabs.pop() {
            if p.is_file() {
                self.open_file(p, w, cx);
                return;
            }
        }
    }

    pub(crate) fn on_go_to_line(&mut self, _: &GoToLine, w: &mut Window, cx: &mut Context<Self>) {
        if self.active.is_none() {
            return;
        }
        self.quick_open_with(":", w, cx);
    }

    pub(crate) fn on_command_palette(
        &mut self,
        _: &CommandPalette,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.quick_open_with(">", w, cx);
    }

    pub(crate) fn go_to_line_col(
        &mut self,
        line: u32,
        col: u32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.active else { return };
        let Some(editor) = self.ensure_doc_editor(ix, w, cx) else {
            return;
        };
        editor.update(cx, |s, cx| {
            let lines = s.value().lines().count().max(1) as u32;
            let pos = Position {
                line: line.clamp(1, lines) - 1,
                character: col.saturating_sub(1),
            };
            s.set_cursor_position(pos, w, cx);
        });
        let h = editor.focus_handle(cx);
        h.focus(w, cx);
    }
}

pub fn parse_line_query(q: &str) -> Option<(u32, u32)> {
    let rest = q.strip_prefix(':')?.trim();
    let mut parts = rest.split([':', ',']);
    let line = parts.next()?.trim().parse().ok()?;
    let col = parts
        .next()
        .and_then(|c| c.trim().parse().ok())
        .unwrap_or(1);
    Some((line, col))
}

#[cfg(test)]
mod tests {
    use super::{commands, parse_line_query};

    #[test]
    fn line_queries() {
        assert_eq!(parse_line_query(":12"), Some((12, 1)));
        assert_eq!(parse_line_query(": 7:3"), Some((7, 3)));
        assert_eq!(parse_line_query(":40,2"), Some((40, 2)));
        assert_eq!(parse_line_query(":"), None);
        assert_eq!(parse_line_query("12"), None);
    }

    #[test]
    fn every_palette_command_has_a_unique_name() {
        let mut names: Vec<&str> = commands().iter().map(|c| c.label).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(n, names.len());
    }
}
