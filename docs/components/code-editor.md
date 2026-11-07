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
Dragging outside the viewport scrolls while the pointer continues moving.

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

Buffer clones and undo snapshots share immutable line chunks. An ordinary edit
copies affected chunks and rebuilds the small chunk index, rather than cloning
every line. Byte and UTF-16 summaries locate native input ranges without joining
the document or scanning all preceding lines. `line(index)` and `iter_lines()`
access shared storage directly. The compatible `lines()` API materializes and
caches a contiguous view; `text()` explicitly allocates the full document.

The viewport caches row boundaries per shared chunk. Unchanged frames reuse the
index; text edits remeasure changed chunks. Work exceeding either 16 KiB or 256
logical lines runs on the background executor, including initial measurement and
wrap-width/font changes. Small incremental updates remain synchronous. Workers
use their own text-layout caches and shared thread-safe font services; they do
not access the window or editor entity.

During width changes, the previous completed wrapping remains usable until the
new layout is ready. Initial loading or a replacement document uses a temporary
unwrapped preview of visible lines, capped at 256 graphemes and 4 KiB per line.
An ellipsis marks a truncated preview. Input still updates the full document;
preview truncation never changes text. Candidate bounds and the caret are withheld
when their position is outside the measured preview, then restored after reflow.

Each job belongs to a document snapshot, font, width, and generation. New requests
cancel superseded work; workers check cancellation between chunks and during long
lines. Results are validated before adoption, so old jobs cannot overwrite newer
text or layout settings. Completion preserves the current logical scroll anchor,
including scrolling performed while the worker ran. Pending input-driven caret
reveal is retained, while explicit scrolling cancels it. Old layout metadata is
also released on a background thread. An unchanged frame schedules no reflow.

Grapheme advances are cached for the monospace layout. Non-wrapped lines are
indexed into segments of at most 128 graphemes during background measurement.
Each segment is shaped as a unit, with its logical tab stops preserved. Horizontal
scrolling paints only segments intersecting the viewport. Cursor reveal, hit
testing, native text geometry, and cross-segment selection use the same segment
origins; line numbers remain fixed even when the first text segment is offscreen.
Wrapped lines continue to shape only their visible visual rows.

Syntax classification and sparse byte/grapheme/UTF-16 checkpoints are computed
alongside layout, with cancellation checks during long scans. Immutable line
chunks share those results across snapshots, redraws, and width/font changes.
Only changed lines receive fresh syntax caches; unchanged neighboring lines retain
their analysis even when chunks are rebuilt. Painting only borrows cached token ranges;
it never classifies an entire line or clones its full token list. Strings and
comments beginning offscreen retain their classification in visible segments.
After analysis completes, position queries binary-search a checkpoint and scan
at most one 128-grapheme interval instead of walking from the beginning of a long
line. `EditorBuffer::syntax_tokens()` remains an explicit synchronous API on a
cache miss and returns an owned token list; repeated calls reuse classification.

Text-position indexes are independent of syntax classification. Edits reuse
prefix checkpoints and scan from a preceding grapheme boundary until the new text
converges with the old suffix, then translate the remaining checkpoints. Line
splits and joins reuse the indexes of both boundary lines. Cached UTF-16 totals
avoid rescanning unchanged text when rebuilding summaries. ASCII-only lines use
byte offsets directly when constructing checkpoints. An uncached position index
is built once on demand; subsequent queries and history restores reuse it.

Large new Unicode position indexes can use up to four scoped workers. Partitions
are aligned using Unicode grapheme boundaries before workers run, so combining
marks, emoji ZWJ sequences, regional indicators, and Indic conjuncts retain the
same columns and UTF-16 offsets as serial indexing. Small inputs, single-core
hosts, and worker-creation failures use serial indexing. Boundary probes use
bounded context; long indivisible clusters and regional-indicator runs fall back
to one serial pass instead of repeatedly scanning the line to find partitions.
These helpers remain synchronous for explicit model APIs such as
`EditorSession::insert`; the interactive surface schedules large pastes separately.

Native or clipboard insertions of at least 64 KiB are prepared on GPUI's background
executor. The surface never joins the preparation worker. Normalization, text
replacement, cursor/index construction, and change detection finish before the
result is published. Verified negative snapshot comparisons prevent the controlled
host from rescanning equally sized documents when accepting an edit. Position
indexes are built outside their publication lock; foreground queries do not wait
for another thread's long initialization.

