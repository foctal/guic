# Property List

`PropertyList` renders stable name/value metadata rows with optional badges.
It is suitable for inspectors, about panels, and settings summaries.

```rust,ignore
use guic::prelude::{BadgeVariant, PropertyItem, PropertyList};

PropertyList::new("Application").items(vec![
    PropertyItem::new("Version", "1.4.0"),
    PropertyItem::new("Channel", "Stable").badge("Active", BadgeVariant::Primary),
])
```

Property lists are read-only. Render editable values as separate controls.
