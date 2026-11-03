# TextArea

`TextArea` is an alias for `TextInput`. Use `TextInput::text_area` to create
a multiline text field; calling the alias's `new` method creates a plain input.

```rust
use guic::prelude::TextInput;

let input = cx.new(|cx| TextInput::text_area("text-area", cx).placeholder("Enter text"));
```

Retain the entity and render `input.clone()`.
Up/Down moves between visual lines, including soft wraps, while preserving
the preferred horizontal position. Shift+Up/Down extends the selection.
Horizontal movement and editing reset that position.

See [TextInput](text-input.md) for editing, callbacks, and keyboard bindings.
