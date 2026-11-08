//! Retained native input and viewport state behind the controlled editor API.

use super::*;
use gpui::{
    Bounds, ContentMask, Context, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, Font, GlobalElementId, InspectorElementId, InteractiveElement as _,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _,
    Pixels, Point, Render, ScrollWheelEvent, ShapedLine, Style, Styled as _, Subscription,
    TextAlign, TextRun, UTF16Selection, UnderlineStyle, div, fill, point, px, size,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};

mod reflow;
mod transaction;
use transaction::{ASYNC_BYTES, PendingEdit, Work};
mod scrollbar;
use reflow::PendingReflow;
mod wrap;
use scrollbar::{Axis, BAR_SIZE, Scrollbar};
use wrap::{Segment, WrapMap};

const LINE_HEIGHT: f32 = 22.0;
const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 8.0;

pub(super) struct EditorSurface {
    pending_edit: Option<PendingEdit>,
    edit_generation: u64,
    props: CodeEditor,
    edit: EditorEdit,
    composition: Option<EditorEdit>,
    marked: Option<Range<usize>>,
    composing_selection: Option<Range<usize>>,
    scroll: Point<Pixels>,
    max_width: Pixels,
    wrap: Option<WrapMap>,
    reflow: Option<PendingReflow>,
    reflow_generation: u64,
    pending_first_line: Option<usize>,
    reveal: bool,
    reveal_after_reflow: bool,
    selecting: bool,
    scrollbar_drag: Option<(Axis, f32)>,
    preferred_x: Option<Pixels>,
    upstream: bool,
    layout: Option<Viewport>,
    blur: Option<Subscription>,
    subscribed_focus: Option<FocusHandle>,
}

impl EditorSurface {
    pub(super) fn new(props: CodeEditor) -> Self {
        Self {
            pending_edit: None,
            edit_generation: 0,
            edit: EditorEdit::new(
                props.buffer.clone(),
                normalize_selections(&props.buffer, props.selections.clone()),
            ),
            scroll: point(px(0.0), px(props.options.first_line as f32 * LINE_HEIGHT)),
            props: props.clone(),
            composition: None,
            marked: None,
            composing_selection: None,
            max_width: px(0.0),
            wrap: None,
            reflow: None,
            reflow_generation: 0,
            pending_first_line: Some(props.options.first_line),
            reveal: false,
            reveal_after_reflow: false,
            selecting: false,
            scrollbar_drag: None,
            preferred_x: None,
            upstream: false,
            layout: None,
            blur: None,
            subscribed_focus: None,
        }
    }

    pub(super) fn configure(
        &mut self,
        props: CodeEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let buffer_changed = props.buffer != self.props.buffer && props.buffer != self.edit.buffer;
        let selection_changed = props.selections != self.props.selections
            && !props.selections.is_empty()
            && props.selections != self.edit.selections;
        let command_result = self
            .pending_edit
            .as_ref()
            .is_some_and(|p| p.accept_host_result);
        if buffer_changed || selection_changed {
            if !command_result {
                self.cancel_edits();
            } else if let Some(pending) = &mut self.pending_edit {
                pending.rebase_history(&self.edit.buffer, &props.buffer);
            }
            // A changed host value is authoritative. An unchanged host value must
            // not erase an in-progress composition or successive native events.
            self.edit = EditorEdit::new(
                props.buffer.clone(),
                normalize_selections(&props.buffer, props.selections.clone()),
            );
            self.composition = None;
            self.marked = None;
            self.composing_selection = None;
            self.max_width = px(0.0);
            self.preferred_x = None;
            self.upstream = false;
            self.reveal = true;
        }
        if props.options.first_line != self.props.options.first_line {
            self.pending_first_line = Some(props.options.first_line);
            self.reveal = false;
            if let Some(pending) = &mut self.pending_edit {
                pending.reveal = false;
            }
            self.reveal_after_reflow = false;
        }
        if props.focus_handle != self.subscribed_focus {
            self.blur = props.focus_handle.as_ref().map(|handle| {
                cx.on_blur(handle, window, |this, window, cx| {
                    this.finish_composition(window, cx);
                    this.selecting = false;
                    this.scrollbar_drag = None;
                    cx.notify();
                })
            });
            self.subscribed_focus = props.focus_handle.clone();
        }
        if self.props.options.soft_wrap != props.options.soft_wrap {
            self.reveal = true;
        }
        self.props = props;
        if !self.editable() {
            self.cancel_edits();
        }
        if !self.editable() && self.composition.is_some() {
            // A host removing editing capabilities cancels uncommitted text.
            if let Some(before) = self.composition.take() {
                self.edit = before;
            }
            self.marked = None;
            self.composing_selection = None;
        }
    }

    fn editable(&self) -> bool {
        self.props.focus_handle.is_some()
            && (self.props.on_edit.is_some() || self.props.on_change.is_some())
    }

    fn selection_bytes(&self) -> Range<usize> {
        self.composing_selection.clone().unwrap_or_else(|| {
            let range = self.edit.selections[0].ordered();
            position_byte(&self.edit.buffer, range.start)
                ..position_byte(&self.edit.buffer, range.end)
        })
    }

    fn publish(&self, before: &EditorEdit, window: &mut Window, cx: &mut Context<Self>) {
        self.publish_changes(
            before.buffer != self.edit.buffer,
            before.selections != self.edit.selections,
            window,
            cx,
        );
    }

    fn publish_changes(
        &self,
        text_changed: bool,
        selection_changed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text_changed && let Some(handler) = &self.props.on_change {
            handler(&self.edit.buffer, window, cx);
        }
        if (text_changed || selection_changed)
            && let Some(handler) = &self.props.on_edit
        {
            handler(&self.edit, window, cx);
        }
        if selection_changed && let Some(handler) = &self.props.on_cursor {
            handler(&self.edit.selections[0].head, window, cx);
        }
    }

