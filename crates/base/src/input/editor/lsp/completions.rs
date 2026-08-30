use crate::input::EditorMode;
use anyhow::Result;
use gpui::{App, Context, EntityInputHandler, Pixels, Task, Window, px};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, InlineCompletionContext,
    InlineCompletionItem, InlineCompletionResponse, InlineCompletionTriggerKind,
    request::Completion,
};
use ropey::Rope;
use std::{cell::RefCell, ops::Range, rc::Rc, time::Duration};
use sum_tree::Bias;

use crate::input::{InputBaseState, RopeExt as _};

/// Default debounce duration for inline completions.
const DEFAULT_INLINE_COMPLETION_DEBOUNCE: Duration = Duration::from_millis(300);

fn completion_context_is_current(
    current_epoch: u64,
    request_epoch: u64,
    current_revision: u64,
    request_revision: u64,
    current_cursor: usize,
    request_cursor: usize,
    current_start: Option<usize>,
    request_start: usize,
    current_query: &str,
    request_query: &str,
) -> bool {
    current_epoch == request_epoch
        && current_revision == request_revision
        && current_cursor == request_cursor
        && current_start == Some(request_start)
        && current_query == request_query
}

/// Display options for the LSP completion popover.
///
/// Accessed through [`super::Lsp::completion_menu`] so embedders can tweak the
/// popover without growing the [`InputBaseState`] API.
#[derive(Debug, Clone, Copy)]
pub struct CompletionMenuOptions {
    /// Maximum width of the popover.
    ///
    /// Defaults to 320 px, which is fine for most identifiers but can
    /// truncate longer labels. Widen this when hosting an editor that
    /// surfaces long completion labels.
    pub max_width: Pixels,
}

impl Default for CompletionMenuOptions {
    fn default() -> Self {
        Self {
            max_width: px(320.),
        }
    }
}

/// A trait for providing code completions based on the current input state and context.
pub trait CompletionProvider {
    /// Fetches completions based on the given byte offset.
    ///
    /// - The `offset` is in bytes of current cursor.
    ///
    /// textDocument/completion
    ///
    /// https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_completion
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        trigger: CompletionContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>>;

    /// Fetches an inline completion suggestion for the given position.
    ///
    /// This is called after a debounce period when the user stops typing.
    /// The provider can analyze the text and cursor position to determine
    /// what inline completion suggestion to show.
    ///
    ///
    /// # Arguments
    /// * `rope` - The current text content
    /// * `offset` - The cursor position in bytes
    ///
    /// textDocument/inlineCompletion
    ///
    /// https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/#textDocument_inlineCompletion
    fn inline_completion(
        &self,
        _rope: &Rope,
        _offset: usize,
        _trigger: InlineCompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<Result<InlineCompletionResponse>> {
        Task::ready(Ok(InlineCompletionResponse::Array(vec![])))
    }

    /// Returns the debounce duration for inline completions.
    ///
    /// Default: 300ms
    #[inline]
    fn inline_completion_debounce(&self) -> Duration {
        DEFAULT_INLINE_COMPLETION_DEBOUNCE
    }

    fn resolve_completions(
        &self,
        _completion_indices: Vec<usize>,
        _completions: Rc<RefCell<Box<[Completion]>>>,
        _: &mut App,
    ) -> Task<Result<bool>> {
        Task::ready(Ok(false))
    }

    /// Determines if the completion should be triggered based on the given byte offset.
    ///
    /// This is called on the main thread.
    fn is_completion_trigger(&self, offset: usize, new_text: &str, cx: &mut App) -> bool;
}

pub(crate) struct InlineCompletion {
    /// Completion item to display as an inline completion suggestion
    pub(crate) item: Option<InlineCompletionItem>,
    /// Task for debouncing inline completion requests
    pub(crate) task: Task<Result<InlineCompletionResponse>>,
}

impl Default for InlineCompletion {
    fn default() -> Self {
        Self {
            item: None,
            task: Task::ready(Ok(InlineCompletionResponse::Array(vec![]))),
        }
    }
}

impl InputBaseState<EditorMode> {
    /// Re-run popup completion at the current cursor without changing the document.
    ///
    /// This is useful when an asynchronous metadata source becomes ready after
    /// the original request. Newline, tab, and line-start positions are ignored.
    pub fn refresh_completion_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.completion_inserting {
            return;
        }
        let cursor = self.cursor();
        let start = self.text.clip_offset(cursor.saturating_sub(1), Bias::Left);
        let Some(last_char) = self.text.char_at(start) else {
            return;
        };
        if !(last_char.is_ascii_alphanumeric()
            || matches!(last_char, '_' | '.' | ' ' | ')' | ']' | '"' | '\''))
        {
            return;
        }
        let text = self.text.slice(start..cursor).to_string();
        self.request_completion(&(start..start), &text, true, window, cx);
    }

