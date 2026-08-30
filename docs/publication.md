# Crate Publication

Publishing, tagging, signing, and release creation are maintainer-only manual
operations. GUIC automation may build and inspect package archives but must not
upload them.

## Public crate set and order

All public crates use version `0.1.0`. Publish in dependency order:

1. `guic-macros`, `guic-tokens`, `guic-assets`, and `guic-webview`
2. `guic-core` and `guic-icons`
3. `guic-components`, `guic-charts`, `guic-editor`, and `guic-terminal`
4. `guic`

Examples, samples, `guic-story`, `quick-xml-compat`, and `xtask` are internal
and set `publish = false`.

## Dry run

Run:

```bash
./scripts/package-check.sh
```

The script builds archives for the independent first-stage crates and lists
package contents for every public crate. Cargo requires registry availability
for downstream dependencies even with local path overrides, so downstream GUIC
archives cannot be prepared until their prerequisites have been uploaded
manually. Before uploading each crate, the maintainer must run
`cargo package -p <crate>` without `--no-verify` after its GUIC dependencies are
visible on crates.io.

Every archive must contain only source, manifest, README/license metadata, and
intentional benchmark or build files. Reject secrets, local fixtures, generated
credentials, platform signing material, and unrelated application assets.

## Support contract

- `guic-core`, `guic-tokens`, `guic-components`, `guic-icons`, `guic-assets`,
  and `guic-macros` are preview APIs.
- `guic-charts`, `guic-terminal`, and `guic-webview` are preview subsystems.
- `guic-editor` is experimental.
- The umbrella `guic` crate inherits the least-stable enabled feature.
- `0.1.x` releases may change preview and experimental APIs between releases.
  Every breaking change must be called out in `CHANGELOG.md`.
- Rust 1.95 is the current MSRV. Raising it requires release notes and CI
  updates.
- Serialized persistence formats require explicit version fields and migration
  or rejection tests before a breaking change.
- The exact `guic-gpui` crate family revision is an integration baseline;
  updates require the complete automated and physical platform matrices.
