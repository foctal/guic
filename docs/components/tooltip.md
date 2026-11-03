# Tooltip

`Tooltip` shows a short message using GPUI's tooltip lifecycle. It preserves
the wrapped element's layout and interaction.

```rust
use guic::prelude::{Button, Tooltip};

Tooltip::new(Button::new("Save"), "Save this document").id("save-tooltip")
```

Supply distinct IDs for wrappers created in a loop or shared factory.
Buttons and icon buttons also provide a `.tooltip(...)` builder.
