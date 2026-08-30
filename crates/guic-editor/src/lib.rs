//! Native code editor primitives for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use gpui::{
    App, ClipboardItem, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use guic_tokens::Theme;
use std::{ops::Range, rc::Rc};
use unicode_segmentation::UnicodeSegmentation;

type PositionHandler = Rc<dyn Fn(&EditorPosition, &mut Window, &mut App)>;
type TextChangeHandler = Rc<dyn Fn(&EditorBuffer, &mut Window, &mut App)>;
type EditHandler = Rc<dyn Fn(&EditorEdit, &mut Window, &mut App)>;
type CommandHandler = Rc<dyn Fn(&EditorCommand, &mut Window, &mut App)>;
type DiagnosticHandler = Rc<dyn Fn(&EditorDiagnostic, &mut Window, &mut App)>;

/// A zero-based editor position.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct EditorPosition {
    /// Zero-based line index.
    pub line: usize,
    /// Zero-based grapheme column.
    pub column: usize,
}

impl EditorPosition {
    /// Creates a position.
    #[must_use]
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A selected range in an editor buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSelection {
    /// Selection anchor.
    pub anchor: EditorPosition,
    /// Selection head.
    pub head: EditorPosition,
}

impl EditorSelection {
    /// Creates a selection.
    #[must_use]
    pub fn new(anchor: EditorPosition, head: EditorPosition) -> Self {
        Self { anchor, head }
    }

    /// Returns the ordered range covered by the selection.
    #[must_use]
    pub fn ordered(&self) -> Range<EditorPosition> {
        if self.anchor <= self.head {
            self.anchor..self.head
        } else {
            self.head..self.anchor
        }
    }

    /// Returns whether the selection has no selected text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Creates a collapsed selection at one position.
    #[must_use]
    pub fn cursor(position: EditorPosition) -> Self {
        Self::new(position, position)
    }
}

/// A controlled editor update containing text and selection state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorEdit {
    /// Updated text buffer.
    pub buffer: EditorBuffer,
    /// Updated selections.
    pub selections: Vec<EditorSelection>,
}

impl EditorEdit {
    /// Creates an editor update.
    #[must_use]
    pub fn new(buffer: EditorBuffer, selections: Vec<EditorSelection>) -> Self {
        Self { buffer, selections }
    }
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    /// Informational note.
    Info,
    /// Warning diagnostic.
    Warning,
    /// Error diagnostic.
    Error,
}

/// A source diagnostic attached to a line range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorDiagnostic {
    range: Range<EditorPosition>,
    severity: DiagnosticSeverity,
    message: SharedString,
}

impl EditorDiagnostic {
    /// Creates a diagnostic.
    #[must_use]
    pub fn new(
        range: Range<EditorPosition>,
        severity: DiagnosticSeverity,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            range,
            severity,
            message: message.into(),
        }
    }

    /// Returns the diagnostic range.
    #[must_use]
    pub fn range(&self) -> &Range<EditorPosition> {
        &self.range
    }

    /// Returns the severity.
    #[must_use]
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Returns the message.
    #[must_use]
    pub fn message(&self) -> &SharedString {
        &self.message
    }
}

/// A search match in an editor buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSearchMatch {
    /// Match range.
    pub range: Range<EditorPosition>,
    /// Matched text.
    pub text: SharedString,
}

/// A completion candidate supplied by an application language adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorCompletion {
    /// Stable completion identifier.
    pub id: SharedString,
    /// User-facing completion label.
    pub label: SharedString,
    /// Text inserted when the completion is accepted.
    pub insert_text: SharedString,
    /// Optional supporting detail, such as a type or module.
    pub detail: Option<SharedString>,
}

impl EditorCompletion {
    /// Creates a completion whose label is also its insertion text.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            id: id.into(),
            insert_text: label.clone(),
            label,
            detail: None,
        }
    }

    /// Replaces the text inserted on acceptance.
    #[must_use]
    pub fn insert_text(mut self, insert_text: impl Into<SharedString>) -> Self {
        self.insert_text = insert_text.into();
        self
    }

    /// Adds supporting completion detail.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Application-owned language behavior used by the native editor model.
pub trait EditorLanguageAdapter: Send + Sync {
    /// Stable language identifier, such as `rust` or `json`.
    fn language_id(&self) -> &str;

    /// Returns syntax tokens for a line. The default uses the built-in classifier.
    fn syntax_tokens(&self, line: &str) -> Vec<SyntaxToken> {
        classify_line(line)
    }

    /// Returns completion candidates for a buffer position.
    fn completions(
        &self,
        _buffer: &EditorBuffer,
        _position: EditorPosition,
    ) -> Vec<EditorCompletion> {
        Vec::new()
    }
}

/// Commands handled by an application-owned [`EditorSession`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorCommand {
    /// Restore the previous edit snapshot.
    Undo,
    /// Restore the next edit snapshot.
    Redo,
}

/// Token categories produced by the built-in syntax classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxTokenKind {
    /// Language keyword.
    Keyword,
    /// String literal.
    String,
    /// Numeric literal.
    Number,
    /// Line comment.
    Comment,
    /// Plain identifier or punctuation.
    Plain,
}

/// A syntax token within one line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxToken {
    /// Token category.
    pub kind: SyntaxTokenKind,
    /// Start column.
    pub start: usize,
    /// End column.
    pub end: usize,
}

/// A large-buffer friendly text model split by lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorBuffer {
    lines: Vec<SharedString>,
}

