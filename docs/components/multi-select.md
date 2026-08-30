# MultiSelect

## Purpose

A controlled multi-selection dropdown. Mirrors `Select` but allows several
options to be active at once; selected options render as chips in the trigger
and each dropdown row toggles its membership.

## Import

```rust
use guic::prelude::{MultiSelect, SelectItem};
```

## Basic Usage

```rust
MultiSelect::new("labels")
    .placeholder("Select labels")
    .items(vec![
        SelectItem::new("bug", "Bug"),
        SelectItem::new("docs", "Docs"),
    ])
    .selected(selected_indices)
    .expanded(is_open)
    .on_toggle(|expanded, _window, _cx| { /* store */ })
    .on_select(|index, _window, _cx| { /* flip membership */ })
```

Host-managed: supply `selected` (the active indices) and `expanded`, then react
to `on_toggle` (the next expanded state) and `on_select` (the index whose
membership should flip on or off).
