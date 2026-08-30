# Listbox

`Listbox` displays a controlled set of options with single or multiple
selection. The host supplies items and selected indices and handles selection
callbacks.

```rust,ignore
Listbox::new("targets")
    .items(vec![SelectItem::new("local", "Local"), SelectItem::new("ci", "CI")])
    .selected(vec![0])
    .on_select(|index, _, _| { /* update controlled selection */ })
```

Enabled options participate in tab traversal and activate with Enter or Space.
Disabled options never receive focus or emit selection callbacks. Directional
active-descendant navigation remains a stable-release hardening item. All
visual states use active theme tokens.
