# guic-build

Native build integration for GUIC applications. This is an unpublished workspace
crate.

Add a path dependency to this crate under `[build-dependencies]`, then call
`guic_build::build()?` from `build.rs`. The [Windows setup example](../../docs/cargo-guic.md#windows-executable-resources)
shows both files.

Configure `[platform.windows.build.windows-resources]` in Guic.toml. Ordinary
Cargo builds read that file; cargo-guic supplies a resolved request and verifies
the build-script acknowledgement. No GUI or packaging backend is linked into the
helper. See [the CLI guide](../../docs/cargo-guic.md) for configuration.
