# Switch

## Purpose

Toggle a binary feature flag with a more immediate on/off affordance than a
checkbox.

## Import

```rust
use guic::prelude::{ComponentSize, Switch};
```

## Basic Usage

```rust
Switch::new("feature-x").label("Feature X").checked(true)
```

## Keyboard Behavior

Call `.focusable(focus_handle)` to opt into keyboard interaction. When focused,
`Space` and `Enter` invoke the toggle handler. Disabled switches cannot be
focused or activated.

## Theming Behavior

Switch track color, knob size, and spacing are derived from the active theme.
Keyboard focus uses the theme's focus-ring color.

Switches expose native switch semantics, including their checked state, to the
platform accessibility tree.
