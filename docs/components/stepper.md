# Stepper

## Purpose

Indicate progress through a multi-step workflow. Steps before the active index
render as completed, the active step is emphasized, and later steps are muted.

## Import

```rust
use guic::prelude::{Step, Stepper};
```

## Basic Usage

```rust
Stepper::new()
    .active(1)
    .steps(vec![
        Step::new("Account"),
        Step::new("Profile").description("Details"),
        Step::new("Review"),
    ])
```

`Stepper` is presentational; the host owns the active index.
