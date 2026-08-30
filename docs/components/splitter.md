# Splitter

`Splitter` lays out two child surfaces horizontally or vertically using a
controlled pane fraction.

```rust,ignore
Splitter::new(left, right)
    .axis(SplitterAxis::Horizontal)
    .fraction(0.35)
```

The fraction is clamped to a safe visible range. The current component is a
layout primitive rather than an interactive resize handle; applications that
need pointer and keyboard resizing should use `Dock` or expose explicit resize
controls. Divider color and spacing use the active `Theme`.
