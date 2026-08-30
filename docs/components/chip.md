# Chip

`Chip` represents a compact selected value, filter, or suggestion. It is a
controlled component: the application supplies `selected` and updates its own
state from `on_click`.

```rust,ignore
Chip::new("Rust")
    .selected(true)
    .on_click(|_event, _window, _cx| {
        // Toggle the value in application state.
    })
    .on_remove(|_event, _window, _cx| {
        // Remove the value from application state.
    });
```

Use `disabled(true)` to suppress both activation and removal. `Tag` remains the
better choice for a non-selectable categorization label.
