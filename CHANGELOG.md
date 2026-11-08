# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Added

- Select rectangular ranges with Shift+Alt-drag across shaped display rows,
  including soft wrapping, short lines, multiple cursors, and pending paste
  selection rebasing through queued edits and undo/redo. Route edits over many
  selections through the ordered background queue.

- Expose disabled accessibility semantics without additional layout or focus nodes,
  numeric control actions, and password/multiline text roles and editable values.
- Add editable numeric fields, calendar month/year navigation with local-date
  defaults, shared indexed table data, and lazily measured variable-height lists.
- Add application-owned translation catalogs, a native application/window facade,
  styled Markdown inline content, and link activation handlers.
- Continue editor drag selection and scrolling while the pointer is stationary
  outside the viewport, render secondary cursors, and offer configurable cursor
  blinking that pauses during composition.
- Read basic input clipboard text asynchronously with ordered commands, an IME
  draft, rebased pointer selections, and cancellation on authoritative changes.

- Read editor clipboard text asynchronously and preview pointer selections during
  pending pastes, preserving input order and rebasing selections through edits
  and queued undo/redo.
- Configure Windows native-menu and WebView example windows through the runtime's
  per-window composition API instead of mutating process-wide environment variables.

- Prepare large native and clipboard pastes asynchronously, with ordered follow-up
  edits, queued clipboard/history commands, a responsive IME composition context,
  progress feedback, and stale-result rejection on host replacement or closure.

- Add `EditorSession::clear_history()` and bounded background release of large
  retired history batches, including redo invalidation and session destruction.

- Virtualize long non-wrapped editor lines horizontally using indexed grapheme
  segments, keeping selection, caret reveal, and IME geometry aligned.
- Share cancellable syntax analysis and byte/grapheme/UTF-16 checkpoints across
  editor snapshots and reflow. Painting borrows cached syntax ranges without
  scanning or copying whole logical lines.

- Run large editor measurements and width/font reflow in cancellable background
  jobs, with bounded initial previews, stale-result rejection, and scroll/caret
  preservation. Release replaced layout metadata off the UI thread.

- Add opt-in CodeEditor soft wrapping with Unicode break opportunities, grapheme
  fallback, visual-row navigation, and shared selection/IME geometry.
- Add draggable editor overflow scrollbars, track paging, outside drag capture,
  and logical viewport anchoring on resize.
- Share large editor buffers and history through persistent line chunks; index
  native UTF-16 ranges and cache incremental wrap metadata. Add direct `line()`
  and `iter_lines()` access while preserving `lines()` compatibility.

- Add native IME composition to CodeEditor, with marked-text decoration and one
  committed host edit per composition; preserve composition across host redraws.
- Add glyph-based editor hit testing, text-span selection, double/triple-click
  selection, two-axis wheel scrolling, and automatic primary-caret reveal.
- Retain editor viewport state, shape visible rows only, and preserve the preferred
  horizontal position during vertical navigation.

- Add bounded TextInput undo/redo with native shortcuts, selection restoration,
  and one history entry per IME composition.
- Add `TextInput::set_value` and `set_disabled` for updating retained entities
  without replacing focus handles. Programmatic value changes reset history and
  do not emit user-edit callbacks.
- Implement native text selection updates and UTF-16 document-length queries.

### Changed

- Update the pinned runtime and platform family to the published `guic-gpui`
  0.6.0 crates.

### Fixed

- Prevent PasswordInput Copy and Cut actions from exposing the masked value
  through the clipboard, while preserving paste and undo behavior.

- Avoid waiting on background position-index initialization and repeating large
  text comparisons when controlled hosts accept prepared edits. Release unused
  prepared snapshots off the foreground thread.

- Partition large new Unicode position indexes across up to four workers at
  verified grapheme boundaries. Preserve synchronous paste/IME ordering and
  fall back to serial indexing when workers cannot be created.

- Preserve per-line position indexes and unchanged syntax caches through edits,
  chunk rebuilding, and undo/redo. Update long-line positions around replacement
  boundaries, including line splits, joins, and Unicode cluster changes.
- Normalize pasted line endings once, avoid whole-document intermediate strings
  for native replacement, and derive multi-cursor paste positions from byte ranges.
  Keep the caret after an inserted combining mark when it joins the preceding text.

