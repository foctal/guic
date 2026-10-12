# cargo-guic

`cargo-guic` supports development and native packaging of GUIC applications.
It resolves Cargo projects, runs development builds, and packages recorded
executables through cargo-packager. The CLI has no GUIC or GPUI runtime
dependency and does not add packaging dependencies to applications.

The [architecture](cargo-guic-design.md) describes the implementation boundaries.
**Normal distribution formats are disabled in this version**: `build` without
`--no-bundle` and `bundle` reject packaging. Use `package-trial` to generate native
packages explicitly. Its reports label outputs as candidates; package generation
does not establish an application's distribution readiness.

External resources support the common installed-resource resolver or an
application-defined resolver. Signing and notarization require configured
`required` policies and explicit `--execute-signing` on `package-trial`.
Cross-compilation and universal binaries are outside this version's scope.

## Install from source

The CLI is not published to crates.io. From a checkout of this repository, use
Rust 1.95 or newer and the native build tools for your operating system:

```sh
cargo install --path crates/cargo-guic --locked
cargo guic --help
```

For development of the CLI itself, use `cargo run -p cargo-guic -- --help` from
this repository. When operating on another application, run the installed CLI
from that application's workspace so its Cargo configuration is discovered.

## Quick start

From an existing application workspace, replace `myapp` with its Cargo package
name and choose a permanent reverse-domain application identifier:

```sh
cargo guic init -p myapp --identifier com.example.myapp
cargo guic inspect -p myapp
cargo guic dev -p myapp
cargo guic build -p myapp --no-bundle
```

`init` writes Guic.toml next to the application's Cargo.toml without replacing
existing files. It requires an explicit identifier and accepts only project
selectors; configure build settings in Guic.toml afterward.

Cargo commands use `--locked` by default. Ensure the application has an up-to-date
Cargo.lock, or pass `--unlocked` to allow Cargo to resolve and update dependencies.
`dev` builds with the dev profile and runs a copy of the executable once, with
the application directory as its working directory. Forward application arguments
with `cargo guic dev -p myapp -- --application-argument`. File watching is not
implemented. `build --no-bundle` defaults to the release profile.

## Create a native package

Configure icons and formats in Guic.toml, then build with `--no-bundle`. The build
report includes an `id`; its saved record is under Cargo's resolved target
directory at `guic/<package-id-hash>/records/<id>.json`.

On macOS, for example:

```sh
cargo guic package-trial -p myapp --formats app \
  --build-record /absolute/path/to/record.json
```

Use the same target, profile and feature selectors as the recorded build. A
custom `--profile ship` requires a corresponding Cargo profile. `--formats`
replaces the configured list; omit it to use all formats configured for the host.

| Native host | Formats |
| --- | --- |
| macOS | `app`, `dmg` |
| Windows with MSVC | `nsis` |
| Linux | `deb`, `appimage` |

`package-trial` requires an explicit record, never compiles the application,
and runs the before-bundle hook once. It creates separate copies for each format
and reports `trial: true` and `acceptance: "not-verified"`. These fields describe
the package generation workflow, not the result of your application's tests.
See the [testing guide](cargo-guic-testing.md) for installation checks.

`build` without `--no-bundle` and `bundle` are reserved for normal distribution
and reject packaging in this version. `bundle` checks its selected record before
reporting that restriction. Use `package-trial` for package generation.

## Commands

| Command | Purpose |
| --- | --- |
| `init` | Create an application's Guic.toml. |
| `inspect` | Show resolved settings and their provenance without running hooks. |
| `doctor` | Check tools and runtime prerequisites; inspect a saved build with `--build-record`. |
| `dev` | Build and run the application once. |
| `build --no-bundle` | Build and save an executable snapshot and build record. |
| `package-trial` | Generate native packages from an explicit build record. |
| `schema` | Print the Guic.toml JSON Schema. |

No command publishes releases or crates, modifies Git history, or opens pull
requests. Explicit notarization submits artifacts to Apple's notary service.

## Configuration

The [JSON Schema](cargo-guic.schema.json) describes configuration version 1.
Unknown keys and incompatible schema versions are errors, including unknown
keys in inactive platform overrides.

