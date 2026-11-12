# Installation

GUIC requires Rust 1.95 or newer. Add the toolkit to the application manifest:

```toml
[dependencies]
guic = "0.6.0"
```

The default `application` feature selects the native platform backend.
`guic::gpui` re-exports the matching runtime; import it as `gpui` when using
GPUI action macros or derives. `guic::application()` creates the application,
and `guic::open_window(cx, options, builder)` initializes GUIC and wraps the
content entity in `Root`. See the [quick start](../README.md#quick-start) and
[hello-world example](../examples/hello_world/src/main.rs).

With `default-features = false`, enable `application` for the backend helper
and select the component and subsystem features your application needs.
Applications can also supply a platform backend and construct `Root` themselves.

## Explicit runtime dependencies

Applications importing GPUI directly should use the versions below with GUIC
0.6.0. The runtime family is pinned to 0.6.2; its version differs from GUIC's.

```toml
[dependencies]
guic = "0.6.0"
gpui = { package = "guic-gpui", version = "=0.6.2" }

[target.'cfg(target_os = "linux")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.6.2", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }

[target.'cfg(target_os = "macos")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.6.2", features = ["font-kit"] }

[target.'cfg(target_os = "windows")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.6.2" }
```

## Source checkouts

Replace the `guic` dependency with a local path or a pinned Git revision:

```toml
guic = { git = "https://github.com/foctal/guic", rev = "<commit>" }
```

Keep any explicit runtime dependencies aligned with the checkout. No sibling
runtime checkout or runtime patch is needed. Avoid moving branches for
reproducible builds and review [API Stability](api-stability.md) before upgrading.

## Linux

The workspace helper installs the native packages needed by GPUI and the
optional WebView integration:

```bash
./scripts/install-linux-deps.sh
```

Applications that do not enable the `webview` feature do not need WebKit at
runtime. See [Platform Support](platform-support.md) for the validation matrix.