- Keep editor candidate geometry current before repaint, preserve grapheme cursor
  bounds after combining-character insertion, and normalize CRLF/CR to LF.
- Avoid repeatedly allocating the remaining line while classifying editor syntax,
  and wire editor undo/redo commands in the gallery.

- Convert IME selection offsets relative to the replacement text, including
  supplementary Unicode characters and normalized line endings; clamp malformed
  or oversized native ranges to valid text boundaries.
- Reject native edits and editing actions on disabled text inputs, normalize
  CRLF and CR line endings, and suppress unchanged-value notifications.
- Stop scheduling another render from TextInput painting, allowing idle inputs
  to remain idle.

### Migration notes

- Keep a stable CodeEditor ID to retain composition and scrolling. `visible_lines`
  now defines a fixed-height viewport; `first_line` initializes or repositions it
  when that option changes. IME preedit updates stay local until committed.
- EditorBuffer now normalizes line endings to LF. Preserve file newline conventions
  in the application's save layer when required.

## 0.5.0 - 2026-11-03

### Changed

- Correct component examples and consolidate usage guides; remove internal
  validation records and planning notes.
- Update the pinned `guic-gpui` runtime and platform family to 0.5.0.
- Update compatible Markdown and TLS dependencies to patched releases; temporarily
  accept seven upstream ZIP and zbus advisories pending migration decisions.
- Apply Select sizing to popup typography and compact option padding.
- Make TreeView rendered row pitch match virtualization, center disclosure cells,
  and add fixed/fill height, body-only presentation, and scroll tracking.
- Add independent secondary Label text sizing and Dock leaf padding, border,
  and active-tab focus presentation without changing serialized layouts.
- Keep modal cards and actions inside the viewport, including nested placement,
  and prevent card and scrim clicks from reaching underlying controls.
- Improve secondary and placeholder text contrast, and select readable accent
  button text for composited normal, hover, and pressed colors.
- Fix feature-dependent component documentation links and check strict rustdoc
  with minimal and full component configurations in the existing CI job.

- Prepare `guic-build` and `cargo-guic` 0.1.0 for crates.io publication, with
  versioned dependencies, packaged licenses, and archive verification. After
  manual publication, install with `cargo install cargo-guic --locked` and run
  `cargo guic`.

## 0.4.0 - 2026-10-16

### Added

- Implement `guic/native` with the new `guic-native` crate: shared build/runtime
  identity, writable application directories, authenticated single-instance
  activation, native menus and tray services, coordinated shutdown, recoverable
  window geometry, persisted theme preference, and owned sidecar processes.
- Add OS light/dark tracking with explicit System/Light/Dark preference, a
  `CommandRouter::resolve_id` path for native commands, and `Menubar::native`
  for the shared menu model on Linux and custom window chrome.
- Add the unpublished `cargo-guic` CLI and `guic-build` helper for project
  configuration, development launch, verified build snapshots, diagnostics,
  external resources, and native packaging (Linux DEB/AppImage, macOS app/DMG,
  and Windows NSIS/MSI/MSIX).
- Package target-specific sidecars from `bundle.sidecars`, snapshot and verify
  them with build records, and use the same executable-relative layout in dev.
- Emit application identity from `guic-build` on all desktop platforms and add
  the runnable `guic-example-desktop-shell` integration example.
- Add opt-in cargo-guic file watching with automatic rebuild, restart, and recovery
  after build failures.
- Support macOS Universal Binary builds and packaging through
  `--target universal-apple-darwin`.
- Add local update key generation and signed update feed preparation, plus the
  optional `guic/updater` feature for in-app update checks and installation.

### Fixed

- Terminate owned Unix process-group descendants when a cargo-guic child exits,
  including background processes left by hooks or development applications.
- Require an executable Bash for Linux AppImage diagnostics and packaging, and
  reject non-executable files during Unix tool discovery.
- Preserve unmatched combining marks during editor search-and-replace, cover
  partial grapheme matches in search selections, and bound visible-line arithmetic.
- Clean up JSON save temporaries after backup failures without removing files
  that the save did not create.
- Normalize file watcher roots and exclusions, including parent components and
  symlinked directory prefixes.
