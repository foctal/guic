# Breadcrumb

## Purpose

Show the navigation path to the current location. The trailing item is treated
as the current page; preceding items are clickable when a handler is set.

## Import

```rust
use guic::prelude::{Breadcrumb, BreadcrumbItem};
```

## Basic Usage

```rust
Breadcrumb::new("nav")
    .items(vec![
        BreadcrumbItem::new("home", "Home"),
        BreadcrumbItem::new("settings", "Settings"),
        BreadcrumbItem::new("profile", "Profile"),
    ])
    .on_select(|index, _window, _cx| { /* navigate */ })
```

`on_select` reports the selected item's index; the current (last) item never
fires it.
