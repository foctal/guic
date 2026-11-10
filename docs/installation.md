# Installation

GUIC requires Rust 1.95 or newer. Add the toolkit, GPUI, and the platform
runtime to the application manifest. These versions apply to GUIC 0.5.0:

```toml
[dependencies]
guic = "0.5.0"
gpui = { package = "guic-gpui", version = "=0.5.0" }

[target.'cfg(target_os = "linux")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.5.0", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }

[target.'cfg(target_os = "macos")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.5.0", features = ["font-kit"] }

[target.'cfg(target_os = "windows")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.5.0" }
```

To use a source checkout, replace the `guic` dependency with a local path or
a pinned Git revision and keep the matching runtime versions:

```toml
guic = { git = "https://github.com/foctal/guic", rev = "<commit>" }
```

The current development checkout uses the published `guic-gpui` and
`guic-gpui-platform` 0.6.1 crates. When using this checkout, change the `gpui`
version and every target-specific `gpui_platform` version above to `=0.6.1`,
preserving their feature lists. No sibling runtime checkout or runtime patch
is needed. Keep the 0.5.0 runtime pins when using the published GUIC 0.5.0 release.

Do not use a moving branch for reproducible builds. GUIC `0.5.x` is a preview;
enable only the subsystem features the application needs and review
[API Stability](api-stability.md) before upgrading.

## Linux

The workspace helper installs the native packages needed by GPUI and the
optional WebView integration:

```bash
./scripts/install-linux-deps.sh
```

Applications that do not enable the `webview` feature do not need WebKit at
runtime. See [Platform Support](platform-support.md) for the validation matrix.

## Development facade

The development checkout also supports a single `guic` dependency. Its default
`application` feature selects the native platform backend, and `guic::gpui`
re-exports the matching runtime. Import it as `gpui` when using GPUI action macros
or derives. `guic::application()` creates the application;
`guic::open_window(cx, options, builder)` initializes GUIC and wraps the content
entity in `Root`. The hello-world example demonstrates this setup.

With `default-features = false`, opt into `application` for the backend helper,
or continue supplying a platform and constructing `Root` yourself. The existing
explicit runtime dependencies and initialization API remain supported.
