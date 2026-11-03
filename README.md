# GUIC

[![Crates.io](https://img.shields.io/crates/v/guic.svg)](https://crates.io/crates/guic)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/foctal/guic/blob/main/LICENSE)

GUIC is a native, cross-platform GUI toolkit for Rust built on
[`guic-gpui`](https://crates.io/crates/guic-gpui). It provides design tokens,
themes, application primitives, and a broad component set for desktop apps.

## Status

GUIC `0.5.x` is a preview release. Core APIs are usable, but breaking changes
are expected. Editor and WebView integrations are experimental. See
[API Stability](https://github.com/foctal/guic/blob/main/docs/api-stability.md)
and [Platform Support](https://github.com/foctal/guic/blob/main/docs/platform-support.md)
before adopting GUIC in a product.

## Installation

GUIC requires Rust 1.95 or newer. Follow the
[installation guide](https://github.com/foctal/guic/blob/main/docs/installation.md)
for Cargo dependencies and Linux packages. GUIC 0.5.0 uses the matching 0.5.0
`guic-gpui` runtime and platform crates.

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
- `guic-native`: desktop identity, activation, menus, trays, and lifecycle
- `guic-tokens`: design tokens, themes, and schema support
- `guic-components`: native controls and application components
- `guic-charts`: native chart models and renderers
- `guic-editor`: experimental editor surface
- `guic-webview`: optional experimental WebView integration
- `guic-icons`, `guic-assets`, and `guic-macros`: supporting APIs

Subsystems are opt-in through the `charts`, `editor`, `webview`,
`data-table`, `tree`, `dock`, `markdown`, and `updater` features. The default feature set
includes the core component, icon, asset, and native APIs.

## Documentation

- [cargo-guic: application development and packaging](docs/cargo-guic.md)
- [Component behavior and host integration](docs/component-behavior.md)

Start with [Getting Started](https://github.com/foctal/guic/blob/main/docs/getting-started.md),
[Components](https://github.com/foctal/guic/blob/main/docs/components.md), and
[Theming](https://github.com/foctal/guic/blob/main/docs/theming.md). Contributors
should run `./scripts/check.sh` before submitting changes.

See [NOTICE.md](https://github.com/foctal/guic/blob/main/NOTICE.md) for attribution.

## License

Licensed under the Apache License, Version 2.0.
