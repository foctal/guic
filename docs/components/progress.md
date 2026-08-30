# Progress

## Purpose

Show determinate or indeterminate task progress.

## Import

```rust
use guic::prelude::{ComponentSize, Progress};
```

## Basic Usage

```rust
Progress::new(64.0).id("upload-progress")
```

## Modes

- Determinate: `Progress::new(64.0)`
- Indeterminate: `Progress::new(0.0).indeterminate(true)`

Determinate progress exposes its current value and range to the platform
accessibility tree. Indeterminate progress is announced as a loading state.
Non-finite input values are normalized to zero.

Set a distinct `.id(...)` on each progress indicator when a view contains more
than one so its accessibility identity remains stable.

## Theming Behavior

Progress colors, radius, and animation timing are driven by the active theme.
