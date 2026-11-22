//! Suggestions in the code editor come from GitHub Copilot only.

use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, EditorState, Rope};
use gpui_kit::{App, AppContext, Entity, Task, Window};
use lsp_types::{CompletionContext, CompletionResponse, InlineCompletionContext, InlineCompletionResponse, InlineCompletionTriggerKind};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// How long the editor waits after the last key before asking Copilot.
const DEBOUNCE: Duration = Duration::from_millis(75);

pub fn attach(editor: &Entity<EditorState>, lang: &'static str, path: &PathBuf, tab_size: usize, copilot: crate::copilot::Copilot, cx: &mut App) {
    // As in VS Code's defaults: no Copilot in plain text and Markdown prose.
    if matches!(lang, "text" | "markdown") {
        return;
    }
    let language_id = crate::copilot::language_id(lang);
    editor.update(cx, |state, cx| {
        state.lsp_mut().completion_provider = Some(Rc::new(Copilot { path: path.clone(), language_id, tab_size, copilot }));
        cx.notify();
    });
}

struct Copilot {
    /// Full path, for the Copilot server's document sync.
    path: PathBuf,
    language_id: String,
    tab_size: usize,
    copilot: crate::copilot::Copilot,
}

impl CompletionProvider for Copilot {
    /// Copilot ghost text.
    fn inline_completion(&self, rope: &Rope, offset: usize, trigger: InlineCompletionContext, _: &mut Window, cx: &mut App) -> Task<Result<InlineCompletionResponse>> {
        if !self.copilot.enabled() {
            return Task::ready(Ok(InlineCompletionResponse::Array(Vec::new())));
        }
        let copilot = self.copilot.clone();
        let path = self.path.clone();
        let language_id = self.language_id.clone();
        let tab_size = self.tab_size;
        let rope = rope.clone();
        // LSP: 1 = Invoked, 2 = Automatic.
        let trigger_kind = if trigger.trigger_kind == InlineCompletionTriggerKind::Invoked { 1 } else { 2 };
        cx.background_spawn(async move {
            // The whole-document copy runs here, off the UI thread.
            let text = rope.to_string();
            let item = copilot.inline(&path, &language_id, &text, offset, tab_size, trigger_kind);
            Ok(InlineCompletionResponse::Array(item.into_iter().collect()))
        })
    }

    fn inline_completion_debounce(&self) -> Duration {
        DEBOUNCE
    }

    /// No completion menu.
    fn completions(&self, _: &Rope, _: usize, _: CompletionContext, _: &mut Window, _: &mut App) -> Task<Result<CompletionResponse>> {
        Task::ready(Ok(CompletionResponse::Array(Vec::new())))
    }

    fn is_completion_trigger(&self, _offset: usize, _new_text: &str, _cx: &mut App) -> bool {
        false
    }
}
