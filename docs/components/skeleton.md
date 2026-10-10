# Skeleton

`Skeleton` reserves space while native application content loads. It uses theme
colors, contains no interactive elements, and schedules no animation frames.
Loading announcements belong on the containing surface, rather than on every
placeholder line.

```rust,no_run
use guic::prelude::*;

let avatar = Skeleton::new().width(40.0).height(40.0).circular(true);
let heading = Skeleton::new().height(20.0);
let detail = Skeleton::new().width(160.0).height(12.0);
```

Width defaults to the parent width; height defaults to 16 logical pixels.
Negative dimensions become zero and non-finite dimensions are ignored. Use equal
width and height for circles. Replace the placeholder with the loaded content in
the host's render method, choosing dimensions that match the expected content to
reduce layout movement.