    fn apply(&mut self, edit: EditorEdit, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit == edit {
            return;
        }
        let before = std::mem::replace(&mut self.edit, edit);
        if before.buffer != self.edit.buffer {
            self.max_width = px(0.0);
        }
        self.reveal = true;
        self.publish(&before, window, cx);
        cx.notify();
    }

    fn finish_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_edit.is_some() {
            self.commit_draft(window, cx);
            return;
        }
        self.marked = None;
        self.composing_selection = None;
        if let Some(before) = self.composition.take() {
            self.publish(&before, window, cx);
            cx.notify();
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Printable characters belong to the platform input handler. Consuming
        // key_char here bypasses IME and can insert the same text twice.
        let key = event.keystroke.key.as_str();
        let command = event.keystroke.modifiers.secondary();
        if !command
            && !matches!(
                key,
                "backspace"
                    | "delete"
                    | "enter"
                    | "tab"
                    | "left"
                    | "right"
                    | "up"
                    | "down"
                    | "home"
                    | "end"
                    | "pageup"
                    | "pagedown"
            )
        {
            return;
        }
        let replaying = self.pending_edit.as_ref().is_some_and(|p| p.replaying);
        let marked = self
            .pending_edit
            .as_ref()
            .map_or(self.marked.is_some(), |p| p.draft.marked.is_some());
        if marked && !replaying {
            return;
        }
        if !editor_key_is_handled(event) {
            return;
        }
        if command && key == "v" && self.editable() && !replaying {
            let copied = self
                .pending_edit
                .as_ref()
                .and_then(|p| (p.clipboard_sequence > 0).then_some(p.clipboard_sequence));
            let read = cx.read_text_from_clipboard_async();
            self.enqueue_edit(Work::Paste { read, copied }, window, cx);
            cx.stop_propagation();
            return;
        }
        if self.pending_edit.is_some() && !replaying {
            let work = if command && matches!(key, "c" | "x") {
                let pending = self.pending_edit.as_mut().expect("pending input");
                pending.clipboard_sequence += 1;
                Work::Clipboard {
                    sequence: pending.clipboard_sequence,
                    cut: key == "x",
                }
            } else if !command && matches!(key, "backspace" | "delete" | "enter" | "tab") {
                Work::EditKey(event.clone())
            } else {
                Work::Key(event.clone())
            };
            self.enqueue_edit(work, window, cx);
            cx.stop_propagation();
            return;
        }
        if let Some(command) = editor_key_command(event) {
            if let Some(handler) = &self.props.on_command {
                handler(&command, window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let navigation = matches!(
            key,
            "left" | "right" | "up" | "down" | "home" | "end" | "pageup" | "pagedown"
        ) || (command && matches!(key, "a" | "c"));
        if !navigation && !self.editable() {
            return;
        }
        if self.props.options.soft_wrap
            && self.wrap.as_ref().is_some_and(|map| !map.is_provisional())
            && !command
            && matches!(key, "home" | "end")
            && self.edit.selections.len() == 1
        {
            if let Some(viewport) = self.layout.clone() {
                self.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            }
            if let Some(map) = &self.wrap {
                let selection = &self.edit.selections[0];
                let row = self.visual_row(selection.head);
                let (_, fragment) = map.fragment(row);
                let head = EditorPosition::new(
                    selection.head.line,
                    if key == "home" {
                        fragment.columns.start
                    } else {
                        fragment.columns.end
                    },
                );
                let next = if event.keystroke.modifiers.shift {
                    EditorSelection::new(selection.anchor, head)
                } else {
                    EditorSelection::cursor(head)
                };
                self.upstream = key == "end";
                self.preferred_x = None;
                self.apply(
                    EditorEdit::new(self.edit.buffer.clone(), vec![next]),
                    window,
                    cx,
                );
                self.reveal = true;
                cx.notify();
            }
        } else if matches!(key, "up" | "down" | "pageup" | "pagedown")
            && self.edit.selections.len() == 1
        {
            self.move_vertical(key, event.keystroke.modifiers.shift, window, cx);
        } else {
            self.preferred_x = None;
            self.upstream = false;
            if let Some(edit) = editor_key_edit(
                event,
                &self.edit.buffer,
                &self.edit.selections,
                self.props.options.visible_lines,
                cx,
            ) {
                self.apply(edit, window, cx);
            }
        }
        cx.stop_propagation();
    }

    fn move_vertical(
        &mut self,
        key: &str,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(viewport) = self.layout.clone() else {
            return;
        };
        self.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
        let selection = &self.edit.selections[0];
        let current = self.visual_row(selection.head);
        let row = self.shape_column(current, selection.head.column, &viewport.font, window, cx);
        let x = self
            .preferred_x
            .unwrap_or_else(|| row.x_for_column(selection.head.column));
        let step = if key.starts_with("page") {
            (f32::from(viewport.body.size.height) / LINE_HEIGHT)
                .floor()
                .max(1.0) as usize
        } else {
            1
        };
        let target_row = if matches!(key, "down" | "pagedown") {
            current
                .saturating_add(step)
                .min(self.wrap.as_ref().expect("prepared wrap map").row_count() - 1)
        } else {
            current.saturating_sub(step)
        };
        let target = self.shape_x(target_row, x, &viewport.font, window, cx);
        let column = target.column_for_x(x);
        self.upstream = column == target.columns.end;
        let head = EditorPosition::new(target.index, column);
        let next = if extend {
            EditorSelection::new(selection.anchor, head)
        } else {
            EditorSelection::cursor(head)
        };
        self.apply(
            EditorEdit::new(self.edit.buffer.clone(), vec![next]),
            window,
            cx,
        );
        self.preferred_x = Some(x);
        self.reveal = true;
        cx.notify();
    }

    fn hit_position(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<EditorPosition> {
        let viewport = self.layout.clone()?;
        self.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
        let y = (f32::from(position.y - viewport.body.top() + self.scroll.y)).max(0.0);
        let visual = ((y / LINE_HEIGHT).floor() as usize).min(self.wrap.as_ref()?.row_count() - 1);
        let x = position.x - viewport.body.left() + self.scroll.x;
        let row = self.shape_x(visual, x, &viewport.font, window, cx);
        let column = row.column_for_x(x);
        self.upstream = column == row.columns.end;
        Some(EditorPosition::new(row.index, column))
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layout) = &self.layout {
            for bar in &layout.scrollbars {
                if bar.track.contains(&event.position) {
                    if bar.thumb.contains(&event.position) {
                        self.scrollbar_drag = Some((bar.axis, bar.grab_fraction(event.position)));
                    } else {
                        let direction = if bar.axis.coordinate(event.position)
                            < bar.axis.coordinate(bar.thumb.origin)
                        {
                            -1.0
                        } else {
                            1.0
                        };
                        match bar.axis {
                            Axis::Horizontal => self.scroll.x += bar.page * direction,
                            Axis::Vertical => self.scroll.y += bar.page * direction,
                        }
                        self.clamp_scroll(layout.body);
                    }
                    self.selecting = false;
                    self.reveal = false;
                    if let Some(pending) = &mut self.pending_edit {
                        pending.reveal = false;
                    }
                    self.reveal_after_reflow = false;
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
            }
            for row in &layout.rows {
                if let Some((bounds, diagnostic, _)) = &row.diagnostic
                    && layout.body.contains(&event.position)
                    && bounds.contains(&event.position)
                    && let Some(handler) = &self.props.on_diagnostic
                {
                    handler(diagnostic, window, cx);
                    cx.stop_propagation();
                    return;
                }
            }
        }
        self.finish_composition(window, cx);
        if let Some(focus) = self.props.focus_handle.clone() {
            focus.focus(window, cx);
        } else if self.props.on_cursor.is_none() {
            return;
        }
        let Some(position) = self.hit_position(event.position, window, cx) else {
            return;
        };
        let selection = if event.modifiers.shift {
            EditorSelection::new(self.pointer_selection().anchor, position)
        } else if event.click_count >= 3 {
            let end = if position.line + 1 < self.edit.buffer.line_count() {
                EditorPosition::new(position.line + 1, 0)
            } else {
                buffer_end(&self.edit.buffer)
            };
            EditorSelection::new(EditorPosition::new(position.line, 0), end)
        } else if event.click_count == 2 {
            let line = &self.edit.buffer.lines[position.line];
            let byte = self
                .edit
                .buffer
                .lines
                .column_byte(position.line, position.column);
            let (start, word) = line
                .split_word_bound_indices()
                .find(|(start, word)| *start <= byte && byte < start + word.len())
                .unwrap_or((line.len(), ""));
            EditorSelection::new(
                EditorPosition::new(position.line, byte_to_grapheme(line, start)),
                EditorPosition::new(position.line, byte_to_grapheme(line, start + word.len())),
            )
        } else {
            EditorSelection::cursor(position)
        };
        self.selecting = true;
        self.preferred_x = None;
        self.reveal = true;
        cx.notify();
        self.select_pointer(selection, window, cx);
        cx.stop_propagation();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((axis, grab)) = self.scrollbar_drag {
            if let Some(bar) = self
                .layout
                .as_ref()
                .and_then(|layout| layout.scrollbars.iter().find(|bar| bar.axis == axis))
            {
                let offset = bar.offset_for_drag(event.position, grab);
                match axis {
                    Axis::Horizontal => self.scroll.x = offset,
                    Axis::Vertical => self.scroll.y = offset,
                }
                self.reveal = false;
                if let Some(pending) = &mut self.pending_edit {
                    pending.reveal = false;
                }
                self.reveal_after_reflow = false;
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if !self.selecting {
            return;
        }
        if let Some(layout) = &self.layout {
            let bounds = layout.body;
            if event.position.y < bounds.top() {
                self.scroll.y -= bounds.top() - event.position.y;
            }
            if event.position.y > bounds.bottom() {
                self.scroll.y += event.position.y - bounds.bottom();
            }
            if event.position.x < bounds.left() {
                self.scroll.x -= bounds.left() - event.position.x;
            }
            if event.position.x > bounds.right() {
                self.scroll.x += event.position.x - bounds.right();
            }
            self.clamp_scroll(bounds);
        }
        if let Some(head) = self.hit_position(event.position, window, cx) {
            let selection = EditorSelection::new(self.pointer_selection().anchor, head);
            self.select_pointer(selection, window, cx);
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
        self.scrollbar_drag = None;
    }

    fn clamp_scroll(&mut self, body: Bounds<Pixels>) {
        let max_y = (self
            .wrap
            .as_ref()
            .map_or(self.edit.buffer.line_count(), WrapMap::row_count) as f64
            * f64::from(LINE_HEIGHT)
            - f64::from(f32::from(body.size.height)))
        .max(0.0)
        .min(f64::from(f32::MAX)) as f32;
        self.scroll.y = px(f32::from(self.scroll.y).clamp(0.0, max_y));
        self.scroll.x = self
            .scroll
            .x
            .max(px(0.0))
            .min((self.max_width + px(PADDING) - body.size.width).max(px(0.0)));
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = &self.layout else {
            return;
        };
        let mut delta = event.delta.pixel_delta(px(LINE_HEIGHT));
        if !f32::from(delta.x).is_finite() || !f32::from(delta.y).is_finite() {
            return;
        }
        if event.modifiers.shift && delta.x == px(0.0) {
            delta = point(delta.y, px(0.0));
        }
        let previous = self.scroll;
        self.scroll -= delta;
        self.clamp_scroll(layout.body);
        if self.scroll != previous {
            self.reveal = false;
            if let Some(pending) = &mut self.pending_edit {
                pending.reveal = false;
            }
            self.reveal_after_reflow = false;
            cx.notify();
            cx.stop_propagation();
        }
    }
}

impl Render for EditorSurface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div()
            .id(self.props.id.clone())
            .debug_selector(|| format!("guic-code-editor-{}", self.props.id))
            .accessibility(AccessibilityProps::new(Role::TextInput).label("Code editor"))
            .w_full()
            .relative()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .overflow_hidden()
            .font_family("monospace")
            .text_size(px(FONT_SIZE))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .child(EditorCanvas(cx.entity()));
        if let Some(pending) = &self.pending_edit {
            let preview = pending.draft.preview();
            let label = if preview.is_empty() {
                "Pasting…".to_owned()
            } else {
                format!("Pasting… {preview}")
            };
            let highlights = pending.draft.marked.as_ref().and_then(|range| {
                let start = range.start.min(preview.len());
                let end = range.end.min(preview.len());
                (start < end).then_some((
                    "Pasting… ".len() + start.."Pasting… ".len() + end,
                    gpui::HighlightStyle {
                        underline: Some(UnderlineStyle {
                            thickness: px(1.0),
                            color: None,
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                ))
            });
            root = root.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .bg(theme.background())
                    .h(px(LINE_HEIGHT))
                    .line_height(px(LINE_HEIGHT))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .px(px(PADDING))
                    .text_color(theme.foreground())
                    .child(gpui::StyledText::new(label).with_highlights(highlights)),
            );
        }
        if let Some(handle) = &self.props.focus_handle {
            root = root
                .track_focus(handle)
                .key_context("GuicCodeEditor")
                .cursor_text()
                .on_key_down(cx.listener(Self::key_down));
        }
        root
    }
}

pub(super) fn position_byte(buffer: &EditorBuffer, position: EditorPosition) -> usize {
    let position = buffer.clamp_position(position);
    buffer.lines.line_byte(position.line) + buffer.lines.column_byte(position.line, position.column)
}

pub(super) fn byte_position(buffer: &EditorBuffer, byte: usize) -> EditorPosition {
    let (line, column) = buffer.lines.byte_line(byte);
    EditorPosition::new(line, buffer.lines.byte_column(line, column))
}

pub(super) fn utf16_byte(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units >= offset {
            return index;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn native_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = utf16_byte(text, range.start);
    start..utf16_byte(text, range.end).max(start)
}

impl EntityInputHandler for EditorSurface {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(pending) = &self.pending_edit {
            let range = native_range(&pending.draft.text, range);
            *actual = Some(pending.draft.units(range.clone()));
            return Some(pending.draft.text[range].to_owned());
        }
        let range = self.edit.buffer.native_range(range);
        *actual = Some(self.edit.buffer.utf16_range(range.clone()));
        Some(self.edit.buffer.byte_text(range))
    }
    fn selected_text_range(
        &mut self,
        ignore_disabled: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if !self.editable() && !ignore_disabled {
            return None;
        }
        if let Some(pending) = &self.pending_edit {
            return Some(UTF16Selection {
                range: pending.draft.units(pending.draft.selected.clone()),
                reversed: false,
            });
        }
        Some(UTF16Selection {
            range: self.edit.buffer.utf16_range(self.selection_bytes()),
            reversed: self.composing_selection.is_none()
                && self.edit.selections[0].anchor > self.edit.selections[0].head,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if let Some(pending) = &self.pending_edit {
            return pending.draft.marked.clone().map(|r| pending.draft.units(r));
        }
        self.marked
            .clone()
            .map(|range| self.edit.buffer.utf16_range(range))
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition(window, cx);
    }
    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.editable()
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        if let Some(pending) = &self.pending_edit {
            return Some(pending.draft.text.encode_utf16().count());
        }
        Some(self.edit.buffer.lines.utf16_len())
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pending) = &mut self.pending_edit {
            pending.draft.selected = native_range(&pending.draft.text, range);
            cx.notify();
            return;
        }
        let range = self.edit.buffer.native_range(range);
        self.upstream = false;
        if self.composition.is_some() {
            self.composing_selection = Some(range.clone());
            self.edit.selections = vec![EditorSelection::new(
                byte_position(&self.edit.buffer, range.start),
                byte_position(&self.edit.buffer, range.end),
            )];
            self.reveal = true;
            cx.notify();
        } else {
            let selection = EditorSelection::new(
                byte_position(&self.edit.buffer, range.start),
                byte_position(&self.edit.buffer, range.end),
            );
            self.apply(
                EditorEdit::new(self.edit.buffer.clone(), vec![selection]),
                window,
                cx,
            );
        }
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        if let Some(pending) = &mut self.pending_edit {
            let primary =
                pending.draft.marked.is_some() || !pending.draft.text.is_empty() || range.is_some();
            if primary {
                if text.len() >= ASYNC_BYTES {
                    let draft = std::mem::take(&mut pending.draft);
                    let bytes = range
                        .map(|r| native_range(&draft.text, r))
                        .or(draft.marked)
                        .unwrap_or(draft.selected);
                    self.enqueue_edit(
                        Work::DraftCommit {
                            draft: draft.text,
                            bytes,
                            text: text.to_owned(),
                        },
                        window,
                        cx,
                    );
                } else {
                    pending.draft.replace(range, text, None, false);
                    self.commit_draft(window, cx);
                }
            } else {
                self.enqueue_edit(
                    Work::Insert {
                        text: text.to_owned(),
                        primary: false,
                    },
                    window,
                    cx,
                );
            }
            return;
        }
        if text.len() >= ASYNC_BYTES {
            let work = if range.is_none() && self.composition.is_none() {
                Work::Insert {
                    text: text.to_owned(),
                    primary: false,
                }
            } else {
                Work::Native {
                    text: text.to_owned(),
                    range,
                    bytes: self.marked.clone(),
                }
            };
            self.enqueue_edit(work, window, cx);
            return;
        }
        self.preferred_x = None;
        self.upstream = false;
        if range.is_none() && self.composition.is_none() {
            self.apply(
                edit_replacing_selections(&self.edit.buffer, &self.edit.selections, text),
                window,
                cx,
            );
            return;
        }
        let replacement = normalize_newlines(text);
        let mut buffer = self.edit.buffer.clone();
        let range = range
            .map(|r| buffer.native_range(r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selection_bytes());
        let before = self.composition.take().unwrap_or_else(|| self.edit.clone());
        buffer.replace_bytes(range.clone(), &replacement);
        let cursor = byte_position(&buffer, range.start + replacement.len());
        self.edit = EditorEdit::new(buffer, vec![EditorSelection::cursor(cursor)]);
        self.marked = None;
        self.composing_selection = None;
        self.reveal = true;
        self.max_width = px(0.0);
        self.publish(&before, window, cx);
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        if let Some(pending) = &mut self.pending_edit {
            pending.draft.replace(range, text, selected, true);
            if text.is_empty() {
                self.commit_draft(window, cx);
            }
            cx.notify();
            return;
        }
        if self.composition.is_none() {
            self.composition = Some(self.edit.clone());
        }
        let mut buffer = self.edit.buffer.clone();
        let range = range
            .map(|r| buffer.native_range(r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selection_bytes());
        let replacement = normalize_newlines(text);
        buffer.replace_bytes(range.clone(), &replacement);
        let selected = selected
            .map(|r| {
                let raw = native_range(text, r);
                range.start + normalize_newlines(&text[..raw.start]).len()
                    ..range.start + normalize_newlines(&text[..raw.end]).len()
            })
            .unwrap_or(range.start + replacement.len()..range.start + replacement.len());
        self.edit = EditorEdit::new(
            buffer.clone(),
            vec![EditorSelection::new(
                byte_position(&buffer, selected.start),
                byte_position(&buffer, selected.end),
            )],
        );
        self.upstream = false;
        self.marked = Some(range.start..range.start + replacement.len());
        self.composing_selection = Some(selected);
        self.preferred_x = None;
        self.reveal = true;
        self.max_width = px(0.0);
        if replacement.is_empty() {
            self.finish_composition(window, cx);
        }
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.pending_edit.is_some() {
            return self.draft_bounds(range, window);
        }
        let viewport = self.layout.clone()?;
        self.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
        let range = self.edit.buffer.native_range(range);
        let start = byte_position(&self.edit.buffer, range.start);
        // Native IMEs can query geometry before the next frame after an edit.
        // Shape current text instead of indexing stale painted glyphs.
        let visual = if start == self.edit.selections[0].head && range.is_empty() {
            self.visual_row(start)
        } else {
            self.wrap.as_ref()?.row_for_position(start)
        };
        let row = self.shape_column(visual, start.column, &viewport.font, window, cx);
        if start.column > row.columns.end {
            return None;
        }
        let y = viewport.body.top() + px(visual as f32 * LINE_HEIGHT) - self.scroll.y;
        if y + px(LINE_HEIGHT) <= viewport.body.top() || y >= viewport.body.bottom() {
            return None;
        }
        let origin = viewport.body.left() - self.scroll.x;
        let end = byte_position(&self.edit.buffer, range.end);
        let left = origin + row.x_for_column(start.column);
        let right = if end.line == start.line {
            origin
                + self
                    .shape_column(visual, end.column, &viewport.font, window, cx)
                    .x_for_column(end.column)
        } else {
            origin + self.wrap.as_ref()?.line_width(start.line)
        };
        Some(Bounds::new(
            point(left, y),
            size((right - left).max(px(1.0)), px(LINE_HEIGHT)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        if self.pending_edit.is_some() {
            return None;
        }
        if !self.layout.as_ref()?.body.contains(&point) {
            return None;
        }
        let upstream = self.upstream;
        let position = self.hit_position(point, window, cx)?;
        self.upstream = upstream;
        Some(
            self.edit
                .buffer
                .lines
                .byte_utf16(position_byte(&self.edit.buffer, position)),
        )
    }
}

#[derive(Clone)]
struct VisualRow {
    index: usize,
    visual: usize,
    columns: Range<usize>,
    x: Pixels,
    number_visible: bool,
    last_in_row: bool,
    line: ShapedLine,
    number: ShapedLine,
    boundaries: Vec<(usize, usize)>,
    origin: Point<Pixels>,
    diagnostic: Option<(Bounds<Pixels>, EditorDiagnostic, ShapedLine)>,
}

impl VisualRow {
    fn x_for_column(&self, column: usize) -> Pixels {
        self.x
            + self.line.x_for_index(
                self.boundaries[column
                    .saturating_sub(self.columns.start)
                    .min(self.boundaries.len() - 1)]
                .1,
            )
    }
    fn column_for_x(&self, x: Pixels) -> usize {
        let byte = self.line.closest_index_for_x(x - self.x);
        let right = self
            .boundaries
            .partition_point(|(_, display)| *display < byte)
            .min(self.boundaries.len() - 1);
        let left = right.saturating_sub(1);
        if f32::from(self.x_for_column(left + self.columns.start) - x).abs()
            < f32::from(self.x_for_column(right + self.columns.start) - x).abs()
        {
            left + self.columns.start
        } else {
            right + self.columns.start
        }
    }
}

#[derive(Clone)]
struct Viewport {
    bounds: Bounds<Pixels>,
    body: Bounds<Pixels>,
    font: Font,
    rows: Vec<VisualRow>,
    selections: Vec<Bounds<Pixels>>,
    caret: Option<Bounds<Pixels>>,
    scrollbars: Vec<Scrollbar>,
}

impl EditorSurface {
    fn visual_row(&self, position: EditorPosition) -> usize {
        let map = self.wrap.as_ref().expect("prepared wrap map");
        let row = map.row_for_position(position);
        if self.upstream
            && row > 0
            && map.fragment(row).1.columns.start == position.column
            && map.fragment(row - 1).0 == position.line
        {
            row - 1
        } else {
            row
        }
    }

    fn update_wrap(&mut self, font: &Font, width: Pixels, window: &Window, cx: &mut Context<Self>) {
        let width = self
            .props
            .options
            .soft_wrap
            .then_some((width - px(PADDING)).max(px(1.0)));
        self.request_reflow(font, width, window, cx);
        self.max_width = self.wrap.as_ref().expect("prepared wrap map").max_width;
        for diagnostic in &self.props.diagnostics {
            let line = diagnostic.range.start.line;
            if line >= self.edit.buffer.line_count() {
                continue;
            }
            let text: SharedString = diagnostic.message.replace(['\r', '\n'], " ").into();
            let width = window
                .text_system()
                .shape_line(
                    text.clone(),
                    px(FONT_SIZE),
                    &[TextRun {
                        len: text.len(),
                        font: font.clone(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                )
                .width();
            self.max_width = self.max_width.max(
                self.wrap
                    .as_ref()
                    .expect("prepared wrap map")
                    .line_width(line)
                    + px(PADDING)
                    + width,
            );
        }
    }

    fn shape_column(
        &self,
        visual: usize,
        column: usize,
        font: &Font,
        window: &Window,
        cx: &App,
    ) -> VisualRow {
        let segment = self
            .wrap
            .as_ref()
            .expect("map")
            .segment_for_column(visual, column);
        self.shape_segment(visual, segment, font, window, cx)
    }
    fn shape_x(
        &self,
        visual: usize,
        x: Pixels,
        font: &Font,
        window: &Window,
        cx: &App,
    ) -> VisualRow {
        let segment = self.wrap.as_ref().expect("map").segment_for_x(visual, x);
        self.shape_segment(visual, segment, font, window, cx)
    }
    fn shape_segment(
        &self,
        visual: usize,
        segment: Segment,
        font: &Font,
        window: &Window,
        cx: &App,
    ) -> VisualRow {
        let theme = Theme::global(cx);
        let (index, row_fragment) = self
            .wrap
            .as_ref()
            .expect("prepared wrap map")
            .fragment(visual);
        let fragment = segment.fragment;
        let full_source = &self.edit.buffer.lines[index];
        let source = &full_source[fragment.bytes.clone()];
        let mut display = String::new();
        let mut boundaries = Vec::new();
        let mut column = fragment.tab_column;
        for (byte, grapheme) in source.grapheme_indices(true) {
            boundaries.push((byte, display.len()));
            if grapheme == "\t" {
                let spaces = 4 - column % 4;
                display.push_str(&" ".repeat(spaces));
                column += spaces;
            } else {
                display.push_str(grapheme);
                column += 1;
            }
        }
        boundaries.push((source.len(), display.len()));
        let truncated_preview = self
            .wrap
            .as_ref()
            .expect("prepared wrap map")
            .is_provisional()
            && fragment.bytes.end < full_source.len();
        if truncated_preview {
            display.push('…');
        }
        let base = TextRun {
            len: display.len(),
            font: font.clone(),
            color: theme.foreground(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let plain = [SyntaxToken {
            kind: SyntaxTokenKind::Plain,
            start: fragment.columns.start,
            end: fragment.columns.end,
        }];
        let tokens = if self.props.options.syntax_highlighting {
            self.edit
                .buffer
                .lines
                .cached_analysis(index)
                .map_or(plain.as_slice(), |a| a.tokens.as_slice())
        } else {
            &plain
        };
        let marked = self.marked.as_ref().map(|marked| {
            let start = self.edit.buffer.lines.line_byte(index) + fragment.bytes.start;
            marked.start.saturating_sub(start).min(source.len())
                ..marked.end.saturating_sub(start).min(source.len())
        });
        let mut runs = Vec::new();
        let first_token = tokens.partition_point(|token| token.end <= fragment.columns.start);
        for token in tokens[first_token..]
            .iter()
            .take_while(|token| token.start < fragment.columns.end)
        {
            // Grapheme-sized runs share the text system's shaping context, so
            // syntax and composition decoration never split glyph layout.
            let start = token.start.max(fragment.columns.start);
            let end = token.end.min(fragment.columns.end);
            if start >= end {
                continue;
            }
            for boundary in
                boundaries[start - fragment.columns.start..=end - fragment.columns.start].windows(2)
            {
                runs.push(TextRun {
                    len: boundary[1].1 - boundary[0].1,
                    color: token_color(token.kind, theme),
                    underline: marked
                        .as_ref()
                        .filter(|range| range.start < boundary[1].0 && range.end > boundary[0].0)
                        .map(|_| UnderlineStyle {
                            thickness: px(1.0),
                            color: None,
                            wavy: false,
                        }),
                    ..base.clone()
                });
            }
        }
        if runs.is_empty() {
            runs.push(base.clone());
        } else if truncated_preview {
            runs.push(TextRun {
                len: '…'.len_utf8(),
                color: theme.muted_foreground(),
                ..base.clone()
            });
        }
        let line = window
            .text_system()
            .shape_line(display.into(), px(FONT_SIZE), &runs, None);
        let number_text: SharedString = (index + 1).to_string().into();
        let number = window.text_system().shape_line(
            number_text.clone(),
            px(FONT_SIZE),
            &[TextRun {
                len: number_text.len(),
                color: theme.muted_foreground(),
                ..base.clone()
            }],
            None,
        );
        let diagnostic = self
            .props
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.range.start.line == index && fragment.bytes.end == full_source.len()
            })
            .map(|diagnostic| {
                let text = SharedString::from(diagnostic.message.replace(['\r', '\n'], " "));
                let shaped = window.text_system().shape_line(
                    text.clone(),
                    px(FONT_SIZE),
                    &[TextRun {
                        len: text.len(),
                        color: severity_color(diagnostic.severity, theme),
                        ..base
                    }],
                    None,
                );
                (Bounds::default(), diagnostic.clone(), shaped)
            });
        VisualRow {
            index,
            visual,
            columns: fragment.columns.clone(),
            x: segment.x,
            number_visible: row_fragment.columns.start == 0,
            last_in_row: fragment.columns.end == row_fragment.columns.end,
            line,
            number,
            boundaries,
            origin: Point::default(),
            diagnostic,
        }
    }

    fn prepare(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Viewport {
        let font = window.text_style().font();
        let gutter = if self.props.options.line_numbers {
            let text: SharedString = self.edit.buffer.line_count().to_string().into();
            let number = window.text_system().shape_line(
                text.clone(),
                px(FONT_SIZE),
                &[TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: Theme::global(cx).muted_foreground(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            (f32::from(number.width()) + PADDING * 2.0).max(52.0)
        } else {
            0.0
        };
        let rail = if self.props.options.scrollbars {
            BAR_SIZE
        } else {
            0.0
        };
        let mut body = Bounds::new(
            point(bounds.left() + px(gutter + PADDING), bounds.top()),
            size(
                (bounds.size.width - px(gutter + PADDING * 2.0 + rail)).max(px(1.0)),
                bounds.size.height,
            ),
        );
        let anchor = self.wrap.as_ref().map(|map| {
            let (line, fragment) =
                map.fragment((f32::from(self.scroll.y) / LINE_HEIGHT).floor() as usize);
            EditorPosition::new(line, fragment.columns.start)
        });
        let resized = self
            .layout
            .as_ref()
            .is_some_and(|layout| layout.body.size.width != body.size.width);
        self.update_wrap(&font, body.size.width, window, cx);
        if self.max_width + px(PADDING) > body.size.width {
            body.size.height = (body.size.height - px(rail)).max(px(1.0));
        }
        if resized && let Some(anchor) = anchor {
            let anchor = self.edit.buffer.clamp_position(anchor);
            self.scroll.y = px(self
                .wrap
                .as_ref()
                .expect("prepared wrap map")
                .row_for_position(anchor) as f32
                * LINE_HEIGHT);
        }
        if let Some(line) = self.pending_first_line.take() {
            self.scroll.y = px(self
                .wrap
                .as_ref()
                .expect("prepared wrap map")
                .first_row(line.min(self.edit.buffer.line_count() - 1))
                as f32
                * LINE_HEIGHT);
        }
        let pointer_selection = self.pointer_selection();
        let cursor = self
            .composing_selection
            .as_ref()
            .map_or(pointer_selection.head, |range| {
                byte_position(&self.edit.buffer, range.end)
            });
        let selection_empty = self
            .composing_selection
            .as_ref()
            .map_or_else(|| pointer_selection.is_empty(), Range::is_empty);
        let cursor_row = self.visual_row(cursor);
        if self.reveal {
            let row = self.shape_column(cursor_row, cursor.column, &font, window, cx);
            let x = row.x_for_column(cursor.column);
            self.max_width = self.max_width.max(row.line.width());
            let y = px(cursor_row as f32 * LINE_HEIGHT);
            if y < self.scroll.y {
                self.scroll.y = y;
            }
            if y + px(LINE_HEIGHT) > self.scroll.y + body.size.height {
                self.scroll.y = y + px(LINE_HEIGHT) - body.size.height;
            }
            if x < self.scroll.x {
                self.scroll.x = x;
            }
            if x + px(2.0) > self.scroll.x + body.size.width {
                self.scroll.x = x + px(2.0) - body.size.width;
            }
        }
        self.clamp_scroll(body);
        let first = (f32::from(self.scroll.y) / LINE_HEIGHT).floor() as usize;
        let last = (((f32::from(self.scroll.y + body.size.height) / LINE_HEIGHT).ceil() as usize)
            .saturating_add(1))
        .min(self.wrap.as_ref().expect("prepared wrap map").row_count());
        let mut rows = Vec::new();
        for visual in first..last {
            let segments = self
                .wrap
                .as_ref()
                .expect("map")
                .visible_segments(visual, self.scroll.x..self.scroll.x + body.size.width);
            for (index, segment) in segments.into_iter().enumerate() {
                let mut row = self.shape_segment(visual, segment, &font, window, cx);
                row.number_visible &= index == 0;
                rows.push(row);
            }
        }
        for row in &rows {
            let width = row.x
                + row.line.width()
                + row
                    .diagnostic
                    .as_ref()
                    .map_or(px(0.0), |(_, _, text)| text.width() + px(PADDING));
            self.max_width = self.max_width.max(width);
        }
        self.clamp_scroll(body);
        let mut selections = Vec::new();
        let mut caret = None;
        for row in &mut rows {
            row.origin = point(
                body.left() - self.scroll.x,
                body.top() + px(row.visual as f32 * LINE_HEIGHT) - self.scroll.y,
            );
            if let Some((rect, _, text)) = &mut row.diagnostic {
                *rect = Bounds::new(
                    point(
                        row.origin.x + row.x + row.line.width() + px(PADDING),
                        row.origin.y,
                    ),
                    size(text.width(), px(LINE_HEIGHT)),
                );
            }
            let displayed = if self
                .pending_edit
                .as_ref()
                .is_some_and(|p| p.pointer.is_some())
            {
                std::slice::from_ref(&pointer_selection)
            } else {
                &self.edit.selections
            };
            for selection in displayed {
                let range = selection.ordered();
                if selection.is_empty()
                    || row.index < range.start.line
                    || row.index > range.end.line
                    || (row.index == range.end.line && range.end.column <= row.columns.start)
                    || (row.index == range.start.line
                        && range.start.column >= row.columns.end
                        && row.columns.end > row.columns.start)
                {
                    continue;
                }
                let start = if row.index == range.start.line {
                    row.x_for_column(range.start.column)
                } else {
                    row.x
                };
                let end = if row.index == range.end.line {
                    row.x_for_column(range.end.column)
                } else {
                    row.x
                        + row.line.width()
                        + if row.last_in_row {
                            px(PADDING)
                        } else {
                            px(0.0)
                        }
                };
                selections.push(Bounds::new(
                    row.origin + point(start, px(0.0)),
                    size((end - start).max(px(1.0)), px(LINE_HEIGHT)),
                ));
            }
            if row.visual == cursor_row
                && selection_empty
                && cursor.column >= row.columns.start
                && cursor.column <= row.columns.end
            {
                caret = Some(Bounds::new(
                    row.origin + point(row.x_for_column(cursor.column), px(0.0)),
                    size(px(2.0), px(LINE_HEIGHT)),
                ));
            }
        }
        let mut scrollbars = Vec::new();
        if self.props.options.scrollbars {
            let vertical_track = Bounds::new(
                point(bounds.right() - px(BAR_SIZE), bounds.top()),
                size(px(BAR_SIZE), body.size.height),
            );
            if let Some(bar) = Scrollbar::new(
                Axis::Vertical,
                vertical_track,
                body.size.height,
                px(self.wrap.as_ref().expect("prepared wrap map").row_count() as f32 * LINE_HEIGHT),
                self.scroll.y,
            ) {
                scrollbars.push(bar);
            }
            let horizontal_track = Bounds::new(
                point(body.left(), bounds.bottom() - px(BAR_SIZE)),
                size(body.size.width, px(BAR_SIZE)),
            );
            if let Some(bar) = Scrollbar::new(
                Axis::Horizontal,
                horizontal_track,
                body.size.width,
                self.max_width + px(PADDING),
                self.scroll.x,
            ) {
                scrollbars.push(bar);
            }
        }
        self.reveal_after_reflow |= self.reveal && self.reflow.is_some();
        self.reveal = false;
        Viewport {
            bounds,
            body,
            font,
            rows,
            selections,
            caret,
            scrollbars,
        }
    }
}

struct EditorCanvas(Entity<EditorSurface>);
impl IntoElement for EditorCanvas {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for EditorCanvas {
    type RequestLayoutState = ();
    type PrepaintState = Viewport;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = px((self.0.read(cx).props.options.visible_lines as f64
            * f64::from(LINE_HEIGHT))
        .min(f64::from(f32::MAX)) as f32)
        .into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Viewport {
        self.0.update(cx, |state, cx| {
            let viewport = state.prepare(bounds, window, cx);
            state.layout = Some(viewport.clone());
            viewport
        })
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        viewport: &mut Viewport,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = self.0.read(cx);
        let focus = state.props.focus_handle.clone();
        let editable = state.editable();
        let line_numbers = state.props.options.line_numbers;
        let accent = Theme::global(cx).primary();
        let scrollbar_color = Theme::global(cx).muted_foreground();
        let entity = self.0.clone();
        // Capture active drags even when the pointer leaves the editor bounds.
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == gpui::DispatchPhase::Capture {
                let state = entity.read(cx);
                if state.selecting || state.scrollbar_drag.is_some() {
                    entity.update(cx, |state, cx| state.mouse_move(event, window, cx));
                }
            }
        });
        if editable && let Some(focus) = &focus {
            window.handle_input(focus, ElementInputHandler::new(bounds, self.0.clone()), cx);
        }
        window.with_content_mask(
            Some(ContentMask {
                bounds: viewport.body,
            }),
            |window| {
                for rect in &viewport.selections {
                    window.paint_quad(fill(*rect, accent.opacity(0.22)));
                }
                for row in &viewport.rows {
                    let _ = row.line.paint(
                        row.origin + point(row.x, px(0.0)),
                        px(LINE_HEIGHT),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                    if let Some((bounds, _, text)) = &row.diagnostic {
                        let _ = text.paint(
                            bounds.origin,
                            px(LINE_HEIGHT),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                }
                if focus.as_ref().is_some_and(|focus| focus.is_focused(window))
                    && let Some(caret) = viewport.caret
                {
                    window.paint_quad(fill(caret, accent));
                }
            },
        );
        for bar in &viewport.scrollbars {
            window.paint_quad(fill(bar.track, scrollbar_color.opacity(0.06)));
            window.paint_quad(fill(bar.thumb, scrollbar_color.opacity(0.5)));
        }

        if line_numbers {
            let gutter = Bounds::new(
                viewport.bounds.origin,
                size(
                    (viewport.body.left() - viewport.bounds.left() - px(PADDING)).max(px(0.0)),
                    viewport.bounds.size.height,
                ),
            );
            window.with_content_mask(Some(ContentMask { bounds: gutter }), |window| {
                for row in &viewport.rows {
                    if !row.number_visible {
                        continue;
                    }
                    let origin = point(
                        gutter.right() - row.number.width() - px(PADDING),
                        row.origin.y,
                    );
                    let _ = row.number.paint(
                        origin,
                        px(LINE_HEIGHT),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            });
        }
    }
}

#[cfg(test)]
mod tests;
