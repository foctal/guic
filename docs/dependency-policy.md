# Dependency and License Policy

GUIC validates dependency sources, duplicate-version policy, licenses, yanked
packages, and RustSec advisories with `cargo-deny`.

## Blocking Checks

CI and `scripts/release-check.sh` reject unknown registries, Git repositories,
unapproved licenses, and any advisory that is not explicitly reviewed in
`deny.toml`. GUIC's resolved dependency graph is expected to use crates.io
sources only.

## Advisory exceptions

`deny.toml` records accepted advisories by ID. Review exceptions
when dependencies change and remove them when maintained replacements become
available. Security advisories are release-blocking unless explicitly reviewed.

The lockfile uses `zbus_xml` 5.2.1, which no longer depends on `quick-xml` 0.39.
The remaining XML dependencies use fixed parser versions, so no local parser
patch is required. Applications
should audit their own lockfiles: library dependencies do not inherit this
workspace's lockfile or advisory exceptions.

Run the current report with:

```sh
cargo deny check
```

## guic-gpui Registry Dependency

The runtime dependency is `guic-gpui` 0.6.1 and uses the dependency alias `gpui`, keeping
existing Rust imports source-compatible. Native applications depend on
`guic-gpui-platform` 0.6.1 through the alias `gpui_platform`, with
target-specific renderer features.

The versions are exact because the forked crate family is released in lockstep
and GUIC validates each update as one platform and accessibility baseline. The
0.6.1 release requires Rust 1.95, which is also GUIC's MSRV.

Use the crates.io runtime family without GPUI Git patches. Native integration
checks are listed in the [platform smoke checklist](platform-smoke.md).

## cargo-guic Backend

The `cargo-guic` CLI uses the workspace-owned `guic-packager` 0.1.0 fork,
with its CLI feature disabled and rustls selected for HTTPS tool downloads.
The fork retains the supplied upstream source's ZIP 9 dependency, replacing
`cargo-packager` 0.11.8 and ZIP 0.6. The backend remains isolated from the
GUIC/GPUI application dependency graph. See [cargo-guic](cargo-guic.md).

Two inherited tool-download constraints remain: ZIP extraction has no expanded-size
budget, and the NSIS ApplicationID plugin is downloaded over HTTPS without a pinned
checksum. Updating ZIP removes the listed dependency advisories but does not resolve
these separate backend hardening tasks.

The `webpki-roots` 1.0.10 and `webpki-root-certs` 1.0.11 certificate-data dependencies use
`CDLA-Permissive-2.0`. Redistribution requires
including the agreement text. Exact-version license exceptions in `deny.toml`
cover these packages only, and the full agreement is retained in
`THIRD-PARTY-LICENSES.txt` in cargo-guic, guic-packager, and guic-packager-updater. Include the file with
CLI binary distributions and review the exception on upgrades.

The `guic-build` helper uses `embed-resource` only on Windows hosts
for native MSVC resource compilation. The resolved version is recorded in
Cargo.lock; the helper does not depend on GUIC, GPUI or cargo-packager. Its
serde/schemars request types are shared with the CLI so configuration
and build-script inputs stay aligned. Runtime resource lookup in guic-assets
uses only the standard library and adds no packaging dependency.

## Optional application updater

The `guic/updater` feature uses the workspace-owned `guic-packager-updater`
0.1.0 fork. It adds a blocking HTTPS client and Minisign verification only when
enabled. The update feed and signing format are retained; CLI preparation signs
copies of completed artifacts and never publishes them.

The macOS installer validates archive paths and links, stages the new bundle on
the destination volume, and atomically exchanges bundles. When permissions
require administrator access, installation uses the system authorization dialog.
Cancellation and failures before the swap preserve the old application. Filesystems
without atomic swap support fail without deleting the old bundle. Authorization
requires UTF-8 paths. Signed/notarized updates and restart behavior require native
application acceptance before distribution.

`guic::updater` retains its module name. The newer upstream `Config` adds
`dangerous_insecure_transport_protocol`: set it to `false` or use
`..Default::default()` in existing struct literals. Release builds require HTTPS
by default. The re-exported client is reqwest 0.13. Types from the upstream
updater and this fork are distinct; use the fork consistently.

The four `guic-packager` crates retain upstream Apache-2.0 OR MIT licensing and
copyright notices. Their `UPSTREAM.md` files identify the source snapshot.

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

The following three advisories remain temporarily accepted by ID in `deny.toml`
pending migration of the Linux accessibility and notification stacks. These
exceptions allow the policy check to pass; the maintenance risks remain unresolved.
New advisories are not covered. The former ZIP 0.6 exceptions have been removed
because the packaging fork uses ZIP 9.

- Linux accessibility and notification dependencies use `zvariant`,
  `zbus_names`, and `zvariant_derive`, which are retired as standalone crates:
  [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370),
  [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371), and
  [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372).
  Their replacement is part of zbus 6, requiring migration of the dependent
  accessibility and notification stacks, including the pinned GPUI family.

Revisit these exceptions when the migration policy is decided or the affected
dependencies change. Remove them when the corresponding advisories no longer
apply.
