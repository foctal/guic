# TreeTable

## Purpose

Render hierarchical records with table columns, expandable branches, and
host-managed row selection.

## Import

```rust
use guic::prelude::{TreeTable, TreeTableColumn, TreeTableRow};
```

## Basic Usage

```rust
TreeTable::new("files")
    .columns(vec![
        TreeTableColumn::new("name", "Name").width(240),
        TreeTableColumn::new("kind", "Kind"),
    ])
    .rows(vec![TreeTableRow::new("src", vec!["src", "Folder"])
        .expanded(true)
        .children(vec![TreeTableRow::new("main", vec!["main.rs", "Rust"])])])
    .on_toggle(|id, _, _| {
        let _ = id;
    })
    .on_select(|id, _, _| {
        let _ = id;
    })
```

## Notes

`TreeTable` expects the application to own row expansion and selection state.
Use `visible_row_ids` when tests or host state need the rendered row order.
