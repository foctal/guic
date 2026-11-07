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

## Editor input and viewport

- In the gallery editor, compose and cancel Japanese, Chinese, or Korean text
  after emoji and in the middle of a selection. Candidate windows must follow
  the caret after vertical/horizontal scrolling and display-scale changes.
- Commit a composition, undo once, and redo; intermediate preedit text must not
  appear as separate history entries. Switch focus during composition and verify
  that the committed result is delivered once.
- Click and drag across combining characters, emoji, tabs, and multiple lines.
  Check Shift+click, double-click words, triple-click lines, and selection direction.
- Scroll long and wide documents, navigate with arrows and Page Up/Down, and shrink
  the document while scrolled. Verify primary-caret reveal and fixed line numbers.
- Confirm that the input remains idle when no event occurs, and exercise copying
  from an editor that has a focus handle but no editing callbacks.

- Enable editor soft wrapping, narrow and widen the window, and select across wrap
  boundaries containing CJK text, combining marks, emoji sequences, and tabs.
  Verify visual-row arrows, Home/End, Page Up/Down, and composition candidate bounds.
- Drag vertical and horizontal scrollbar thumbs beyond the editor, release outside,
  and click the tracks. Selection and composition must remain unchanged. Repeat in
  a read-only editor and after shrinking the document until bars disappear.
- Open 100,000 logical lines and a long single line. Measure initial layout,
  resizing, repeated edits near the end, scrolling, undo/redo, and retained memory.
  Separate background layout completion from UI input latency and idle frames.
- Resize repeatedly while a large document is still being measured. Type, compose,
  scroll, replace the document, and close the editor during pending reflow. Verify
  that the latest text/width wins, scroll anchors remain stable, cancelled work
  stops, and no old caret or IME bounds appear over replacement text.

- Disable soft wrapping and scroll to the middle and end of a very long line.
  Check fixed line numbers, strings/comments beginning offscreen, tabs, combining
  marks, emoji, selection across segment boundaries, vertical preferred-column
  movement, IME candidate placement, and undo/redo after editing near the end.

- Paste large CRLF documents and single long Unicode lines into the beginning,
  middle, and end of existing text. Undo/redo repeatedly while layout is pending,
  then type again. Check text, selection, native UTF-16 ranges, and the latest
  layout; include combining marks joining adjacent text and newline split/join.
  Record input-to-frame latency and history-eviction memory costs separately from
  model benchmark times.

- Paste a large mixed Unicode single line, then immediately start and commit an
  IME composition. Check the native selection, candidate placement, and separate
  undo steps. Clear history, branch after undo, lower the history limit, and close
  a large session; verify live shared views remain valid and memory is eventually
  released. Measure disposal submission separately from background completion.

- During a large asynchronous paste, keep typing, use Backspace/Enter, start an
  IME composition, and scroll with the wheel and scrollbars. Check the temporary
  composition indicator, native candidate geometry, final input order, and
  separate undo steps. Repeat paste → undo → redo → typing and copy → cut → paste
  before preparation finishes. Change the clipboard after requesting a paste.
- Replace or disable the editor while a paste is pending, and close it while a
  queued history command is waiting for a frame. No stale edit may publish after
  cancellation. Verify that cancelling/unmarking a staged composition and losing
  focus preserve the documented commit behavior on each native input method.