impl EditorBuffer {
    /// Creates a buffer from text.
    #[must_use]
    pub fn from_text(text: impl AsRef<str>) -> Self {
        let mut lines: Vec<SharedString> = text.as_ref().split('\n').map(Into::into).collect();
        if lines.is_empty() {
            lines.push(SharedString::from(""));
        }
        Self { lines }
    }

    /// Returns the full buffer text.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns all lines.
    #[must_use]
    pub fn lines(&self) -> &[SharedString] {
        &self.lines
    }

    /// Returns the line count.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the grapheme count of a line, or zero when the line does not exist.
    #[must_use]
    pub fn line_len(&self, line: usize) -> usize {
        self.lines
            .get(line)
            .map(|line| line.graphemes(true).count())
            .unwrap_or(0)
    }

    /// Clamps a position to a valid grapheme boundary in this buffer.
    #[must_use]
    pub fn clamp_position(&self, position: EditorPosition) -> EditorPosition {
        let line = position.line.min(self.lines.len().saturating_sub(1));
        EditorPosition::new(line, position.column.min(self.line_len(line)))
    }

    /// Returns the text covered by a range.
    #[must_use]
    pub fn text_in_range(&self, range: Range<EditorPosition>) -> String {
        let start = self.clamp_position(range.start);
        let end = self.clamp_position(range.end);
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let start_byte = grapheme_to_byte(&self.lines[start.line], start.column);
        let end_byte = grapheme_to_byte(&self.lines[end.line], end.column);
        if start.line == end.line {
            return self.lines[start.line][start_byte..end_byte].to_string();
        }
        let mut text = String::from(&self.lines[start.line][start_byte..]);
        for line in &self.lines[start.line + 1..end.line] {
            text.push('\n');
            text.push_str(line);
        }
        text.push('\n');
        text.push_str(&self.lines[end.line][..end_byte]);
        text
    }

    /// Replaces a range with new text.
    pub fn replace_range(&mut self, range: Range<EditorPosition>, replacement: &str) {
        let start = self.clamp_position(range.start);
        let end = self.clamp_position(range.end);
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let start_byte = grapheme_to_byte(&self.lines[start.line], start.column);
        let end_byte = grapheme_to_byte(&self.lines[end.line], end.column);
        let prefix = &self.lines[start.line][..start_byte];
        let suffix = &self.lines[end.line][end_byte..];
        let replacement_lines = replacement.split('\n').collect::<Vec<_>>();
        let mut inserted = Vec::with_capacity(replacement_lines.len());
        if replacement_lines.len() == 1 {
            inserted.push(SharedString::from(format!(
                "{prefix}{}{suffix}",
                replacement_lines[0]
            )));
        } else {
            inserted.push(SharedString::from(format!(
                "{prefix}{}",
                replacement_lines[0]
            )));
            inserted.extend(
                replacement_lines[1..replacement_lines.len() - 1]
                    .iter()
                    .map(|line| SharedString::from((*line).to_string())),
            );
            inserted.push(SharedString::from(format!(
                "{}{suffix}",
                replacement_lines[replacement_lines.len() - 1]
            )));
        }
        self.lines.splice(start.line..=end.line, inserted);
    }

    /// Inserts text at a position.
    pub fn insert(&mut self, position: EditorPosition, text: &str) {
        self.replace_range(position..position, text);
    }

    /// Searches for plain text and returns line-aware matches.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<EditorSearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut matches = Vec::new();
        for (line_index, line) in self.lines.iter().enumerate() {
            let mut byte_start = 0usize;
            while let Some(offset) = line.as_ref()[byte_start..].find(query) {
                let start_byte = byte_start + offset;
                let end_byte = start_byte + query.len();
                let start = byte_to_grapheme(line, start_byte);
                let end = byte_to_grapheme(line, end_byte);
                matches.push(EditorSearchMatch {
                    range: EditorPosition::new(line_index, start)
                        ..EditorPosition::new(line_index, end),
                    text: SharedString::from(query),
                });
                byte_start = end_byte;
            }
        }
        matches
    }

    /// Replaces every non-overlapping plain-text match and returns the count.
    pub fn replace_all(&mut self, query: &str, replacement: &str) -> usize {
        let mut matches = self.search(query);
        let count = matches.len();
        matches.sort_by_key(|matched| std::cmp::Reverse(matched.range.start));
        for matched in matches {
            self.replace_range(matched.range, replacement);
        }
        count
    }

    /// Classifies a line with the built-in lightweight syntax strategy.
    #[must_use]
    pub fn syntax_tokens(&self, line_index: usize) -> Vec<SyntaxToken> {
        let Some(line) = self.lines.get(line_index) else {
            return Vec::new();
        };
        classify_line(line)
    }
}

/// Stateful editing model with selection and bounded undo/redo history.
///
/// Applications can keep this model in their own GPUI entity and pass
/// [`EditorSession::buffer`] and [`EditorSession::selections`] to
/// [`CodeEditor`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSession {
    buffer: EditorBuffer,
    selections: Vec<EditorSelection>,
    undo: Vec<EditorEdit>,
    redo: Vec<EditorEdit>,
    history_limit: usize,
}

impl EditorSession {
    /// Creates a session with one cursor at the start of the buffer.
    #[must_use]
    pub fn new(buffer: EditorBuffer) -> Self {
        Self {
            buffer,
            selections: vec![EditorSelection::cursor(EditorPosition::default())],
            undo: Vec::new(),
            redo: Vec::new(),
            history_limit: 100,
        }
    }

