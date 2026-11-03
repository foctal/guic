# Accordion

Stack collapsible sections. Expansion is host-managed so the host can implement
single-open or multi-open policies.

```rust
use guic::prelude::{Accordion, AccordionSection};

Accordion::new("settings")
    .section(AccordionSection::new("General", Label::new("…")).expanded(true))
    .section(AccordionSection::new("Advanced", Label::new("…")))
    .on_toggle(|index, _window, _cx| { /* flip section at index */ })
```

`on_toggle` reports the index of the toggled section.
