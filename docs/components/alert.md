# Alert

## Purpose

Surface a contextual message that needs stronger emphasis than a normal label.

## Import

```rust
use guic::prelude::{Alert, AlertVariant};
```

## Basic Usage

```rust
Alert::new("Saved successfully").title("Status").success()
```

## Variants

- `AlertVariant::Neutral`
- `AlertVariant::Info`
- `AlertVariant::Success`
- `AlertVariant::Warning`
- `AlertVariant::Danger`

## Keyboard Behavior

Alerts are passive by default. When a close button is enabled, the close action
currently responds to pointer interaction only.

## Theming Behavior

Alert background, foreground, border, spacing, and radius values all come from
the active theme tokens.
