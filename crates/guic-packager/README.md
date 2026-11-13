# guic-packager

Native application packaging for GUIC, maintained as a fork of
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager).

The library produces macOS app bundles and DMGs, Windows NSIS and WiX installers,
and Linux Debian, AppImage, and Pacman packages. `cargo-guic` uses this library;
applications normally use `cargo guic build` or `cargo guic bundle`.

The standalone `guic-packager` executable is enabled by the `cli` feature. It
accepts `Packager.toml`, `packager.json`, and Cargo's `package.metadata.packager`.
Existing `CARGO_PACKAGER_*` environment variables and package formats are retained.

```sh
cargo run -p guic-packager -- --help
```

Library users can disable default features and select `rustls-tls` or `native-tls`.
The `schema` feature exposes JSON Schema support for configuration types.
Generate the checked-in schema with:

```sh
cargo run -p guic-packager --example schema --features schema > crates/guic-packager/schema.json
```

Related crates are `guic-packager-updater`, `guic-packager-utils`, and
`guic-packager-resource-resolver`. Their initial fork version is `0.1.0`,
independent of upstream versions. Types from these crates and upstream crates
are distinct; use one family consistently.

See [UPSTREAM.md](UPSTREAM.md) for provenance. The original Apache-2.0 OR MIT
licensing and copyright notices are retained.

Include `THIRD-PARTY-LICENSES.txt` with binary distributions containing the
certificate data dependencies.