```toml
schema-version = 1

[app]
identifier = "com.example.myapp"
product-name = "My Application"

[build]
package = "myapp"
bin = "myapp"
# profile = "ship"
# features = ["feature-a"]
# default-features = false
# inputs = ["assets/embedded-data.json"]

[bundle]
icons = ["assets/app.png"]

# Formats used by package-trial on each native host.
[bundle.formats]
macos = ["app", "dmg"]
windows = ["nsis"]
linux = ["deb", "appimage"]

[platform.macos.build]
deployment-target = "12.0"

[signing]
code-signing = "off"
notarization = "off"

[backend.cargo-packager]
publisher = "Example Organization"
```

A virtual workspace with multiple executable packages requires `--package` or
`build.package` from an explicitly selected configuration. Multiple binaries
require `--bin`, `build.bin`, or Cargo's `default-run`. Conflicting package/bin
selectors fail. Cargo metadata supplies inherited version, authors, description,
license, and homepage; executable and display names remain distinct.

`--config` identifies its adjacent Cargo.toml unless `--manifest-path` is also
supplied. CLI paths are relative to the invocation directory. Configured file
paths and hook directories are relative to Guic.toml. CLI options override the
selected `[platform.macos.*]`, `[platform.windows.*]`, or `[platform.linux.*]`
tables, which override common tables. Tables merge by key; arrays replace.
`--formats` replaces the format list. `inspect` includes field provenance.

Cargo configuration is resolved from the invocation directory, as with Cargo.
This includes hierarchical configuration, environment target selection, target
directories, and compiler settings. Explicit native targets are passed to Cargo;
multiple or non-native targets fail. macOS deployment targets are passed to the
compiler and backend metadata consistently. Forced conflicting Cargo environment
settings fail. This does not validate a platform deployment baseline.

`build.inputs` declares files that affect the executable, such as embedded data.
Their content hashes are part of record compatibility. A before-build hook may
create missing declared inputs; all must exist after it finishes. Inputs are
rechecked after compilation and copied alongside the executable snapshot.
Undeclared source files are not a source-tree attestation: rebuild when the
current source is required.

## External resources

External resources require an explicit versioned runtime contract:

```toml
[bundle]
resource-contract = "guic-native-v1"

[[bundle.resources]]
source = "assets/data/**/*.json"
destination = "data"
```

An exact file maps to the exact destination. A directory maps its descendant
files beneath the destination. A glob preserves paths relative to the static
prefix before its first wildcard. Empty matches fail; expansion is sorted and
collisions, path traversal, reserved Windows names and symlinks are rejected.
Resources are copied and hashed after before-bundle, then copied separately for
each format. Resource content is packaging-only; changing the runtime contract
requires rebuilding.

`guic-native-v1` uses `guic_assets::ResourceRoot::discover("cargo-bin-name")`.
Use guic-assets from a checkout containing this API; do not assume an older
crates.io release includes it.
It resolves macOS `Contents/Resources`, Windows files beside the executable,
and Linux `../lib/<cargo-bin-name>` relative to the installed `usr/bin` executable.
The Linux layout is shared by the pinned backend's deb and mounted/extracted
AppImage outputs, allowing the same snapshot for both. All paths are independent
of the working directory. The root must exist; logical resource paths reject
traversal and escaping symlinks. `FileAssetSource::installed` integrates the root
with GPUI's existing asset interface. These inputs are not writable user storage.

`dev` stages resources, supplies an absolute `GUIC_RESOURCE_DIR`, and runs a copy
of the executable. Windows DLL resources are placed beside that development
copy. Ordinary Cargo execution can set the same absolute environment variable
or call `ResourceRoot::from_directory`. No source-tree fallback is applied by
the resolver. The `application-managed-v1` contract is available for
applications with a custom resolver. Test installed resource loading for each
application and format.

## Platform metadata

`bundle.file-associations` entries accept `extensions`, `name`, `description`
and `mime-type`. `bundle.protocols` entries accept `schemes` and `name`. Both
are arrays of tables and map through the backend. Installation behavior still
requires native acceptance.

The backend allowlist includes `publisher`, `copyright`, `long-description`,
`category`, `license-file`, `macos-info-plist`, `macos-entitlements`,
`macos-frameworks`, `dmg-background`, `deb-depends`, `deb-desktop-template`,
`nsis-installer-icon`, `nsis-header-image` and `nsis-sidebar-image`. Paths are
relative to Guic.toml and are copied into isolated staging. macOS Info.plist
extensions may add usage descriptions but cannot override CLI-owned identity,
version, executable, product name or deployment target. Frameworks preserve
relative internal symlinks only; escaping, absolute or dangling links fail.

