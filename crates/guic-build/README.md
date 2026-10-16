# guic-build

Native build integration for GUIC applications.

After the initial crates.io publication, add `guic-build = "0.1.0"` under
`[build-dependencies]`. Before publication, use a path dependency to
`crates/guic-build` in your checkout. Then call
`guic_build::build()?` from `build.rs`. The [Windows setup example](https://github.com/foctal/guic/blob/main/docs/cargo-guic.md#windows-executable-resources)
shows both files.

Configure `[platform.windows.build.windows-resources]` in Guic.toml. Ordinary
Cargo builds read that file; cargo-guic supplies a resolved request and verifies
the build-script acknowledgement. No GUI or packaging backend is linked into the
helper. See [the CLI guide](https://github.com/foctal/guic/blob/main/docs/cargo-guic.md) for configuration.
