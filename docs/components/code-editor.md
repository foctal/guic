# CodeEditor

## Purpose

Render a controlled native code editor surface backed by a line-oriented buffer,
selection metadata, diagnostics, search, and lightweight syntax classification.

## Import

```rust
use guic::prelude::{CodeEditor, CodeEditorOptions, EditorBuffer, EditorSession};
```

## Basic Usage

```rust
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

The keyboard surface supports text insertion, selection replacement,
Backspace/Delete, Enter, Tab, arrows, Home/End, Shift selection extension,
Select All, and clipboard copy/cut/paste. Buffer operations preserve Unicode
grapheme boundaries and edit only the affected lines.

## Support Level

`guic-editor` is a dedicated subsystem crate. Enable it through the `editor`
feature on `guic`, or depend on `guic-editor` directly for editor-focused
applications. It remains a preview subsystem: pointer-accurate cursor placement,
pixel-level selection and caret painting, scroll-wheel integration, IME,
language adapters, and large-file rendering benchmarks are still release gates.
