# cargo-guic

Application development and packaging tooling for GUIC. See the
[usage guide](https://github.com/foctal/guic/blob/main/docs/cargo-guic.md) and
[configuration schema](https://github.com/foctal/guic/blob/main/docs/cargo-guic.schema.json).

The packaging backend is the workspace-owned `guic-packager` fork.
The existing `[backend.cargo-packager]` configuration key is retained.

The CLI supports project discovery, configuration inspection, prerequisite checks,
development execution and immutable build records. `build` compiles and packages;
`bundle` packages a saved executable. Native formats include macOS app/DMG,
Windows NSIS/MSI/MSIX, and Linux deb/AppImage, with staged resources and optional signing
and notarization through explicit `--execute-signing`. `package-trial` uses the
same pipeline for candidate artifacts. Publication remains a separate manual
operation.

`dev --watch` rebuilds and restarts on edits. macOS builds can target
`universal-apple-darwin`. `updater keygen` and `updater prepare` generate signed
update feeds for applications using the optional `guic::updater` API.

Install released versions with Rust 1.95 or newer and your platform's native
build tools:

```sh
cargo install cargo-guic --locked
cargo guic --help
```

Use `--locked` to retain the packaged dependency versions. See the
[dependency policy](https://github.com/foctal/guic/blob/main/docs/dependency-policy.md#cargo-guic-backend)
for resolution constraints and known advisories.

To install from source, run from the repository root:

```sh
cargo install --path crates/cargo-guic --locked
```
