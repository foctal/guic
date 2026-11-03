# TieredMenu

Render hierarchical command groups with nested menu items.

```rust
use guic::prelude::{MenuItem, TieredMenu};

TieredMenu::new("create-menu")
    .items(vec![
        MenuItem::new("new", "New").children(vec![
            MenuItem::new("project", "Project"),
            MenuItem::new("file", "File"),
        ]),
        MenuItem::separator(),
        MenuItem::new("settings", "Settings"),
    ])
    .on_activate(|id, _, _| {
        let _ = id;
    })
```

`TieredMenu` is host-managed. It renders nested children recursively and emits
activation only for enabled leaf action items.
