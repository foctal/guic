# Platform Smoke Record

Complete this record on physical macOS, Windows, and Linux systems for each
release candidate. Enter the OS version, architecture, GPU, desktop/session,
result, and issue link in release notes. A blank result is not a pass.

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

| Runnable target | macOS | Windows | Linux | Target-specific checks |
| --- | --- | --- | --- | --- |
| `guic-example-hello-world` | Pending | Pending | Pending | startup, root sizing, basic component activation |
| `guic-example-assets-demo` | Pending | Pending | Pending | packaged asset paths, missing/corrupt asset fallback |
| `guic-example-charts-dashboard` | Pending | Pending | Pending | resize, dense labels, hover/keyboard interactions, repeated updates |
| `guic-example-terminal-demo` | Pending | Pending | Pending | default shell, input/IME, copy/paste, resize, exit/restart |
| `guic-example-terminal-workspace` | Pending | Pending | Pending | tabs/splits, PTY lifecycle, persistence recovery, many panes |
| `guic-example-webview` | Pending | Pending | Pending | runtime availability, navigation, focus/clipboard, failure UX |
| `guic-sample-component-gallery` | Pending | Pending | Pending | every story, keyboard traversal, overlays, scrolling, scale factors |

Linux records must name Wayland or X11 and the desktop environment. Windows
records must include a standard user and exercise cmd plus available PowerShell
profiles. macOS records must include both keyboard navigation and VoiceOver.

## Release Sign-off

The maintainer should copy the completed table into the release record and add:

- GUIC revision and pinned `guic-gpui` release
- Rust version and build profile
- optional feature set
- known platform exceptions with issue links
- installer format and signing/notarization verification
