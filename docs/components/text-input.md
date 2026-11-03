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

## Keyboard bindings

The default editing bindings use Command+A/C/X/V on macOS and Control+A/C/X/V
on Windows and Linux. They are scoped to `GuicTextInput`, so controls outside
that context (including terminal views) keep their own Control+C behavior.
Initialize GUIC before installing application overrides; repeated initialization
does not overwrite those overrides.

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
