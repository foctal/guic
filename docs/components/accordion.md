# Accordion

## Purpose

Stack collapsible sections. Expansion is host-managed so the host can implement
single-open or multi-open policies.

## Import

```rust
use guic::prelude::{Accordion, AccordionSection};
```

## Basic Usage

```rust
Accordion::new("settings")
    .section(AccordionSection::new("General", Label::new("…")).expanded(true))
    .section(AccordionSection::new("Advanced", Label::new("…")))
    .on_toggle(|index, _window, _cx| { /* flip section at index */ })
```

`on_toggle` reports the index of the toggled section.
