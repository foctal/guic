# Radio

Represent a single option in a mutually exclusive choice group.

```rust
use guic::prelude::{Radio, ComponentSize};

Radio::new("plan-pro").label("Pro").checked(true)
```

## Keyboard

Call `.focusable(focus_handle)` to opt into keyboard interaction. When focused,
`Space` and `Enter` select the radio. Radio-group roving focus remains
host-managed.