    /// Sets the maximum number of undo snapshots retained by the session.
    #[must_use]
    pub fn history_limit(mut self, history_limit: usize) -> Self {
        self.history_limit = history_limit;
        self.trim_history();
        self
    }

    /// Returns the current buffer.
    #[must_use]
    pub fn buffer(&self) -> &EditorBuffer {
        &self.buffer
    }

    /// Returns the current selections.
    #[must_use]
    pub fn selections(&self) -> &[EditorSelection] {
        &self.selections
    }

    /// Replaces selections after clamping them to the current buffer.
    pub fn set_selections(&mut self, selections: Vec<EditorSelection>) {
        self.selections = normalize_selections(&self.buffer, selections);
    }

    /// Applies a controlled edit and records the previous state for undo.
    pub fn apply(&mut self, edit: EditorEdit) {
        let selections = normalize_selections(&edit.buffer, edit.selections);
        if self.buffer == edit.buffer {
            self.selections = selections;
            return;
        }
        let previous = self.snapshot();
        self.buffer = edit.buffer;
        self.selections = selections;
        self.redo.clear();
        if self.history_limit > 0 {
            self.undo.push(previous);
            self.trim_history();
        }
    }

    /// Replaces every selection with text and collapses cursors after the insertion.
    pub fn insert(&mut self, text: &str) {
        self.apply(edit_replacing_selections(
            &self.buffer,
            &self.selections,
            text,
        ));
    }

    /// Removes selected text, or the preceding grapheme at each cursor.
    pub fn backspace(&mut self) {
        self.apply(edit_deleting(&self.buffer, &self.selections, true));
    }

    /// Removes selected text, or the following grapheme at each cursor.
    pub fn delete(&mut self) {
        self.apply(edit_deleting(&self.buffer, &self.selections, false));
    }

    /// Indents every line touched by a selection.
    pub fn indent(&mut self, indentation: &str) {
        if indentation.is_empty() {
            return;
        }
        self.apply(edit_indenting_lines(
            &self.buffer,
            &self.selections,
            indentation,
            false,
        ));
    }

    /// Removes one matching indentation prefix from every selected line.
    pub fn dedent(&mut self, indentation: &str) {
        if indentation.is_empty() {
            return;
        }
        self.apply(edit_indenting_lines(
            &self.buffer,
            &self.selections,
            indentation,
            true,
        ));
    }

    /// Selects the next search match after the primary cursor, wrapping once.
    pub fn select_next_match(&mut self, query: &str) -> bool {
        let matches = self.buffer.search(query);
        if matches.is_empty() {
            return false;
        }
        let cursor = self
            .selections
            .first()
            .map_or(EditorPosition::default(), |selection| selection.head);
        let matched = matches
            .iter()
            .find(|matched| matched.range.start > cursor)
            .unwrap_or(&matches[0]);
        self.selections = vec![EditorSelection::new(matched.range.start, matched.range.end)];
        true
    }

    /// Replaces all matches as one undoable session edit.
    pub fn replace_all(&mut self, query: &str, replacement: &str) -> usize {
        let mut buffer = self.buffer.clone();
        let count = buffer.replace_all(query, replacement);
        if count > 0 {
            let selections = normalize_selections(&buffer, self.selections.clone());
            self.apply(EditorEdit::new(buffer, selections));
        }
        count
    }

    /// Restores the previous edit snapshot.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(previous);
        true
    }

    /// Restores the next edit snapshot.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.trim_history();
        self.restore(next);
        true
    }

    fn snapshot(&self) -> EditorEdit {
        EditorEdit::new(self.buffer.clone(), self.selections.clone())
    }

    fn restore(&mut self, edit: EditorEdit) {
        self.buffer = edit.buffer;
        self.selections = edit.selections;
    }

    fn trim_history(&mut self) {
        let excess = self.undo.len().saturating_sub(self.history_limit);
        if excess > 0 {
            self.undo.drain(..excess);
        }
    }
}

/// Rendering options for [`CodeEditor`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeEditorOptions {
    first_line: usize,
    visible_lines: usize,
    line_numbers: bool,
    syntax_highlighting: bool,
}

impl Default for CodeEditorOptions {
    fn default() -> Self {
        Self {
            first_line: 0,
            visible_lines: 24,
            line_numbers: true,
            syntax_highlighting: true,
        }
    }
}

impl CodeEditorOptions {
    /// Sets the first rendered line.
    #[must_use]
    pub fn first_line(mut self, first_line: usize) -> Self {
        self.first_line = first_line;
        self
    }

    /// Sets the maximum visible line count.
    #[must_use]
    pub fn visible_lines(mut self, visible_lines: usize) -> Self {
        self.visible_lines = visible_lines.max(1);
        self
    }

    /// Sets whether line numbers are rendered.
    #[must_use]
    pub fn line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }

    /// Sets whether built-in syntax highlighting is rendered.
    #[must_use]
    pub fn syntax_highlighting(mut self, syntax_highlighting: bool) -> Self {
        self.syntax_highlighting = syntax_highlighting;
        self
    }
}

/// A controlled native code editor surface.
#[derive(gpui::IntoElement)]
pub struct CodeEditor {
    id: SharedString,
    buffer: EditorBuffer,
    selections: Vec<EditorSelection>,
    diagnostics: Vec<EditorDiagnostic>,
    options: CodeEditorOptions,
    focus_handle: Option<FocusHandle>,
    on_cursor: Option<PositionHandler>,
    on_change: Option<TextChangeHandler>,
    on_edit: Option<EditHandler>,
    on_command: Option<CommandHandler>,
    on_diagnostic: Option<DiagnosticHandler>,
}

