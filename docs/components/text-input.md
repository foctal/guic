# TextInput

`TextInput` is the baseline GPUI-backed single-line field in GUIC.

Highlights:

- Native GPUI input handling via `EntityInputHandler`
- Clipboard actions and selection shortcuts
- Token-driven sizing and focus styling
- Platform text-input semantics with an accessible label and disabled state
- Shared implementation used by `SearchInput`, `PasswordInput`, and `TextArea`

Use `.accessible_label(...)` when the visible label differs from the
placeholder. Otherwise, the placeholder is used as the accessibility fallback.

Story coverage:

- `crates/guic-story/src/main.rs`

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
