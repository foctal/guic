# Native Application Integration

GUIC supplies the desktop runtime adapters in `guic-native`, re-exported as
`guic::native` by the default `native` feature. Applications retain ownership of
commands, document state, confirmation policy, and service lifetimes. The
`guic-core` service traits also accept application-specific implementations.

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
trigger, editable code surface with undo/redo routing, dialogs,
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

## Standard desktop shell

```bash
cargo run -p guic-example-desktop-shell
```

This example uses the packaging identity from `Guic.toml`, persistent native
preferences and window geometry, single-instance forwarding, a tray menu,
commands, confirmation before close/quit/restart, and OS theme tracking.
Its state is stored under `org.guic.desktop-shell` in platform user directories.
Run it a second time with file arguments to exercise activation forwarding.
Protocol registration takes effect when an installer registers `guic-example`.

The startup sequence is:

1. Call `guic_build::build()` in the application build script and create the
   identity with `guic::native::application_identity!()?`. This macro reads the
   calling package's identity and Cargo version, not the library's version.
2. Open `DesktopState`, then call `Instance::acquire` with its `AppPaths` and
   `ActivationRequest::current()`. Exit normally on `Instance::Forwarded`.
3. Retain the primary `ActivationOwner`, call its `bind(&application)` before
   `Application::run`, then start `dispatch` after installing application state.
4. Inside the run callback, call `guic::init(cx)` followed by
   `desktop.install(cx)`. Retain the returned quit subscription.
5. Restore `desktop.windows.get(id)?` through `SavedWindow::options(cx)` before
   opening each window. Call `desktop.track_window(id, window, cx)` and retain
   the returned task. Wrap content in `Root` after installing the theme controller.
6. Keep menu, activation, and geometry tasks alive until shutdown.

`AppPaths` separates config, data, machine-local data, cache, and logs from
read-only application resources. `AppPaths::under` supports portable roots and
isolated tests. `DesktopState::set_theme` persists a preference before applying
it, so a write failure does not silently change the saved setting.

## Activation

The owner holds a per-user filesystem lock and an authenticated loopback TCP
listener. The endpoint and lock live under the application local-data directory;
on Unix the endpoint directory is private and the endpoint file is owner-only.
A secondary process exits successfully only after queue acknowledgement. The
startup request is queued before the UI exists. Arguments and cwd preserve
native OS encoding, and retry IDs suppress recent duplicate deliveries.
Requests are bounded to 64 KiB, with a 64-entry delivery queue. A full queue or
unresponsive owner returns a failure instead of claiming successful forwarding.

The callback receives argv, the sender's cwd, OS URLs, and reopen intent.
`file_argument` resolves selected file arguments against the sender's cwd;
`files` converts OS file URLs. Applications parse their own command-line grammar
and route other schemes. Dispatch raises the active or first window after the
callback, allowing a tray-resident application to create a window first.
The owner installs GPUI's single URL/reopen callback slots; do not replace them
later. `SingleInstanceAdapter` implements the original service trait when
injection is preferable to synchronous pre-event-loop acquisition.

## Menus and trays

`DesktopMenus` owns UI-thread handles. Its cloneable `DesktopMenuHandle`
implements `ApplicationMenuService` and `TrayService`; install it in
`NativeServices` when using service injection. Retain `DesktopMenus::run(cx)`.
Use `DesktopMenuHandle::dispatch` with an application-owned `CommandRouter`
and active-scope callback. It rechecks command registration, enabled state,
and scope at delivery time. Updating checked state or labels uses `set_menu`
or `set_tray`. Replacement uses fresh internal IDs, so stale events do not
invoke commands removed from a menu.

| Platform | Application menu | Tray/status item |
| --- | --- | --- |
| macOS | System menubar | Native status item; PNG/ICO icon or text label |
| Windows | Native menu attached with `attach_window` | Notification-area icon; PNG/ICO required |
| Linux | `Menubar::native(&handle.menu())` in the GPUI window | StatusNotifierItem over D-Bus; PNG/ICO required |

Top-level application entries must be submenus. In-window activations call
`handle.invoke(id)` to use the same command stream. Menubar open state remains
host-controlled. Tray clicks are available through `next_tray_click`; tray menu
commands are the portable interaction across desktop environments. Linux needs
a StatusNotifier host (some environments require an extension). Installation
errors propagate; do not advertise tray availability solely from the OS name.
Only one `DesktopMenus` owner may run in a process; do not install competing
muda/tray-icon global event handlers or consume their receivers separately.

## Lifecycle and geometry

`Lifecycle::guard` returns `Proceed` or `Cancel` asynchronously. All guards run
before work cancellation. Approved transitions cancel registered tasks, execute
cleanup/save callbacks in registration order, then close, quit, or restart.
Errors retain the UI and expose `ShutdownState::Failed`; a subsequent request
can retry, so cleanup callbacks must be idempotent. Register window work by its
`WindowId`; app-wide work is kept alive when only a window closes.
`SidecarProcesses::bind` terminates retained sidecars on app quit/restart.

Use `Lifecycle::request` for menu/shortcut quit and restart. Use
`Lifecycle::restart_after` for updater installation: confirmation and persistence
finish before the installer runs, and only successful installation triggers a
restart. Cancelling a guard prevents installation entirely. `protect_window` covers ordinary native window
close requests. GPUI 0.4.0's final OS quit callback occurs after shutdown has
started: it permits bounded final cleanup, but cannot veto a direct OS quit or
run an interactive guard. Forced process termination cannot promise saves.
Application actions that require confirmation must enter through `request`.

Window state is versioned and backed up by `JsonStore`. Bounds use logical
pixels, preserve maximized/fullscreen restore rectangles, and are clamped to
the current monitor's visible area. Missing monitors fall back to the display
with greatest overlap. Periodic writes run on a worker; ordered revisions stop
an older write from replacing final close geometry. Explicit close/quit/restart
saves exact current geometry before removing the window. Linux AppImage restart
uses the absolute `APPIMAGE` launcher when it exists.

## Sidecars

`Sidecars::discover()` resolves configured names beside `current_exe()`, never
through PATH or cwd. `command(name)` passes literal OS arguments without a
shell. `spawn()` uses null streams; `spawn_piped()` exposes stdin/stdout/stderr
for protocols. Drain stdout and stderr concurrently, and perform blocking waits
on an executor worker. `wait` supports cooperative cancellation and timeouts.

Children are owned, killed and reaped on drop. Unix children have their own
process group; the leader is kept unreaped until group cleanup to prevent PID
reuse races. Windows children start suspended, join a kill-on-close Job Object,
then resume, so descendants cannot escape during startup. Helpers must not
deliberately daemonize out of their managed process group. `SidecarProcesses`
retains app-wide children and binds them to coordinated shutdown. `DesktopState`
also monitors them to reap completed children; standalone use can retain
`SidecarProcesses::monitor` or call `reap_finished`. See
[packaging sidecars](cargo-guic.md#sidecar-executables) for the build contract.
