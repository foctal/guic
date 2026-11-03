# Crate Publication

Maintainers publish crates and create releases manually. The package-check
script builds and verifies archives without uploading them.

## Public crate set and order

The GUI library crates use version `0.4.0`. Publish in dependency order:

1. `guic-macros`, `guic-tokens`, `guic-assets`, and `guic-webview`
2. `guic-core` and `guic-icons`
3. `guic-components`, `guic-charts`, `guic-editor`, and `guic-native`
4. `guic`

Examples, samples, `guic-story`, `quick-xml-compat`, and `xtask` are internal
and set `publish = false`.

The tooling crates use version `0.1.0` and have an independent publication
order: publish `guic-build` first, then `cargo-guic`. Neither depends on the GUI
library crates. Install released tooling with `cargo install cargo-guic --locked` and invoke it with `cargo guic`. See the
[cargo-guic guide](cargo-guic.md).

## Dry run

Run:

```bash
./scripts/package-check.sh
```

The script builds and verifies both tooling archives together using Cargo's
temporary registry, so they can be checked before either is published. It also
builds archives for the independent first-stage GUI crates and lists package
contents for every public crate. To check only the tooling crates, run:

```bash
cargo package --locked --allow-dirty -p guic-build -p cargo-guic
```

For individual downstream package checks, Cargo requires registry versions
of dependencies even when local path overrides are present. Run
`cargo package -p <crate>` without `--no-verify` after its prerequisites are
available on crates.io.

Every archive must contain only source, manifest, README/license metadata, and
intentional benchmark or build files. Reject secrets, local fixtures, generated
credentials, platform signing material, and unrelated application assets.

See [API stability](api-stability.md) for crate status and
[platform support](platform-support.md) for validation requirements.
