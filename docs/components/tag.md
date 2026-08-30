# Tag

## Purpose

Display a tinted categorization label, optionally with a leading status dot and
a remove affordance for filter-chip workflows.

## Import

```rust
use guic::prelude::{Tag, TagVariant};
```

## Basic Usage

```rust
Tag::new("backend")
    .variant(TagVariant::Info)
    .dot(true)
    .on_remove(|_event, _window, _cx| { /* drop the filter */ })
```

Unlike `Badge`, `Tag` uses a tinted surface and supports removal. Removal is
host-managed: react to `on_remove` and drop the item from your model.
