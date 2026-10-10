# Platform Smoke Record

Complete this record on physical macOS, Windows, and Linux systems for each
release candidate. Enter the OS version, architecture, GPU, desktop/session,
result, and issue link in release notes. A blank result is not a pass.

## v0.3.0 Verification Status

The maintainer confirmed on 2026-10-10 that platform smoke verification was
completed successfully on macOS, Windows, and Linux. The results below record
that manual verification; they do not represent a new automated test run.
Detailed environment information was not supplied with this confirmation and
should be included in the release record.

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
| `guic-example-hello-world` | Passed | Passed | Passed | startup, root sizing, basic component activation |
| `guic-example-assets-demo` | Passed | Passed | Passed | packaged asset paths, missing/corrupt asset fallback |
| `guic-example-charts-dashboard` | Passed | Passed | Passed | resize, dense labels, hover/keyboard interactions, repeated updates |
| `guic-example-webview` | Passed | Passed | Passed | runtime availability, navigation, focus/clipboard, failure UX |
| `guic-sample-component-gallery` | Passed | Passed | Passed | every story, keyboard traversal, overlays, scrolling, scale factors |

Linux records must name Wayland or X11 and the desktop environment. Windows
records must include a standard user. macOS records must include both keyboard
navigation and VoiceOver.

## Release Sign-off

The maintainer should copy the completed table into the release record and add:

- GUIC revision and pinned `guic-gpui` release
- Rust version and build profile
- optional feature set
- known platform exceptions with issue links
- installer format and signing/notarization verification