## Windows executable resources

Installer artwork is separate from Windows EXE resources. Configure the latter:

```toml
[platform.windows.build.windows-resources]
icon = "packaging/app.ico"
# Enable when GPUI already links its windows-manifest resource.
use-dependency-manifest = true
# manifest = "packaging/app.manifest"
company-name = "Example Organization"
copyright = "Copyright Example Organization"
```

The helper is not published to crates.io. Add a path dependency to your GUIC
checkout in the application's Cargo.toml (adjust the relative path):

```toml
[build-dependencies]
guic-build = { path = "../guic/crates/guic-build" }
```

Call it from the application's build.rs, incorporating this call into any existing
build script:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    guic_build::build()?;
    Ok(())
}
```

The helper embeds the selected binary's ICO, Cargo version, product name and
manifest. Its default manifest
requests standard-user privileges, common controls, PerMonitorV2 DPI awareness
and long paths. A custom manifest replaces that default. Numeric Windows version
components must fit u16; the full Cargo version including prerelease text remains
in the string version resource. The helper does not affect installer artwork.

GPUI 0.4.0 enables its own Windows manifest, including through
`guic-gpui-platform`. Set `use-dependency-manifest = true` for these applications
to embed only branding resources and keep GPUI's manifest. Otherwise MSVC reports
duplicate MANIFEST resources. This option defaults to false and cannot be
combined with `manifest`; the dependency's manifest determines DPI, privileges
and other runtime settings, including whether long paths are enabled.

Native input content hashes participate in build compatibility. The CLI checks
the selected package's Cargo build-script acknowledgement, so omitting the helper
fails instead of silently producing an unbranded executable. Changes invalidate
Cargo's build-script cache. With ordinary Cargo, the helper reads Guic.toml and
its Windows overrides; set `build.bin` explicitly for multi-binary applications.
The helper is native-Windows-only for resource compilation and has no GPUI or
packager dependency. All other hosts keep ordinary Cargo builds working.

Backend lifecycle hooks, executable paths, versions, targets, outputs, signing
secrets and unclassified fields cannot be overridden.

## Signing and notarization

For a signed macOS package:

```toml
[signing]
code-signing = "required"
notarization = "required"
macos-identity = "Developer ID Application: Example Organization (TEAMID)"
macos-keychain-profile = "example-notary-profile"
```

Store credentials separately using the OS tooling, then explicitly run
`package-trial --execute-signing` with the usual project/record/format selectors.
The profile is a credential reference, not a password. The backend signs nested
macOS code; the worker checks the signature against the requested identity.
DMGs are also signed. Required notarization submits an app ZIP or DMG, requires
an `Accepted` result, staples and validates the ticket, then re-verifies the
signature. These commands contact Apple's service only when explicitly executed
with `--execute-signing`; inspection and ordinary builds never submit anything.

Windows signing uses `windows-thumbprint` (40 hexadecimal characters) and
`windows-timestamp-url` (an HTTPS RFC 3161 endpoint) with `code-signing =
"required"`. The backend signs staged inputs and the NSIS output; the worker
independently runs `signtool verify /pa /all`. Verify credential access and
installer/uninstaller behavior on the target system. Linux OS code signing and
updater signatures are outside this CLI's scope.

An off policy never imports signing fields into the backend or runs signing
tools, even if ambient signing environment variables exist. An enabled policy
without `--execute-signing` fails before hooks or packaging. Every requested
verification must succeed; backend success without verified outcomes fails.
Reports retain per-output verification and aggregate status without credentials.

## Hooks

Hooks use a program and argument array, run once for their respective phase,
and fail the operation on any nonzero exit:

```toml
[hooks.before-build]
program = "asset-generator"
args = ["--output", "generated/data.bin"]
cwd = "."
```

A shell is never implicit. To use one, explicitly select `sh` with `-c`, or the
appropriate Windows shell and arguments. Relative program paths containing a
separator resolve from Guic.toml; bare names use PATH. The before-build hook may
generate declared inputs, but cannot change the build configuration during the
operation. `bundle` never runs before-build. `inspect` and `doctor` run neither
hook. They may query Cargo metadata and compiler versions.

## Reports

Reports are JSON on stdout; human progress, Cargo diagnostics, hooks, and the
application's output go to stderr. `--json` also emits versioned error objects
with stage/application/target context when available. Hook arguments, Cargo environment values,
and credentials are omitted from inspection and records; only a configuration
digest is retained. Do not place credentials in hook arguments or build flags:
external tools control their own stderr. Ctrl-C cancels owned child processes
and never produces a success report.

## Diagnostics

`doctor` checks native build tools and SDK tools, packaging prerequisites,
Linux development packages, display/FUSE hints and explicitly requested WebView
availability. `doctor --build-record <record.json>` additionally verifies the
snapshot and inspects imports using `dumpbin`, `otool` or `readelf`; it never
executes that application. Configure application-specific requirements:

```toml
[runtime]
webview = true

