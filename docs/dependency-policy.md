# Dependency and License Policy

GUIC validates dependency sources, duplicate-version policy, licenses, yanked
packages, and RustSec advisories with `cargo-deny`.

## Blocking Checks

CI and `scripts/release-check.sh` reject unknown registries, Git repositories,
unapproved licenses, and any advisory that is not explicitly reviewed in
`deny.toml`. GUIC's resolved dependency graph is expected to use crates.io
sources only.

## Reviewed Upstream Maintenance Risks

The resolved dependency graph contains upstream maintenance issues that GUIC
cannot safely rewrite in place:

- GUIC pins the independently maintained Apache-2.0 `guic-gpui` crate family
  to one exact release. Review its complete transitive graph whenever that
  release or enabled features change.
- The graph contains unmaintained GTK3, async-std, font, SVG, and macro crates.
- The graph may contain yanked releases or crates without manifest license
  metadata; these findings require an explicit release disposition.
- `zbus_xml 5.1.1` constrains its dependency name to `quick-xml 0.39`. GUIC
  patches that package name to a local, source-free compatibility crate which
  re-exports security-fixed `quick-xml 0.41`. The compatibility crate must be
  removed once upstream accepts the current release.

Directly upgradable vulnerable lockfile entries must be updated immediately.
Each accepted maintenance advisory is pinned by ID in `deny.toml`. This keeps
the advisory gate green without allowing a new advisory to pass unnoticed.
Security vulnerabilities are release-blocking; reviewed unmaintained or yanked
transitive packages must be reconsidered for every release.

Run the complete local report with:

```bash
cargo deny check
```

GUIC may claim no known unmitigated security vulnerabilities only when the
advisory output contains maintenance/yank notices alone. It must not claim that
the dependency graph is free of maintenance risk while those notices remain.

### 2026-08-16 Local Advisory Disposition

`cargo deny check advisories` reported no vulnerability-class advisory in the
locally resolved graph. It did report the following maintenance risks:

- `RUSTSEC-2024-0412` through `RUSTSEC-2024-0420`: the GTK3 Rust bindings used
  by the optional Linux `wry` WebView path are unmaintained.
- `RUSTSEC-2025-0057`: `fxhash` is unmaintained in a target-specific upstream
  dependency path.
- `RUSTSEC-2024-0436`: `paste` is unmaintained and is pulled in by `metal`
  through `guic-gpui` on macOS.
- `RUSTSEC-2024-0370`: `proc-macro-error` is unmaintained in a target-specific
  upstream dependency path.
- `RUSTSEC-2026-0173`: `proc-macro-error2` is unmaintained and is pulled in by
  `stacksafe-macro` through `guic-gpui`.
- `RUSTSEC-2026-0206` and `RUSTSEC-2026-0192`: `rustybuzz` and `ttf-parser` are
  unmaintained in the `usvg`/`fontdb` path through `guic-gpui`.
- `spin 0.9.8` is yanked and is pulled in by `flume` through
  `guic-gpui-scheduler`.

These notices are accepted for the source preview because they are transitive,
no vulnerability-class advisory was present, and replacing them independently
would fork or destabilize the pinned renderer/platform stack. They remain
stable-release risks. Re-review them whenever `guic-gpui`, `wry`, `metal`,
`stacksafe`, `usvg`, `fontdb`, `flume`, or the lockfile changes. Remove each
exception as soon as its upstream path upgrades, disappears, or gains a
maintained compatible replacement. Any newly reported vulnerability remains
release-blocking regardless of this disposition.

## guic-gpui Registry Dependency

GUIC depends on `guic-gpui` 0.2.0 and uses the dependency alias `gpui`, keeping
existing Rust imports source-compatible. Native applications depend on
`guic-gpui-platform` 0.2.0 through the alias `gpui_platform`, with
target-specific renderer features.

The versions are exact because the forked crate family is released in lockstep
and GUIC validates each update as one platform and accessibility baseline. The
0.2.0 release requires Rust 1.95, which is also GUIC's MSRV.

The migration removes GUIC's Zed Git dependency and its local `ztracing`
compatibility patch. `Cargo.lock` must contain no Git sources, `ztracing`, or
`zlog` packages after dependency updates.

## gpui-component Comparison

`gpui-component` remains useful architectural precedent, but its dependency
choices do not define GUIC's policy. The license of an application crate does
not replace the licenses of its transitive dependencies; GUIC reviews the
resolved `guic-gpui` graph directly.