impl CodeEditor {
    /// Creates an editor from a buffer.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, buffer: EditorBuffer) -> Self {
        Self {
            id: id.into(),
            buffer,
            selections: Vec::new(),
            diagnostics: Vec::new(),
            options: CodeEditorOptions::default(),
            focus_handle: None,
            on_cursor: None,
            on_change: None,
            on_edit: None,
            on_command: None,
            on_diagnostic: None,
        }
    }

    /// Replaces selections.
    #[must_use]
    pub fn selections(mut self, selections: Vec<EditorSelection>) -> Self {
        self.selections = selections;
        self
    }

    /// Replaces diagnostics.
    #[must_use]
    pub fn diagnostics(mut self, diagnostics: Vec<EditorDiagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Replaces options.
    #[must_use]
    pub fn options(mut self, options: CodeEditorOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets a focus handle so the editor can receive keyboard input.
    #[must_use]
    pub fn focusable(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Registers a cursor request handler.
    #[must_use]
    pub fn on_cursor(
        mut self,
        handler: impl Fn(&EditorPosition, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cursor = Some(Rc::new(handler));
        self
    }

    /// Registers a controlled text-change handler for keyboard editing.
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(&EditorBuffer, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Registers a controlled text-and-selection edit handler.
    ///
    /// This is preferred over [`CodeEditor::on_change`] for editable surfaces
    /// because it preserves cursor and selection updates. When both handlers
    /// are set, both are called.
    #[must_use]
    pub fn on_edit(
        mut self,
        handler: impl Fn(&EditorEdit, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for undo and redo shortcuts.
    #[must_use]
    pub fn on_command(
        mut self,
        handler: impl Fn(&EditorCommand, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_command = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for activating a rendered diagnostic.
    #[must_use]
    pub fn on_diagnostic(
        mut self,
        handler: impl Fn(&EditorDiagnostic, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_diagnostic = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CodeEditor {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let end =
            (self.options.first_line + self.options.visible_lines).min(self.buffer.line_count());
        let mut root = div()
            .id(self.id)
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .overflow_hidden()
            .flex()
            .flex_col();
        if let Some(handle) = &self.focus_handle {
            root = root.key_context("GuicCodeEditor").track_focus(handle);
        }
        if let Some(handle) = self.focus_handle.clone()
            && (self.on_change.is_some() || self.on_edit.is_some() || self.on_command.is_some())
        {
            let base_buffer = self.buffer.clone();
            let base_selections = normalize_selections(&base_buffer, self.selections.clone());
            let on_change = self.on_change.clone();
            let on_edit = self.on_edit.clone();
            let on_command = self.on_command.clone();
            let page_lines = self.options.visible_lines;
            root = root
                .cursor_text()
                .on_click(move |_, window, cx| window.focus(&handle, cx))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let handled = editor_key_is_handled(event);
                    let command = editor_key_command(event);
                    let edit =
                        editor_key_edit(event, &base_buffer, &base_selections, page_lines, cx);
                    if handled {
                        cx.stop_propagation();
                    }
                    if let (Some(command), Some(on_command)) = (command, on_command.as_ref()) {
                        on_command(&command, window, cx);
                    }
                    let Some(edit) = edit else {
                        return;
                    };
                    if let Some(on_change) = &on_change {
                        on_change(&edit.buffer, window, cx);
                    }
                    if let Some(on_edit) = &on_edit {
                        on_edit(&edit, window, cx);
                    }
                });
        }

        for line_index in self.options.first_line..end {
            let mut row = div()
                .w_full()
                .min_h(px(22.0))
                .flex()
                .items_start()
                .text_color(theme.foreground())
                .font_family("monospace")
                .text_size(px(13.0));
            if self.options.line_numbers {
                row = row.child(
                    div()
                        .w(px(52.0))
                        .px_2()
                        .text_color(theme.muted_foreground())
                        .child(format!("{}", line_index + 1)),
                );
            }
            let selected = self
                .selections
                .iter()
                .any(|selection| selection_intersects_line(selection, line_index));
            let mut code = div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .px_2()
                .text_color(theme.foreground())
                .bg(if selected {
                    theme.secondary().opacity(0.32)
                } else {
                    theme.background()
                });
            if self.options.syntax_highlighting {
                code = render_tokens(code, &self.buffer, line_index, theme);
            } else if let Some(line) = self.buffer.lines().get(line_index) {
                code = code.child(line.clone());
            }
            if let Some(diagnostic) = self
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.range.start.line == line_index)
            {
                let diagnostic = diagnostic.clone();
                let badge = div()
                    .ml_2()
                    .text_color(severity_color(diagnostic.severity, theme))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(diagnostic.message.clone());
                let badge = if let Some(handler) = self.on_diagnostic.clone() {
                    badge
                        .id(format!("guic-editor-diagnostic-{line_index}"))
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            handler(&diagnostic, window, cx);
                        })
                        .into_any_element()
                } else {
                    badge.into_any_element()
                };
                code = code.child(badge);
            }
            let line = row.child(code);
            root = if let Some(on_cursor) = self.on_cursor.clone() {
                let position = EditorPosition::new(line_index, 0);
                root.child(
                    line.id(format!("guic-code-editor-line-{line_index}"))
                        .cursor_text()
                        .on_click(move |_, window, cx| {
                            on_cursor(&position, window, cx);
                        }),
                )
            } else {
                root.child(line)
            };
        }
        root
    }
}

fn editor_key_is_handled(event: &KeyDownEvent) -> bool {
    let command = event.keystroke.modifiers.secondary();
    let key = event.keystroke.key.as_str();
    if command && matches!(key, "a" | "c" | "x" | "v" | "z" | "y") {
        return true;
    }
    matches!(
        key,
        "backspace"
            | "delete"
            | "enter"
            | "left"
            | "right"
            | "up"
            | "down"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
    ) || key == "tab"
        || (!command
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.alt
            && !event.keystroke.modifiers.function
            && event.keystroke.key_char.is_some())
}

fn editor_key_command(event: &KeyDownEvent) -> Option<EditorCommand> {
    if !event.keystroke.modifiers.secondary() {
        return None;
    }
    match event.keystroke.key.as_str() {
        "z" if event.keystroke.modifiers.shift => Some(EditorCommand::Redo),
        "z" => Some(EditorCommand::Undo),
        "y" => Some(EditorCommand::Redo),
        _ => None,
    }
}

fn editor_key_edit(
    event: &KeyDownEvent,
    buffer: &EditorBuffer,
    selections: &[EditorSelection],
    page_lines: usize,
    cx: &App,
) -> Option<EditorEdit> {
    let command = event.keystroke.modifiers.secondary();
    let key = event.keystroke.key.as_str();
    if command && key == "a" {
        let end = buffer_end(buffer);
        return Some(EditorEdit::new(
            buffer.clone(),
            vec![EditorSelection::new(EditorPosition::default(), end)],
        ));
    }
    if command && key == "c" {
        let text = selections
            .iter()
            .filter(|selection| !selection.is_empty())
            .map(|selection| buffer.text_in_range(selection.ordered()))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
        return None;
    }
    if command && key == "x" {
        let text = selections
            .iter()
            .filter(|selection| !selection.is_empty())
            .map(|selection| buffer.text_in_range(selection.ordered()))
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            return None;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        return Some(edit_replacing_selections(buffer, selections, ""));
    }
    if command && key == "v" {
        return cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .map(|text| edit_replacing_selections(buffer, selections, &text));
    }
    if command && matches!(key, "z" | "y") {
        return None;
    }

    match key {
        "backspace" => Some(edit_deleting(buffer, selections, true)),
        "delete" => Some(edit_deleting(buffer, selections, false)),
        "enter" => Some(edit_replacing_selections(buffer, selections, "\n")),
        "tab" => Some(edit_indenting_lines(
            buffer,
            selections,
            "    ",
            event.keystroke.modifiers.shift,
        )),
        "left" | "right" | "up" | "down" | "home" | "end" => Some(EditorEdit::new(
            buffer.clone(),
            move_selections(buffer, selections, key, event.keystroke.modifiers.shift),
        )),
        "pageup" | "pagedown" => Some(EditorEdit::new(
            buffer.clone(),
            move_selections_by_page(
                buffer,
                selections,
                page_lines.max(1),
                key == "pagedown",
                event.keystroke.modifiers.shift,
            ),
        )),
        _ if !command
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.alt
            && !event.keystroke.modifiers.function =>
        {
            event
                .keystroke
                .key_char
                .as_deref()
                .map(|text| edit_replacing_selections(buffer, selections, text))
        }
        _ => None,
    }
}

fn edit_indenting_lines(
    buffer: &EditorBuffer,
    selections: &[EditorSelection],
    indentation: &str,
    dedent: bool,
) -> EditorEdit {
    let selections = normalize_selections(buffer, selections.to_vec());
    let mut lines = selections
        .iter()
        .flat_map(|selection| {
            let range = selection.ordered();
            let end = if range.end.column == 0 && range.end.line > range.start.line {
                range.end.line - 1
            } else {
                range.end.line
            };
            range.start.line..=end
        })
        .collect::<Vec<_>>();
    lines.sort_unstable();
    lines.dedup();

    let indentation_columns = indentation.graphemes(true).count();
    let mut next = buffer.clone();
    let mut adjusted = selections;
    for line in lines.into_iter().rev() {
        let removes = dedent
            && next
                .lines()
                .get(line)
                .is_some_and(|text| text.starts_with(indentation));
        if dedent && !removes {
            continue;
        }
        let range = EditorPosition::new(line, 0)
            ..EditorPosition::new(line, usize::from(removes) * indentation_columns);
        let replacement = if dedent { "" } else { indentation };
        next.replace_range(range, replacement);
        for selection in &mut adjusted {
            for position in [&mut selection.anchor, &mut selection.head] {
                if position.line == line {
                    position.column = if dedent {
                        position.column.saturating_sub(indentation_columns)
                    } else {
                        position.column + indentation_columns
                    };
                }
            }
        }
    }
    EditorEdit::new(next, adjusted)
}

fn move_selections_by_page(
    buffer: &EditorBuffer,
    selections: &[EditorSelection],
    page_lines: usize,
    forwards: bool,
    extend: bool,
) -> Vec<EditorSelection> {
    normalize_selections(buffer, selections.to_vec())
        .into_iter()
        .map(|selection| {
            let line = if forwards {
                selection
                    .head
                    .line
                    .saturating_add(page_lines)
                    .min(buffer.line_count().saturating_sub(1))
            } else {
                selection.head.line.saturating_sub(page_lines)
            };
            let head = EditorPosition::new(line, selection.head.column.min(buffer.line_len(line)));
            if extend {
                EditorSelection::new(selection.anchor, head)
            } else {
                EditorSelection::cursor(head)
            }
        })
        .collect()
}

fn edit_replacing_selections(
    buffer: &EditorBuffer,
    selections: &[EditorSelection],
    replacement: &str,
) -> EditorEdit {
    let selections = normalize_selections(buffer, selections.to_vec());
    let mut ordered = selections
        .iter()
        .map(EditorSelection::ordered)
        .collect::<Vec<_>>();
    ordered.sort_by_key(|range| range.start);
    let mut ranges: Vec<Range<EditorPosition>> = Vec::with_capacity(ordered.len());
    for range in ordered {
        if let Some(previous) = ranges.last_mut()
            && range.start <= previous.end
        {
            previous.end = previous.end.max(range.end);
        } else {
            ranges.push(range);
        }
    }
    ranges.sort_by_key(|range| std::cmp::Reverse(range.start));

    let mut next = buffer.clone();
    let mut cursors: Vec<EditorSelection> = Vec::with_capacity(ranges.len());
    for range in ranges {
        let start = range.start;
        for cursor in &mut cursors {
            cursor.anchor =
                translate_position_after_replacement(cursor.anchor, &range, replacement);
            cursor.head = cursor.anchor;
        }
        next.replace_range(range, replacement);
        cursors.push(EditorSelection::cursor(position_after_text(
            start,
            replacement,
        )));
    }
    cursors.sort_by_key(|selection| selection.head);
    EditorEdit::new(next, cursors)
}

fn translate_position_after_replacement(
    position: EditorPosition,
    range: &Range<EditorPosition>,
    replacement: &str,
) -> EditorPosition {
    if position <= range.end {
        return position;
    }
    let inserted_end = position_after_text(range.start, replacement);
    if position.line == range.end.line {
        EditorPosition::new(
            inserted_end.line,
            inserted_end.column + position.column.saturating_sub(range.end.column),
        )
    } else {
        EditorPosition::new(
            position.line - (range.end.line - range.start.line)
                + (inserted_end.line - range.start.line),
            position.column,
        )
    }
}

fn edit_deleting(
    buffer: &EditorBuffer,
    selections: &[EditorSelection],
    backwards: bool,
) -> EditorEdit {
    let selections = normalize_selections(buffer, selections.to_vec())
        .into_iter()
        .map(|selection| {
            if !selection.is_empty() {
                return selection;
            }
            let adjacent = if backwards {
                previous_position(buffer, selection.head)
            } else {
                next_position(buffer, selection.head)
            };
            if backwards {
                EditorSelection::new(adjacent, selection.head)
            } else {
                EditorSelection::new(selection.head, adjacent)
            }
        })
        .collect::<Vec<_>>();
    edit_replacing_selections(buffer, &selections, "")
}

fn move_selections(
    buffer: &EditorBuffer,
    selections: &[EditorSelection],
    key: &str,
    extend: bool,
) -> Vec<EditorSelection> {
    normalize_selections(buffer, selections.to_vec())
        .into_iter()
        .map(|selection| {
            let ordered = selection.ordered();
            let head = if !extend && !selection.is_empty() {
                if matches!(key, "left" | "up" | "home") {
                    ordered.start
                } else {
                    ordered.end
                }
            } else {
                move_position(buffer, selection.head, key)
            };
            if extend {
                EditorSelection::new(selection.anchor, head)
            } else {
                EditorSelection::cursor(head)
            }
        })
        .collect()
}

fn move_position(buffer: &EditorBuffer, position: EditorPosition, key: &str) -> EditorPosition {
    let position = buffer.clamp_position(position);
    match key {
        "left" => previous_position(buffer, position),
        "right" => next_position(buffer, position),
        "up" => EditorPosition::new(
            position.line.saturating_sub(1),
            position
                .column
                .min(buffer.line_len(position.line.saturating_sub(1))),
        ),
        "down" => {
            let line = (position.line + 1).min(buffer.line_count().saturating_sub(1));
            EditorPosition::new(line, position.column.min(buffer.line_len(line)))
        }
        "home" => EditorPosition::new(position.line, 0),
        "end" => EditorPosition::new(position.line, buffer.line_len(position.line)),
        _ => position,
    }
}

fn normalize_selections(
    buffer: &EditorBuffer,
    selections: Vec<EditorSelection>,
) -> Vec<EditorSelection> {
    if selections.is_empty() {
        return vec![EditorSelection::cursor(buffer_end(buffer))];
    }
    selections
        .into_iter()
        .map(|selection| {
            EditorSelection::new(
                buffer.clamp_position(selection.anchor),
                buffer.clamp_position(selection.head),
            )
        })
        .collect()
}

fn position_after_text(start: EditorPosition, text: &str) -> EditorPosition {
    let mut parts = text.split('\n');
    let first = parts.next().unwrap_or_default();
    let remaining = parts.collect::<Vec<_>>();
    if let Some(last) = remaining.last() {
        EditorPosition::new(start.line + remaining.len(), last.graphemes(true).count())
    } else {
        EditorPosition::new(start.line, start.column + first.graphemes(true).count())
    }
}

fn buffer_end(buffer: &EditorBuffer) -> EditorPosition {
    let line = buffer.line_count().saturating_sub(1);
    EditorPosition::new(line, buffer.line_len(line))
}

fn selection_intersects_line(selection: &EditorSelection, line: usize) -> bool {
    let range = selection.ordered();
    !selection.is_empty()
        && (range.start.line..=range.end.line).contains(&line)
        && !(range.end.line == line && range.end.column == 0 && range.start.line != line)
}

fn render_tokens(
    mut code: gpui::Div,
    buffer: &EditorBuffer,
    line_index: usize,
    theme: &Theme,
) -> gpui::Div {
    let Some(line) = buffer.lines().get(line_index) else {
        return code;
    };
    let tokens = buffer.syntax_tokens(line_index);
    if tokens.is_empty() {
        return code.child(line.clone());
    }
    let graphemes = line.graphemes(true).collect::<Vec<_>>();
    for token in tokens {
        let text = graphemes[token.start..token.end].concat();
        code = code.child(
            div()
                .text_color(token_color(token.kind, theme))
                .whitespace_nowrap()
                .child(text),
        );
    }
    code
}

fn previous_position(buffer: &EditorBuffer, position: EditorPosition) -> EditorPosition {
    if position.column > 0 {
        return EditorPosition::new(position.line, position.column - 1);
    }
    if position.line == 0 {
        return position;
    }
    let previous_line = position.line - 1;
    let previous_column = buffer.lines()[previous_line].graphemes(true).count();
    EditorPosition::new(previous_line, previous_column)
}

fn next_position(buffer: &EditorBuffer, position: EditorPosition) -> EditorPosition {
    let position = buffer.clamp_position(position);
    if position.column < buffer.line_len(position.line) {
        return EditorPosition::new(position.line, position.column + 1);
    }
    if position.line + 1 < buffer.line_count() {
        return EditorPosition::new(position.line + 1, 0);
    }
    position
}

fn classify_line(line: &str) -> Vec<SyntaxToken> {
    let graphemes = line.graphemes(true).collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < graphemes.len() {
        let rest = graphemes[index..].concat();
        if rest.starts_with("//") {
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Comment,
                start: index,
                end: graphemes.len(),
            });
            break;
        }
        if graphemes[index] == "\"" {
            let start = index;
            index += 1;
            while index < graphemes.len() && graphemes[index] != "\"" {
                index += 1;
            }
            index = (index + 1).min(graphemes.len());
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::String,
                start,
                end: index,
            });
        } else if graphemes[index].chars().all(|c| c.is_ascii_digit()) {
            let start = index;
            while index < graphemes.len() && graphemes[index].chars().all(|c| c.is_ascii_digit()) {
                index += 1;
            }
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Number,
                start,
                end: index,
            });
        } else if graphemes[index]
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '_')
        {
            let start = index;
            while index < graphemes.len()
                && graphemes[index]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                index += 1;
            }
            let word = graphemes[start..index].concat();
            tokens.push(SyntaxToken {
                kind: if is_keyword(&word) {
                    SyntaxTokenKind::Keyword
                } else {
                    SyntaxTokenKind::Plain
                },
                start,
                end: index,
            });
        } else {
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Plain,
                start: index,
                end: index + 1,
            });
            index += 1;
        }
    }
    tokens
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "else"
            | "enum"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "return"
            | "self"
            | "struct"
            | "trait"
            | "type"
            | "use"
            | "where"
            | "while"
    )
}

