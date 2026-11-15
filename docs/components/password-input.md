# PasswordInput

`PasswordInput` is an alias for `TextInput`. Use `TextInput::password` to create
a masked single-line field; calling the alias's `new` method creates a plain input.

```rust
use guic::prelude::TextInput;

let input = cx.new(|cx| TextInput::password("password-input", cx).placeholder("Enter text"));
```

Retain the entity and render `input.clone()`.
Masking changes rendering; the entity still stores the original value.
Copy and Cut do not export password text or change the selection or value.
Paste and Undo/Redo remain available.

Masked fields display one centered bullet per grapheme, including combined
Unicode characters. Call `set_password_visible(visible, cx)` on the retained
entity to implement a reveal control. Revealing changes only the glyphs:
the field remains a password input, does not expose its value through
accessibility, and continues to block Copy and Cut.

See [TextInput](text-input.md) for editing, callbacks, and keyboard bindings.
