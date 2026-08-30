# Panel

## Purpose

A lighter-weight titled container than `Card`, with an optional collapse
toggle in the header. Useful for sidebars, inspectors, and settings groups.

## Import

```rust
use guic::prelude::Panel;
```

## Basic Usage

```rust
Panel::new("filters", "Filters")
    .collapsible(true)
    .collapsed(false)
    .on_toggle(|collapsed, _window, _cx| { /* persist */ })
    .child(Label::new("Status: active"))
```

Collapsing is host-managed: supply the `collapsed` flag and update it from the
`on_toggle` callback (the argument is the next collapsed state).
