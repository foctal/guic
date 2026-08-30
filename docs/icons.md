# Icons

GUIC includes a small set of bundled SVG icons.

- Each built-in `IconName` resolves to a bundled SVG file in `crates/guic-icons/assets/`
- `Icon` renders through GPUI's SVG element with standard theme tinting
- `Icon::label()` opts an icon into semantic accessibility metadata
- `IconButton::label()` gives icon-only actions an accessible name
- No application-level asset source is required for built-in icons because GUIC uses absolute crate-local SVG paths

Applications can provide additional icons through their own asset source.
