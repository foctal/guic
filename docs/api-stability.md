# API Stability

GUIC `0.0.x` releases are source previews. Stability labels apply to public
Rust APIs, feature flags, persistence formats, and host contracts.

| Level | Contract |
| --- | --- |
| Preview | Usable for migration prototypes. Breaking changes require changelog entries but may ship in a `0.0.x` release. |
| Experimental | Design is expected to change and applications should isolate usage behind their own adapter. |
| Internal | Not published and not part of the support contract. |

`guic-core`, tokens, components, icons, assets, macros, charts, terminal, and
WebView are Preview. `guic-editor` is Experimental. Examples, samples, story,
compatibility crates, and tooling are Internal.

Public enums may gain variants while APIs are Preview; exhaustive application
matches should include a fallback. Builder methods preserve controlled-state
ownership: GUIC emits typed intents and the host supplies the next value.
Callbacks execute on the GPUI application thread unless their documentation
explicitly states otherwise. Errors expose stable classifications where hosts
need recovery policy and retain platform details in their display text.

Feature flags only add optional subsystems. Default features must remain a
coherent native component baseline. Removing or renaming a feature is a
breaking change. The MSRV is Rust 1.95. Raising it requires CI, installation,
and changelog updates.

Serialized formats must include a version when their schema can outlive one
process. Readers must migrate or explicitly reject older/newer versions; native
handles, tasks, PTYs, WebViews, and credentials must never be serialized.

The `guic-gpui` family is pinned exactly because renderer and platform crates
move together. An update requires all-feature tests, dependency review,
benchmarks, gallery validation, and the physical platform matrix before it can
become a release baseline.
