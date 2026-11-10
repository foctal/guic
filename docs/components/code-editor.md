# CodeEditor

`CodeEditor` is a controlled native editor backed by a line-oriented buffer,
grapheme selections, diagnostics, search, and lightweight syntax classification.
Enable the `editor` feature on `guic`, or depend on `guic-editor` directly.

```rust
use guic::prelude::{CodeEditor, CodeEditorOptions, EditorBuffer, EditorSession};

let session = EditorSession::new(EditorBuffer::from_text(
    "fn main() {\n    println!(\"hello\");\n}",
));

CodeEditor::new("source", session.buffer().clone())
    .selections(session.selections().to_vec())
    .options(CodeEditorOptions::default().visible_lines(16).soft_wrap(true))
    .on_edit(|edit, _, _| {
        // Apply the edit to application-owned state and render again.
        let _ = edit;
    })
```

## Ownership and input

Retain an `EditorSession` and a GPUI focus handle in the application view. Pass
`focusable(handle)` to enable keyboard focus and install native text input when
`on_edit` or `on_change` is present. Without either edit callback, a focusable
editor permits selection and copying but rejects text changes.

`EditorSession` owns the committed buffer, grapheme-based selections, and bounded
undo/redo history. In `on_edit`, call `session.apply(edit.clone())` and notify the
host entity. Supply an `on_command` callback that calls `session.undo()` or
`session.redo()` for the corresponding `EditorCommand`, then notifies the host.
The component gallery demonstrates this wiring. Large paste callbacks are
asynchronous. Apply an undo/redo command synchronously in its callback and notify
the host: queued input resumes after the resulting controlled value is rendered.

Keep the editor's ID stable and unique within its parent. Internal state retains
composition, pointer selection, and scroll offsets between renders. A changed
host buffer or changed explicit selections override that state; repeating the
same props does not erase pending native input. Removing the editor or changing
its ID discards its transient state.

Text insertion goes through GPUI's native input handler, including Japanese,
Chinese, and Korean composition. UTF-16 offsets from the platform are converted
to bounded Unicode text ranges; composition-relative selection is interpreted
against the replacement text. The marked text is underlined. Candidate geometry
uses window-local logical pixels and includes both scroll offsets and the gutter.

During composition, preedit text is displayed locally. `on_edit` and `on_change`
receive the committed result when the IME commits or unmarks text, or when an
active field loses focus. An `EditorSession` therefore records the composition
as one undoable edit. Cancelling a preedit that leaves the original text and
selection unchanged emits no edit. Applications do not receive intermediate
preedit changes. Changing the host document cancels the old composition.

`on_edit` reports text and selection changes; `on_change` only reports text
changes. `on_cursor` reports the primary selection head, including pointer hits
at actual grapheme columns. Use `on_edit` for editable surfaces so the host keeps
selection state as well as text. IME composition targets the primary selection;
ordinary insertion and deletion retain the existing multiple-selection model.

`EditorBuffer` normalizes CRLF and lone CR line endings to LF. This also applies
to insertion, paste, and replacement. Applications preserving a file's original
line-ending convention should convert LF when saving.

## Selection and navigation

The same shaped glyph positions drive painting, hit testing, and native input
geometry. Selections highlight their text spans rather than entire rows.
Clicks and drag selections snap to grapheme boundaries, including combining
characters and emoji sequences. Shift+click extends from the anchor, double-click
selects a word segment, and triple-click selects a line including its newline.
Dragging outside the viewport scrolls even while the pointer is stationary.
Release or blur stops the timer. All visible collapsed selections draw a cursor.
Focused cursors blink at idle and remain visible during composition; set
`CodeEditorOptions::cursor_blink(false)` for a steady cursor.

Keyboard editing includes Backspace/Delete, Enter, Tab indentation, arrows,
Home/End, Page Up/Down, Shift selection extension, Select All, and clipboard
copy/cut/paste. Up/Down preserve the preferred horizontal pixel position across
short lines. Editing and keyboard navigation reveal the primary caret in both
axes. Tabs use four-column stops, with matching painting and pointer geometry.

## Viewport and diagnostics

`visible_lines` defines a fixed-height viewport, even for a short document.
The wheel or trackpad scrolls vertically and horizontally; Shift+vertical-wheel
scrolls horizontally. Line numbers remain fixed while text scrolls. Only visible
text fragments and a small overscan are shaped for painting. The input surface
does not request another frame from painting.

`soft_wrap(true)` wraps at Unicode line-break opportunities and falls back to
whole graphemes for words wider than the viewport. It is disabled by default.
Continuation rows start at the left text margin; their line number is omitted.
Tabs retain their logical four-column stops across wrapped fragments. Wrapping
does not insert newlines or change buffer positions, diagnostics, or undo history.
With one active selection, Up/Down, Page Up/Down, and unmodified Home/End follow
visual rows. A caret at the end of a wrapped row retains its upstream affinity until
navigation or input changes it. IME bounds and pointer hits use the same fragments.

