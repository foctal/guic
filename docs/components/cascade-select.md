# CascadeSelect

## Purpose

Pick one path from a hierarchical option set using adjacent cascading columns.

## Import

```rust
use guic::prelude::{CascadeOption, CascadeSelect};
```

## Basic Usage

```rust
CascadeSelect::new("region")
    .expanded(true)
    .path(vec![0, 1])
    .options(vec![CascadeOption::new("americas", "Americas").children(vec![
        CascadeOption::new("canada", "Canada"),
        CascadeOption::new("us", "United States"),
    ])])
    .on_toggle(|expanded, _, _| {
        let _ = expanded;
    })
    .on_select(|path, _, _| {
        let _ = path;
    })
```

## Notes

`CascadeSelect` is host-managed. The application owns the selected path and
expanded state, then updates both after `on_select` or `on_toggle` fires.
