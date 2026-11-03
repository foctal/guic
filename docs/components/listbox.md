# Listbox

`Listbox` displays options with controlled single or multiple selection.
`on_selection_change` receives the next list of selected indices.

```rust
use guic::prelude::{Listbox, ListboxSelectionMode, SelectItem};

Listbox::new("targets")
    .items(vec![SelectItem::new("local", "Local"), SelectItem::new("ci", "CI")])
    .selection_mode(ListboxSelectionMode::Multiple)
    .selected(vec![0])
    .on_selection_change(|_indices, _window, _cx| { /* update selection */ })
```

Enabled options participate in tab traversal. Arrow keys and Home/End move
focus; Enter or Space selects an option. Disabled options cannot receive focus
or emit selection callbacks.
