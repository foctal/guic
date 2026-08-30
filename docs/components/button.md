# Button

## Purpose

Trigger a primary, secondary, ghost, or destructive action.

## Import

```rust
use guic::prelude::{Button, ButtonVariant, ComponentSize};
```

## Basic Usage

```rust
Button::new("Save").primary()
```

## Variants

- `ButtonVariant::Solid`
- `ButtonVariant::Primary`
- `ButtonVariant::Secondary`
- `ButtonVariant::Ghost`
- `ButtonVariant::Danger`

## Keyboard Behavior

Enabled buttons with an action participate in tab navigation. Enter and Space
activate the focused button. Disabled buttons are omitted from this interaction
path.

## Theming Behavior

Button colors, border radius, spacing, and typography all come from the active
theme tokens.
