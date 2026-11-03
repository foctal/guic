# AutoComplete

`AutoComplete` combines an IME-aware search input with a bounded, accessible
suggestion list.

Prefix matches rank before substring matches, with source order breaking ties.
Disabled items are excluded. `max_results` caps the displayed suggestions;
`empty_message` sets the no-result text.

Applications own the source items and selection side effects:

```rust
use guic::prelude::AutoComplete;

let search = cx.new(|cx| {
    AutoComplete::new("command-search", cx)
        .items(commands)
        .max_results(20)
        .empty_message("No matching commands")
        .on_select(|_item, _window, _cx| { /* apply selection */ })
});
```

Retain the entity and render `search.clone()`. For large remote sources, filter
the item collection in the application and replace `items` when results arrive.
