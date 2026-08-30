# Image

## Purpose

Render native image assets with GUIC framing, sizing, loading, and fallback
states.

## Import

```rust
use guic::prelude::{Image, ImageFit};
```

## Basic Usage

```rust
Image::new("asset://screenshots/dashboard.png")
    .alt("Dashboard screenshot")
    .fit(ImageFit::Cover)
    .width(320.0)
    .height(180.0)
```

## Notes

`Image` wraps GPUI's native image element. It supports asset paths, file paths,
and other GPUI-compatible image sources. Alternate text is used for loading and
fallback states.
