# CodeEditor

Render a controlled native code editor surface backed by a line-oriented buffer,
selection metadata, diagnostics, search, and lightweight syntax classification.

```rust
use guic::prelude::{CodeEditor, CodeEditorOptions, EditorBuffer, EditorSession};

let session = EditorSession::new(EditorBuffer::from_text(
    "fn main() {\n    println!(\"hello\");\n}",
));

CodeEditor::new("source", session.buffer().clone())
    .selections(session.selections().to_vec())
    .options(CodeEditorOptions::default().visible_lines(16))
    .on_edit(|edit, _, _| {
        // Apply the edit to application-owned state and render again.
        let _ = edit;
    })
```

## Editing Model

`EditorSession` owns a buffer, grapheme-based selections, and bounded undo/redo
history. `CodeEditor::on_edit` reports both the updated buffer and selections so
controlled applications do not lose cursor state between renders.

Supply `focusable(handle)` to enable keyboard editing. It supports text insertion,
selection replacement, Backspace/Delete, Enter, Tab, arrows, Home/End, Shift selection extension,
Select All, and clipboard copy/cut/paste. Buffer operations preserve Unicode
grapheme boundaries and edit only the affected lines.

## Limitations

Enable the `editor` feature on `guic`, or depend on `guic-editor` directly.
The editor is experimental: it does not provide IME input, pixel-accurate caret
placement, or built-in scroll-wheel handling. Click callbacks identify a line's
start; selection highlighting covers whole lines. Applications control the
visible range through `CodeEditorOptions`.
