# Native Application Integration

GUIC owns native UI surfaces; the application owns operating-system services.
Keep those services behind traits so UI state and platform adapters can be
tested independently.

| Capability | Recommended application boundary |
| --- | --- |
| Application menu | Register app commands in `CommandRouter`; map native menu callbacks to the same command identifiers. |
| File dialogs | Inject an async `FileDialogService`; return paths or handles to application state. |
| Clipboard | Use GPUI clipboard APIs from commands and expose domain-specific copy/paste operations. |
| Notifications | Inject a `NotificationService`; request permission and apply product policy outside component code. |
| Tray/status item | Keep the native handle in the app shell and route actions through the command registry. |
| Drag/drop files | Normalize platform paths/URLs at the window boundary before updating application state. |
| Deep links | Parse and validate URLs in a single-instance app-shell adapter, then dispatch typed commands. |
| Preferences | Store non-secret values with `JsonStore`; keep secrets in the platform credential vault. |
| Window lifecycle | Persist serializable bounds/workspace metadata, cancel or detach background work explicitly, and never serialize native handles. |
| Background work | Spawn through GPUI/application executors, return typed progress events, and make cancellation ownership explicit. |

Platform adapters should report unsupported capabilities rather than silently
doing nothing. User-visible failures need a retry or recovery action. Test the
service traits with deterministic fakes, then run the platform smoke checklist
against real adapters before release.

`guic-core::NativeServices` provides typed contracts for file dialogs,
notifications, application menus, tray/status items, secure credentials, deep
links, single-instance forwarding, and updater handoff. `NativeCapabilities`
makes missing adapters explicit, `ServiceErrorKind` separates failure policy,
and `CancellationToken` provides cooperative cancellation without binding
services to one async runtime. `DroppedItem` normalizes platform drag/drop.

The runnable `guic-example-native-reference` demonstrates a controlled Dock
workspace containing a dashboard, validated form and native file import
trigger, editable code surface with undo/redo routing, terminal, dialogs,
retry/error states, and cancellable work state:

```bash
cargo run -p guic-example-native-reference
```

The reference exercises a timer-driven cancellable background operation,
primary/backup workspace recovery through `JsonStore`, dependency-free chart
SVG export, runtime theme switching, asset-manifest registration, and creation
of an independent secondary window. State and export files are written under
the operating system temporary directory in `guic-native-reference` so the
example does not mutate the source tree.
