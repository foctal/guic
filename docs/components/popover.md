# Popover

`Popover` displays content below a trigger without changing the trigger's layout.
The application controls its open state and wires the trigger's action.

```rust
use guic::prelude::{Button, Label, Popover};

Popover::new("help", Button::new("Help"), Label::new("Choose a project to continue."))
    .open(is_open)
    .width(280.0)
```

Use `autofocus_target(handle)` to focus a child when opening. Keep the popover
rendered with `open(false)` on close to restore focus. See
[focus lifecycle](../component-behavior.md#focus-lifecycle).
