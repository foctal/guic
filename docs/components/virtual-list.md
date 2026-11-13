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

## Variable-height rows

Use `VariableList` with a retained GPUI `ListState` for content whose rows have
unequal heights. Measurement is limited to the viewport and overdraw region.

```rust
use guic::gpui::{ListAlignment, ListOffset, ListState, px};
use guic::components::VariableList;

let state = ListState::new(10_000, ListAlignment::Top, px(100.0));
// Retain `state`; create only visible elements in the callback.
let list = VariableList::new(state.clone(), |index, _window, _cx| row(index))
    .height(px(400.0));
// An explicit logical offset also works for rows not measured yet.
state.scroll_to(ListOffset { item_ix: 9000, offset_in_item: px(0.0) });
```

Use `state.splice(old_range, replacement_count)` when rows or their heights
change. Notify the owning view after changing scroll or list state. Do not
recreate the state on every render: it owns measurements and the scroll anchor.
