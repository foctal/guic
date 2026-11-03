# Toolbar

Group actions and controls in a bordered horizontal row, with separators
between logical groups and a flexible spacer for trailing-edge alignment.

```rust
use guic::prelude::Toolbar;

Toolbar::new()
    .child(Button::new("New"))
    .child(Button::new("Open"))
    .separator()
    .child(Button::new("Save"))
    .spacer()
    .child(Button::new("Settings"))
```
