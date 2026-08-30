# Checkbox

## Purpose

Capture a binary selection state with an explicit checkmark affordance.

## Import

```rust
use guic::prelude::{Checkbox, ComponentSize};
```

## Basic Usage

```rust
Checkbox::new("terms").label("Accept terms").checked(true)
```

## Keyboard Behavior

Call `.focusable(focus_handle)` to opt into keyboard interaction. When focused,
`Space` and `Enter` invoke the toggle handler. Disabled checkboxes cannot be
focused or activated.

## Theming Behavior

Checkbox colors, border radius, and sizing all come from the active theme.
