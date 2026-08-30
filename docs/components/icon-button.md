# IconButton

## Purpose

Trigger a compact action when an icon-only affordance is sufficient.

## Import

```rust
use guic::prelude::{ButtonVariant, IconButton, IconName};
```

## Basic Usage

```rust
IconButton::new(IconName::Search)
    .variant(ButtonVariant::Secondary)
    .label("Search")
```

## Keyboard Behavior

Enabled icon buttons with an action participate in tab navigation. Enter and
Space activate the focused button. Disabled icon buttons are omitted from this
interaction path.

## Theming Behavior

Icon buttons share the same token-driven sizing, border, and variant palette as
`Button`.
