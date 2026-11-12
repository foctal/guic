# Changelog

All notable changes to this project will be documented in this file.

## 0.6.0 - 2026-11-10

### Added

- Add `guic::application()`, `guic::open_window()`, and a matching `gpui` re-export
  for applications using a single toolkit dependency.
- Add application-owned translation catalogs, editable numeric inputs, calendar
  month/year navigation, shared table rows, and variable-height virtual lists.
- Render Markdown inline styles and clickable links with cached parsing.
- Add native editor IME composition, shaped selection and hit testing, soft
  wrapping, draggable scrollbars, rectangular selection, and secondary cursors.
- Prepare large editor edits and clipboard pastes asynchronously while preserving
  input order, pointer selections, undo/redo, and composition.
- Add shared editor snapshots and position indexes, background reflow, horizontal
  virtualization of long lines, and `EditorSession::clear_history()`.
- Add TextInput undo/redo and retained-entity `set_value`/`set_disabled` methods.
- Expose disabled accessibility semantics, numeric actions, and password and
  multiline text roles.

### Changed

- Maintain the packaging backend, updater, utilities, and resource resolver as
  workspace-owned `guic-packager` crates. Keep existing GUIC configuration keys.
- Pin the runtime and platform family to `guic-gpui` 0.6.2, including fixes
  for native child-window resizing and transparent backgrounds on Linux with
  the VMware SVGA3D OpenGL renderer.
- Use per-window composition settings in Windows native-menu and WebView examples.
- Replace the local XML compatibility patch with `zbus_xml` 5.2.1, which no longer
  uses the affected parser. Replace yanked lockfile entries for `spin`,
  `async-executor`, and `num-modular` with compatible available releases.
- Verify all public crate archives together before publication. Run Windows
  sidecar cleanup tests in the existing CI job.

### Fixed

- Pass macOS signing requirements as inline expressions during verification.
- Validate macOS update archives and atomically replace application bundles.
  Request system authorization when needed and preserve the old app on cancellation.
- Resolve umbrella string macros through GUIC for single-dependency applications.
- Decrement the mounted-root count when a root view is released.
- Prevent password Copy/Cut actions from exposing secrets through the clipboard.
- Preserve asynchronous clipboard order and reject stale edits after host
  replacement, disabling, or editor removal.
- Correct Unicode and normalized-newline selection offsets, native range bounds,
  and composition undo boundaries. Reject native edits on disabled text inputs.
- Avoid idle TextInput redraws and repeated large-document scans during editing.
- Make dense editor search walk each line's grapheme boundaries once.
- Preserve Markdown quote and list structure, ordered-list starting numbers,
  literal ampersands, and suppressed inline HTML content.
- Reject Windows device names containing superscript digits in packaged resources.
- Bound the total activation-request receive time so trickling clients cannot
  keep application shutdown waiting indefinitely.

### Migration notes

- Update direct GUIC crate dependencies to 0.6.0. Applications with explicit
  `guic-gpui` or `guic-gpui-platform` dependencies must pin them to `=0.6.2`.
- `guic::updater` now re-exports `guic-packager-updater`. Direct upstream types
  are distinct. `Config` adds `dangerous_insecure_transport_protocol`; set it
  to `false` or use `..Default::default()`. The HTTP client re-export uses reqwest 0.13.
- `application` joins the default features and selects the platform backend.
  Applications supplying their own backend can disable default features and
  explicitly enable the required subsystems.
- Keep a stable CodeEditor ID to retain composition and scrolling. `visible_lines`
  defines a fixed-height viewport; changing `first_line` repositions it.
- Editor preedit updates stay local until committed. Apply undo/redo synchronously
  in `on_command` and notify the host so queued input can resume.
- EditorBuffer normalizes line endings to LF. Preserve file newline conventions
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
