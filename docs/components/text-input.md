# TextInput

`TextInput` is the baseline GPUI-backed single-line field in GUIC.

Create the input as an entity, retain it in the view, and render a clone:

```rust
use guic::prelude::TextInput;

let input = cx.new(|cx| TextInput::new("name", cx).placeholder("Name"));
```

Read its value with `input.read(cx).current_value()`, or register `on_change`
when constructing it. `accessible_label(...)` sets the accessible name; otherwise
the placeholder is used. `disabled(true)` prevents editing.

Use `TextInput::search`, `password`, or `text_area` for the related input modes.
They share clipboard, selection, and grapheme-aware editing behavior.

For a retained entity, use `set_value(value, cx)` to apply a form reset or a
programmatic update, and `set_disabled(disabled, cx)` to change editability.
`set_value` preserves the entity and focus handle, moves the caret to the end,
and clears composition and undo/redo history. It does not invoke `on_change`
or emit `ValueChange`; those report user edits. Disabling finishes the local
composition and prevents both native input and editing actions.

CRLF and lone CR line endings become LF in text areas and spaces in single-line
fields. Native selection and composition ranges use UTF-16 code units; GUIC
clamps out-of-range offsets and keeps selections on Unicode scalar boundaries.

## Keyboard bindings

The default editing bindings use Command+A/C/X/V on macOS and Control+A/C/X/V
on Windows and Linux. They are scoped to `GuicTextInput`, so controls outside
that context (including terminal views) keep their own Control+C behavior.
Initialize GUIC before installing application overrides; repeated initialization
does not overwrite those overrides.

Undo uses Command+Z on macOS and Control+Z on Windows and Linux. Redo uses
Command+Shift+Z on macOS, and Control+Shift+Z or Control+Y elsewhere. The public
`Undo` and `Redo` actions can also be rebound. Each edit is one history entry;
an IME composition, including its intermediate updates, is one entry. Ordinary
typing is not coalesced into words. Undo restores the text and selection, and
a new edit after undo discards the redo branch. No-op edits emit no change event.
Each history stack retains at most 100 snapshots and 4 MiB of text, dropping
oldest snapshots first. A snapshot larger than that budget is not retained.

Typed actions are public under `guic_components::text_input` (also available
through `guic::components::text_input`). For example:

```rust,ignore
use guic_components::text_input::SelectAll;

cx.bind_keys([gpui::KeyBinding::new(
    "ctrl-a",
    SelectAll,
    Some("GuicTextInput"),
)]);
```

Single-line inputs propagate `Newline` so a containing command palette can
handle submission. Multiline inputs insert a newline.

Clipboard text is acquired asynchronously. Input and history commands received
while a read is pending replay in order, and clicks show a selection preview
whose offsets are rebased through the preceding edits. Native composition uses
a small temporary context until the paste completes; its committed text remains
one history entry. Empty clipboard reads do not erase selections. `set_value`
and disabling the field cancel pending input so stale clipboard results cannot
replace application-owned state. Text storage, shaping, and history snapshots
remain synchronous; use `CodeEditor` for large documents.
