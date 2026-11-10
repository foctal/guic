# API Stability

GUIC is a preview. Public APIs, feature flags, and persistence formats
may change between releases. See the [changelog](../CHANGELOG.md) before upgrading.

| Scope | Status |
| --- | --- |
| Core, tokens, components, icons, assets, macros, charts, and native integration | Preview |
| Editor and WebView integration | Experimental |
| `cargo-guic`, `guic-build`, and the `guic-packager` crate family | Preview tooling, versioned separately as `0.1.x` |
| Examples, gallery, and build tasks | Not published |

Experimental integrations may require application-specific adapters and have
the limitations described in their guides. The `guic` umbrella crate inherits
the status of its enabled features.

Controlled components emit callbacks; the application supplies the next state.
Callbacks run on the GPUI application thread unless documented otherwise.
See [component behavior](component-behavior.md) for layout and focus conventions.

GUIC requires Rust 1.95 or newer. The `guic-gpui` runtime and platform crates
are pinned to matching exact versions. See [installation](installation.md)
for the pins that apply to a release or source checkout.

## The 0.6 compatibility contract

Starting with 0.6.0, patch releases preserve the documented core, token,
component, chart, asset, icon, macro, and native integration APIs and feature
names. Additive capabilities use new methods or opt-in features. Bug fixes may
change behavior that contradicted the documentation; the changelog describes
observable changes. A source-breaking change requires a minor release and
migration guidance. Applications should retain their lockfiles for reproducible
runtime and transitive dependency versions.

Existing documented persistence schemas remain readable throughout 0.6.x.
Writers introducing a new schema version must preserve a reader or explicit
migration for the previous version. Undocumented internal structures are not
persistence formats. Editor and WebView retain the experimental limitations
listed above; this contract does not turn unvalidated behavior into a guarantee.

The compatibility-preserving `accessibility` extension keeps its original
concrete return type. New component implementations should use `accessible`,
which also exports disabled semantics through a transparent element wrapper.
