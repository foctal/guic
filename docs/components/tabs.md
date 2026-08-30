# Tabs

## Purpose

Switch between multiple related content panels while preserving a compact
navigation surface.

## Import

```rust
use guic::prelude::{TabItem, Tabs};
```

## Basic Usage

```rust
Tabs::new("settings-tabs").items(vec![
    TabItem::new("general", "General"),
    TabItem::new("account", "Account"),
])
```

## Keyboard Behavior

Keyboard navigation is still pending. The current implementation provides the
selection API and pointer interaction needed to exercise tab layouts in stories
and examples.

## Theming Behavior

Selected-state color, underline, spacing, and height derive from the active
theme tokens.
