
use crate::core;
use crate::workbench::Workbench;
use gpui_kit::component::input::Position;
use gpui_kit::*;
use std::path::PathBuf;
use std::time::Duration;

const FRAMES: usize = 40;
const FRAME_MS: u64 = 16;
const BYTE_BUDGET: usize = 400 * 1024;
const MIN_FRAMES: usize = 4;
const HANDS_OFF: Duration = Duration::from_secs(4);

pub fn frames_for(len: usize) -> usize {
    let n = (BYTE_BUDGET / len.max(1)).min(FRAMES);
    if n < MIN_FRAMES {
        0
    } else {
        n
    }
}

impl Workbench {
    pub(crate) fn agent_follow_edit(&mut self, p: PathBuf) {
        if !self.settings.agent_follow_edits || self.agent.bg {
            return;
        }
        self.agent.follow_q.retain(|x| *x != p);
        self.agent.follow_q.push(p);
    }

    pub(crate) fn apply_agent_follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.agent.follow_q.pop() else { return };
        self.agent.follow_q.clear();
        if self.active.is_some_and(|a| self.docs[a].dirty && self.docs[a].path != p) || self.agent.user_nav.is_some_and(|t| t.elapsed() < HANDS_OFF) {
            return;
        }
        self.agent_finish_replay(window, cx);
        let Ok(f) = core::read_text(&p) else { return };
        self.collab_mark_ai(&p);
        let open = self.docs.iter().position(|d| d.path == p);
        let before = match open {
            Some(ix) if self.docs[ix].dirty || self.docs[ix].image.is_some() => return,
            Some(ix) => self.docs[ix].text(cx),
            None => match self.agent.changes.iter().find(|ch| ch.path == p) {
                Some(ch) => ch.before.clone().unwrap_or_default(),
                None => return,
            },
        };
        let had = window.focused(cx);
        match open {
            Some(ix) => self.activate(ix, window, cx),
            None => self.open_file_now(p.clone(), window, cx),
        }
        if let Some(h) = had.filter(|h| !self.docs.iter().any(|d| d.editor.as_ref().is_some_and(|e| e.focus_handle(cx) == *h))) {
            h.focus(window, cx);
        }
        let Some(ix) = self.docs.iter().position(|d| d.path == p) else { return };
        let Some(ed) = self.ensure_doc_editor(ix, window, cx) else { return };
        self.agent.reloads.retain(|x| *x != p);
        let steps = frames_for(f.text.len().max(before.len()));
        let frames = if steps == 0 { Vec::new() } else { crate::diff::typing_frames(&before, &f.text, steps) };
        if frames.is_empty() {
            let line = ed.read(cx).cursor_position().line;
            ed.update(cx, |s, cx| {
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
                    let Some(ed) = this.docs.iter().find(|d| d.path == p && !d.dirty).and_then(|d| d.editor.as_ref()) else {
                        return false;
                    };
                    ed.update(cx, |s, cx| {
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

    fn agent_finish_replay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((p, text)) = self.agent.replay.take() else { return };
        self.agent.replay_gen += 1;
        if let Some(ed) = self.docs.iter().find(|d| d.path == p && !d.dirty).and_then(|d| d.editor.as_ref()) {
            if ed.read(cx).value().to_string() != text {
                let line = ed.read(cx).cursor_position().line;
                ed.update(cx, |s, cx| {
                    s.set_value(text, window, cx);
                    s.set_cursor_position(Position::new(line, 0), window, cx);
                });
            }
        }
        self.collab_on_path(&p, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::frames_for;

    #[test]
    fn bigger_files_get_fewer_steps() {
        assert_eq!(frames_for(2_000), 40);
        assert_eq!(frames_for(51_200), 8);
        assert_eq!(frames_for(200_000), 0);
    }
}
