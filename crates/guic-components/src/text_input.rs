use crate::{ComponentSize, TextChangeHandler};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement as _, Pixels, Point,
    Render, SharedString, StatefulInteractiveElement as _, Style, Styled as _, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role, ValueChange};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::{collections::VecDeque, ops::Range, rc::Rc};
use unicode_segmentation::UnicodeSegmentation;

mod pending;
use pending::{PendingAction, PendingInput};

actions!(
    guic_text_input,
    [
        /// Deletes the previous grapheme or selection.
        Backspace,
        /// Deletes the next grapheme or selection.
        Delete,
        /// Moves the cursor left.
        Left,
        /// Moves the cursor right.
        Right,
        /// Moves the cursor up one visual line.
        Up,
        /// Moves the cursor down one visual line.
        Down,
        /// Extends the selection up one visual line.
        SelectUp,
        /// Extends the selection down one visual line.
        SelectDown,
        /// Extends the selection left.
        SelectLeft,
        /// Extends the selection right.
        SelectRight,
        /// Selects all text.
        SelectAll,
        /// Moves to the beginning of the line.
        Home,
        /// Moves to the end of the line.
        End,
        /// Inserts a newline, or propagates submission for single-line inputs.
        Newline,
        /// Opens the native character palette.
        ShowCharacterPalette,
        /// Pastes text from the clipboard.
        Paste,
        /// Copies and removes the selected text.
        Cut,
        /// Copies the selected text.
        Copy,
        /// Undoes the last edit or IME composition.
        Undo,
        /// Reapplies the last undone edit.
        Redo,
    ]
);

struct TextInputKeyBindingsInstalled;

impl gpui::Global for TextInputKeyBindingsInstalled {}

/// Shared text-field variants.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextInputKind {
    /// A standard single-line text field.
    #[default]
    Text,
    /// A search-oriented text field with an inline search icon.
    Search,
    /// A masked password field.
    Password,
    /// A multi-line text field.
    TextArea,
}

/// A native text input component backed by GPUI's input handler.
pub struct TextInput {
    pending: Option<PendingInput>,
    input_generation: u64,
    id: SharedString,
    kind: TextInputKind,
    focus_handle: FocusHandle,
    value: String,
    placeholder: SharedString,
    accessible_label: Option<SharedString>,
    disabled: bool,
    size: ComponentSize,
    full_width: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    composition_start: Option<InputSnapshot>,
    undo: VecDeque<InputSnapshot>,
    redo: VecDeque<InputSnapshot>,
    last_layout: Option<TextLayoutState>,
    preferred_x: Option<Pixels>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    on_change: Option<TextChangeHandler>,
}

// Bound retained text as well as the number of edits. Large inputs must not
// multiply their memory use by the full snapshot count.
const HISTORY_LIMIT: usize = 100;
const HISTORY_BYTES: usize = 4 * 1024 * 1024;

struct InputSnapshot {
    value: String,
    selection: Range<usize>,
    reversed: bool,
    // Inverse replacements in edit order; restore applies them in reverse.
    inverse_edits: Vec<(Range<usize>, usize)>,
}

fn push_history(history: &mut VecDeque<InputSnapshot>, snapshot: InputSnapshot) {
    history.push_back(snapshot);
    let mut bytes = history.iter().map(|item| item.value.len()).sum::<usize>();
    while history.len() > HISTORY_LIMIT || bytes > HISTORY_BYTES {
        if let Some(oldest) = history.pop_front() {
            bytes -= oldest.value.len();
        }
    }
}

