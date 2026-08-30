# DataView

## Purpose

Render a controlled collection as either a dense list or a card-style grid.

## Import

```rust
use guic::prelude::{DataView, DataViewItem, DataViewLayout};
```

## Basic Usage

```rust
DataView::new("release-areas")
    .layout(DataViewLayout::Grid)
    .selected("runtime")
    .items(vec![
        DataViewItem::new("runtime", "Runtime")
            .description("Core focus, overlays, and theme systems")
            .metadata("Updated today")
            .badge("Preview"),
        DataViewItem::new("components", "Components")
            .description("Reusable native controls")
            .badge("Stable"),
    ])
    .on_select(|id, _, _| {
        let _ = id;
    })
```

## Notes

`DataView` is host-managed. The application owns selection and updates the
selected item id after `on_select` fires. Use `DataTable` when comparison across
columns is more important than rich per-record presentation.
