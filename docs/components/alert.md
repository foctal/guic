# Alert

Surface a contextual message that needs stronger emphasis than a normal label.

```rust
use guic::prelude::{Alert, AlertVariant};

Alert::new("Saved successfully").title("Status").success()
```

## Variants

- `AlertVariant::Neutral`
- `AlertVariant::Info`
- `AlertVariant::Success`
- `AlertVariant::Warning`
- `AlertVariant::Danger`

## Keyboard

Alerts are passive by default. With `closable(true)` and `on_close` set, the
close button supports pointer activation and Enter/Space when focused.
