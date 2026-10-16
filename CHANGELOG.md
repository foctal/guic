# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Changed

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