[platform.linux.runtime]
glibc-max = "2.31"
pkg-config = ["libexample"]
dynamic-libraries = ["libexample.so.1"]

[platform.windows.runtime]
dynamic-libraries = ["example.dll"]
```

`glibc-max` is checked against imported symbol versions, and is enforced before
packaging. Build on the oldest supported distribution; packaging cannot reduce
an executable's ABI requirements. Build packages and runtime libraries are
reported separately. Linux runtime lookup checks the host loader cache; Windows
checks configured root DLL resources and System32. Explicit dynamic requirements
cover libraries absent from static import tables. Resource packaging alone does
not set ELF rpaths or macOS loader paths. macOS load commands are included for
review of rpaths and deployment metadata. Dependencies of plugins/frameworks,
loader search order and destination-machine compatibility still require native
acceptance; host discovery is not a redistribution guarantee.

WebView is opt-in. Windows checks the installed Evergreen WebView2 registration;
macOS checks system WebKit; Linux checks WebKitGTK 4.1. Runtime installers are
never downloaded or run by doctor. Bundle authorized DLLs explicitly as root
resources, or document the application's runtime prerequisite. Fixed-version
WebView2 installations require application-managed discovery and acceptance.

A failed or unresolved required check returns a nonzero exit status. JSON uses
`passed: null` for unverified checks, rather than a successful result. Diagnostics
record located tool paths, queried tool versions, and relevant lockfile package
versions under `lockfile:` keys (these are lockfile candidates, not proof that a
particular feature used them). No dependencies are installed automatically.

## Build records and backend boundary

Data lives in Cargo's resolved target directory:

```text
guic/<package-id-hash>/
  application.lock
  records/build-<unique-id>.json
  snapshots/build-<unique-id>/<executable>
  snapshots/build-<unique-id>/input-<index>
  artifacts/attempt-<unique-id>/
```

The executable comes from the matching successful Cargo `compiler-artifact`
message, including custom profiles. A fresh copy and SHA-256 digest protect
later bundling from another Cargo build replacing the original. Records are
written atomically without replacing existing records. Stored snapshots are
logically immutable and hashes are verified before use; filesystem permissions
are not a security boundary.

`bundle` verifies package, binary, identifier, application version, target,
profile, feature selection, resource layout, and build configuration. Pass
matching selectors for nondefault profiles/features. Implicit selection requires
exactly one compatible record; two compatible builds require `--build-record`.
Missing, modified, incompatible, and unsupported-version records fail. Packaging
metadata such as icons and publisher does not require rebuilding. Manifest,
compiler, Cargo configuration, declared input, and build-hook changes are treated
conservatively as build-affecting. Identity changes require a new build; installer
upgrade continuity still requires native acceptance.

The adapter uses **cargo-packager 0.11.8**, with its CLI feature disabled. Packaging
runs in an isolated worker with copied inputs, a controlled environment and
serial access to the shared backend cache. Missing or invalid outputs fail the
operation. If a later format fails, earlier outputs remain available with an
incomplete report. See the [architecture](cargo-guic-design.md) for record,
worker and checksum details.

The backend may download external tools over HTTPS and caches them under the OS
cache directory in `.cargo-packager`. The CLI's lock coordinates cargo-guic
processes; it does not coordinate separate cargo-packager processes. Packaging
does not guarantee offline operation or byte-for-byte reproducibility. Include
[THIRD-PARTY-LICENSES.txt](../crates/cargo-guic/THIRD-PARTY-LICENSES.txt) when
redistributing the CLI.

## Testing

See the [testing guide](cargo-guic-testing.md) for automated commands, example
applications, and native installation checks. The CLI and build helper are
unpublished workspace crates (`publish = false`).

Windows integration references: Microsoft's [VERSIONINFO resource specification](https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource)
and [WebView2 Runtime distribution and detection guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).
