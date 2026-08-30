# GUIC

GUIC is a native, cross-platform GUI toolkit for Rust built on
[`guic-gpui`](https://crates.io/crates/guic-gpui). It provides design tokens,
themes, application primitives, and a broad component set for desktop apps.

## Status

GUIC `0.1.x` is a preview release. Core APIs are usable, but breaking changes
are expected and the editor, terminal, charts, and WebView integrations remain
experimental. See [API Stability](https://github.com/foctal/guic/blob/main/docs/api-stability.md)
and [Platform Support](https://github.com/foctal/guic/blob/main/docs/platform-support.md)
before adopting GUIC in a product.

## Installation

After the crates are published, add GUIC and its platform runtime:

```toml
[dependencies]
guic = "0.1.0"
gpui = { package = "guic-gpui", version = "=0.2.0" }

[target.'cfg(target_os = "linux")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.2.0", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }

[target.'cfg(target_os = "macos")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.2.0", features = ["font-kit"] }

[target.'cfg(target_os = "windows")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.2.0" }
```

GUIC requires Rust 1.95 or newer. Linux users must also install the native
dependencies listed in the
[installation guide](https://github.com/foctal/guic/blob/main/docs/installation.md).

## Quick Start

```rust,no_run
use gpui::{AppContext as _, Context, IntoElement, Render, Window, div, prelude::*};
use guic::prelude::*;

struct AppView;

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Button::new("Hello, GUIC"))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);
        cx.open_window(Default::default(), |window, cx| {
            let view = cx.new(|_| AppView);
            cx.new(|cx| guic::core::Root::new(view, window, cx))
        })
        .expect("failed to open window");
    });
}
```

Run the component gallery from a source checkout:

```bash
cargo run -p guic-sample-component-gallery
```

## Crates

- `guic`: umbrella crate and feature re-exports
- `guic-core`: runtime, focus, overlays, commands, services, and persistence
- `guic-tokens`: design tokens, themes, and schema support
- `guic-components`: native controls and application components
- `guic-charts`: native chart models and renderers
- `guic-editor`: experimental editor surface
- `guic-terminal`: experimental terminal model, renderer, and PTY host
- `guic-webview`: optional experimental WebView integration
- `guic-icons`, `guic-assets`, and `guic-macros`: supporting APIs

Subsystems are opt-in through the `charts`, `editor`, `terminal`, `webview`,
`data-table`, `tree`, `dock`, and `markdown` features. The default feature set
includes the core component, icon, asset, and native APIs.

## Documentation

Start with [Getting Started](https://github.com/foctal/guic/blob/main/docs/getting-started.md),
[Components](https://github.com/foctal/guic/blob/main/docs/components.md), and
[Theming](https://github.com/foctal/guic/blob/main/docs/theming.md). Contributors
should run `./scripts/check.sh` before submitting changes.

GUIC studies `gpui-component` as a technical reference for GPUI usage patterns.
See [NOTICE.md](https://github.com/foctal/guic/blob/main/NOTICE.md) for attribution.

## License

Licensed under the Apache License, Version 2.0.
