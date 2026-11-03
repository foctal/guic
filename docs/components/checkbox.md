# Checkbox

Display a checked or unchecked option.

```rust
use guic::prelude::{Checkbox, ComponentSize};

Checkbox::new("terms").label("Accept terms").checked(true)
```

## Keyboard

Call `.focusable(focus_handle)` to opt into keyboard interaction. When focused,
`Space` and `Enter` invoke the toggle handler. Disabled checkboxes cannot be
focused or activated.
