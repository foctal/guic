# SearchInput

`SearchInput` is an alias for `TextInput`. Use `TextInput::search` to create
a single-line search field; calling the alias's `new` method creates a plain input.

```rust
use guic::prelude::TextInput;

let input = cx.new(|cx| TextInput::search("search-input", cx).placeholder("Enter text"));
```

Retain the entity and render `input.clone()`.

See [TextInput](text-input.md) for editing, callbacks, and keyboard bindings.
