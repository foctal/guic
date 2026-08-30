# Architecture

The repository follows a multi-crate layout that separates runtime concerns,
tokens, components, optional WebView integration, and tooling.

## Crate topology

- `guic-core` — runtime, focus, overlay, and platform abstractions.
- `guic-tokens` — theming and design tokens.
- `guic-components` — the single canonical home for all reusable widgets,
  including optional subsystems (data table, tree, dock, markdown) gated behind
  feature flags.
- `guic-icons` — icon assets and icon APIs.
- `guic-assets` — asset loading.
- `guic-webview` — optional embedded web surface.
- `guic` — umbrella re-export crate. Its subsystem feature flags (`data-table`,
  `tree`, `dock`, `markdown`) are thin re-exports of the matching
  `guic-components` features and do not introduce a separate taxonomy.
There is intentionally no separate "advanced component" crate. Whether a widget
is compiled is controlled by feature flags on `guic-components`, not by which
crate it lives in.

## Specialized subsystem crates

`guic-components` is the default home for reusable UI widgets. A widget does
not receive a separate crate merely because it is large or optional. For
example, `Dock`, `DataTable`, and `TreeView` remain component subsystems because
they share GUIC's normal rendering, token, focus, and interaction model.

A specialized crate is justified only when a subsystem has one or more of these
properties:

- an independent runtime or state engine;
- substantial platform-specific behavior or native integrations;
- heavyweight dependencies or a materially different compile-time cost;
- high-throughput rendering or storage requirements; or
- an independent API and stabilization lifecycle.

The planned specialized crates are:

- `guic-terminal` — a terminal subsystem with PTY transport, ANSI/VT parsing,
  scrollback, selection, clipboard, resize handling, and high-throughput
  rendering;
- `guic-editor` — a code-editor subsystem with large-buffer editing,
  selections, undo/redo, diagnostics, search, syntax infrastructure, and
  virtualized rendering; and
- `guic-charts` — chart primitives, axes, layout, datasets, interaction, and
  chart-specific rendering infrastructure.

`guic-webview` already follows this rule: its native WebView integration and
Wry dependency are isolated from the default native component stack.

Specialized crates must depend only on `guic-core`, `guic-tokens`, GPUI, and
their own subsystem dependencies. They must not depend on `guic-components`.
The `guic` umbrella crate may expose optional re-export features for each
specialized crate once that crate contains real implementation. No empty public
feature flag or placeholder crate should be added in advance.
