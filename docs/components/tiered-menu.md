# TieredMenu

## Purpose

Render hierarchical command groups with nested menu items.

## Import

```rust
use guic::prelude::{MenuItem, TieredMenu};
```

## Basic Usage

```rust
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

## Notes

`TieredMenu` is host-managed. It renders nested children recursively and emits
activation only for enabled leaf action items.
