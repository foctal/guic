# Radio

## Purpose

Represent a single option in a mutually exclusive choice group.

## Import

```rust
use guic::prelude::{Radio, ComponentSize};
```

## Basic Usage

```rust
Radio::new("plan-pro").label("Pro").checked(true)
```

## Keyboard Behavior

Call `.focusable(focus_handle)` to opt into keyboard interaction. When focused,
`Space` and `Enter` select the radio. Radio-group roving focus remains
host-managed.

## Theming Behavior

Radio colors and sizing derive from the active theme tokens.
