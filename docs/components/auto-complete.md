# AutoComplete

`AutoComplete` combines an IME-aware search input with a bounded, accessible
suggestion list.

Highlights:

- Prefix matches rank before substring matches.
- Disabled suggestions are excluded.
- Stable source order breaks equal ranking scores.
- `max_results` keeps rendering bounded for large suggestion sources.
- `empty_message` provides explicit no-result feedback.
- The popup and rows expose listbox and option semantics.

Applications own the source items and selection side effects:

```rust
AutoComplete::new("command-search", cx)
    .items(commands)
    .max_results(20)
    .empty_message("No matching commands")
    .on_select(|item, _window, _cx| {
        dispatch_command(item.id.as_ref());
    })
```

For very large remote sources, pre-filter the controlled item collection in the
host and replace `items` when results arrive.
