# Contributing

Thank you for contributing to GUIC.

## Guidelines

- Keep all repository content in English.
- Follow the current crate boundaries in [Architecture](docs/architecture.md).
- Prefer additive, well-documented changes with tests for stateful logic.
- Avoid introducing WebView functionality into the core workspace.

## Local checks

```bash
./scripts/check.sh
```

## cargo-guic configuration schema

After changing cargo-guic configuration types, regenerate the checked-in schema
and verify that it matches:

```sh
cargo run -p cargo-guic --locked -- schema > docs/cargo-guic.schema.json
cargo test -p cargo-guic --locked configuration_schema_is_current
```

The redirection assumes a UTF-8 shell. On Windows PowerShell 5.1, save the output
as UTF-8 without a BOM instead of using its default redirection encoding. Keep
examples in the [CLI guide](docs/cargo-guic.md) aligned with configuration changes.
