# Splitter

`Splitter` lays out two child surfaces horizontally or vertically using a
controlled pane fraction.

```rust,ignore
Splitter::new("workspace", left, right)
    .axis(SplitterAxis::Horizontal)
    .fraction(0.35)
```

The fraction is clamped to 0.1–0.9. The divider is not interactive. Use `Dock`
or application controls when users need to resize panes.