// Native ranges may exceed the document or land inside a surrogate pair.
// Round to the following scalar boundary, and clamp to the available text.
fn utf16_offset(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units >= offset {
            return index;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn utf16_range(text: &str, range: &Range<usize>) -> Range<usize> {
    let start = utf16_offset(text, range.start);
    start..utf16_offset(text, range.end).max(start)
}

impl EventEmitter<ValueChange<String>> for TextInput {}

struct TextLayoutState {
    lines: Vec<WrappedLine>,
    line_starts: Vec<usize>,
    line_height: Pixels,
}

/// Search input alias for [`TextInput`].
pub type SearchInput = TextInput;
/// Password input alias for [`TextInput`].
pub type PasswordInput = TextInput;
/// Multi-line text area alias for [`TextInput`].
pub type TextArea = TextInput;

impl TextInput {
    /// Creates a new single-line text input.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_kind(id, TextInputKind::Text, cx)
    }

    /// Creates a new search input.
    #[must_use]
    pub fn search(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_kind(id, TextInputKind::Search, cx)
    }

    /// Creates a new password input.
    #[must_use]
    pub fn password(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_kind(id, TextInputKind::Password, cx)
    }

    /// Creates a new multi-line text area.
    #[must_use]
    pub fn text_area(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_kind(id, TextInputKind::TextArea, cx)
    }

    fn with_kind(id: impl Into<SharedString>, kind: TextInputKind, cx: &mut Context<Self>) -> Self {
        Self {
            pending: None,
            input_generation: 0,
            id: id.into(),
            kind,
            focus_handle: cx.focus_handle().tab_stop(true),
            value: String::new(),
            placeholder: match kind {
                TextInputKind::Text => "Enter text".into(),
                TextInputKind::Search => "Search".into(),
                TextInputKind::Password => "Enter password".into(),
                TextInputKind::TextArea => "Write a longer note".into(),
            },
            accessible_label: None,
            disabled: false,
            size: ComponentSize::Medium,
            full_width: true,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            composition_start: None,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            last_layout: None,
            preferred_x: None,
            last_bounds: None,
            is_selecting: false,
            on_change: None,
        }
    }

    /// Replaces the placeholder text.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the label announced by platform accessibility services.
    #[must_use]
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Replaces the current value.
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.reset_value(value.into());
        self
    }

    /// Replaces the value of a retained input entity without emitting a change event.
    ///
    /// Programmatic replacement resets selection to the end and clears composition
    /// and undo/redo history. Use this for form resets or application-owned updates.
    pub fn set_value(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        self.reset_value(value.into());
        cx.notify();
    }

    fn reset_value(&mut self, value: String) {
        self.cancel_pending_input();
        self.value = self.normalize_inserted_text(&value);
        let cursor = self.value.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.composition_start = None;
        self.undo.clear();
        self.redo.clear();
        self.last_layout = None;
        self.preferred_x = None;
        self.is_selecting = false;
    }

    /// Sets the size of the text field.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Updates the size of a retained field without resetting text or history.
    pub fn set_size(&mut self, size: ComponentSize, cx: &mut Context<Self>) {
        if self.size != size {
            self.size = size;
            cx.notify();
        }
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Changes whether a retained input entity accepts user edits.
    ///
    /// Disabling commits the current composition to local history and ends mouse
    /// selection. Programmatic updates through [`Self::set_value`] remain allowed.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            if disabled {
                self.cancel_pending_input();
                self.finish_composition();
                self.is_selecting = false;
            }
            self.disabled = disabled;
            cx.notify();
        }
    }

    /// Makes the field fill the available width.
    #[must_use]
    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    /// Registers a change handler.
    #[must_use]
    pub fn on_change(mut self, on_change: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// Returns the current value.
    #[must_use]
    pub fn current_value(&self) -> &str {
        &self.value
    }

    fn text_metrics(&self, theme: &Theme) -> (Pixels, Pixels, Pixels, usize) {
        match (self.kind, self.size) {
            (TextInputKind::TextArea, ComponentSize::Small) => (
                px(96.0),
                px(theme.spacing.x3),
                px(theme.typography.text_sm),
                4,
            ),
            (TextInputKind::TextArea, ComponentSize::Medium) => (
                px(120.0),
                px(theme.spacing.x4),
                px(theme.typography.text_md),
                5,
            ),
            (TextInputKind::TextArea, ComponentSize::Large) => (
                px(148.0),
                px(theme.spacing.x5),
                px(theme.typography.text_lg),
                6,
            ),
            _ => {
                let metrics = self.size.control_metrics(theme);
                (
                    metrics.height,
                    metrics.horizontal_padding,
                    metrics.font_size,
                    1,
                )
            }
        }
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Left, window, cx) {
            return;
        }
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Right, window, cx) {
            return;
        }
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn vertical(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        if self.kind != TextInputKind::TextArea || self.marked_range.is_some() {
            cx.propagate();
            return;
        }
        if self.disabled {
            return;
        }
        let Some(layout) = self.last_layout.as_ref() else {
            return;
        };
        let Some(position) =
            layout.display_position_for_index(self.actual_to_display_offset(self.cursor_offset()))
        else {
            return;
        };
        let x = self.preferred_x.unwrap_or(position.x);
        let y = position.y + layout.line_height * if down { 1.5 } else { -0.5 };
        let offset = if y < px(0.0) {
            0
        } else {
            self.display_to_actual_offset(layout.closest_display_index_for_position(point(x, y)))
        };
        if select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.preferred_x = Some(x);
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Up, window, cx) {
            return;
        }
        self.vertical(false, false, cx);
    }
    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Down, window, cx) {
            return;
        }
        self.vertical(true, false, cx);
    }
    fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::SelectUp, window, cx) {
            return;
        }
        self.vertical(false, true, cx);
    }
    fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::SelectDown, window, cx) {
            return;
        }
        self.vertical(true, true, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::SelectLeft, window, cx) {
            return;
        }
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::SelectRight, window, cx) {
            return;
        }
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::SelectAll, window, cx) {
            return;
        }
        self.move_to(0, cx);
        self.select_to(self.value.len(), cx);
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Home, window, cx) {
            return;
        }
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::End, window, cx) {
            return;
        }
        self.move_to(self.value.len(), cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Newline, window, cx) {
            return;
        }
        if self.kind == TextInputKind::TextArea {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            cx.propagate();
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Backspace, window, cx) {
            return;
        }
        if self.disabled {
            return;
        }
        let range = self.marked_range.clone().unwrap_or_else(|| {
            if self.selected_range.is_empty() {
                self.previous_boundary(self.cursor_offset())..self.cursor_offset()
            } else {
                self.selected_range.clone()
            }
        });
        self.replace_text_in_range(Some(self.range_to_utf16(&range)), "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Delete, window, cx) {
            return;
        }
        if self.disabled {
            return;
        }
        let range = self.marked_range.clone().unwrap_or_else(|| {
            if self.selected_range.is_empty() {
                self.cursor_offset()..self.next_boundary(self.cursor_offset())
            } else {
                self.selected_range.clone()
            }
        });
        self.replace_text_in_range(Some(self.range_to_utf16(&range)), "", window, cx);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let handle = self.focus_handle(cx);
        window.focus(&handle, cx);

        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else if event.click_count >= 3 {
            let start = self.value[..offset]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            let end = self.value[offset..]
                .find('\n')
                .map_or(self.value.len(), |index| offset + index + 1);
            self.move_to(start, cx);
            self.select_to(end, cx);
        } else if event.click_count == 2 {
            let (start, text) = self
                .value
                .split_word_bound_indices()
                .find(|(start, word)| *start <= offset && offset < start + word.len())
                .unwrap_or((offset, ""));
            let end = start + text.len();
            self.move_to(start, cx);
            self.select_to(end, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.drain_pending_input(window, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
            self.drain_pending_input(window, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let read = cx.read_text_from_clipboard_async();
        self.queue_paste(read, window, cx);
    }

    fn copy(&mut self, _: &Copy, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Copy, window, cx) {
            return;
        }
        if self.kind == TextInputKind::Password {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.value[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Cut, window, cx) {
            return;
        }
        if self.disabled || self.kind == TextInputKind::Password {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.value[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.queue_pointer(offset..offset, false, cx) {
            return;
        }
        self.preferred_x = None;
        self.selection_reversed = false;
        self.selected_range = offset..offset;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if layout.lines.is_empty() {
            return 0;
        }
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.value.len();
        }

        let local = point(position.x - bounds.left(), position.y - bounds.top());
        let display_index = layout.closest_display_index_for_position(local);
        self.display_to_actual_offset(display_index)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if let Some(pending) = self.pending.as_ref().filter(|pending| !pending.replaying) {
            let (range, reversed) = pending
                .pointer
                .clone()
                .unwrap_or((self.selected_range.clone(), self.selection_reversed));
            let anchor = if reversed { range.end } else { range.start };
            self.queue_pointer(anchor.min(offset)..anchor.max(offset), offset < anchor, cx);
            return;
        }

        self.preferred_x = None;
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.value.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        utf16_range(&self.value, range_utf16)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.value
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.value
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.value.len())
    }

    fn display_text(&self) -> SharedString {
        match self.kind {
            TextInputKind::Password => "*".repeat(self.value.graphemes(true).count()).into(),
            TextInputKind::TextArea => self.value.clone().into(),
            TextInputKind::Text | TextInputKind::Search => self.value.clone().into(),
        }
    }

    fn display_to_actual_offset(&self, display_offset: usize) -> usize {
        match self.kind {
            TextInputKind::Password => self
                .value
                .grapheme_indices(true)
                .nth(display_offset)
                .map_or(self.value.len(), |(index, _)| index),
            _ => display_offset.min(self.value.len()),
        }
    }

    fn actual_to_display_offset(&self, actual_offset: usize) -> usize {
        match self.kind {
            TextInputKind::Password => self
                .value
                .grapheme_indices(true)
                .take_while(|(index, _)| *index < actual_offset)
                .count(),
            _ => actual_offset.min(self.value.len()),
        }
    }

    fn normalize_inserted_text(&self, text: &str) -> String {
        match self.kind {
            TextInputKind::TextArea => text.replace("\r\n", "\n").replace('\r', "\n"),
            TextInputKind::Text | TextInputKind::Search | TextInputKind::Password => {
                text.replace("\r\n", " ").replace(['\r', '\n'], " ")
            }
        }
    }

    fn emit_change(&self, previous: String, window: &mut Window, cx: &mut Context<Self>) {
        if previous == self.value {
            return;
        }
        if let Some(on_change) = &self.on_change {
            (on_change)(self.value.as_str(), window, cx);
        }
        cx.emit(ValueChange::new(previous, self.value.clone()));
    }

    fn snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            value: self.value.clone(),
            selection: self.selected_range.clone(),
            reversed: self.selection_reversed,
            inverse_edits: Vec::new(),
        }
    }

    fn record_edit(&mut self, before: InputSnapshot) {
        if before.value != self.value {
            push_history(&mut self.undo, before);
            self.redo.clear();
        }
    }

    fn finish_composition(&mut self) {
        self.marked_range = None;
        if let Some(before) = self.composition_start.take() {
            self.record_edit(before);
        }
    }

    fn restore(&mut self, snapshot: InputSnapshot, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pending) = &mut self.pending {
            for (range, inserted) in snapshot.inverse_edits.iter().rev() {
                pending.rebase(range, *inserted);
            }
        }
        let previous = std::mem::replace(&mut self.value, snapshot.value);
        self.selected_range = snapshot.selection;
        self.selection_reversed = snapshot.reversed;
        self.preferred_x = None;
        self.last_layout = None;
        self.emit_change(previous, window, cx);
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Undo, window, cx) {
            return;
        }
        if self.disabled {
            return;
        }
        self.finish_composition();
        if let Some(before) = self.undo.pop_back() {
            let mut current = self.snapshot();
            current.inverse_edits = before
                .inverse_edits
                .iter()
                .rev()
                .map(|(range, inserted)| (range.start..range.start + inserted, range.len()))
                .collect();
            push_history(&mut self.redo, current);
            self.restore(before, window, cx);
        }
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_input_action(PendingAction::Redo, window, cx) {
            return;
        }
        if self.disabled {
            return;
        }
        self.finish_composition();
        if let Some(after) = self.redo.pop_back() {
            let mut current = self.snapshot();
            current.inverse_edits = after
                .inverse_edits
                .iter()
                .rev()
                .map(|(range, inserted)| (range.start..range.start + inserted, range.len()))
                .collect();
            push_history(&mut self.undo, current);
            self.restore(after, window, cx);
        }
    }

    fn computed_line_height(&self, text_size: Pixels) -> Pixels {
        text_size + px(10.0)
    }
}

