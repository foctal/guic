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