- Keep chart coordinates finite for axis ranges spanning extreme finite values.
- Make umbrella-crate examples compile without default features and allow
  `guic::theme_name!` without a direct `guic-tokens` dependency.
- Canonicalize MSIX test fixture paths so tests accept macOS temporary directories
  reached through the system `/var` symlink.

### Security

- Update locked rustls to 0.23.46 to address RUSTSEC-2026-0346 through
  RUSTSEC-2026-0349.

### Migration notes

- The default-enabled `native` feature now includes the `guic-native` desktop
  services dependency instead of acting as a placeholder. Applications that do
  not need these services can select features with `default-features = false`.
- Applications using shared native identity should call `guic_build::build()`
  from their build script on every desktop platform. The helper and cargo-guic
  remain checkout-only tools, not published crates.

## 0.3.0 - 2026-10-10

- Upgrade the guic-gpui runtime family to 0.4.0.
- Verify command palette accessibility focus with GPUI's headless tree builder.
- Fix Linux WebView example startup and embedding with GTK initialization, an
  XCB-to-Xlib parent handle adapter, and GTK event processing. Use X11/XWayland
  and a compatible WebKit rendering default; document native Wayland limits.
- Keep CommandPalette accessibility focus on its search input only.
- Bound and scroll palette results, including wrapped descriptions, and reveal
  the active command on keyboard navigation and query changes.
- Use native Command/Control editing shortcuts and expose typed text-input
  actions for application overrides.
- Add InputGroup and InputGroupAddon for inputs with adjacent units or actions,
  and Skeleton for static loading placeholders.
- Support Up/Down and Shift+Up/Down navigation in TextArea across visual lines,
  including soft wraps, while preserving the preferred horizontal position.
- Move focus between child tab stops when Root receives unhandled Tab or
  Shift+Tab keys.
- Center ConfirmDialog in the viewport, constrain confirmation overlay sizes,
  and allow long dialog content to scroll. Focus the cancel action on opening,
  trap Tab navigation within the dialog, and restore focus on closing.
- Make interactive InputNumber controls keyboard-focusable by default and avoid
  reporting unchanged values at range limits.
- Fix Slider endpoint snapping, extreme finite ranges, and small value changes;
  focus on click, exclude disabled sliders from tab order, and ignore unrelated
  drags.
- Include partially visible rows in VirtualListMetrics, keep ranges safe after
  data shrinks, and bound total height for extreme content sizes.
- Add stable Chip and Tag identities independent of labels. Prevent chip removal
  from activating the chip, suppress disabled removal, and use native keyboard
  activation.
- Standardize action icons and improve Dock close-button keyboard and
  accessibility behavior.
- Accept CRLF checkout line endings during theme schema validation.

### Migration notes

- Supply a unique, stable `.id(...)` for sibling Chip and Tag instances created
  at the same call site, such as in a loop. Both components now use the
  constructor call site as their default identity.
- Keep ConfirmDialog rendered with `.open(false)` after closing so it can
  restore focus to the previously focused control.
- Text-input editing shortcuts use Command on macOS and Control on other
  platforms. Register application overrides with the public typed actions in
  `guic_components::text_input` under the `GuicTextInput` key context.

## 0.2.0 - 2026-09-06

- Upgrade the guic-gpui runtime family to 0.3.1.
- Unify single-line control sizing and stabilize component identity.
- Preserve wrapper geometry and improve Select layout, overlay focus restoration,
  and modal keyboard navigation.
- Add Dock customization, persistent split identities, resize constraints,
  protected pinned tabs, and host-mediated tab transfers.
- Improve terminal rendering, Unicode handling, search, shell integration, shared
  model access, and PTY lifecycle reporting.
- Add regression coverage and document component behavior and host contracts.

### Migration notes

- Direct `DockNode::Split` construction requires an `id`; older serialized
  layouts remain supported through `DockLayout::from_json`.
- Direct Dock drag payload construction must initialize `source_dock_id` and
  `source_window_id`. Hosts handle cross-Dock transfers explicitly.
- MultiSelect triggers stay on one line and clip excess selected chips.
- Review the [component behavior and host integration guide](docs/component-behavior.md)
  for control sizing, stable IDs, controlled focus lifecycles, and terminal ownership.

## 0.1.0 - 2026-08-30

- Initial public release.