impl TextLayoutState {
    fn new(lines: Vec<WrappedLine>, line_height: Pixels) -> Self {
        let mut line_starts = Vec::with_capacity(lines.len());
        let mut offset = 0usize;
        for (index, line) in lines.iter().enumerate() {
            line_starts.push(offset);
            offset += line.text.len();
            if index + 1 < lines.len() {
                offset += 1;
            }
        }

        Self {
            lines,
            line_starts,
            line_height,
        }
    }

    fn line_y_offset(&self, line_index: usize) -> Pixels {
        self.lines
            .iter()
            .take(line_index)
            .fold(px(0.0), |height, line| {
                height + line.size(self.line_height).height
            })
    }

    fn line_end_index(&self, line_index: usize) -> usize {
        self.line_starts[line_index] + self.lines[line_index].text.len()
    }

    fn display_position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        for (line_index, line) in self.lines.iter().enumerate() {
            let line_start = self.line_starts[line_index];
            let line_end = self.line_end_index(line_index);
            if index <= line_end || line_index + 1 == self.lines.len() {
                let local_index = index.saturating_sub(line_start).min(line.text.len());
                let mut position = line.position_for_index(local_index, self.line_height)?;
                position.y += self.line_y_offset(line_index);
                return Some(position);
            }
        }

