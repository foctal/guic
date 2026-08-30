# PickList

`PickList` provides two panes for building a selected subset from a supplied
item list. It is controlled: pass selected indices and store the next index list
from `on_change`.

```rust,ignore
PickList::new("reviewers")
    .items(vec![
        SelectItem::new("ada", "Ada Lovelace"),
        SelectItem::new("grace", "Grace Hopper"),
    ])
    .selected(assigned_reviewer_indices.clone())
    .on_change(cx.listener(|view, selection, _, cx| {
        view.assigned_reviewer_indices = selection.clone();
        cx.notify();
    }));
```

Click an available item to add it and a selected item to remove it. Disabled
items stay visible but cannot move between panes.
