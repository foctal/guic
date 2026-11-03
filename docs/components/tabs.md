# Tabs

Switch between multiple related content panels while preserving a compact
navigation surface.

```rust
use guic::prelude::{TabItem, Tabs};

Tabs::new("settings-tabs").items(vec![
    TabItem::new("general", "General"),
    TabItem::new("account", "Account"),
])
```

## Keyboard

With `on_select` set, enabled tabs participate in tab navigation. Arrow keys
request the previous/next enabled tab, wrapping at the ends. Home/End requests
the first/last enabled tab; Enter/Space activates the focused tab. Store the
requested index and pass it back through `selected`.
