//! Follow the agent's edits. When it changes a file, the file comes up in the editor
//! and the change is typed in the way a person would: removed lines go, added lines
//! appear one by one from the top, and the cursor (so the view) rides along.
//!
//! It is a replay of what is already on disk, never in the agent's way: the agent
//! goes on at once, a replay takes at most ~40 frames (well under a second), and a
//! newer edit finishes the running replay on the spot. Files edited at the same time
//! all reload at once; the newest one is the one replayed. Nothing replays into a
//! file with unsaved changes of the user's, and the focus stays where the user has it
//! (typing in the chat goes on while the editor follows).

use crate::core;
use crate::workbench::Workbench;
use gpui_kit::component::input::Position;
use gpui_kit::*;
use std::path::PathBuf;
use std::time::Duration;

/// Frames in one replay, and time between them.
const FRAMES: usize = 40;
const FRAME_MS: u64 = 16;
/// Larger files reload at once (each frame rewrites the whole text).
const MAX_BYTES: usize = 300 * 1024;

impl Workbench {
    /// The agent finished writing `p`: replay it on the next frame.
    pub(crate) fn agent_follow_edit(&mut self, p: PathBuf) {
        if !self.settings.agent_follow_edits {
            return;
        }
        self.agent.follow_q.retain(|x| *x != p);
        self.agent.follow_q.push(p);
    }

    /// Runs before the reloads (render has the Window).
    pub(crate) fn apply_agent_follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.agent.follow_q.pop() else { return };
        self.agent.follow_q.clear();
        // The user is changing the file on screen: leave their view alone.
        if self.active.is_some_and(|a| self.docs[a].dirty && self.docs[a].path != p) {
            return;
        }
        self.agent_finish_replay(window, cx);
        let Ok(f) = core::read_text(&p) else { return };
        self.collab_mark_ai(&p);
        // What the file looked like before: the open editor's text, else as the agent
        // found it (the Changes card keeps it).
        let open = self.docs.iter().position(|d| d.path == p);
        let before = match open {
            Some(ix) if self.docs[ix].dirty || self.docs[ix].image.is_some() => return,
            Some(ix) => self.docs[ix].editor.read(cx).value().to_string(),
            None => match self.agent.changes.iter().find(|ch| ch.path == p) {
                Some(ch) => ch.before.clone().unwrap_or_default(),
                None => return,
            },
        };
        // Open or bring up the file without taking the keyboard from the user.
        let had = window.focused(cx);
        match open {
            Some(ix) => self.activate(ix, window, cx),
            None => self.open_file(p.clone(), window, cx),
        }
        if let Some(h) = had.filter(|h| !self.docs.iter().any(|d| d.editor.focus_handle(cx) == *h)) {
            h.focus(window, cx);
        }
        let Some(ix) = self.docs.iter().position(|d| d.path == p) else { return };
        // Its reload is this replay's last frame.
        self.agent.reloads.retain(|x| *x != p);
        let frames = if f.text.len() > MAX_BYTES { Vec::new() } else { crate::diff::typing_frames(&before, &f.text, FRAMES) };
        if frames.is_empty() {
            let line = self.docs[ix].editor.read(cx).cursor_position().line;
            self.docs[ix].editor.update(cx, |s, cx| {
                s.set_value(f.text.clone(), window, cx);
                s.set_cursor_position(Position::new(line, 0), window, cx);
            });
            self.docs[ix].crlf = f.crlf;
            self.collab_on_path(&p, cx);
            return;
        }
        self.docs[ix].crlf = f.crlf;
        self.agent.replay_gen += 1;
        let gen = self.agent.replay_gen;
        self.agent.replay = Some((p.clone(), f.text));
        cx.spawn_in(window, async move |this, cx| {
            for (text, line) in frames {
                cx.background_executor().timer(Duration::from_millis(FRAME_MS)).await;
                let go = this.update_in(cx, |this, window, cx| {
                    if this.agent.replay_gen != gen {
                        return false;
                    }
                    let Some(d) = this.docs.iter().find(|d| d.path == p && !d.dirty) else { return false };
                    d.editor.update(cx, |s, cx| {
                        s.set_value(text, window, cx);
                        s.set_cursor_position(Position::new(line, 0), window, cx);
                    });
                    cx.notify();
                    true
                });
                if !matches!(go, Ok(true)) {
                    break;
                }
            }
            _ = this.update_in(cx, |this, window, cx| {
                if this.agent.replay_gen == gen {
                    this.agent_finish_replay(window, cx);
                }
            });
        })
        .detach();
    }

    /// End the running replay: its file gets its final text now.
    fn agent_finish_replay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((p, text)) = self.agent.replay.take() else { return };
        self.agent.replay_gen += 1;
        if let Some(d) = self.docs.iter().find(|d| d.path == p && !d.dirty) {
            if d.editor.read(cx).value().to_string() != text {
                let line = d.editor.read(cx).cursor_position().line;
                d.editor.update(cx, |s, cx| {
                    s.set_value(text, window, cx);
                    s.set_cursor_position(Position::new(line, 0), window, cx);
                });
            }
        }
        self.collab_on_path(&p, cx);
        cx.notify();
    }
}
