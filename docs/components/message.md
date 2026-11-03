# Message

A compact, inline severity note intended to sit beside form fields and inline
content. Lighter than `Alert`: a single-line, accent-bordered row with no title
or dismiss button.

```rust
use guic::prelude::{Message, MessageVariant};

Message::new("Password must be at least 8 characters")
    .variant(MessageVariant::Danger)
```

Convenience methods `.success()`, `.warning()`, and `.danger()` mirror the
variants.
