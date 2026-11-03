# Button

Trigger a primary, secondary, ghost, or destructive action.

```rust
use guic::prelude::{Button, ButtonVariant, ComponentSize};

Button::new("Save").primary()
```

## Variants

- `ButtonVariant::Solid`
- `ButtonVariant::Primary`
- `ButtonVariant::Secondary`
- `ButtonVariant::Ghost`
- `ButtonVariant::Danger`

## Keyboard

Enabled buttons with an action participate in tab navigation. Enter and Space
activate the focused button. Disabled buttons are omitted from this interaction
path.
