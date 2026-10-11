# cargo-guic

Application development and packaging tooling for GUIC. See the
[usage guide](../../docs/cargo-guic.md), [configuration schema](../../docs/cargo-guic.schema.json),
[architecture](../../docs/cargo-guic-design.md), and [testing guide](../../docs/cargo-guic-testing.md).

The CLI supports project discovery, configuration inspection, prerequisite checks,
development execution and immutable build records. Native packaging is available
through explicit `package-trial` commands, with staged resources and optional
signing and notarization. Normal `build`/`bundle` distribution formats are disabled
in this version. Publication remains a separate manual operation.

```sh
cargo run -p cargo-guic -- --help
cargo test -p cargo-guic --locked
```
