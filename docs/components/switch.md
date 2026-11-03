# Switch

Display an on/off setting.

```rust
use guic::prelude::{ComponentSize, Switch};

Switch::new("feature-x").label("Feature X").checked(true)
```

## Keyboard

Call `.focusable(focus_handle)` to opt into keyboard interaction. When focused,
`Space` and `Enter` invoke the toggle handler. Disabled switches cannot be
focused or activated.
