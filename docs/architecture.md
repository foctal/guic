# Architecture

GUIC is a Cargo workspace. Applications normally depend on `guic`, which
re-exports the libraries below.

| Crate | Responsibility |
| --- | --- |
| `guic-core` | Window root, focus, overlays, commands, services, and persistence |
| `guic-native` | Application identity, activation, menus, trays, lifecycle, and sidecars |
| `guic-tokens` | Themes, design tokens, and theme schema |
| `guic-components` | Controls, layouts, and optional table, tree, dock, and Markdown components |
| `guic-charts` | Chart data, rendering, interaction, and export |
| `guic-editor` | Text buffers, selections, editing history, and code editor rendering |
| `guic-icons` | Embedded SVG icons |
| `guic-assets` | Asset sources and installed resource lookup |
| `guic-macros` | String and theme-name macros |
| `guic-webview` | Optional Wry-based WebView integration |
| `guic-build` | Application identity and Windows executable resources at build time |
| `cargo-guic` | Application development and packaging CLI |
| `guic-packager` | Native packaging backend and standalone CLI |
| `guic-packager-updater` | Signed update downloads and installation; re-exported as `guic::updater` |
| `guic-packager-utils` | Shared packaging formats and executable-path helpers |
| `guic-packager-resource-resolver` | Resource lookup for the packager's native layouts |

`guic` enables `components`, `icons`, `assets`, `native`, and `application` by default.
The `application` feature selects the platform backend for `guic::application()`.
The `data-table`, `tree`, `dock`, and `markdown` features enable the corresponding
`guic-components` modules. `charts`, `editor`, `webview`, and `updater` add
separate optional dependencies.

The chart, editor, and WebView crates do not depend on `guic-components`.
The build helper and CLI do not link the GUI runtime.