`scrollbars(true)` is the default. Overflow shows a draggable thumb; clicking the
track pages the viewport. Drag capture continues outside the editor and never
changes text selection. The vertical rail reserves space to avoid a width/reflow
feedback loop; a horizontal rail is reserved only for overflow. A wrapped document
normally needs only a vertical bar, but an indivisible wide grapheme or inline
diagnostic may still require horizontal scrolling. Set `scrollbars(false)` to hide
the rails while retaining wheel and keyboard scrolling.

`first_line` addresses a logical buffer line and sets the initial viewport
position. Changing it later repositions the viewport; repeated renders with the
same value preserve user scrolling. Resizing preserves the logical position of
the top visible fragment. When a document shrinks, offsets are clamped.

Diagnostics retain inline text, severity coloring, and `on_diagnostic` activation.
They appear after the final fragment of their logical line and contribute to the
horizontal extent. Activating a diagnostic does not move the selection.

## Large documents

Buffer clones and undo snapshots share immutable line chunks. Use `line(index)`
and `iter_lines()` to inspect them without allocating a contiguous copy.
`lines()` materializes and caches a contiguous view; `text()` allocates the full
document. Explicit model operations, including initial construction and uncached
queries, are synchronous.

The surface measures large documents and width/font changes on background
workers. Only visible rows or horizontal segments are shaped for painting.
Unchanged text shares syntax and position indexes. While measurement is pending,
the previous layout remains visible; a new document uses a bounded preview with
an ellipsis for truncated lines. Preview truncation never changes the buffer.
Caret and IME geometry become available when their position is measured.
Superseded work is cancelled and cannot replace a newer document or layout.

Clipboard reads and large paste preparation run asynchronously. Subsequent
edits, clipboard commands, navigation, and undo/redo retain input order, while
scrolling stays responsive. Each paste and committed composition is a separate
undo transaction. Pointer selections preview immediately and are rebased through
queued edits. Unknown host history transitions use the host's restored selection.
An empty or unavailable clipboard leaves the selection intact.

During pending input, IME queries use a small composition context beside the
"Pasting…" indicator. This limits surrounding-text context until the queue drains.
Replacing the host document, disabling editing, or removing the editor cancels
pending work. Application callbacks and copying borrowed native input can still
perform synchronous work.

`EditorSession::clear_history()` discards undo and redo without changing text or
selections. Large retired history batches are released by a bounded background
worker. A full queue or unavailable worker falls back to synchronous release.
Other owners keep shared snapshots alive; deferred disposal temporarily retains
memory until the worker finishes.

Run `cargo bench -p guic-editor --bench editor_models --profile dev --locked` for
model workloads covering large buffers, dense search, Unicode edits, snapshots,
and history disposal. These timings do not measure native frame latency,
clipboard acquisition, or completion of background reclamation.

## Limitations

The editor remains experimental. It does not provide click-to-add cursors,
folding, or an integrated LSP client. Each changed logical line is a contiguous
string, so editing it copies its bytes. Very long lines, large indivisible
graphemes, and multiple edits on one line can still be expensive. Background
processing does not guarantee a latency bound for every operation.

The layout is intended for monospace text. Full bidirectional editing and
accessibility text ranges are not supported. Native IME, screen-reader behavior,
and interactive latency require the [platform smoke checks](../platform-smoke.md);
automated model and rendering tests do not establish those guarantees.

## Rectangular selection

Hold **Shift+Alt** and drag with the left mouse button to select a rectangle
(on macOS, Alt is Option). Each display row contributes one selection, including
soft-wrapped rows. A vertical drag at the same horizontal position creates
multiple cursors. Ordinary clicks and Shift-clicks keep their existing behavior.

Hit testing uses shaped text and grapheme boundaries, including tabs, emoji,
and combining characters. Short rows clamp to their end without narrowing the
rectangle on longer rows; no padding or virtual spaces are inserted. Ranges are
ordered from top to bottom. Dragging outside the viewport continues scrolling;
release, blur, or an authoritative host replacement ends the drag.

Typing and pasting use the existing multiple-selection behavior: the same text
replaces every selected range. Edits over many selections use the ordered
background queue, including small typed replacements and deletion. Copy and cut
join nonempty selected ranges with
newlines. IME composition follows the native primary-selection contract and
commits one edit; Undo restores the previous text and rectangular selections.
During a pending paste, the rectangle is previewed immediately and all ranges
are rebased through the queued edits and history operations before later input.