The last committed document stays visible with a small "Pasting…" indicator.
Wheel and scrollbar input continue immediately. Subsequent typing, editing keys,
clipboard commands, and undo/redo are processed in arrival order. Clipboard pastes
capture their requested contents, including a preceding queued copy/cut. Undo/redo
wait for the host to acknowledge preceding edits and render the command result
before later input runs. Each paste and committed IME composition remains a
separate undo transaction. Pointer selection of the document is temporarily disabled
while its replacement is pending; keyboard navigation is queued.

While preparation or queued input is pending, native IME queries address a small
composition context in the indicator, not the large document. Its UTF-16 text,
selection, and marked ranges remain immediately queryable; preedit is underlined
and candidate geometry follows that context. It survives worker completion until
the composition commits, unmarks, cancels, or loses focus. The normal document
input context is restored only after this context and the queue have drained.
This intentionally limits surrounding-text context during a pending paste.

Results carry a generation and source snapshot/selection. A newly supplied host
value, loss of editability, or removal of the editor invalidates pending work.
Superseded results never publish over a replacement already delivered by the host.
Preparation checks cancellation between stages and position-index updates;
a running indivisible scan can finish before cancellation is observed. Completion
and cancellation release unused prepared snapshots away from the foreground.
Clipboard acquisition and the required ownership copy of borrowed native input
still follow GPUI's synchronous platform API; document preparation does not.
Application callbacks can also perform their own synchronous work.

`EditorSession::clear_history()` discards both history stacks without changing
text or selections. Large retired batches from explicit clearing, limit trimming,
redo invalidation, disabled history, and session destruction are handed to one
background release worker. Its queue holds at most eight batches. Small batches,
an unavailable worker, or a full queue use ordinary synchronous destruction;
there is no unbounded disposal queue. Other owners of a shared snapshot retain
it normally, and releasing their final reference may still be synchronous.
Deferred disposal temporarily retains the queued batches' memory.

Pastes borrow LF-only input, normalize CRLF/CR in one allocation when needed,
and construct replacement lines directly. Multi-cursor edits resolve original
byte ranges once, then derive final cursors from byte deltas; inserting combining
marks does not move the caret past the following grapheme. Undo/redo restore shared
text and position indexes, and pending layout jobs still validate their snapshot
before applying results. New syntax classification remains in background reflow
for large changes.

Run `cargo bench -p guic-editor --bench editor_models --profile dev --locked` to
measure model operations on large documents and long lines. Workloads cover
100,000-line buffers, shared snapshots, tail-range queries, cached syntax,
replacement and undo/redo, ASCII and Unicode pastes, and history disposal.
These benchmarks measure model work and disposal submission, not native frame
latency, clipboard acquisition, or background reclamation completion. Use the
[platform smoke checklist](../platform-smoke.md) to validate interactive behavior.

## Remaining limitations

The editor remains experimental. It does not yet provide timer-driven drag
autoscroll while the pointer is stationary, a multi-cursor creation gesture,
rectangular selection, folding, or an integrated LSP client. Constructing or
serializing the buffer and explicit uncached model queries still take synchronous
work. Each changed logical line remains a contiguous string, so rewriting it
copies its bytes; explicit model insertion still waits for initial indexing.
The native surface moves large paste preparation to the background. Sparse
checkpoint vectors and the root chunk index are copied during editing. Grapheme
boundary changes can require scanning a long suffix, and multiple edits on the
same line still copy that line separately. History disposal can still fall back
to synchronous release when its bounded worker queue is full or unavailable.
These remaining costs need native latency and memory profiling before a general
responsiveness guarantee. Word selection/search, unusually
large indivisible graphemes, and overlapping zero-advance clusters also need
further profiling. Segment shaping
is intended for the editor's monospace layout; full contextual/bidirectional text
editing remains outside its current contract. Background analysis and viewport
segmentation are not a latency guarantee for every editing or file-loading operation.

The primary caret is rendered without blinking. Full accessibility text ranges,
bidirectional editing, and native IME behavior with real desktop input methods
still require platform validation. Automated native-handler and rendering tests
do not replace the [platform smoke checklist](../platform-smoke.md).
