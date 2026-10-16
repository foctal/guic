# cargo-guic

Application development and packaging tooling for GUIC. See the
[usage guide](https://github.com/foctal/guic/blob/main/docs/cargo-guic.md) and
[configuration schema](https://github.com/foctal/guic/blob/main/docs/cargo-guic.schema.json).

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

After the initial crates.io publication, install with Rust 1.95 or newer and
your platform's native build tools:

```sh
cargo install cargo-guic --locked
cargo guic --help
```

`--locked` is currently required: `cargo-packager 0.11.8` depends on
`zip 0.6`, whose releases are all yanked. The packaged lockfile retains the
resolved version; installation without `--locked` fails dependency resolution.

Before publication, install from the repository root with
`cargo install --path crates/cargo-guic --locked`. For CLI development:

```sh
cargo run -p cargo-guic -- --help
cargo test -p cargo-guic --locked
```
