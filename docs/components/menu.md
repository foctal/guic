# Menu

## Purpose

A reusable menu surface: a bordered panel of actionable rows, separators, and
section headers. It is the building block shared by `Menubar` and `ContextMenu`,
and can also be embedded directly (for example, inside a `Popover`).

## Import

```rust
use guic::prelude::{Menu, MenuItem};
use guic::prelude::IconName;
```

## Basic Usage

```rust
Menu::new("file-menu")
    .items(vec![
        MenuItem::new("new", "New").icon(IconName::Plus).shortcut("⌘N"),
        MenuItem::separator(),
        MenuItem::header("Danger zone"),
        MenuItem::new("delete", "Delete").danger(true),
    ])
    .on_activate(|id, _window, _cx| { /* dispatch command */ })
```

`MenuItem` supports `.icon()`, `.shortcut()`, `.disabled()`, and `.danger()`.
`MenuItem::separator()` and `MenuItem::header(label)` are non-interactive.

## Keyboard

Call `.focusable(handle)` with a host-owned `FocusHandle` to make the menu
dismiss on `Escape` via `.on_close(...)`.
Focusable menus support:

- `Up` and `Down` with wrapping while skipping headers, separators, and
  disabled actions
- `Home` and `End`
- single-character, case-insensitive prefix search
- `Enter` and `Space` activation
- `Escape` dismissal

Keyboard highlighting is controlled by the host. Pass the current row through
`active_index` and update it from `on_highlight`; this keeps open-state and
focus policy in the application:

```rust,ignore
Menu::new("file-menu")
    .focusable(menu_focus)
    .items(items)
    .active_index(active_item)
    .on_highlight(cx.listener(|view, index, _, cx| {
        view.active_item = Some(*index);
        cx.notify();
    }))
```
