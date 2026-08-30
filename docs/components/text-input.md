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
