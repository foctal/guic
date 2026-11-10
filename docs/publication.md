# Crate Publication

Maintainers publish crates and create releases manually. The package-check
script builds and verifies archives without uploading them.

## Public crate set and order

The GUI library crates use version `0.5.0`. Publish in dependency order:

1. `guic-macros`, `guic-tokens`, `guic-assets`, and `guic-webview`
2. `guic-core` and `guic-icons`
3. `guic-components`, `guic-charts`, `guic-editor`, and `guic-native`
4. `guic`

Examples, samples, `guic-story`, and `xtask` are internal
and set `publish = false`.

The tooling crates use version `0.1.0` and have an independent publication
order: publish `guic-packager-utils` first, followed by `guic-packager`,
`guic-packager-updater`, and `guic-packager-resource-resolver`. Publish
`guic-build` before `cargo-guic`, and `guic-packager-updater` before `guic`.
The packaging fork family also starts at `0.1.0`. These tooling crates do not depend on the GUI
library crates. Install released tooling with `cargo install cargo-guic --locked` and invoke it with `cargo guic`. See the
[cargo-guic guide](cargo-guic.md).

## Dry run

Run:

```bash
./scripts/package-check.sh
```

The script checks license files and builds every public archive together.
Cargo's temporary registry resolves dependencies between these archives, so
verification does not require publishing prerequisites first. Nothing is uploaded.
To check only the tooling crates, run:

```bash
cargo package --locked --allow-dirty -p guic-packager-utils -p guic-packager \
  -p guic-packager-updater -p guic-build -p cargo-guic
```

Before publishing 0.6.0, update the GUI crate versions and their path-dependency
version requirements together, regenerate `Cargo.lock`, and repeat validation.
Update installation examples and move the changelog's Unreleased section to the
release version and date. The tooling versions remain independent.

Every archive must contain only source, manifest, README/license metadata, and
intentional benchmark or build files. Reject secrets, local fixtures, generated
credentials, platform signing material, and unrelated application assets.

See [API stability](api-stability.md) for crate status and
[platform support](platform-support.md) for validation requirements.
