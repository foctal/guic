# Icons

`guic-icons` embeds its SVG assets in the executable. Built-in icons do not
require source files or an application asset source at runtime.

```rust
use guic::prelude::{Icon, IconName};

Icon::new(IconName::Info).label("Information")
```

Icons use the active theme's foreground by default. Set `.color(...)` and
`.size(...)` to override their appearance. Unlabelled icons are decorative;
`.label(...)` exposes an image role and accessible name. Use
`IconButton::label(...)` for icon-only actions.

Applications can load additional SVG assets through GPUI's asset source.
