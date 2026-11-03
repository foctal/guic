# Dependency and License Policy

GUIC validates dependency sources, duplicate-version policy, licenses, yanked
packages, and RustSec advisories with `cargo-deny`.

## Blocking Checks

CI and `scripts/release-check.sh` reject unknown registries, Git repositories,
unapproved licenses, and any advisory that is not explicitly reviewed in
`deny.toml`. GUIC's resolved dependency graph is expected to use crates.io
sources only.

## Maintenance exceptions

`deny.toml` records accepted maintenance advisories by ID. Review exceptions
when dependencies change and remove them when maintained replacements become
available. Security advisories are release-blocking unless explicitly reviewed.

`zbus_xml` 5.1.1 requires `quick-xml` 0.39. A local compatibility crate
re-exports `quick-xml` 0.41 under that dependency name. The corresponding
advisory exceptions apply to the compatibility version, which contains no
upstream parser source. Remove the patch once upstream accepts the newer parser.

Run the current report with:

```sh
cargo deny check
```

## guic-gpui Registry Dependency

The runtime dependency is `guic-gpui` 0.5.0 and uses the dependency alias `gpui`, keeping
existing Rust imports source-compatible. Native applications depend on
`guic-gpui-platform` 0.5.0 through the alias `gpui_platform`, with
target-specific renderer features.

The versions are exact because the forked crate family is released in lockstep
and GUIC validates each update as one platform and accessibility baseline. The
0.5.0 release requires Rust 1.95, which is also GUIC's MSRV.

Use the crates.io runtime family without GPUI Git patches. Native integration
checks are listed in the [platform smoke checklist](platform-smoke.md).

## cargo-guic Backend

CLI installation currently requires `--locked`: `cargo-packager 0.11.8` depends
on `zip ^0.6`, whose releases are all yanked. The existing lockfile still permits
archive verification and installation. `num-modular 0.6.6` in the lockfile is also
yanked. Revisit these dependencies when updating the backend; do not advertise
unlocked CLI installation until fresh dependency resolution succeeds.

The `cargo-guic` CLI pins `cargo-packager = 0.11.8`, disables its CLI
feature, and selects rustls for backend HTTPS tool downloads. The backend is
isolated from the GUIC/GPUI application dependency graph. See [cargo-guic](cargo-guic.md) for supported workflows and restrictions.

The `webpki-roots` 0.26.11 and 1.0.10 certificate-data dependencies use
`CDLA-Permissive-2.0`. Redistribution requires
including the agreement text. Exact-version license exceptions in `deny.toml`
cover these two packages only, and the full agreement is retained in
`crates/cargo-guic/THIRD-PARTY-LICENSES.txt`. Include that file with
CLI binary distributions and review the exception on upgrades.

The `guic-build` helper uses `embed-resource` only on Windows hosts
for native MSVC resource compilation. The resolved version is recorded in
Cargo.lock; the helper does not depend on GUIC, GPUI or cargo-packager. Its
serde/schemars request types are shared with the CLI so configuration
and build-script inputs stay aligned. Runtime resource lookup in guic-assets
uses only the standard library and adds no packaging dependency.

## Optional application updater

The `guic/updater` feature pins `cargo-packager-updater = 0.2.3` and adds its
blocking HTTPS client and Minisign verification support only when enabled. It
uses the cargo-packager update format; CLI preparation signs copies of completed
artifacts and never publishes them. Review the updater's download, signature
verification and platform installation paths when upgrading. Native installation
and restart behavior must be tested in the application.

## Native desktop adapters

`guic-native` uses `tray-icon`/`muda` under their MIT/Apache-2.0 terms. Linux
selects the `ksni` StatusNotifierItem backend rather than requiring a GTK event
loop in a GPUI application. `ksni 0.3.6` declares Unlicense; its public-domain
dedication, permission statement, and warranty text are shipped in
`crates/guic-native/THIRD-PARTY-LICENSES.txt`. The cargo-deny exception is scoped
to that exact dependency version and must be reviewed on upgrade. Other new
native dependencies remain covered by the existing license allowlist.

## Known dependency constraints

The lockfile includes fixes for Markdown and certificate advisories:
`pulldown-cmark` 0.13.6, `rustls-webpki` 0.103.16, and `webpki-roots` 1.0.10.
The Markdown dependency now requires at least 0.13.6. GUIC's Markdown parser
currently enables tables, not the affected wikilinks option.

The following upstream dependency chains have unresolved advisories and are
not exempted from the advisory policy:

- `cargo-packager` 0.11.8 requires `zip` 0.6.6. RustSec reports
  [RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367),
  [RUSTSEC-2026-0368](https://rustsec.org/advisories/RUSTSEC-2026-0368),
  [RUSTSEC-2026-0369](https://rustsec.org/advisories/RUSTSEC-2026-0369), and
  [RUSTSEC-2026-0429](https://rustsec.org/advisories/RUSTSEC-2026-0429).
  The fixed ZIP major version cannot satisfy that backend's current dependency
  constraint. Migrating or replacing the backend's extraction dependency needs
  separate native packaging validation.
- Linux accessibility and notification dependencies use `zvariant`,
  `zbus_names`, and `zvariant_derive`, which are retired as standalone crates:
  [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370),
  [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371), and
  [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372).
  Their replacement is part of zbus 6, requiring migration of the dependent
  accessibility and notification stacks, including the pinned GPUI family.

Review these unresolved advisories before publication.
