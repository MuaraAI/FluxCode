//! VS Code's commands and shortcuts that FluxCode adds on top of the editor's own:
//! line editing (Ctrl+/, Alt+Up/Down, Shift+Alt+Up/Down, Ctrl+Shift+K, Ctrl+Enter),
//! Go to Line (Ctrl+G), the Command Palette (Ctrl+Shift+P, F1), zoom, reopening a
//! closed tab and closing all of them, and the word-wrap toggle (Alt+Z).
//!
//! The palette lists every command with its shortcut, from `COMMANDS`, so the list,
//! the bindings in `workbench::bind_keys` and the README table stay one set.

use crate::editor_cmds::{self, Edit};
use crate::workbench::*;
use gpui_kit::component::input::Position;
use gpui_kit::*;

/// One palette entry: its name (translated when shown), the shortcut as VS Code
/// writes it, and the action it runs.
pub struct Command {
    pub label: &'static str,
    pub keys: &'static str,
    pub action: fn() -> Box<dyn Action>,
}

macro_rules! cmd {
    ($label:expr, $keys:expr, $action:expr) => {
        Command { label: $label, keys: $keys, action: || Box::new($action) }
    };
}

/// Every command the palette offers, in VS Code's category order.
pub fn commands() -> Vec<Command> {
    vec![
        cmd!("File: Go to File", "Ctrl+P", QuickOpenFile),
        cmd!("File: Open File", "Ctrl+O", OpenFile),
        cmd!("File: Open Folder", "Ctrl+K Ctrl+O", OpenFolder),
        cmd!("File: Save", "Ctrl+S", Save),
        cmd!("File: Save All", "Ctrl+K S", SaveAll),
        cmd!("View: Close Editor", "Ctrl+W", CloseTab),
        cmd!("View: Close All Editors", "Ctrl+K Ctrl+W", CloseAllTabs),
        cmd!("View: Reopen Closed Editor", "Ctrl+Shift+T", ReopenClosedTab),
        cmd!("View: Next Editor", "Ctrl+Tab", NextTab),
        cmd!("View: Previous Editor", "Ctrl+Shift+Tab", PrevTab),
        cmd!("Go to Line", "Ctrl+G", GoToLine),
        cmd!("Toggle Line Comment", "Ctrl+/", ToggleLineComment),
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
        cmd!("View: Toggle Panel", "Ctrl+J", TogglePanel),
        cmd!("View: Toggle Terminal", "Ctrl+`", ToggleTerminal),
        cmd!("View: Show Output", "Ctrl+Shift+U", ShowOutput),
        cmd!("Run: Run File", "F5", RunFile),
        cmd!("Run: Stop", "Shift+F5", StopRun),
        cmd!("Markdown / HTML: Open Preview to the Side", "Ctrl+K V", TogglePreview),
        cmd!("Agent: Open or Close the Agent", "Ctrl+L", ToggleAgent),
        cmd!("Agent: Agent Settings", "", AgentShowOptions),
        cmd!("Preferences: Open Settings", "Ctrl+,", ShowSettings),
        cmd!("Preferences: Next Color Theme", "Ctrl+K Ctrl+T", NextTheme),
        cmd!("Help: What's New", "", ShowWhatsNew),
        cmd!("Help: Check for Updates", "", CheckUpdates),
        cmd!("Help: About FluxCode", "", About),
    ]
}

