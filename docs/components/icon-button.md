# IconButton

Display an action as an icon with an accessible label.

```rust
use guic::prelude::{ButtonVariant, IconButton, IconName};

IconButton::new(IconName::Search)
    .variant(ButtonVariant::Secondary)
    .label("Search")
```

## Keyboard

Enabled icon buttons with an action participate in tab navigation. Enter and
Space activate the focused button. Disabled icon buttons are omitted from this
interaction path.