    pub(crate) fn handle_completion_trigger(
        &mut self,
        range: &Range<usize>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_completion(range, new_text, false, window, cx);
    }

    fn request_completion(
        &mut self,
        range: &Range<usize>,
        new_text: &str,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.completion_inserting {
            return;
        }

        let Some(provider) = self.extras.lsp.completion_provider.clone() else {
            return;
        };

        // Always schedule inline completion (debounced).
        // It will check if menu is open before showing the suggestion.
        self.schedule_inline_completion(window, cx);

        let start = range.end;
        let new_offset = self.cursor();

        if !force && !provider.is_completion_trigger(start, new_text, cx) {
            return;
        }

        let start_offset = self
            .extras
            .context_menu_content
            .completion
            .trigger_start_offset
            .unwrap_or(start);
        if new_offset < start_offset {
            return;
        }

        let query = self
            .text_for_range(
                self.range_to_utf16(&(start_offset..new_offset)),
                &mut None,
                window,
                cx,
            )
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        self.extras
            .context_menu_content
            .completion
            .trigger_start_offset = Some(start_offset);
        self.extras
            .context_menu_content
            .completion
            .query
            .clone_from(&query);

        let completion_context = CompletionContext {
            trigger_kind: if force {
                lsp_types::CompletionTriggerKind::INVOKED
            } else {
                lsp_types::CompletionTriggerKind::TRIGGER_CHARACTER
            },
            trigger_character: Some(query),
        };

        let request_id = self.next_completion_request_id();
        let document_revision = self.document_revision();
        let query = self.extras.context_menu_content.completion.query.clone();
        let provider_responses =
            provider.completions(&self.text, new_offset, completion_context, window, cx);
        self.extras.context_menu_task = cx.spawn_in(window, async move |editor, cx| {
            let mut completions: Vec<CompletionItem> = vec![];
            if let Some(provider_responses) = provider_responses.await.ok() {
                match provider_responses {
                    CompletionResponse::Array(items) => completions.extend(items),
                    CompletionResponse::List(list) => completions.extend(list.items),
                }
            }

            if completions.is_empty() {
                editor.update_in(cx, |editor, window, cx| {
                    if !editor.completion_request_is_current(
                        request_id,
                        document_revision,
                        new_offset,
                        start_offset,
                        &query,
                        window,
                    ) {
                        return;
                    }
                    editor.extras.context_menu_content.completion.open = false;
                    editor.extras.context_menu_content.completion.items.clear();
                    editor.extras.context_menu_content.completion.bump();
                    cx.notify();
                })?;
                return Ok(());
            }

            editor
                .update_in(cx, |editor, window, cx| {
                    if !editor.completion_request_is_current(
                        request_id,
                        document_revision,
                        new_offset,
                        start_offset,
                        &query,
                        window,
                    ) {
                        return;
                    }

                    editor.extras.context_menu_content.completion.items = completions;
                    editor.extras.context_menu_content.completion.open = !editor
                        .extras
                        .context_menu_content
                        .completion
                        .items
                        .is_empty();
                    editor.extras.context_menu_content.completion.bump();

                    cx.notify();
                })
                .ok();

            Ok(())
        });
    }

    fn next_completion_request_id(&mut self) -> u64 {
        self.extras.annotations.completion_epoch =
            self.extras.annotations.completion_epoch.saturating_add(1);
        self.extras.annotations.completion_epoch
    }

    fn completion_request_is_current(
        &self,
        request_id: u64,
        document_revision: u64,
        cursor: usize,
        start_offset: usize,
        query: &str,
        window: &Window,
    ) -> bool {
        self.focus_handle.is_focused(window)
            && completion_context_is_current(
                self.extras.annotations.completion_epoch,
                request_id,
                self.document_revision(),
                document_revision,
                self.cursor(),
                cursor,
                self.extras
                    .context_menu_content
                    .completion
                    .trigger_start_offset,
                start_offset,
                &self.extras.context_menu_content.completion.query,
                query,
            )
    }

    pub(crate) fn hide_context_menu(&mut self, cx: &mut Context<Self>) {
        self.extras.context_menu_content.completion.open = false;
        self.extras.context_menu_content.code_action.open = false;
        self.extras.context_menu_task = Task::ready(Ok(()));
        cx.notify();
    }

    pub(crate) fn is_context_menu_open(&self, _cx: &gpui::App) -> bool {
        self.extras.context_menu_content.completion.open
            || self.extras.context_menu_content.code_action.open
    }

