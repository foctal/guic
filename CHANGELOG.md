# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

- Made dependency advisories release-blocking in CI and local release checks,
  with reviewed upstream maintenance notices pinned explicitly in `deny.toml`.
- Added complete crates.io README, homepage, keyword, and category metadata to
  every public crate, clarified the consumer installation path, and made
  independent package checks compile the generated archives.
- Replaced the obsolete implementation plan with current design principles and
  tightened preview wording in the component gallery and release documentation.

- Fixed chart tooltips that could render as empty popovers or fail to appear by
  keeping their active view in a window-and-chart-scoped weak registry instead
  of recreating transient state during parent rendering. Native tooltips now
  open with zero delay, follow pointer movement, update values without a click,
  and dismiss on exit. Cartesian tooltips now require both X and Y to intersect
  painted data geometry by default, with continuous nearest selection available
  explicitly, plus nearest/shared-index/dataset grouping. Tooltips
  render a title, color keys, dataset or point labels, and formatted values.
- Added exact bar/slice hit geometry, configurable point radius and row bounds,
  independently updated tooltip state, and cached derived axes for
  allocation-free category/bar pointer hit testing.
- Unified chart painting and hit testing around the same shared, stacked, or
  named value-axis transform, fixing vertical tooltip misses in multi-dataset
  line and stacked bar charts. The charts dashboard now scrolls vertically.
- Added typed native-service contracts for dialogs, notifications, menus, tray
  items, credential vaults, deep links, single-instance routing, updater
  handoff, capability detection, and cooperative cancellation.
- Added directional keyboard navigation and interaction coverage across
  collection, selection, tab, calendar, and tree-select components.
- Added chart selection callbacks, bounded label-collision policies, SVG image
  export, application overlay hooks, and bounded accessibility/tooltips.
- Added editor language/completion contracts, search/replace, page movement,
  indentation, diagnostic activation, widget undo/redo routing, and a
  100,000-line model benchmark.
- Added long-session and mixed-control terminal benchmarks, retained-memory
  estimation, and compact trailing scrollback cells.
- Added an integrated native reference application with file import/export,
  cancellable background work, errors/retry, crash-resistant persistence,
  theme and asset integration, multi-window creation, Dock, editor, and
  terminal surfaces; converted ignored Rustdoc examples into compiling
  `no_run` examples.
- Added public crate dependency versions, package-content checks, publication
  order, and explicit API stability policy.

- Migrated from the pinned Zed GPUI Git revision to the crates.io
  `guic-gpui` and `guic-gpui-platform` 0.2.0 releases, removed the obsolete
  `ztracing` compatibility patch, and raised the MSRV to Rust 1.95.
- Added crash-resistant JSON persistence with backup recovery for settings,
  recent files, workspace metadata, and window state.
- Added form layout, field metadata, validation feedback, and validation summary
  components.
- Added a searchable and keyboard-operated command palette with accessible
  result semantics and GPUI interaction tests.
- Added complete focusable-menu keyboard navigation, activation, typeahead, and
  GPUI interaction coverage.
- Replaced vulnerable `quick-xml 0.39` implementation code with a local
  compatibility bridge to security-fixed `quick-xml 0.41`.
- Added native integration, packaging, Tauri migration, persistence, and
  automated release-check guidance.
- Added a terminal conformance matrix and repeatable stress benchmarks for dense
  output, scrollback, resize reflow, and many-pane workloads.
- Added Windows ConPTY integration coverage for cmd and available PowerShell
  profiles, including terminal query replies, working-directory inheritance,
  resize propagation, restart, graceful exit, and force close.
- Fixed Windows shell discovery to mark the first available shell as the
  default, treated ConPTY's post-termination OS success code as successful, and
  ensured dropping a local PTY session terminates its child process.
- Fixed duplicate printable-key delivery by routing character input exclusively
  through GPUI's native text-input/IME path while retaining direct key handling
  for control, navigation, and modified terminal keys.
- Replaced per-cell terminal element rendering with a native GPU canvas that
  merges background spans and batches compatible shaped text by row.
- Fixed native terminal font fallback and cell measurement, automatic PTY
  output refresh, pane-bound resizing, and Dock split overflow in the terminal
  examples.
- Removed prototype terminal output injection and exposed all terminal workspace
  actions through a horizontally scrollable toolbar.
- Added common SGR text attributes and colon-delimited colors, full ANSI
  background rendering, background-color erase, combining-mark preservation,
  RIS/DECSTR reset handling, complete cursor-state restoration, Secondary DA,
  and DEC private-mode status reports.
- Added selection-aware editor edits, Unicode-safe range operations, clipboard
  shortcuts, cursor navigation, and a bounded undo/redo session model.
- Made action-backed buttons and icon buttons keyboard-focusable with native
  Enter and Space activation.
- Established the initial multi-crate workspace.
- Added the first runtime and theme skeletons.
- Added schema generation through `xtask`.
