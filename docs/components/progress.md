# Progress

Show determinate or indeterminate task progress.

```rust
use guic::prelude::{ComponentSize, Progress};

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
