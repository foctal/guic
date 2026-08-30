# Property List

`PropertyList` renders stable name/value metadata rows with optional badges.
It is suitable for inspectors, about panels, and settings summaries.

```rust,ignore
PropertyList::new()
    .item(PropertyItem::new("Version", "1.4.0"))
    .item(PropertyItem::new("Channel", "Stable").badge("Active"))
```

Property lists are read-only. Interactive values should be rendered as real
controls outside the list. Layout, separators, typography, and colors are
derived from theme tokens.
