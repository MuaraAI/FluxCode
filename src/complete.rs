use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, EditorState, HoverProvider, Rope};
use gpui_kit::{px, App, AppContext, Entity, Task, Window};
use lsp_types::{
    CompletionContext, CompletionResponse, Hover, InlineCompletionContext, InlineCompletionResponse,
    InlineCompletionTriggerKind,
};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(75);
const MAX_COMPLETION_BYTES: usize = 512 * 1024;

pub fn attach(
    editor: &Entity<EditorState>,
    lang: &'static str,
    path: &Path,
    tab_size: usize,
    copilot: crate::copilot::Copilot,
    cx: &mut App,
) {
    if matches!(lang, "text" | "markdown") {
        return;
    }
    let language_id = crate::copilot::language_id(lang);
    let prov = Rc::new(Provider {
        path: path.to_path_buf(),
        language_id,
        tab_size,
        copilot,
    });
    editor.update(cx, |state, cx| {
        state.lsp_mut().completion_menu.max_width = px(420.);
        state.lsp_mut().completion_provider = Some(prov.clone());
        state.lsp_mut().hover_provider = Some(prov);
        cx.notify();
    });
}

struct Provider {
    path: PathBuf,
    language_id: String,
    tab_size: usize,
    copilot: crate::copilot::Copilot,
}

impl Provider {
    fn is_manifest(&self) -> bool {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("manifest.json"))
    }

    fn is_script(&self) -> bool {
        matches!(
            self.language_id.as_str(),
            "javascript" | "typescript" | "javascriptreact" | "typescriptreact"
        )
    }
}

impl CompletionProvider for Provider {
    fn inline_completion(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: InlineCompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<InlineCompletionResponse>> {
        if !self.copilot.enabled() || rope.len() > MAX_COMPLETION_BYTES {
            return Task::ready(Ok(InlineCompletionResponse::Array(Vec::new())));
        }
        let copilot = self.copilot.clone();
        let path = self.path.clone();
        let language_id = self.language_id.clone();
        let tab_size = self.tab_size;
        let rope = rope.clone();
        let trigger_kind = if trigger.trigger_kind == InlineCompletionTriggerKind::Invoked {
            1
        } else {
            2
        };
        cx.background_spawn(async move {
            let text = rope.to_string();
            let item = copilot.inline(&path, &language_id, &text, offset, tab_size, trigger_kind);
            Ok(InlineCompletionResponse::Array(item.into_iter().collect()))
        })
    }

    fn inline_completion_debounce(&self) -> Duration {
        DEBOUNCE
    }

    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        _: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        if rope.len() > MAX_COMPLETION_BYTES {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        }
        let items = if self.is_manifest() {
            crate::ext::mcbe_types::complete_manifest(&rope.to_string(), offset)
        } else if self.is_script() {
            crate::ext::mcbe_types::complete_script(&rope.to_string(), offset, &self.path)
        } else {
            Vec::new()
        };
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        if !(self.is_manifest() || (self.is_script() && crate::ext::mcbe_types::is_workspace_active())) {
            return false;
        }
        new_text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '"' | '\'' | '/' | '@' | '{' | '_' | '-'))
    }
}

impl HoverProvider for Provider {
    fn hover(
        &self,
        rope: &Rope,
        offset: usize,
        _: &mut Window,
        _: &mut App,
    ) -> Task<Result<Option<Hover>>> {
        if !self.is_script() || rope.len() > MAX_COMPLETION_BYTES {
            return Task::ready(Ok(None));
        }
        let hov = crate::ext::mcbe_types::hover_script(&rope.to_string(), offset);
        Task::ready(Ok(hov))
    }
}
