# Splitter

`Splitter` lays out two child surfaces horizontally or vertically using a
controlled pane fraction.

```rust,ignore
Splitter::new("workspace", left, right)
    .axis(SplitterAxis::Horizontal)
    .fraction(0.35)
```

The fraction is clamped to 0.1–0.9. Supply `on_resize` to make the divider
draggable and focusable. Store the requested fraction in your view and pass it
back on the next render. Arrow keys on the split axis move the divider by five
percentage points. Home and End select the minimum and maximum. Assistive
technologies can query its orientation and range and request either adjustment.
