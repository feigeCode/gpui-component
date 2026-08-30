use crate::input::EditorMode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, HighlightStyle, Hsla, Pixels, SharedString, WeakEntity,
};
use ropey::Rope;
use sum_tree::Bias;

use super::{InputBaseState, RopeExt as _};

/// A feature-owned marker anchored to a logical editor row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GutterMarker {
    id: SharedString,
    logical_row: usize,
    icon: SharedString,
    tooltip: Option<SharedString>,
    enabled: bool,
}

impl GutterMarker {
    pub fn new(
        id: impl Into<SharedString>,
        logical_row: usize,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            logical_row,
            icon: icon.into(),
            tooltip: None,
            enabled: true,
        }
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn logical_row(&self) -> usize {
        self.logical_row
    }

    pub fn icon(&self) -> &SharedString {
        &self.icon
    }

    pub fn tooltip(&self) -> Option<&SharedString> {
        self.tooltip.as_ref()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Application-owned presentation for a gutter marker.
pub type GutterMarkerRenderer = std::rc::Rc<dyn Fn(&GutterMarker) -> AnyElement>;

/// Geometric presentation for an editor range decoration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RangeDecorationStyle {
    /// Fill the continuous visual range.
    Fill,
    /// Draw a continuous one-pixel frame around the visual range.
    #[default]
    Frame,
}

/// A geometric decoration over a UTF-8 byte range.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeDecoration {
    id: SharedString,
    range: Range<usize>,
    style: RangeDecorationStyle,
    color: Option<Hsla>,
}

