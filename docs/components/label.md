# Label

Display short text with optional secondary text.

```rust
use guic::prelude::Label;

Label::new("Welcome").secondary("Ready")
```

Use `secondary_text_size(gpui::px(12.))` to size the secondary fragment
independently. Without it, secondary text inherits the label's text size.
