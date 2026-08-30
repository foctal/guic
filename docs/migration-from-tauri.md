# Migrating from Tauri

GUIC is not a WebView wrapper. A migration replaces the frontend/runtime split
with native Rust views and explicit application services.

| Tauri pattern | GUIC-native pattern |
| --- | --- |
| JavaScript store | Rust application/entity state with controlled component values |
| `invoke` command | Typed Rust method or `CommandRouter` command |
| emitted event | Typed GPUI event, callback, or channel message |
| Web frontend route | Native view state, tabs, Dock layout, dialog, or window |
| browser local storage | `JsonStore` for non-secret settings and workspace metadata |
| frontend fetch | Rust service client running on an application executor |
| plugin file dialog | Injected native file-dialog service |
| DOM component library | GUIC component plus host-managed domain state |
| xterm.js | `guic-terminal` with application-owned PTY lifecycle |
| charting JavaScript | `guic-charts` with application-owned data and interaction state |

Migrate one workflow at a time:

1. Move the command payload and business logic into a Rust service independent
   of Tauri.
2. Model loading, success, empty, validation, cancellation, and failure as
   explicit Rust state.
3. Build a GUIC view that renders that state and emits typed intents.
4. Route keyboard/menu actions through stable command identifiers.
5. Persist only serializable recovery state and recreate resources such as PTYs
   on restore.
6. Replace platform plugins with small injected service traits.
7. Add GPUI interaction tests, service tests, and real-platform smoke records.

Keep a WebView only for content that genuinely requires a browser engine. The
optional `guic-webview` crate should remain isolated from native application
state and must be reviewed on every target platform.

## Executable workflow map

| Migrated workflow | Executable GUIC reference |
| --- | --- |
| Router/sidebar and split views | `guic-example-native-reference` controlled `DockLayout` |
| JavaScript chart dashboard | Native charts with interaction state and SVG/CSV export APIs |
| Embedded code editor | Controlled `EditorSession` and `CodeEditor` with routed undo/redo |
| xterm.js panel | Bounded `TerminalModel`; real PTY lifecycle in `guic-example-terminal-workspace` |
| Browser form validation | Native `Form`, `FormField`, and typed validation severity |
| Tauri dialog plugin | `FilePicker` routed to `App::prompt_for_paths`, or `FileDialogService` |
| Promise cancellation/retry | `CancellationToken` and explicit progress/error/retry state |
| localStorage recovery | `JsonStore::load_recovering` with synchronized adjacent backup |
| Notification/keychain/updater plugins | Optional typed adapters in `NativeServices` |

This is a source-level migration reference, not physical validation of every
operating-system adapter.
