# Card

## Purpose

Group related content in an elevated surface with an optional header
(title/subtitle plus trailing actions), a body, and a footer.

## Import

```rust
use guic::prelude::Card;
```

## Basic Usage

```rust
Card::new()
    .title("Usage")
    .subtitle("Last 30 days")
    .child(Label::new("1,204 requests"))
    .footer(Button::new("View report"))
```

The body accepts arbitrary children via repeated `.child(...)` calls.