        None
    }

    fn closest_display_index_for_position(&self, position: Point<Pixels>) -> usize {
        let mut y_offset = px(0.0);
        for (line_index, line) in self.lines.iter().enumerate() {
            let line_height = line.size(self.line_height).height;
            let line_start = self.line_starts[line_index];
            if position.y <= y_offset + line_height || line_index + 1 == self.lines.len() {
                let local = point(position.x, position.y - y_offset);
                let local_index = line
                    .closest_index_for_position(local, self.line_height)
                    .unwrap_or_else(|index| index);
                return line_start + local_index;
            }
            y_offset += line_height;
        }

        0
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(pending) = &self.pending {
            let range = utf16_range(&pending.text, &range_utf16);
            actual_range.replace(pending.units(&range));
            return Some(pending.text[range].to_owned());
        }
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.value[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.disabled && !ignore_disabled_input {
            return None;
        }
        if let Some(pending) = &self.pending {
            return Some(UTF16Selection {
                range: pending.units(&pending.selected),
                reversed: false,
            });
        }
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        if let Some(pending) = &self.pending {
            return pending.marked.as_ref().map(|range| pending.units(range));
        }
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            self.commit_pending_text();
            self.drain_pending_input(_window, cx);
            cx.notify();
            return;
        }
        self.finish_composition();
        cx.notify();
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        !self.disabled
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some(pending) = &self.pending {
            return Some(pending.text.encode_utf16().count());
        }
        Some(self.value.encode_utf16().count())
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        if let Some(pending) = &mut self.pending {
            pending.selected = utf16_range(&pending.text, &range_utf16);
            cx.notify();
            return;
        }
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        self.preferred_x = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        if self
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.replaying)
        {
            let normalized = self.normalize_inserted_text(new_text);
            if let Some(pending) = &mut self.pending {
                pending.replace(range_utf16, &normalized, None, false);
            }
            self.commit_pending_text();
            self.drain_pending_input(window, cx);
            cx.notify();
            return;
        }
        let mut before = self
            .composition_start
            .take()
            .unwrap_or_else(|| self.snapshot());
        self.preferred_x = None;
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let replacement = self.normalize_inserted_text(new_text);
        if self.value[range.clone()] != replacement {
            before
                .inverse_edits
                .push((range.start..range.start + replacement.len(), range.len()));
            if let Some(pending) = &mut self.pending {
                pending.rebase(&range, replacement.len());
            }
        }

        let previous = self.value.clone();
        self.value =
            self.value[0..range.start].to_owned() + &replacement + &self.value[range.end..];
        let cursor = range.start + replacement.len();
        self.selected_range = cursor..cursor;
        self.marked_range.take();
        self.selection_reversed = false;
        self.record_edit(before);
        self.emit_change(previous, window, cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        if self
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.replaying)
        {
            let normalized = self.normalize_inserted_text(new_text);
            let selection = new_selected_range_utf16.map(|range: Range<usize>| {
                let raw = utf16_range(new_text, &range);
                self.normalize_inserted_text(&new_text[..raw.start])
                    .encode_utf16()
                    .count()
                    ..self
                        .normalize_inserted_text(&new_text[..raw.end])
                        .encode_utf16()
                        .count()
            });
            if let Some(pending) = &mut self.pending {
                pending.replace(range_utf16, &normalized, selection, true);
            }
            if normalized.is_empty() {
                self.commit_pending_text();
                self.drain_pending_input(window, cx);
            }
            cx.notify();
            return;
        }
        if self.composition_start.is_none() {
            self.composition_start = Some(self.snapshot());
        }
        self.preferred_x = None;
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let replacement = self.normalize_inserted_text(new_text);
        if self.value[range.clone()] != replacement {
            if let Some(before) = &mut self.composition_start {
                before
                    .inverse_edits
                    .push((range.start..range.start + replacement.len(), range.len()));
            }
            if let Some(pending) = &mut self.pending {
                pending.rebase(&range, replacement.len());
            }
        }

        let previous = self.value.clone();
        self.value =
            self.value[0..range.start].to_owned() + &replacement + &self.value[range.end..];
        if replacement.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(range.start..range.start + replacement.len());
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| {
                let raw = utf16_range(new_text, range_utf16);
                self.normalize_inserted_text(&new_text[..raw.start]).len()
                    ..self.normalize_inserted_text(&new_text[..raw.end]).len()
            })
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| {
                let cursor = range.start + replacement.len();
                cursor..cursor
            });

        self.selection_reversed = false;
        if replacement.is_empty() {
            self.finish_composition();
        }
        self.emit_change(previous, window, cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = if self.pending.is_some() {
            self.cursor_offset()..self.cursor_offset()
        } else {
            self.range_from_utf16(&range_utf16)
        };
        let display_start = self.actual_to_display_offset(range.start);
        let display_end = self.actual_to_display_offset(range.end);
        let start = last_layout.display_position_for_index(display_start)?;
        let end = last_layout.display_position_for_index(display_end)?;
        Some(Bounds::from_corners(
            point(bounds.left() + start.x, bounds.top() + start.y),
            point(
                bounds.left() + end.x,
                bounds.top() + end.y + last_layout.line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let last_layout = self.last_layout.as_ref()?;
        let local = bounds.localize(&point)?;
        if let Some(pending) = &self.pending {
            return Some(pending.units(&pending.selected).end);
        }
        let display_index = last_layout.closest_display_index_for_position(local);
        Some(self.offset_to_utf16(self.display_to_actual_offset(display_index)))
    }
}

struct TextInputElement {
    input: Entity<TextInput>,
    id: SharedString,
}

struct PrepaintState {
    layout: Option<TextLayoutState>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name(self.id.clone()))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let (_, _, text_size, visible_lines) = input.text_metrics(Theme::global(cx));
        let line_height = input.computed_line_height(text_size);
        let visible_lines = visible_lines as f32;
        let mut style = Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = (line_height * visible_lines).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = Theme::global(cx);
        let input = self.input.read(cx);
        let display_text = input.display_text();
        let content_is_placeholder = display_text.is_empty();
        let display_text = if content_is_placeholder {
            input.placeholder.clone()
        } else {
            display_text
        };
        let style = window.text_style();
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: if input.disabled || content_is_placeholder {
                theme.muted_foreground()
            } else {
                style.color
            },
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            let display_start = input.actual_to_display_offset(marked_range.start);
            let display_end = input.actual_to_display_offset(marked_range.end);
            vec![
                TextRun {
                    len: display_start,
                    ..run.clone()
                },
                TextRun {
                    len: display_end.saturating_sub(display_start),
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len().saturating_sub(display_end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = input.computed_line_height(font_size);
        let wrap_width = (input.kind == TextInputKind::TextArea).then_some(bounds.size.width);
        let lines = window
            .text_system()
            .shape_text(display_text, font_size, &runs, wrap_width, None)
            .expect("text input layout should shape");
        let layout = TextLayoutState::new(lines.into_vec(), line_height);

        let preview_selection = input
            .pending
            .as_ref()
            .and_then(|pending| pending.pointer.as_ref())
            .map(|(range, _)| range)
            .unwrap_or(&input.selected_range);
        let cursor_offset = input
            .pending
            .as_ref()
            .and_then(|pending| pending.pointer.as_ref())
            .map(|(range, reversed)| if *reversed { range.start } else { range.end })
            .unwrap_or_else(|| input.cursor_offset());
        let cursor_index = input.actual_to_display_offset(cursor_offset);
        let selected_start = input.actual_to_display_offset(preview_selection.start);
        let selected_end = input.actual_to_display_offset(preview_selection.end);
        let (selection, cursor) = if preview_selection.is_empty() {
            let cursor_pos = layout
                .display_position_for_index(cursor_index)
                .unwrap_or(point(px(0.0), px(0.0)));
            (
                Vec::new(),
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos.x, bounds.top() + cursor_pos.y),
                        gpui::size(px(2.0), line_height),
                    ),
                    theme.primary(),
                )),
            )
        } else {
            let mut quads = Vec::new();
            for line_index in 0..layout.lines.len() {
                let line_start = layout.line_starts[line_index];
                let line_end = layout.line_end_index(line_index);
                let start = selected_start.max(line_start);
                let end = selected_end.min(line_end);
                if start >= end {
                    continue;
                }

                let start_pos = layout
                    .display_position_for_index(start)
                    .unwrap_or(point(px(0.0), px(0.0)));
                let end_pos = layout
                    .display_position_for_index(end)
                    .unwrap_or(point(px(0.0), start_pos.y));

                quads.push(fill(
                    Bounds::from_corners(
                        point(bounds.left() + start_pos.x, bounds.top() + start_pos.y),
                        point(
                            bounds.left() + end_pos.x.max(start_pos.x + px(2.0)),
                            bounds.top() + start_pos.y + line_height,
                        ),
                    ),
                    theme.primary().opacity(0.18),
                ));
            }
            (quads, None)
        };

        PrepaintState {
            layout: Some(layout),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, disabled) = {
            let input = self.input.read(cx);
            (input.focus_handle.clone(), input.disabled)
        };

        if !disabled {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.input.clone()),
                cx,
            );
        }

        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }

        let layout = prepaint
            .layout
            .take()
            .expect("text input layout should exist");
        let mut y_offset = px(0.0);
        for line in &layout.lines {
            let _ = line.paint(
                point(bounds.origin.x, bounds.origin.y + y_offset),
                layout.line_height,
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
            y_offset += line.size(layout.line_height).height;
        }

        if focus_handle.is_focused(window)
            && !disabled
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(layout);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (height, padding_x, text_size, visible_lines) = self.text_metrics(theme);
        let line_height = self.computed_line_height(text_size);
        let focused = self.focus_handle.is_focused(_window);
        let border = if focused {
            theme.ring()
        } else {
            theme.border()
        };
        let accessible_label = self
            .accessible_label
            .clone()
            .unwrap_or_else(|| self.placeholder.clone());
        let mut field = div()
            .id(self.id.clone())
            .debug_selector(|| format!("guic-text-input-{}", self.id))
            .accessible(
                AccessibilityProps::new(match self.kind {
                    TextInputKind::Password => Role::PasswordInput,
                    TextInputKind::TextArea => Role::MultilineTextInput,
                    _ => Role::TextInput,
                })
                .label(accessible_label)
                .disabled(self.disabled),
            )
            .key_context("GuicTextInput")
            .track_focus(&self.focus_handle(cx).tab_stop(!self.disabled))
            .cursor(if self.disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .line_height(line_height)
            .text_size(text_size)
            .px(padding_x)
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(border)
            .bg(theme.background())
            .text_color(theme.foreground())
            .flex()
            .items_center()
            .gap_2()
            .hover({
                let hover = theme.secondary().opacity(0.25);
                move |style: gpui::StyleRefinement| style.bg(hover)
            });

        field = field.aria_placeholder(self.placeholder.clone());
        if self.kind != TextInputKind::Password {
            field = field.aria_value(self.value.clone());
        }
        if !self.disabled {
            field = field
                .on_a11y_action(gpui::accesskit::Action::SetValue, {
                    let input = cx.entity().downgrade();
                    move |data, window, cx| {
                        if let Some(gpui::accesskit::ActionData::Value(value)) = data {
                            let _ = input.update(cx, |input, cx| {
                                let end = input.value.encode_utf16().count();
                                input.replace_text_in_range(Some(0..end), value, window, cx);
                            });
                        }
                    }
                })
                .on_action(cx.listener(Self::undo))
                .on_action(cx.listener(Self::redo))
                .on_action(cx.listener(Self::backspace))
                .on_action(cx.listener(Self::delete))
                .on_action(cx.listener(Self::up))
                .on_action(cx.listener(Self::down))
                .on_action(cx.listener(Self::select_up))
                .on_action(cx.listener(Self::select_down))
                .on_action(cx.listener(Self::left))
                .on_action(cx.listener(Self::right))
                .on_action(cx.listener(Self::select_left))
                .on_action(cx.listener(Self::select_right))
                .on_action(cx.listener(Self::select_all))
                .on_action(cx.listener(Self::home))
                .on_action(cx.listener(Self::end))
                .on_action(cx.listener(Self::newline))
                .on_action(cx.listener(Self::show_character_palette))
                .on_action(cx.listener(Self::paste))
                .on_action(cx.listener(Self::cut))
                .on_action(cx.listener(Self::copy))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                .on_mouse_move(cx.listener(Self::on_mouse_move));
        }

        if let Some(pending) = &self.pending {
            let preview = if self.kind == TextInputKind::Password {
                "*".repeat(pending.text.graphemes(true).take(64).count())
            } else {
                pending.text.chars().take(64).collect()
            };
            field = field.child(
                gpui::div()
                    .text_color(theme.muted_foreground())
                    .child(format!(
                        "{} {preview}",
                        guic_core::translate(cx, "input.pasting", "Pasting...")
                    )),
            );
        }
        if self.full_width {
            field = field.w_full();
        }

        if self.kind == TextInputKind::Search {
            field = field.child(Icon::new(IconName::Search).color(theme.muted_foreground()));
        }

        if self.kind == TextInputKind::TextArea {
            field = field.items_start().py(px(theme.spacing.x2)).min_h(height);
        } else {
            field = field.h(height).flex_shrink_0();
        }

        let input_element = div()
            .flex_1()
            .min_h(line_height * visible_lines as f32)
            .child(TextInputElement {
                input: cx.entity(),
                id: self.id.clone(),
            });

        field = field.child(input_element);

        if self.disabled {
            field.opacity(0.55)
        } else {
            field
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub(crate) fn init_key_bindings(cx: &mut App) {
    if !cx.has_global::<TextInputKeyBindingsInstalled>() {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some("GuicTextInput")),
            KeyBinding::new("delete", Delete, Some("GuicTextInput")),
            KeyBinding::new("left", Left, Some("GuicTextInput")),
            KeyBinding::new("right", Right, Some("GuicTextInput")),
            KeyBinding::new("up", Up, Some("GuicTextInput")),
            KeyBinding::new("down", Down, Some("GuicTextInput")),
            KeyBinding::new("shift-up", SelectUp, Some("GuicTextInput")),
            KeyBinding::new("shift-down", SelectDown, Some("GuicTextInput")),
            KeyBinding::new("enter", Newline, Some("GuicTextInput")),
            KeyBinding::new("shift-left", SelectLeft, Some("GuicTextInput")),
            KeyBinding::new("shift-right", SelectRight, Some("GuicTextInput")),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                SelectAll,
                Some("GuicTextInput"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-v"
                } else {
                    "ctrl-v"
                },
                Paste,
                Some("GuicTextInput"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-c"
                } else {
                    "ctrl-c"
                },
                Copy,
                Some("GuicTextInput"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-x"
                } else {
                    "ctrl-x"
                },
                Cut,
                Some("GuicTextInput"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-z"
                } else {
                    "ctrl-z"
                },
                Undo,
                Some("GuicTextInput"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-shift-z"
                } else {
                    "ctrl-shift-z"
                },
                Redo,
                Some("GuicTextInput"),
            ),
            KeyBinding::new("home", Home, Some("GuicTextInput")),
            KeyBinding::new("end", End, Some("GuicTextInput")),
        ]);
        #[cfg(not(target_os = "macos"))]
        cx.bind_keys([KeyBinding::new("ctrl-y", Redo, Some("GuicTextInput"))]);
        #[cfg(target_os = "macos")]
        cx.bind_keys([KeyBinding::new(
            "ctrl-cmd-space",
            ShowCharacterPalette,
            Some("GuicTextInput"),
        )]);
        cx.set_global(TextInputKeyBindingsInstalled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Keystroke, TestAppContext, VisualContext as _};

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    fn painting_does_not_notify_observers(cx: &mut TestAppContext) {
        init(cx);
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("idle", cx).value("idle"));
        let notifications = Rc::new(std::cell::Cell::new(0));
        let observed = notifications.clone();
        let _subscription =
            cx.update(|_, cx| cx.observe(&input, move |_, _| observed.set(observed.get() + 1)));
        for _ in 0..3 {
            cx.draw(
                point(px(0.0), px(0.0)),
                gpui::size(px(400.0), px(100.0)),
                |_, _| input.clone().into_any_element(),
            );
        }
        assert_eq!(notifications.get(), 0);
    }

    #[gpui::test]
    #[allow(
        clippy::reversed_empty_ranges,
        reason = "Exercise malformed native input ranges"
    )]
    fn native_ranges_and_normalized_composition_stay_in_bounds(cx: &mut TestAppContext) {
        init(cx);
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("ime", cx).value("prefix"));
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "a\r\n日本", Some(3..4), window, cx);
                assert_eq!(input.current_value(), "prefixa 日本");
                assert_eq!(input.selected_range, 8..11);
                input.replace_and_mark_text_in_range(None, "🙂", Some(1..usize::MAX), window, cx);
                assert_eq!(input.selected_range, 10..10);
                input.unmark_text(window, cx);
                let mut actual = None;
                assert_eq!(
                    input.text_for_range(usize::MAX..0, &mut actual, window, cx),
                    Some(String::new())
                );
                assert_eq!(actual, Some(8..8));
                input.set_selected_text_range(6..usize::MAX, window, cx);
                assert_eq!(input.selected_range, 6..10);
                assert_eq!(input.text_length_utf16(window, cx), Some(8));
                input.replace_text_in_range(Some(usize::MAX..0), "!", window, cx);
                assert_eq!(input.current_value(), "prefix🙂!");
            });
        });
    }

    #[gpui::test]
    fn undo_groups_composition_and_preserves_selection(cx: &mut TestAppContext) {
        init(cx);
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("ime", cx).value("abc"));
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.selected_range = 1..3;
                input.selection_reversed = true;
                input.replace_and_mark_text_in_range(None, "n", Some(1..1), window, cx);
                input.replace_and_mark_text_in_range(None, "日本", Some(2..2), window, cx);
                input.replace_text_in_range(None, "日本", window, cx);
                assert_eq!(input.undo.len(), 1);
                assert!(!input.selection_reversed);
                input.undo(&Undo, window, cx);
                assert_eq!(input.current_value(), "abc");
                assert_eq!(input.selected_range, 1..3);
                assert!(input.selection_reversed);
                input.redo(&Redo, window, cx);
                assert_eq!(input.current_value(), "a日本");
                assert_eq!(input.selected_range, 7..7);
                input.undo(&Undo, window, cx);
                input.replace_text_in_range(None, "x", window, cx);
                input.redo(&Redo, window, cx);
                assert_eq!(input.current_value(), "ax");
            });
        });
    }

    #[gpui::test]
    fn cancelled_composition_preserves_redo_and_unmark_commits(cx: &mut TestAppContext) {
        init(cx);
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("ime", cx));
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, "first", window, cx);
                input.undo(&Undo, window, cx);
                input.replace_and_mark_text_in_range(None, "n", Some(1..1), window, cx);
                input.replace_and_mark_text_in_range(None, "", None, window, cx);
                input.redo(&Redo, window, cx);
                assert_eq!(input.current_value(), "first");
                input.replace_and_mark_text_in_range(None, "日本", None, window, cx);
                input.unmark_text(window, cx);
                input.undo(&Undo, window, cx);
                assert_eq!(input.current_value(), "first");
            });
        });
    }

    #[gpui::test]
    fn native_undo_redo_shortcuts_restore_grapheme_deletion(cx: &mut TestAppContext) {
        init(cx);
        let text = "a👨‍👩‍👧‍👦";
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("edit", cx).value(text));
        cx.update(|window, cx| input.read(cx).focus_handle.clone().focus(window, cx));
        let modifier = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        let window = cx.window_handle();
        for (key, expected) in [
            ("backspace".to_owned(), "a"),
            (format!("{modifier}-z"), text),
            (format!("{modifier}-shift-z"), "a"),
        ] {
            cx.dispatch_keystroke(window, Keystroke::parse(&key).expect("shortcut"));
            input.update(cx, |input, _| {
                assert_eq!(input.current_value(), expected);
                assert_eq!(input.selected_range, expected.len()..expected.len());
            });
        }
    }

    #[gpui::test]
    fn disabled_input_rejects_native_and_action_edits(cx: &mut TestAppContext) {
        init(cx);
        let (input, cx) = cx.add_window_view(|_, cx| {
            TextInput::new("disabled", cx)
                .value("original")
                .disabled(true)
        });
        cx.update(|window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("clipboard".into()));
            input.update(cx, |input, cx| {
                input.selected_range = 0..8;
                input.replace_text_in_range(None, "changed", window, cx);
                input.replace_and_mark_text_in_range(None, "marked", None, window, cx);
                input.set_selected_text_range(0..0, window, cx);
                input.cut(&Cut, window, cx);
                input.paste(&Paste, window, cx);
                input.backspace(&Backspace, window, cx);
                input.delete(&Delete, window, cx);
                input.undo(&Undo, window, cx);
                input.redo(&Redo, window, cx);
                assert_eq!(input.current_value(), "original");
                assert_eq!(input.selected_range, 0..8);
                assert!(!input.accepts_text_input(window, cx));
                assert!(input.selected_text_range(false, window, cx).is_none());
                assert!(input.selected_text_range(true, window, cx).is_some());
                assert!(input.undo.is_empty());
                assert_eq!(
                    cx.read_from_clipboard()
                        .and_then(|item| item.text())
                        .as_deref(),
                    Some("clipboard")
                );
            });
        });
    }

    #[gpui::test]
    fn line_endings_and_noop_edits(cx: &mut TestAppContext) {
        init(cx);
        for kind in [TextInputKind::Text, TextInputKind::TextArea] {
            let changes = Rc::new(std::cell::Cell::new(0));
            let observed = changes.clone();
            let (input, cx) = cx.add_window_view(|_, cx| {
                TextInput::with_kind("lines", kind, cx)
                    .on_change(move |_, _, _| observed.set(observed.get() + 1))
            });
            cx.update(|window, cx| {
                input.update(cx, |input, cx| {
                    input.replace_text_in_range(None, "a\r\nb\rc\n", window, cx);
                    let expected = if kind == TextInputKind::TextArea {
                        "a\nb\nc\n"
                    } else {
                        "a b c "
                    };
                    assert_eq!(input.current_value(), expected);
                    input.replace_text_in_range(None, "", window, cx);
                    input.delete(&Delete, window, cx);
                    assert_eq!(changes.get(), 1);
                    assert_eq!(input.undo.len(), 1);
                });
            });
        }
    }

    #[test]
    fn history_is_bounded_by_count_and_text_bytes() {
        let snapshot = |value: String| InputSnapshot {
            value,
            selection: 0..0,
            reversed: false,
            inverse_edits: Vec::new(),
        };
        let mut history = VecDeque::new();
        for _ in 0..HISTORY_LIMIT + 5 {
            push_history(&mut history, snapshot("small".into()));
        }
        assert_eq!(history.len(), HISTORY_LIMIT);
        push_history(&mut history, snapshot("a".repeat(HISTORY_BYTES)));
        assert_eq!(history.len(), 1);
        push_history(&mut history, snapshot("b".repeat(HISTORY_BYTES + 1)));
        assert!(history.is_empty());
    }

    #[gpui::test]
    fn retained_input_updates_reset_history_without_emitting_changes(cx: &mut TestAppContext) {
        init(cx);
        let changes = Rc::new(std::cell::Cell::new(0));
        let observed = changes.clone();
        let (input, cx) = cx.add_window_view(|_, cx| {
            TextInput::new("form", cx)
                .value("initial")
                .on_change(move |_, _, _| observed.set(observed.get() + 1))
        });
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                let focus = input.focus_handle.clone();
                input.replace_and_mark_text_in_range(None, "pending", None, window, cx);
                input.set_disabled(true, cx);
                assert!(input.marked_range.is_none());
                assert_eq!(input.undo.len(), 1);
                input.set_value("reset\r\nvalue", cx);
                assert_eq!(input.current_value(), "reset value");
                assert_eq!(input.selected_range, 11..11);
                assert!(input.undo.is_empty());
                assert!(input.redo.is_empty());
                assert_eq!(input.focus_handle, focus);
                assert_eq!(changes.get(), 1);
                input.set_disabled(false, cx);
                input.replace_text_in_range(None, "!", window, cx);
                assert_eq!(input.current_value(), "reset value!");
                assert_eq!(changes.get(), 2);
            });
        });
    }

    #[gpui::test]
    fn composition_selection_is_relative_to_replacement(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("ime", cx).value("🙂"));
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "a", Some(1..1), window, cx);
                assert_eq!(input.current_value(), "🙂a");
                assert_eq!(input.selected_range, 5..5);
                input.replace_and_mark_text_in_range(None, "日本", Some(1..2), window, cx);
                assert_eq!(input.selected_range, 7..10);
                input.replace_text_in_range(None, "日本語", window, cx);
                assert_eq!(input.current_value(), "🙂日本語");
                assert_eq!(input.selected_range, 13..13);
            });
        });
    }

    #[gpui::test]
    fn password_shortcuts_preserve_clipboard_and_allow_paste(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
            cx.write_to_clipboard(ClipboardItem::new_string("replacement".into()));
        });
        let secret = "private-\u{65e5}\u{672c}-\u{1f642}";
        let (input, cx) =
            cx.add_window_view(|_, cx| TextInput::password("password-clipboard", cx).value(secret));
        cx.update(|window, cx| input.read(cx).focus_handle.clone().focus(window, cx));
        let modifier = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        let window = cx.window_handle();
        for key in ["a", "c", "x"] {
            cx.dispatch_keystroke(
                window,
                Keystroke::parse(&format!("{modifier}-{key}")).expect("editing shortcut"),
            );
            cx.update(|_, cx| {
                assert_eq!(input.read(cx).current_value(), secret);
                assert_eq!(input.read(cx).selected_range, 0..secret.len());
                assert_eq!(
                    cx.read_from_clipboard()
                        .and_then(|item| item.text())
                        .as_deref(),
                    Some("replacement"),
                );
            });
        }
        cx.dispatch_keystroke(
            window,
            Keystroke::parse(&format!("{modifier}-v")).expect("paste shortcut"),
        );
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(input.read(cx).current_value(), "replacement"));
        cx.dispatch_keystroke(
            window,
            Keystroke::parse(&format!("{modifier}-z")).expect("undo shortcut"),
        );
        cx.run_until_parked();
        cx.update(|_, cx| assert_eq!(input.read(cx).current_value(), secret));
    }

    #[gpui::test]
    fn native_editing_shortcuts_copy_cut_and_paste(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        for kind in [
            TextInputKind::Text,
            TextInputKind::Search,
            TextInputKind::TextArea,
        ] {
            let (input, cx) = cx.add_window_view(|_, cx| {
                TextInput::with_kind("edit", kind, cx).value("hello \u{65e5}\u{672c}\u{8a9e}")
            });
            cx.update(|window, cx| input.read(cx).focus_handle.clone().focus(window, cx));
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            let window = cx.window_handle();
            for key in ["a", "c", "x"] {
                cx.dispatch_keystroke(
                    window,
                    Keystroke::parse(&format!("{modifier}-{key}"))
                        .expect("editing shortcut should parse"),
                );
                if key == "c" {
                    cx.update(|_, cx| {
                        assert_eq!(
                            input.read(cx).current_value(),
                            "hello \u{65e5}\u{672c}\u{8a9e}"
                        );
                        assert_eq!(
                            cx.read_from_clipboard()
                                .expect("copied text")
                                .text()
                                .as_deref(),
                            Some("hello \u{65e5}\u{672c}\u{8a9e}")
                        );
                    });
                }
            }
            cx.update(|_, cx| {
                assert_eq!(input.read(cx).current_value(), "");
                assert_eq!(
                    cx.read_from_clipboard()
                        .expect("cut text")
                        .text()
                        .as_deref(),
                    Some("hello \u{65e5}\u{672c}\u{8a9e}")
                );
            });
            cx.dispatch_keystroke(
                window,
                Keystroke::parse(&format!("{modifier}-v")).expect("test value should be available"),
            );
            cx.run_until_parked();
            cx.update(|_, cx| {
                assert_eq!(
                    input.read(cx).current_value(),
                    "hello \u{65e5}\u{672c}\u{8a9e}"
                )
            });
        }
    }

    #[gpui::test]
    fn later_typed_binding_overrides_default(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
            let key = if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            };
            cx.bind_keys([KeyBinding::new(key, End, Some("GuicTextInput"))]);
            // Reinitialization must not replace host keybindings.
            crate::init(cx);
        });
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("edit", cx).value("hello"));
        cx.update(|window, cx| input.read(cx).focus_handle.clone().focus(window, cx));
        let key = if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        };
        let window = cx.window_handle();
        cx.dispatch_keystroke(
            window,
            Keystroke::parse(key).expect("test value should be available"),
        );
        cx.update(|_, cx| assert_eq!(input.read(cx).selected_range, 5..5));
    }
}