impl Workbench {
    /// Run a line edit on the active editor as one Undo step.
    fn edit_active(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&str, std::ops::Range<usize>, &str) -> Option<Edit>) {
        let Some(ix) = self.active else { return };
        let Some(d) = self.docs.get(ix) else { return };
        if d.read_only || d.image.is_some() {
            return;
        }
        let name = d.name.to_string();
        d.editor.update(cx, |s, cx| {
            let text = s.value().to_string();
            let sel = s.selected_range();
            if let Some(e) = f(&text, sel, &name) {
                s.set_selected_range(e.range.clone(), cx);
                s.replace(e.text.clone(), window, cx);
                s.set_selected_range(e.select.clone(), cx);
            }
        });
    }

    pub(crate) fn on_toggle_line_comment(&mut self, _: &ToggleLineComment, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, name| editor_cmds::comment_prefix(name).map(|p| editor_cmds::toggle_comment(t, sel, p)));
    }
    pub(crate) fn on_move_line_up(&mut self, _: &MoveLineUp, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| editor_cmds::move_lines(t, sel, true));
    }
    pub(crate) fn on_move_line_down(&mut self, _: &MoveLineDown, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| editor_cmds::move_lines(t, sel, false));
    }
    pub(crate) fn on_copy_line_up(&mut self, _: &CopyLineUp, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| Some(editor_cmds::copy_lines(t, sel, true)));
    }
    pub(crate) fn on_copy_line_down(&mut self, _: &CopyLineDown, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| Some(editor_cmds::copy_lines(t, sel, false)));
    }
    pub(crate) fn on_delete_line(&mut self, _: &DeleteLine, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| Some(editor_cmds::delete_lines(t, sel)));
    }
    pub(crate) fn on_insert_line_below(&mut self, _: &InsertLineBelow, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| Some(editor_cmds::insert_line(t, sel, false)));
    }
    pub(crate) fn on_insert_line_above(&mut self, _: &InsertLineAbove, w: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(w, cx, |t, sel, _| Some(editor_cmds::insert_line(t, sel, true)));
    }

    pub(crate) fn on_toggle_word_wrap(&mut self, _: &ToggleWordWrap, w: &mut Window, cx: &mut Context<Self>) {
        self.settings.word_wrap = !self.settings.word_wrap;
        self.status = if self.settings.word_wrap { crate::i18n::tr("Word wrap on") } else { crate::i18n::tr("Word wrap off") }.into();
        self.apply_settings(w, cx);
    }

    fn zoom(&mut self, size: f32, cx: &mut Context<Self>) {
        self.settings.font_size = size.clamp(8., 32.);
        self.settings.save();
        self.status = crate::trf!("Editor font: {n} px"; n => self.settings.font_size).into();
        cx.notify();
    }
    pub(crate) fn on_zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom(self.settings.font_size + 1., cx);
    }
    pub(crate) fn on_zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom(self.settings.font_size - 1., cx);
    }
    pub(crate) fn on_zoom_reset(&mut self, _: &ZoomReset, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom(crate::settings::Settings::default().font_size, cx);
    }

    pub(crate) fn on_close_all_tabs(&mut self, _: &CloseAllTabs, w: &mut Window, cx: &mut Context<Self>) {
        while !self.docs.is_empty() {
            let last = self.docs.len() - 1;
            self.close(last, w, cx);
        }
    }

    pub(crate) fn on_reopen_closed_tab(&mut self, _: &ReopenClosedTab, w: &mut Window, cx: &mut Context<Self>) {
        while let Some(p) = self.closed_tabs.pop() {
            if p.is_file() {
                self.open_file(p, w, cx);
                return;
            }
        }
    }

    /// Ctrl+G: the quick-open box with ":" typed, as in VS Code.
    pub(crate) fn on_go_to_line(&mut self, _: &GoToLine, w: &mut Window, cx: &mut Context<Self>) {
        if self.active.is_none() {
            return;
        }
        self.quick_open_with(":", w, cx);
    }

    /// Ctrl+Shift+P / F1: the quick-open box with ">" typed.
    pub(crate) fn on_command_palette(&mut self, _: &CommandPalette, w: &mut Window, cx: &mut Context<Self>) {
        self.quick_open_with(">", w, cx);
    }

    /// Jump to `line` (1-based, clamped) in the active editor, and to column `col`.
    pub(crate) fn go_to_line_col(&mut self, line: u32, col: u32, w: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active else { return };
        let editor = self.docs[ix].editor.clone();
        editor.update(cx, |s, cx| {
            let lines = s.value().lines().count().max(1) as u32;
            let pos = Position { line: line.clamp(1, lines) - 1, character: col.saturating_sub(1) };
            s.set_cursor_position(pos, w, cx);
        });
        let h = editor.focus_handle(cx);
        h.focus(w, cx);
    }
}

/// ":12" or ":12:5" or ":12,5" -> (line, column).
pub fn parse_line_query(q: &str) -> Option<(u32, u32)> {
    let rest = q.strip_prefix(':')?.trim();
    let mut parts = rest.split([':', ',']);
    let line = parts.next()?.trim().parse().ok()?;
    let col = parts.next().and_then(|c| c.trim().parse().ok()).unwrap_or(1);
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
