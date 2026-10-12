# Testing cargo-guic

## Automated checks

Use Rust 1.95 or newer and install the platform dependencies described in
[Platform Support](platform-support.md). Unlike building cargo-guic alone, these
checks include GUI crates and examples. Run from the repository root:

```sh
cargo test -p cargo-guic -p guic-build -p guic-assets --locked
cargo clippy -p cargo-guic -p guic-build -p guic-assets --all-targets --locked -- -D warnings
cargo check -p guic-example-assets-demo -p guic-example-webview --all-features --locked
cargo fmt --all -- --check
cargo deny check
```

Tests cover application selection, Cargo metadata and output parsing,
configuration precedence, build compatibility, immutable snapshots, resource
paths, build-script acknowledgements, cancellation and backend failures. Backend
and signing unit tests use test doubles. They do not create installers or use
credentials. The Windows-only integration test compiles executable resources and
reads their version information on a native Windows host.

A separate test invokes the real native packaging backend. Run it explicitly
when package generation is intended:

```sh
cargo test -p cargo-guic --locked --test cli manual_native_backend_candidate \
  -- --ignored --nocapture
```

It creates a candidate package and retains its temporary directory for inspection.
It is ignored in routine test runs and does not replace installation testing.

## Example applications

Two examples provide packaging configurations:

- `examples/assets_demo/Guic.toml` exercises external resources, the common
  installed-resource resolver, Windows executable branding and native formats.
  Its configuration enables `packaged-resources`; ordinary Cargo without that
  feature retains the example's source-tree resource loading.
- `examples/webview/Guic.toml` exercises optional WebView requirements, including
  Linux GTK/WebKitGTK dependencies. Use the example's URL argument for a
  controlled navigation test.

Run on the native target host:

```sh
cargo run -p cargo-guic --locked -- inspect -p guic-example-assets-demo
cargo run -p cargo-guic --locked -- dev -p guic-example-assets-demo
cargo run -p cargo-guic --locked -- build -p guic-example-assets-demo --no-bundle
cargo run -p cargo-guic --locked -- doctor -p guic-example-assets-demo --build-record BUILD_RECORD.json
cargo run -p cargo-guic --locked -- package-trial -p guic-example-assets-demo --build-record BUILD_RECORD.json
```

Replace `BUILD_RECORD.json` with the saved `<id>.json` beneath Cargo's resolved
`target_directory/guic/<application-key>/records/`. Match the `id` returned by
build; do not pass the executable snapshot. Repeat with
`-p guic-example-webview` for WebView coverage.

The default build profile is release; development uses a separate dev profile.
Doctor includes the normal-distribution restriction in its report; inspect the
individual prerequisite checks when using `package-trial`.

## Installation checks

Record the OS, architecture, tool versions, configuration, command, artifact
hashes and results alongside the application's release tests. Run outside the
checkout and from an unrelated working directory. Include paths with spaces and
non-ASCII characters.

| Platform | Checks |
| --- | --- |
| macOS | Finder launch, resources, framework loading, deployment baseline, app and DMG behavior. |
| Windows | EXE icon/version/manifest, standard-user installation, DLL and WebView2 prerequisites, upgrade and uninstall. |
| Linux | Declared runtime packages, oldest supported glibc baseline, X11/Wayland, desktop launch, mounted and extracted AppImages. WebView examples require X11/XWayland. |

Verify GUI interactions, resource loading, WebView navigation where enabled,
upgrade identity and preservation of user data. Test missing runtime prerequisites
on a clean system. Test required signatures and notarization separately with
explicit `--execute-signing` and credentials provisioned through OS tooling.
Package creation does not upload or publish artifacts.

## Windows development environment

Run `doctor` from an x64 Native Tools Command Prompt for Visual Studio, or
initialize the corresponding developer environment before invoking it. Rust can
locate MSVC automatically while `doctor` requires `link.exe` and `dumpbin.exe`
on `PATH`. Windows SDK tools such as `rc.exe` and `fxc.exe` are also discovered
from installed SDK directories.

For examples that configure NSIS, `doctor` still reports `ready: false` because
normal distribution is disabled. Check the individual tool, runtime and
`unsigned-candidate-prerequisites` results separately. A successful build or
prerequisite report does not certify an installer.

### Local verification on 2026-10-12

Verified on Windows 11 Home 10.0.26200, x86_64 MSVC, Rust/Cargo 1.95.0,
MSVC 14.44.35207 and Windows SDK 10.0.26100.0:

- The automated test command above passed 48 tests; the manual native backend
  acceptance test remained ignored.
- Clippy with warnings denied, formatting, and both examples' all-feature checks
  passed. `cargo deny check` passed with dependency-policy warnings.
- `cargo-guic build --no-bundle --profile dev` succeeded for both example
  applications. The assets example preserved GPUI's manifest and embedded
  version `0.1.0`, product `GUIC Resources Fixture`, and company `GUIC Fixture`.
- Assets snapshot import inspection passed in the Visual Studio developer
  environment. WebView2 runtime detection found version `154.0.4258.62`.

This verifies development builds and CLI behavior, including paths with spaces
and Japanese characters. Release-profile example builds, interactive GUI
behavior, NSIS generation, installation, upgrade, uninstall and signing were
not exercised. Packaging and release acceptance remain manual maintainer work.

## Updating the configuration schema

After changing configuration types, regenerate the checked-in schema:

```sh
cargo run -p cargo-guic --locked -- schema > docs/cargo-guic.schema.json
cargo test -p cargo-guic --locked configuration_schema_is_current
```

The redirection above assumes a UTF-8 shell. On Windows PowerShell 5.1, write the
captured output as UTF-8 without a BOM rather than using its default redirection
encoding. The schema consistency test detects differences from the generated
schema. Keep the CLI guide's examples aligned with configuration changes.
