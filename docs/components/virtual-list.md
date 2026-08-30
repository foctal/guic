# VirtualList

## Purpose

Render large uniform-height collections efficiently by drawing only the visible
range.

## Import

```rust
use guic::prelude::VirtualList;
```

## Basic Usage

```rust
VirtualList::new("items", 100, move |range, _window, _cx| {
    range.map(|index| row(index)).collect()
})
```

## Notes

The current implementation is a thin GUIC wrapper around GPUI's `uniform_list`.
It is production-useful for uniform-height data sets, while richer virtualized
widgets still belong on the roadmap.
