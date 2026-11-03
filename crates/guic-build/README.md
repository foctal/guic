# guic-build

Build-time integration for GUIC application identity and Windows executable
resources. Add `guic-build = "0.1.0"` under `[build-dependencies]` and call
`guic_build::build()?` from `build.rs` on every platform. For source development,
use a path dependency to `crates/guic-build` in your checkout.

The helper reads `Guic.toml` for ordinary Cargo builds. cargo-guic supplies
resolved settings and verifies that the build script applied them. On Windows,
configure `[platform.windows.build.windows-resources]` to embed icons, version
metadata, and a manifest. The helper does not link the GUI or packaging backend.

See the [CLI guide](https://github.com/foctal/guic/blob/main/docs/cargo-guic.md#windows-executable-resources)
for configuration and a complete build-script example.
