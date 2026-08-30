# Installation

GUIC requires Rust 1.95 or newer. Add the toolkit, GPUI, and the platform
runtime to the application manifest:

```toml
[dependencies]
guic = "0.0.1"
gpui = { package = "guic-gpui", version = "=0.2.0" }

[target.'cfg(target_os = "linux")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.2.0", features = ["font-kit", "x11", "wayland", "runtime_shaders"] }

[target.'cfg(target_os = "macos")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.2.0", features = ["font-kit"] }

[target.'cfg(target_os = "windows")'.dependencies]
gpui_platform = { package = "guic-gpui-platform", version = "=0.2.0" }
```

Before the public crates are available, replace the `guic` dependency with a
pinned Git revision:

```toml
guic = { git = "https://github.com/foctal/guic", rev = "<audited-revision>" }
```

Do not use a moving branch for reproducible builds. GUIC `0.0.x` is a preview;
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
