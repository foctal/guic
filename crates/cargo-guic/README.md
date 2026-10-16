# cargo-guic

Application development and packaging tooling for GUIC. See the
[usage guide](../../docs/cargo-guic.md) and
[configuration schema](../../docs/cargo-guic.schema.json).

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

```sh
cargo run -p cargo-guic -- --help
cargo test -p cargo-guic --locked
```