fn grapheme_to_byte(line: &str, column: usize) -> usize {
    line.grapheme_indices(true)
        .nth(column)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn byte_to_grapheme(line: &str, byte: usize) -> usize {
    line.grapheme_indices(true)
        .take_while(|(index, _)| *index < byte)
        .count()
}

fn token_color(kind: SyntaxTokenKind, theme: &Theme) -> gpui::Hsla {
    match kind {
        SyntaxTokenKind::Keyword => theme.primary(),
        SyntaxTokenKind::String => theme.success(),
        SyntaxTokenKind::Number => theme.info(),
        SyntaxTokenKind::Comment => theme.muted_foreground(),
        SyntaxTokenKind::Plain => theme.foreground(),
    }
}

fn severity_color(severity: DiagnosticSeverity, theme: &Theme) -> gpui::Hsla {
    match severity {
        DiagnosticSeverity::Info => theme.info(),
        DiagnosticSeverity::Warning => theme.warning(),
        DiagnosticSeverity::Error => theme.danger(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EditorBuffer, EditorPosition, EditorSelection, EditorSession, SyntaxTokenKind,
        edit_deleting, edit_indenting_lines, edit_replacing_selections, move_selections,
        move_selections_by_page,
    };

    #[test]
    fn buffer_replaces_cross_line_range() {
        let mut buffer = EditorBuffer::from_text("let a = 1;\nlet b = 2;");
        buffer.replace_range(
            EditorPosition::new(0, 4)..EditorPosition::new(1, 5),
            "value",
        );

        assert_eq!(buffer.text(), "let value = 2;");
    }

    #[test]
    fn search_returns_line_aware_matches() {
        let buffer = EditorBuffer::from_text("alpha\nbeta alpha");
        let matches = buffer.search("alpha");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[1].range.start, EditorPosition::new(1, 5));
    }

    #[test]
    fn syntax_classifier_marks_keywords_and_comments() {
        let buffer = EditorBuffer::from_text("fn main() // entry");
        let tokens = buffer.syntax_tokens(0);

        assert_eq!(tokens[0].kind, SyntaxTokenKind::Keyword);
        assert_eq!(
            tokens.last().map(|token| token.kind),
            Some(SyntaxTokenKind::Comment)
        );
    }

    #[test]
    fn range_text_and_replacement_respect_grapheme_columns() {
        let mut buffer = EditorBuffer::from_text("a👍🏽b\ncafé");
        let range = EditorPosition::new(0, 1)..EditorPosition::new(1, 2);

        assert_eq!(buffer.text_in_range(range.clone()), "👍🏽b\nca");
        buffer.replace_range(range, "x");
        assert_eq!(buffer.text(), "axfé");
    }

    #[test]
    fn selection_replacement_updates_cursor_across_lines() {
        let buffer = EditorBuffer::from_text("alpha beta");
        let edit = edit_replacing_selections(
            &buffer,
            &[EditorSelection::new(
                EditorPosition::new(0, 6),
                EditorPosition::new(0, 10),
            )],
            "one\ntwo",
        );

        assert_eq!(edit.buffer.text(), "alpha one\ntwo");
        assert_eq!(
            edit.selections,
            vec![EditorSelection::cursor(EditorPosition::new(1, 3))]
        );
    }

    #[test]
    fn multi_cursor_replacement_translates_later_cursors() {
        let buffer = EditorBuffer::from_text("abcd");
        let edit = edit_replacing_selections(
            &buffer,
            &[
                EditorSelection::cursor(EditorPosition::new(0, 1)),
                EditorSelection::cursor(EditorPosition::new(0, 3)),
            ],
            "\nx",
        );

        assert_eq!(edit.buffer.text(), "a\nxbc\nxd");
        assert_eq!(
            edit.selections,
            vec![
                EditorSelection::cursor(EditorPosition::new(1, 1)),
                EditorSelection::cursor(EditorPosition::new(2, 1)),
            ]
        );
    }

    #[test]
    fn backspace_and_delete_join_lines() {
        let buffer = EditorBuffer::from_text("one\ntwo");
        let backspace = edit_deleting(
            &buffer,
            &[EditorSelection::cursor(EditorPosition::new(1, 0))],
            true,
        );
        let delete = edit_deleting(
            &buffer,
            &[EditorSelection::cursor(EditorPosition::new(0, 3))],
            false,
        );

        assert_eq!(backspace.buffer.text(), "onetwo");
        assert_eq!(delete.buffer.text(), "onetwo");
        assert_eq!(backspace.selections[0].head, EditorPosition::new(0, 3));
    }

    #[test]
    fn movement_clamps_columns_and_extends_selection() {
        let buffer = EditorBuffer::from_text("abcdef\nx");
        let cursor = EditorSelection::cursor(EditorPosition::new(0, 5));
        let moved = move_selections(&buffer, std::slice::from_ref(&cursor), "down", false);
        let extended = move_selections(&buffer, &[cursor], "left", true);

        assert_eq!(moved[0].head, EditorPosition::new(1, 1));
        assert_eq!(extended[0].anchor, EditorPosition::new(0, 5));
        assert_eq!(extended[0].head, EditorPosition::new(0, 4));
    }

    #[test]
    fn session_tracks_bounded_undo_and_redo_history() {
        let mut session = EditorSession::new(EditorBuffer::from_text("a")).history_limit(2);
        session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(0, 1))]);
        session.insert("b");
        session.insert("c");
        session.insert("d");
        session.apply(super::EditorEdit::new(
            session.buffer().clone(),
            vec![EditorSelection::cursor(EditorPosition::new(0, 1))],
        ));

        assert_eq!(session.buffer().text(), "abcd");
        assert!(session.undo());
        assert_eq!(session.buffer().text(), "abc");
        assert!(session.undo());
        assert_eq!(session.buffer().text(), "ab");
        assert!(!session.undo());
        assert!(session.redo());
        assert_eq!(session.buffer().text(), "abc");
    }

    #[test]
    fn replace_all_is_counted_and_undoable() {
        let mut session = EditorSession::new(EditorBuffer::from_text("one two one"));
        assert_eq!(session.replace_all("one", "three"), 2);
        assert_eq!(session.buffer().text(), "three two three");
        assert!(session.undo());
        assert_eq!(session.buffer().text(), "one two one");
        assert_eq!(session.replace_all("", "ignored"), 0);
    }

    #[test]
    fn selected_lines_indent_and_dedent_with_adjusted_selections() {
        let buffer = EditorBuffer::from_text("alpha\nbeta\ngamma");
        let selection = EditorSelection::new(EditorPosition::new(0, 2), EditorPosition::new(2, 0));
        let indented = edit_indenting_lines(&buffer, &[selection], "  ", false);
        assert_eq!(indented.buffer.text(), "  alpha\n  beta\ngamma");
        assert_eq!(indented.selections[0].anchor, EditorPosition::new(0, 4));
        let dedented = edit_indenting_lines(&indented.buffer, &indented.selections, "  ", true);
        assert_eq!(dedented.buffer.text(), buffer.text());
    }

    #[test]
    fn page_movement_clamps_and_can_extend_selection() {
        let buffer = EditorBuffer::from_text("zero\none\ntwo\nthree\nfour");
        let cursor = EditorSelection::cursor(EditorPosition::new(1, 3));
        let down = move_selections_by_page(&buffer, std::slice::from_ref(&cursor), 3, true, false);
        let up = move_selections_by_page(&buffer, &[cursor], 3, false, true);
        assert_eq!(down[0].head, EditorPosition::new(4, 3));
        assert_eq!(up[0].anchor, EditorPosition::new(1, 3));
        assert_eq!(up[0].head, EditorPosition::new(0, 3));
    }

    #[test]
    fn search_navigation_wraps_after_the_last_match() {
        let mut session = EditorSession::new(EditorBuffer::from_text("one two one"));
        session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(0, 10))]);
        assert!(session.select_next_match("one"));
        assert_eq!(
            session.selections()[0].ordered().start,
            EditorPosition::new(0, 0)
        );
        assert!(!session.select_next_match("absent"));
    }
}
