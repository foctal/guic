# ScrollArea

Wrap overflowing content in a scroll container.

```rust
use guic::prelude::ScrollArea;

ScrollArea::new("logs", content).vertical(true)
```

`ScrollArea` uses GPUI scrolling. Constrain the container's size so content can overflow.
