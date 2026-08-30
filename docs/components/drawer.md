# Drawer

## Purpose

A controlled edge-anchored panel that slides in over a dismiss scrim — for side
navigation, detail panes, and settings sheets.

## Import

```rust
use guic::prelude::{Drawer, DrawerSide, Label};
```

## Basic Usage

```rust
Drawer::new("details")
    .open(is_open)
    .side(DrawerSide::Right)
    .title("Details")
    .size(360.0)
    .on_close(|_event, _window, _cx| { /* close */ })
    .child(Label::new("Selected item"))
    .footer(Button::new("Done"))
```

Host-managed: supply `open` and react to `on_close` (fired by both the scrim and
the header close button). `side` accepts `Left`, `Right`, `Top`, or `Bottom`;
`size` is the panel width (left/right) or height (top/bottom).
