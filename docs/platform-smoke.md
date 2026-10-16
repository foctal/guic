# Platform Smoke Checklist

Complete these checks on physical macOS, Windows, and Linux systems for each
release candidate. Enter the OS version, architecture, GPU, desktop/session,
result, and issue link in release notes.

## Shared Checks

For every runnable target:

- launch and clean shutdown
- window resize, minimize, restore, display move, and scale-factor change
- mouse, trackpad where available, keyboard-only navigation, and focus return
- text entry, selection, clipboard, non-ASCII input, and IME composition
- light/dark theme, disabled/focus/hover contrast, and system font fallback
- accessibility tree inspection and representative screen-reader navigation
- error, empty, loading, cancellation, and recovery flows that the target exposes

## Target Matrix

| Runnable target | Target-specific checks |
| --- | --- |
| `guic-example-hello-world` | startup, root sizing, basic component activation |
| `guic-example-assets-demo` | packaged asset paths, missing/corrupt asset fallback |
| `guic-example-charts-dashboard` | resize, dense labels, hover/keyboard interactions, repeated updates |
| `guic-example-desktop-shell` | single-instance argv/cwd forwarding, native menu/tray commands, close/quit/restart confirmation, theme and window-state recovery |
| `guic-example-webview` | runtime availability, navigation, focus/clipboard, failure UX |
| `guic-sample-component-gallery` | every story, keyboard traversal, overlays, scrolling, scale factors |

Linux records must name Wayland or X11 and the desktop environment. Windows
records must include a standard user. macOS records must include both keyboard
navigation and VoiceOver.

## Release Sign-off

The maintainer should record results for each target and platform separately
from this checklist, together with:

- GUIC revision and pinned `guic-gpui` release
- Rust version and build profile
- optional feature set
- known platform exceptions with issue links
- installer format and signing/notarization verification

## Native desktop integration

- Launch a second process with spaces and non-ASCII file arguments from another
  working directory; verify delivery to the primary and successful secondary exit.
- Open a registered URL and file association both before and after UI startup.
- Cancel close, quit, and restart with unsaved work; verify no work was cancelled.
  Approve them and verify save completion, process cleanup, and window behavior.
- Close all windows while retaining a tray, then reopen via a tray command or
  another launch. Verify disabled and scoped commands cannot execute.
- In desktop-shell, press `Ctrl+Shift+T`, inspect the native checked item, then
  click it to toggle back. Each action must increment the visible execution
  count exactly once. Click the disabled item and press `Ctrl+Shift+D`; neither
  may execute. Repeat after focusing a button and reopening the window.
- Switch System/Light/Dark and change OS appearance while System is selected;
  restart and verify the saved preference. Explicit preferences must remain fixed.
- Restore normal, maximized, and fullscreen windows after disconnecting a monitor
  and changing scale factors; confirm controls remain inside a visible work area.
- Install a package containing a helper and invoke it from an unrelated cwd.
  Verify Windows job cleanup and Unix process-group cleanup, cancellation,
  timeout, piped output, and nonzero exit handling.
- Exercise final OS quit/logoff separately from application-managed guarded quit;
  GPUI's final quit hook has a bounded cleanup deadline and cannot veto shutdown.

Local development validation on 2026-10-14 used macOS with a temporary app
launcher around the debug desktop-shell example. Native menu activation,
unsaved-change prompts and cancellation, native close prompting, restart,
theme persistence, geometry persistence, and secondary-process forwarding were
checked. This was not installer/signing acceptance. Windows job-object code was
cross-compiled independently; the full Windows cross-check was blocked by the
missing MinGW C compiler. Windows and Linux native desktop interactions still
require the physical-platform checks above.

The subsequent [Windows validation record](windows-validation.md) distinguishes
native MSVC builds, automated process tests, local installation checks, direct
GUI observations, and any remaining manual acceptance work.
