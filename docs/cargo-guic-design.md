# cargo-guic Architecture

This document explains the implementation boundaries and invariants for
contributors. For application setup and command examples, see the
[CLI guide](cargo-guic.md). Regression and installation procedures are in the
[testing guide](cargo-guic-testing.md).

## Responsibilities

| Component | Responsibility |
| --- | --- |
| Application | Identity, branding, features, resources, runtime dependencies and distribution policy. |
| cargo-guic | Project resolution, Cargo invocation, build records, staging, diagnostics and artifact reports. |
| cargo-packager | Native bundle and installer generation through a library adapter. |
| guic-build | Windows executable resources and the build-script acknowledgement. |
| guic-assets | Application-facing installed resource lookup. |
| Application publisher | Credentials, installation testing, release approval and publication. |

The CLI has no GUIC or GPUI runtime dependency. Application runtime code does not
depend on the CLI or cargo-packager. The build helper shares its request types
with the CLI but has no GUI or packager dependency.

The backend is pinned to cargo-packager 0.11.8 with its CLI feature disabled.
The adapter supplies resolved configuration; it does not delegate application
selection to the backend. Backend hooks and automatic credential discovery are
not part of the application configuration surface.

## Command boundary

`build --no-bundle` produces a recorded executable. `package-trial` packages an
explicit record without compiling it and optionally executes configured signing
policies. Normal distribution through `build` and `bundle` remains disabled in
this version. Trial reports retain `acceptance: "not-verified"`; they do not
encode the outcome of application-specific installation tests.

Inspection and diagnostics do not run project hooks. Development builds and runs
once. No command publishes crates or releases or modifies Git history. Explicit
notarization submits artifacts to Apple's service; it is separate from release
publication.

## Project and build planning

Cargo metadata selects one application package and binary, including inherited
metadata and Cargo's default-run setting. Ambiguous or conflicting selectors
fail. Cargo configuration is resolved from the invocation directory, including
its target, compiler configuration and output directory. The target must match
the native host; Windows requires MSVC.

Configuration merges common settings with the selected platform overrides, then
applies CLI options. Tables merge by key and arrays replace. Unknown fields and
unsupported schema versions fail. Paths in configuration are relative to the
configuration file; CLI paths are relative to the invocation directory.

The resolved plan includes package identity, binary, target, profile, features,
default-feature policy, deployment target, resource contract and input hashes.
Hooks use explicit programs and argument arrays. The before-build hook runs once;
the CLI then resolves the plan again and rejects changed build configuration.
Declared generated inputs must exist before compilation.

Cargo receives structured arguments and emits JSON messages. The CLI requires a
successful build and exactly one matching executable artifact. It never guesses
an executable path from the profile or target name.

## Records and compatibility

Build records, artifact reports and worker messages carry schema versions.
Build records include the executable hash, immutable snapshot location, known
build conditions, build input hashes, tool versions and CLI executable hash.
Records are written atomically without replacing an existing record.

Compatibility checks cover application identity/version, package, binary, target,
profile, features, resource contract and build-affecting configuration. Windows
executable resources are build-affecting; installer artwork and packaged resource
contents are packaging inputs. The CLI checks input and snapshot hashes before
packaging. A record is not an attestation of the current source tree and does not
establish reproducibility.

Records and snapshots live under Cargo's resolved target directory in
`guic/<package-id-hash>/`. Each packaging attempt has separate output directories.
Snapshots are copied rather than hard-linked into mutable staging directories.

## Packaging and worker isolation

An application lock protects its managed artifact area. A cross-process lock in
the OS cache directory serializes cargo-guic access to the backend's shared tool
cache. Separate cargo-packager invocations are outside this locking protocol.

The before-bundle hook runs once. Inputs are collected and hashed, then copied
into isolated per-format staging directories. Resource expansion is deterministic;
empty matches, destination collisions, unsafe paths and resource symlinks fail.
Frameworks may contain relative internal symlinks whose resolved targets remain
inside the framework.

The parent launches the private worker with a versioned request and an environment
allowlist. The worker calls the pinned library adapter and returns output paths,
verification results or a structured failure. Backend hooks stay disabled.

Outputs must match the requested format and remain inside the worker's output
area. The parent verifies their existence, type, size and checksum. Directory
checksums include sorted relative paths, file hashes and validated symlink targets.
A later format failure preserves earlier outputs with an incomplete report.
No post-signing mutation is performed by the managed pipeline.

The backend may acquire external tools over HTTPS. Pinning the Rust dependency
alone does not guarantee offline operation or reproducible native packages.
Backend upgrades require review under the [dependency policy](dependency-policy.md)
and tests of the affected platform formats.

## Runtime resources and native integration

`guic-assets::ResourceRoot` implements `guic-native-v1`: macOS bundle resources,
files beside the Windows executable, and the Linux resource directory relative
to `usr/bin`. The pinned backend uses that Linux layout for both deb and mounted
or extracted AppImages. Applications with their own resolver select
`application-managed-v1`.

Development stages resource files and supplies an absolute `GUIC_RESOURCE_DIR`.
Ordinary Cargo development may supply the same variable or an explicit resource
root. Installed resources are read-only inputs; writable application data belongs
in separate user directories.

`guic-build` reads Guic.toml during ordinary Cargo builds. Under cargo-guic it
receives a resolved request, compiles Windows resources and emits an acknowledgement
through Cargo's build-script output. The CLI checks both the selected package and
request hash. Missing or stale acknowledgements fail the build.

Diagnostics separate build tools and development packages from runtime imports
and explicitly configured dynamic libraries. Import inspection does not execute
the application. Host discovery cannot prove destination-system availability,
plugin dependencies or loader search behavior. Application installation tests
cover those conditions. WebView requirements are explicitly configured rather
than inferred from dependency names.

## Signing

Code signing and notarization have separate off/required policies. Notarization
requires code signing. Executing either requires explicit `--execute-signing` on
`package-trial`. Configuration contains public identities or credential references,
not keys, passwords or certificate payloads. An off policy does not import ambient
signing credentials or invoke signing tools.

The worker verifies requested signatures independently of the backend's success
return. Required macOS notarization must return Accepted, followed by stapling,
ticket validation and signature verification. Reports retain per-output and
aggregate verification states without credentials. A required verification failure
fails the packaging operation.

## Source layout

The private modules in `crates/cargo-guic/src` separate responsibilities:

| Modules | Purpose |
| --- | --- |
| `main.rs`, `cli.rs` | Command parsing, dispatch and output. |
| `config.rs`, `project.rs`, `plan.rs` | Configuration, Cargo metadata and resolved plans. |
| `build.rs`, `process.rs` | Build records, subprocess execution and cancellation. |
| `resources.rs`, `staging.rs` | Input collection, copies, locks and output verification. |
| `backend/` | Worker protocol and cargo-packager adapter. |
| `platform.rs`, `runtime.rs`, `doctor.rs` | Native tools and dependency diagnostics. |
| `signing.rs`, `report.rs` | Signing verification and versioned reports. |

Public compatibility follows [API Stability](api-stability.md). CLI/runtime
compatibility is based on versioned contracts, not identical crate versions.
Cross-compilation, universal binaries, additional package formats, file watching
and updater integration are outside the current scope.

## References

- [cargo-packager library API](https://docs.rs/cargo-packager/0.11.8/cargo_packager/)
- [Cargo external tools and JSON messages](https://doc.rust-lang.org/cargo/reference/external-tools.html)
- [GUIC asset APIs](assets.md)
- [Platform support](platform-support.md)
