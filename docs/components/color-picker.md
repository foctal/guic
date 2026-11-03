# ColorPicker

Select a color from a controlled set of swatches.

```rust
use guic::prelude::{ColorPicker, ColorSwatch};

ColorPicker::new("accent")
    .value("#3366ff")
    .swatches(vec![
        ColorSwatch::new("#3366ff", "Blue"),
        ColorSwatch::new("#10b981", "Green"),
    ])
    .on_change(|value, _, _| {
        let _ = value;
    })
```

`ColorPicker` is host-managed. It emits the requested swatch value through
`on_change`; the application decides whether to apply that value.