    pub(crate) fn handle_action_for_context_menu(
        &mut self,
        action: Box<dyn gpui::Action>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let closes_overlay =
            crate::input::Enter::is_primary(&*action) || action.partial_eq(&crate::input::Escape);
        let kind = if self.extras.context_menu_content.completion.open {
            Some(super::InputOverlayKind::Completion)
        } else if self.extras.context_menu_content.code_action.open {
            Some(super::InputOverlayKind::CodeAction)
        } else {
            None
        };
        let Some((kind, handler)) = kind.zip(self.overlay_action_handler.clone()) else {
            return false;
        };
        let handled = handler(kind, action, window, cx);
        if handled && closes_overlay {
            match kind {
                super::InputOverlayKind::Completion => {
                    self.extras.context_menu_content.completion.open = false
                }
                super::InputOverlayKind::CodeAction => {
                    self.extras.context_menu_content.code_action.open = false
                }
            }
            cx.notify();
        }
        handled
    }

    /// Schedule an inline completion request after debouncing.
    pub(crate) fn schedule_inline_completion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clear any existing inline completion on text change
        self.clear_inline_completion(cx);

        let Some(provider) = self.extras.lsp.completion_provider.clone() else {
            return;
        };

        let offset = self.cursor();
        let text = self.text.clone();
        let request_id = self.next_completion_request_id();
        let document_revision = self.document_revision();
        let debounce = provider.inline_completion_debounce();
        let background_executor = cx.background_executor().clone();

        self.extras.inline_completion.task = cx.spawn_in(window, async move |editor, cx| {
            // Debounce: wait before fetching to avoid unnecessary requests while typing
            background_executor.timer(debounce).await;

            // Now fetch the inline completion after the debounce period
            let task = editor.update_in(cx, |editor, window, cx| {
                // Check if cursor has moved during debounce
                if editor.extras.annotations.completion_epoch != request_id
                    || editor.document_revision() != document_revision
                    || editor.cursor() != offset
                    || editor.text != text
                {
                    return None;
                }

                // Don't fetch if completion menu is open
                if editor.is_context_menu_open(cx) {
                    return None;
                }

                let trigger = InlineCompletionContext {
                    trigger_kind: InlineCompletionTriggerKind::Automatic,
                    selected_completion_info: None,
                };

                Some(provider.inline_completion(&text, offset, trigger, window, cx))
            })?;

            let Some(task) = task else {
                return Ok(InlineCompletionResponse::Array(vec![]));
            };

            let response = task.await?;

            editor.update_in(cx, |editor, _window, cx| {
                // Only apply if cursor still hasn't moved
                if editor.extras.annotations.completion_epoch != request_id
                    || editor.document_revision() != document_revision
                    || editor.cursor() != offset
                    || editor.text != text
                {
                    return;
                }

                // Don't show if completion menu opened while we were fetching
                if editor.is_context_menu_open(cx) {
                    return;
                }

                if let Some(item) = match response.clone() {
                    InlineCompletionResponse::Array(items) => items.into_iter().next(),
                    InlineCompletionResponse::List(comp_list) => comp_list.items.into_iter().next(),
                } {
                    editor.extras.inline_completion.item = Some(item);
                    cx.notify();
                }
            })?;

            Ok(response)
        });
    }

    /// Check if an inline completion suggestion is currently displayed.
    #[inline]
    pub(crate) fn has_inline_completion(&self) -> bool {
        self.extras.inline_completion.item.is_some()
    }

    /// Clear the inline completion suggestion.
    pub(crate) fn clear_inline_completion(&mut self, cx: &mut Context<Self>) {
        self.extras.inline_completion = InlineCompletion::default();
        cx.notify();
    }

    /// Accept the inline completion, inserting it at the cursor position.
    /// Returns true if a completion was accepted, false if there was none.
    pub(crate) fn accept_inline_completion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(completion_item) = self.extras.inline_completion.item.take() else {
            return false;
        };

        let cursor = self.cursor();
        let range_utf16 = self.range_to_utf16(&(cursor..cursor));
        let completion_text = completion_item.insert_text;
        self.replace_text_in_range_silent(Some(range_utf16), &completion_text, window, cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::completion_context_is_current;

    #[test]
    fn completion_context_rejects_every_stale_dimension() {
        let current = |epoch, revision, cursor, start, query: &str| {
            completion_context_is_current(epoch, 7, revision, 11, cursor, 5, start, 2, query, "abc")
        };

        assert!(current(7, 11, 5, Some(2), "abc"));
        assert!(!current(8, 11, 5, Some(2), "abc"));
        assert!(!current(7, 12, 5, Some(2), "abc"));
        assert!(!current(7, 11, 6, Some(2), "abc"));
        assert!(!current(7, 11, 5, Some(1), "abc"));
        assert!(!current(7, 11, 5, Some(2), "abcd"));
    }
}
