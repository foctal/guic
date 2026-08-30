# ContextMenu

## Purpose

A right-click context menu attached to a trigger element. Host-managed: a
secondary (right) press on the trigger fires `on_request` with the pointer
position; the host stores the position plus an open flag and passes them back.

## Import

```rust
use guic::prelude::{ContextMenu, MenuItem, Label};
```

## Basic Usage

```rust
ContextMenu::new("row-context", Label::new("Right-click me"))
    .items(vec![MenuItem::new("rename", "Rename")])
    .open(is_open)
    .anchor(anchor_position)
    .on_request(|position, _window, _cx| { /* store position + open */ })
    .on_activate(|id, _window, _cx| { /* dispatch */ })
    .on_close(|_window, _cx| { /* close */ })
```

While open, a full-window scrim closes the menu on outside click. Call
`.focusable(handle)` so `Escape` also closes it.