#[cfg(test)]
mod vertical_navigation_tests {
    use super::TextInput;
    use gpui::{AppContext as _, Keystroke, TestAppContext, VisualContext as _};

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    fn vertical_keys_preserve_column_and_extend_selection(cx: &mut TestAppContext) {
        init(cx);
        let (input, cx) =
            cx.add_window_view(|_, cx| TextInput::text_area("area", cx).value("abcd\nx\nabcd"));
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| {
            input.update(cx, |input, cx| input.focus_handle.focus(window, cx));
        })
        .expect("focus");
        for (key, cursor) in [("up", 6), ("up", 4), ("down", 6), ("down", 11)] {
            cx.dispatch_keystroke(window, Keystroke::parse(key).expect("key"));
            input.update(cx, |input, _| assert_eq!(input.cursor_offset(), cursor));
        }
        cx.dispatch_keystroke(window, Keystroke::parse("shift-up").expect("key"));
        input.update(cx, |input, _| assert_eq!(input.selected_range, 6..11));
        cx.dispatch_keystroke(window, Keystroke::parse("shift-down").expect("key"));
        input.update(cx, |input, _| assert_eq!(input.selected_range, 11..11));
    }

    #[gpui::test]
    fn vertical_keys_cross_soft_wraps_without_splitting_unicode(cx: &mut TestAppContext) {
        init(cx);
        let text = "日本語🙂".repeat(100);
        let (input, cx) =
            cx.add_window_view(|_, cx| TextInput::text_area("wrapped", cx).value(text));
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| {
            input.update(cx, |input, cx| input.focus_handle.focus(window, cx));
        })
        .expect("focus");
        cx.dispatch_keystroke(window, Keystroke::parse("up").expect("key"));
        input.update(cx, |input, _| {
            assert!(input.cursor_offset() < input.value.len());
            assert!(input.value.is_char_boundary(input.cursor_offset()));
        });
        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("key"));
        input.update(cx, |input, _| {
            assert_eq!(input.cursor_offset(), input.value.len())
        });
    }
}
