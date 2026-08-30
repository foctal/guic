use crate::{ComponentSize, TextChangeHandler};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement as _, Pixels, Point,
    Render, SharedString, Style, Styled as _, TextAlign, TextRun, UTF16Selection, UnderlineStyle,
    Window, WrappedLine, actions, div, fill, point, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role, ValueChange};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::{ops::Range, rc::Rc};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    guic_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Newline,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
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
    last_layout: Option<TextLayoutState>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    on_change: Option<TextChangeHandler>,
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
            id: id.into(),
            kind,
            focus_handle: cx.focus_handle(),
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
            last_layout: None,
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
        self.value = self.normalize_inserted_text(&value.into());
        let cursor = self.value.len();
        self.selected_range = cursor..cursor;
        self
    }

    /// Sets the size of the text field.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
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
            (_, ComponentSize::Small) => (
                px(30.0),
                px(theme.spacing.x3),
                px(theme.typography.text_sm),
                1,
            ),
            (_, ComponentSize::Medium) => (
                px(36.0),
                px(theme.spacing.x4),
                px(theme.typography.text_md),
                1,
            ),
            (_, ComponentSize::Large) => (
                px(44.0),
                px(theme.spacing.x5),
                px(theme.typography.text_lg),
                1,
            ),
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.value.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.value.len(), cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.kind == TextInputKind::TextArea {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            cx.propagate();
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
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

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
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
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let normalized = self.normalize_inserted_text(&text);
            self.replace_text_in_range(None, &normalized, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.value[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.value[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
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

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.value.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
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
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
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
            TextInputKind::TextArea => text.to_owned(),
            TextInputKind::Text | TextInputKind::Search | TextInputKind::Password => {
                text.replace('\n', " ")
            }
        }
    }

    fn emit_change(&self, previous: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(on_change) = &self.on_change {
            (on_change)(self.value.as_str(), window, cx);
        }
        cx.emit(ValueChange::new(previous, self.value.clone()));
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
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.value[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
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
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let replacement = self.normalize_inserted_text(new_text);

        let previous = self.value.clone();
        self.value =
            self.value[0..range.start].to_owned() + &replacement + &self.value[range.end..];
        let cursor = range.start + replacement.len();
        self.selected_range = cursor..cursor;
        self.marked_range.take();
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
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let replacement = self.normalize_inserted_text(new_text);

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
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| {
                let cursor = range.start + replacement.len();
                cursor..cursor
            });

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
        let range = self.range_from_utf16(&range_utf16);
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
                theme.muted()
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

        let cursor_index = input.actual_to_display_offset(input.cursor_offset());
        let selected_start = input.actual_to_display_offset(input.selected_range.start);
        let selected_end = input.actual_to_display_offset(input.selected_range.end);
        let (selection, cursor) = if input.selected_range.is_empty() {
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

        self.input.update(cx, |input, cx| {
            input.last_layout = Some(layout);
            input.last_bounds = Some(bounds);
            cx.notify();
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
            .accessibility(
                AccessibilityProps::new(Role::TextInput)
                    .label(accessible_label)
                    .disabled(self.disabled),
            )
            .key_context("GuicTextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(if self.disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .line_height(line_height)
            .text_size(text_size)
            .px(padding_x)
            .py(px(theme.spacing.x2))
            .min_h(height)
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
            })
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
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

        if self.full_width {
            field = field.w_full();
        }

        if self.kind == TextInputKind::Search {
            field = field.child(Icon::new(IconName::Search).color(theme.muted_foreground()));
        }

        if self.kind == TextInputKind::TextArea {
            field = field.items_start();
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
            KeyBinding::new("enter", Newline, Some("GuicTextInput")),
            KeyBinding::new("shift-left", SelectLeft, Some("GuicTextInput")),
            KeyBinding::new("shift-right", SelectRight, Some("GuicTextInput")),
            KeyBinding::new("cmd-a", SelectAll, Some("GuicTextInput")),
            KeyBinding::new("cmd-v", Paste, Some("GuicTextInput")),
            KeyBinding::new("cmd-c", Copy, Some("GuicTextInput")),
            KeyBinding::new("cmd-x", Cut, Some("GuicTextInput")),
            KeyBinding::new("home", Home, Some("GuicTextInput")),
            KeyBinding::new("end", End, Some("GuicTextInput")),
            KeyBinding::new(
                "ctrl-cmd-space",
                ShowCharacterPalette,
                Some("GuicTextInput"),
            ),
        ]);
        cx.set_global(TextInputKeyBindingsInstalled);
    }
}
