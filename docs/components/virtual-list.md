# VirtualList

Render large uniform-height collections efficiently by drawing only the visible
range.

```rust
use guic::prelude::VirtualList;

VirtualList::new("items", 100, move |range, _window, _cx| {
    range.map(|index| row(index)).collect()
})
```

`VirtualList` wraps GPUI's `uniform_list`. Rows must have a uniform height.
For structured collections, see [DataTable](data-table.md) and
[TreeView](tree-view.md).
