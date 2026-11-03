# API Stability

GUIC `0.5.x` is a preview. Public APIs, feature flags, and persistence formats
may change between releases. See the [changelog](../CHANGELOG.md) before upgrading.

| Scope | Status |
| --- | --- |
| Core, tokens, components, icons, assets, macros, charts, and native integration | Preview |
| Editor and WebView integration | Experimental |
| `cargo-guic` and `guic-build` | Preview tooling, versioned separately as `0.1.x` |
| Examples, gallery, compatibility packages, and build tasks | Not published |

Experimental integrations may require application-specific adapters and have
the limitations described in their guides. The `guic` umbrella crate inherits
the status of its enabled features.

Controlled components emit callbacks; the application supplies the next state.
Callbacks run on the GPUI application thread unless documented otherwise.
See [component behavior](component-behavior.md) for layout and focus conventions.

GUIC requires Rust 1.95 or newer. The `guic-gpui` runtime and platform crates
are pinned to matching exact versions. See [installation](installation.md)
for the pins that apply to a release or source checkout.