impl RangeDecoration {
    pub fn new(id: impl Into<SharedString>, range: Range<usize>) -> Self {
        Self {
            id: id.into(),
            range,
            style: RangeDecorationStyle::default(),
            color: None,
        }
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn range(&self) -> &Range<usize> {
        &self.range
    }

    pub fn style(&self) -> RangeDecorationStyle {
        self.style
    }

    pub fn color(&self) -> Option<Hsla> {
        self.color
    }

    pub fn with_style(mut self, style: RangeDecorationStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// Non-document text painted at a UTF-8 byte offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineWidget {
    id: SharedString,
    offset: usize,
    text: SharedString,
}

impl InlineWidget {
    pub fn new(id: impl Into<SharedString>, offset: usize, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            offset,
            text: text.into(),
        }
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn text(&self) -> &SharedString {
        &self.text
    }
}

#[derive(Default)]
pub(crate) struct EditorAnnotations {
    pub(crate) document_revision: u64,
    pub(crate) completion_epoch: u64,
    pub(crate) gutter_markers: Vec<GutterMarker>,
    pub(crate) gutter_lane_reserved: bool,
    pub(crate) gutter_marker_bounds: Rc<RefCell<HashMap<SharedString, Bounds<Pixels>>>>,
    pub(crate) range_decorations: Vec<RangeDecoration>,
    pub(crate) inline_widgets: Vec<InlineWidget>,
    pub(crate) gutter_marker_renderer: Option<GutterMarkerRenderer>,
}

impl EditorAnnotations {
    pub(crate) fn adjust_for_edit(&mut self, edited_range: &Range<usize>, inserted_len: usize) {
        for decoration in &mut self.range_decorations {
            decoration.range = adjust_range_for_edit(&decoration.range, edited_range, inserted_len);
        }
        self.range_decorations
            .retain(|decoration| !decoration.range.is_empty());
        for widget in &mut self.inline_widgets {
            widget.offset = adjust_offset_for_edit(widget.offset, edited_range, inserted_len);
        }
    }
}

fn adjust_offset_for_edit(
    offset: usize,
    edited_range: &Range<usize>,
    inserted_len: usize,
) -> usize {
    if offset <= edited_range.start {
        return offset;
    }
    if offset < edited_range.end {
        return edited_range.start.saturating_add(inserted_len);
    }
    let removed_len = edited_range.end.saturating_sub(edited_range.start);
    if inserted_len >= removed_len {
        offset.saturating_add(inserted_len - removed_len)
    } else {
        offset.saturating_sub(removed_len - inserted_len)
    }
}

/// A presentation style applied to a UTF-8 byte range in an input.
///
/// This is the GPUI [`HighlightStyle`] counterpart of Monaco's
/// [`IModelDeltaDecoration`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IModelDeltaDecoration.html).
#[derive(Clone, Debug, PartialEq)]
pub struct TextDecoration {
    pub range: Range<usize>,
    pub style: HighlightStyle,
}

impl TextDecoration {
    /// Create a text decoration from a UTF-8 byte range and a GPUI style.
    pub fn new(range: Range<usize>, style: HighlightStyle) -> Self {
        Self { range, style }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TextDecorationCollectionId(usize);

/// An independently managed collection of [`TextDecoration`]s.
///
/// This is the GPUI Component counterpart of Monaco's
/// [`IEditorDecorationsCollection`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html).
#[derive(Clone, Debug)]
pub struct TextDecorationCollection {
    state: WeakEntity<InputBaseState<EditorMode>>,
    id: TextDecorationCollectionId,
}

impl TextDecorationCollection {
    /// Replace all decorations in this collection.
    ///
    /// This corresponds to Monaco's
    /// [`IEditorDecorationsCollection.set`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#set).
    pub fn set(&self, decorations: Vec<TextDecoration>, cx: &mut App) {
        let _ = self.state.update(cx, |state, cx| {
            let decorations = normalize(&state.text, decorations);
            if state.extras.decorations.set(self.id, decorations) {
                cx.notify();
            }
        });
    }

    /// Add decorations to this collection.
    ///
    /// This corresponds to Monaco's
    /// [`IEditorDecorationsCollection.append`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#append).
    pub fn append(&self, decorations: Vec<TextDecoration>, cx: &mut App) {
        let _ = self.state.update(cx, |state, cx| {
            let decorations = normalize(&state.text, decorations);
            if state.extras.decorations.append(self.id, decorations) {
                cx.notify();
            }
        });
    }

    /// Remove all decorations from this collection.
    ///
    /// This corresponds to Monaco's
    /// [`IEditorDecorationsCollection.clear`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#clear).
    pub fn clear(&self, cx: &mut App) {
        self.set(Vec::new(), cx);
    }

    /// Return the UTF-8 byte ranges in this collection.
    ///
    /// This corresponds to Monaco's
    /// [`IEditorDecorationsCollection.getRanges`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IEditorDecorationsCollection.html#getRanges).
    pub fn get_ranges(&self, cx: &App) -> Vec<Range<usize>> {
        self.state
            .read_with(cx, |state, _| {
                state
                    .extras
                    .decorations
                    .get(self.id)
                    .unwrap_or_default()
                    .iter()
                    .map(|decoration| decoration.range.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Default)]
pub(crate) struct DecorationCollections {
    entries: Vec<(TextDecorationCollectionId, Vec<TextDecoration>)>,
}

impl DecorationCollections {
    fn create(&mut self, decorations: Vec<TextDecoration>) -> TextDecorationCollectionId {
        let id = TextDecorationCollectionId(self.entries.len());
        self.entries.push((id, decorations));
        id
    }

    fn set(&mut self, id: TextDecorationCollectionId, decorations: Vec<TextDecoration>) -> bool {
        let Some((_, current)) = self
            .entries
            .iter_mut()
            .find(|(entry_id, _)| *entry_id == id)
        else {
            return false;
        };
        *current = decorations;
        true
    }

    fn append(&mut self, id: TextDecorationCollectionId, decorations: Vec<TextDecoration>) -> bool {
        let Some((_, current)) = self
            .entries
            .iter_mut()
            .find(|(entry_id, _)| *entry_id == id)
        else {
            return false;
        };
        current.extend(decorations);
        true
    }

    fn get(&self, id: TextDecorationCollectionId) -> Option<&[TextDecoration]> {
        self.entries
            .iter()
            .find(|(entry_id, _)| *entry_id == id)
            .map(|(_, decorations)| decorations.as_slice())
    }

    pub(super) fn adjust_for_edit(&mut self, edited_range: &Range<usize>, inserted_len: usize) {
        for (_, decorations) in &mut self.entries {
            decorations.retain_mut(|decoration| {
                decoration.range =
                    adjust_range_for_edit(&decoration.range, edited_range, inserted_len);
                !decoration.range.is_empty()
            });
        }
    }

    pub(super) fn clear(&mut self) {
        for (_, decorations) in &mut self.entries {
            decorations.clear();
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &[TextDecoration]> {
        self.entries
            .iter()
            .map(|(_, decorations)| decorations.as_slice())
    }
}

fn adjust_range_for_edit(
    range: &Range<usize>,
    edited_range: &Range<usize>,
    inserted_len: usize,
) -> Range<usize> {
    let removed_len = edited_range.end.saturating_sub(edited_range.start);
    let shift = |offset: usize| {
        if inserted_len >= removed_len {
            offset.saturating_add(inserted_len - removed_len)
        } else {
            offset.saturating_sub(removed_len - inserted_len)
        }
    };

    if edited_range.is_empty() {
        let start = if range.start < edited_range.start {
            range.start
        } else {
            shift(range.start)
        };
        let end = if range.end <= edited_range.start {
            range.end
        } else {
            shift(range.end)
        };
        return start..end;
    }

    let inserted_end = edited_range.start + inserted_len;
    let start = if range.start <= edited_range.start {
        range.start
    } else if range.start >= edited_range.end {
        shift(range.start)
    } else {
        edited_range.start
    };
    let end = if range.end <= edited_range.start {
        range.end
    } else if range.end >= edited_range.end {
        shift(range.end)
    } else {
        inserted_end
    };
    start..end
}

fn normalize(text: &Rope, decorations: Vec<TextDecoration>) -> Vec<TextDecoration> {
    decorations
        .into_iter()
        .filter_map(|decoration| {
            let range = text.clip_offset(decoration.range.start, Bias::Left)
                ..text.clip_offset(decoration.range.end, Bias::Right);
            (!range.is_empty()).then_some(TextDecoration {
                range,
                style: decoration.style,
            })
        })
        .collect()
}

impl InputBaseState<EditorMode> {
    /// Monotonic content revision. Selection, focus and scrolling do not change it.
    pub fn document_revision(&self) -> u64 {
        self.extras.annotations.document_revision
    }

    /// Replace all gutter markers. The marker lane remains reserved after first use.
    pub fn set_gutter_markers(&mut self, markers: Vec<GutterMarker>, cx: &mut Context<Self>) {
        self.extras.annotations.gutter_markers = markers;
        self.extras.annotations.gutter_lane_reserved = true;
        cx.notify();
    }

    pub fn clear_gutter_markers(&mut self, cx: &mut Context<Self>) {
        if !self.extras.annotations.gutter_markers.is_empty() {
            self.extras.annotations.gutter_markers.clear();
            cx.notify();
        }
    }

    pub fn gutter_markers(&self) -> &[GutterMarker] {
        &self.extras.annotations.gutter_markers
    }

    pub fn gutter_marker_bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        self.extras
            .annotations
            .gutter_marker_bounds
            .borrow()
            .get(id)
            .copied()
    }

    pub fn set_gutter_marker_renderer(
        &mut self,
        renderer: GutterMarkerRenderer,
        cx: &mut Context<Self>,
    ) {
        self.extras.annotations.gutter_marker_renderer = Some(renderer);
        cx.notify();
    }

    #[doc(hidden)]
    pub fn project_gutter_marker_renderer(&mut self, renderer: GutterMarkerRenderer) {
        self.extras.annotations.gutter_marker_renderer = Some(renderer);
    }

    pub fn set_range_decorations(
        &mut self,
        decorations: Vec<RangeDecoration>,
        cx: &mut Context<Self>,
    ) {
        self.extras.annotations.range_decorations = normalize_ranges(&self.text, decorations);
        cx.notify();
    }

    pub fn clear_range_decorations(&mut self, cx: &mut Context<Self>) {
        if !self.extras.annotations.range_decorations.is_empty() {
            self.extras.annotations.range_decorations.clear();
            cx.notify();
        }
    }

    pub fn range_decorations(&self) -> &[RangeDecoration] {
        &self.extras.annotations.range_decorations
    }

    pub fn set_inline_widgets(&mut self, widgets: Vec<InlineWidget>, cx: &mut Context<Self>) {
        self.extras.annotations.inline_widgets = normalize_widgets(&self.text, widgets);
        cx.notify();
    }

    pub fn clear_inline_widgets(&mut self, cx: &mut Context<Self>) {
        if !self.extras.annotations.inline_widgets.is_empty() {
            self.extras.annotations.inline_widgets.clear();
            cx.notify();
        }
    }

    pub fn inline_widgets(&self) -> &[InlineWidget] {
        &self.extras.annotations.inline_widgets
    }

    /// Create an independently managed collection of text decorations.
    ///
    /// This follows Monaco's
    /// [`createDecorationsCollection`](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.ICodeEditor.html#createDecorationsCollection)
    /// ownership model. Ranges use UTF-8 byte offsets into [`Self::value`].
    ///
    /// Decoration ranges follow text edits and do not need to be set again
    /// after each change. Insertions at a range boundary do not expand the
    /// range, matching Monaco's
    /// [`NeverGrowsWhenTypingAtEdges`](https://microsoft.github.io/monaco-editor/typedoc/enums/editor_editor_api.editor.TrackedRangeStickiness.html#NeverGrowsWhenTypingAtEdges)
    /// behavior. Decorations are not rendered while the input is masked.
    /// Collections live until their [`InputBaseState`] is dropped.
    ///
    /// Collections are layered in insertion order; the first collection wins
    /// when overlapping decorations set the same [`HighlightStyle`] property.
    /// Callers should avoid conflicting overlaps within one collection.
    pub fn create_decorations_collection(
        &mut self,
        decorations: Vec<TextDecoration>,
        cx: &mut Context<Self>,
    ) -> TextDecorationCollection {
        let decorations = normalize(&self.text, decorations);
        let id = self.extras.decorations.create(decorations);
        cx.notify();
        TextDecorationCollection {
            state: cx.entity().downgrade(),
            id,
        }
    }
}

fn normalize_ranges(text: &Rope, decorations: Vec<RangeDecoration>) -> Vec<RangeDecoration> {
    decorations
        .into_iter()
        .filter_map(|mut decoration| {
            decoration.range = text.clip_offset(decoration.range.start, Bias::Left)
                ..text.clip_offset(decoration.range.end, Bias::Right);
            (!decoration.range.is_empty()).then_some(decoration)
        })
        .collect()
}

fn normalize_widgets(text: &Rope, widgets: Vec<InlineWidget>) -> Vec<InlineWidget> {
    widgets
        .into_iter()
        .map(|mut widget| {
            widget.offset = text.clip_offset(widget.offset, Bias::Left);
            widget
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collections_are_independent_and_ranges_are_clipped() {
        let text = Rope::from("héllo");
        let first_style = HighlightStyle {
            font_weight: Some(gpui::FontWeight::BOLD),
            ..Default::default()
        };
        let second_style = HighlightStyle {
            background_color: Some(gpui::red()),
            ..Default::default()
        };
        let mut collections = DecorationCollections::default();

        let first = collections.create(normalize(
            &text,
            vec![TextDecoration::new(2..4, first_style)],
        ));
        let second = collections.create(normalize(
            &text,
            vec![TextDecoration::new(5..100, second_style)],
        ));

        assert_ne!(first, second);
        assert_eq!(
            collections.get(first),
            Some(&[TextDecoration::new(1..4, first_style)][..])
        );
        assert_eq!(
            collections.get(second),
            Some(&[TextDecoration::new(5..6, second_style)][..])
        );

        assert!(collections.append(first, vec![TextDecoration::new(4..5, second_style)]));
        assert_eq!(
            collections.get(first),
            Some(
                &[
                    TextDecoration::new(1..4, first_style),
                    TextDecoration::new(4..5, second_style),
                ][..]
            )
        );

        assert!(collections.set(first, Vec::new()));
        assert_eq!(collections.get(first), Some(&[][..]));
        assert_eq!(
            collections.get(second),
            Some(&[TextDecoration::new(5..6, second_style)][..])
        );
    }

    #[test]
    fn decoration_ranges_follow_text_edits() {
        let style = HighlightStyle::default();
        let mut collections = DecorationCollections::default();
        let collection = collections.create(vec![TextDecoration::new(2..6, style)]);

        collections.adjust_for_edit(&(0..0), 2);
        assert_eq!(
            collections.get(collection),
            Some(&[TextDecoration::new(4..8, style)][..])
        );

        collections.adjust_for_edit(&(6..6), 2);
        assert_eq!(
            collections.get(collection),
            Some(&[TextDecoration::new(4..10, style)][..])
        );

        collections.adjust_for_edit(&(4..10), 3);
        assert_eq!(
            collections.get(collection),
            Some(&[TextDecoration::new(4..7, style)][..])
        );

        assert_eq!(adjust_range_for_edit(&(2..6), &(2..2), 2), 4..8);
        assert_eq!(adjust_range_for_edit(&(2..6), &(6..6), 2), 2..6);
        assert_eq!(adjust_range_for_edit(&(2..6), &(2..6), 3), 2..5);
    }

    #[test]
    fn geometric_decorations_and_widgets_follow_utf8_edits() {
        let mut annotations = EditorAnnotations {
            range_decorations: vec![RangeDecoration::new("range", 2..6)],
            inline_widgets: vec![InlineWidget::new("hint", 6, "hint")],
            ..Default::default()
        };

        annotations.adjust_for_edit(&(0..0), "é".len());
        assert_eq!(annotations.range_decorations[0].range(), &(4..8));
        assert_eq!(annotations.inline_widgets[0].offset(), 8);

        annotations.adjust_for_edit(&(5..7), 1);
        assert_eq!(annotations.range_decorations[0].range(), &(4..7));
        assert_eq!(annotations.inline_widgets[0].offset(), 7);
    }

    #[test]
    fn extension_ranges_and_offsets_clip_to_utf8_boundaries() {
        let text = Rope::from("éx");
        let decorations = normalize_ranges(&text, vec![RangeDecoration::new("range", 1..3)]);
        let widgets = normalize_widgets(&text, vec![InlineWidget::new("hint", 1, "hint")]);

        assert_eq!(decorations[0].range(), &(0..3));
        assert_eq!(widgets[0].offset(), 0);
    }
}
