# Paginator

## Purpose

A host-managed page navigation control. Renders previous/next buttons and a
truncated run of page buttons (with ellipses) around the current page — pairs
naturally with `DataTable`.

## Import

```rust
use guic::prelude::Paginator;
```

## Basic Usage

```rust
Paginator::new("results")
    .page_count(12)
    .page(current_page) // zero-based
    .on_select(|page, _window, _cx| { /* load that page */ })
```

Derive the page count from a total with `.from_total(total_items, page_size)`.
Control how many pages surround the current one with `.sibling_count(n)`.
`on_select` reports the chosen zero-based page index; page numbers display as
one-based. Prev/Next disable automatically at the bounds.
