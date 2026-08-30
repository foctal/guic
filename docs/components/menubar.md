# Menubar

## Purpose

A horizontal application menu bar. Clicking a top-level label opens its
dropdown. Open state is host-managed so it composes with application command
state.

## Import

```rust
use guic::prelude::{Menubar, MenubarMenu, MenuItem};
```

## Basic Usage

```rust
Menubar::new("app-menubar")
    .open(open_index)
    .menus(vec![
        MenubarMenu::new("File", vec![MenuItem::new("open", "Open")]),
        MenubarMenu::new("Edit", vec![MenuItem::new("undo", "Undo")]),
    ])
    .on_open(|next, _window, _cx| { /* store Option<usize> */ })
    .on_activate(|activation, _window, _cx| {
        // activation.menu: usize, activation.item: SharedString
    })
```

`on_open` reports the next open index (or `None` to close). `on_activate`
reports a `MenubarActivation { menu, item }`.
