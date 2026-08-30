# Terminal

For process ownership, resize, lifecycle, resource-limit, and failure-handling
requirements, see the [terminal host contract](../terminal-host-contract.md).

## Purpose

Render a controlled native terminal emulator model with ANSI styling,
scrollback, cursor state, and a transport boundary for PTY-backed hosts.

## Import

```rust
use guic::prelude::{
    discover_shell_profiles, LocalPtySession, Terminal, TerminalGridSize,
    TerminalInputState, TerminalModel, TerminalOptions, TerminalPosition,
    TerminalSelection, TerminalTransport,
};
```

## Basic Usage

```rust
let mut model = TerminalModel::new(80, 24);
model.write("\u{1b}[32mready\u{1b}[0m\n");
model.set_selection(TerminalSelection::new(
    TerminalPosition { row: 0, column: 0 },
    TerminalPosition { row: 0, column: 4 },
));

Terminal::new("terminal", model).options(TerminalOptions::default().measured_font())
```

## PTY Integration

`LocalPtySession` provides a cross-platform `portable-pty` backed transport for
native demos and applications. `TerminalTransport` is the small boundary used by
hosts that need to substitute their own PTY, remote shell, or multiplexed pane
transport.
Use `LocalPtySession::spawn_shell_with_notifier` or
`LocalPtySession::spawn_command_with_notifier` when the host needs to wake its
UI after PTY output or process exit.
Use `LocalPtySession::spawn_shell_in_dir`,
`LocalPtySession::spawn_profile_in_dir`, or
`LocalPtySession::spawn_command_in_dir` when restoring a terminal workspace or
opening a pane in a known project directory. `LocalPtySession::restart`
preserves that initial working directory.

On Windows, `LocalPtySession` uses ConPTY through `portable-pty`. ConPTY may send
a cursor-position query during shell startup and wait for the reply, so hosts
must continuously feed PTY output into `TerminalModel::write` and immediately
write `TerminalModel::take_response_bytes()` back to the session. The Windows
integration suite covers cmd, available PowerShell profiles, cwd inheritance,
resize propagation, restart, graceful exit, and force close. Dropping a
`LocalPtySession` also terminates a still-running child.
Use `discover_shell_profiles` to build host UI for the current platform's
common shells. Each profile reports an id, label, command, availability, and an
unavailable-shell diagnostic. `LocalPtySession::spawn_profile` launches one of
those profiles after checking that it is available.
`LocalPtySession::process_status`, `is_running`, `try_exit_status`, and
`try_exit_code` let hosts supervise pane lifecycle without losing the cached
exit status after the first read.
`LocalPtySession::request_graceful_close`, `force_close`, `close`, and
`restart` provide reusable lifecycle controls for terminal workspaces. Graceful
close sends shell-appropriate exit input, while force close terminates the child
process through the PTY backend.
`TerminalTabStatus` is a host-managed helper for workspace tabs that need to
track process lifecycle together with unread output (`dirty`) and recent input
activity (`busy`).
The `guic-example-terminal-workspace` example demonstrates Dock-backed
workspace persistence by saving JSON for tabs, splits, active pane, shell
profile ids, working directories, terminal grid sizes, and tab lifecycle hints.
Restoring the snapshot recreates PTY sessions rather than serializing live
processes or scrollback contents. Before any live session is replaced, restore
validates snapshot version, unique stack/tab/pane identities, exact layout-pane
membership, active selection consistency, and positive grid geometry. Saves use
a synced temporary file, recoverable backup rotation, and rename replacement;
reads fall back to the backup when an interrupted write leaves the primary
missing or invalid.
After feeding PTY output into `TerminalModel::write`, hosts should call
`TerminalModel::take_response_bytes` and write any returned bytes back to the
transport. This carries terminal-generated responses for DA, DECID, DSR, CPR,
and related VT queries back to shell applications.

`TerminalModel::paste_bytes` and `TerminalModel::clipboard_paste_bytes` encode
pasted text with bracketed paste markers when the running application enables
that mode. `TerminalModel::copy_selection_to_clipboard` copies the current live
grid selection through GPUI's platform clipboard.

Mouse reporting is exposed through `terminal_mouse_event_bytes`, which supports
legacy, SGR, and URXVT mouse encodings based on the current `TerminalModes`.
`terminal_focus_event_bytes` and `terminal_alternate_scroll_bytes` expose focus
reporting and alternate-screen wheel behavior for hosts that wire those events
outside the stock `Terminal` component.
`TerminalGridMetrics` maps GPUI window coordinates into terminal cells; the
`Terminal` component uses the same rendered cell metrics for pointer selection
and mouse-reporting events. Use `Terminal::on_selection` when the host owns
selection state, and `Terminal::on_viewport_scroll` when the host owns
scrollback viewport state. Selection and copy helpers can read from the current
viewport or from rendered rows that include visible scrollback.
`TerminalGridMetrics::bounds_for_position` returns the rendered cell bounds for
cursor, IME candidate, and overlay positioning.

Use `Terminal::on_resize` when the terminal is backed by a PTY. The callback
receives a `TerminalGridSize` derived from the rendered pane bounds and active
cell metrics; hosts should apply it to both `TerminalModel::resize` and
`TerminalTransport::resize` when the size changes. `TerminalModel::resize`
tracks soft-wrapped rows and reflows those rows when the column count changes,
while preserving hard line breaks.

The parser handles core VT behavior needed by terminal applications, including
scroll regions, origin mode, auto-wrap mode, application cursor mode,
application keypad mode, alternate screen, cursor style, true color, indexed
color with semicolon or colon syntax, bold/faint/italic/underline/blink/inverse/
hidden/strikethrough rendition state, background-color erase, Unicode combining
marks, OSC title, OSC 8 hyperlinks, DEC special graphics, configurable tab
stops, RIS and DECSTR reset, scroll up/down, repeat preceding character, cursor
next/previous line, horizontal/vertical relative positioning, forward/back tab
navigation, and common DEC private modes. Saved cursors restore position,
rendition, character set, origin mode, and auto-wrap state. The model tracks
cursor keys, keypad mode, 132-column mode, reverse video, origin mode, auto-wrap,
reverse wraparound, auto-repeat, cursor visibility/blink, left/right margin
mode, alternate screen, alternate-scroll mode, focus reporting, bracketed paste,
synchronized output,
X10/button/drag/all-motion mouse reporting, UTF-8 mouse, SGR mouse, URXVT mouse,
and meta/alt escape policy. It also supports private-mode save/restore through
`CSI ? Ps s` and `CSI ? Ps r`, and queues host-bound response bytes for device
attributes, Secondary DA, DEC private-mode state, status reports, and
cursor-position reports.
Cursor style requests are rendered as block, underline, or vertical-bar cursor
shapes.

The stock `Terminal` renderer uses one native GPUI canvas for the terminal
surface. It merges adjacent background cells into quads and shapes compatible
text into row batches before submitting them to GPUI's GPU renderer. This keeps
the retained element tree independent of the terminal's cell count while
preserving per-cell colors, rendition attributes, selection, links, and cursor
styles.

OSC 8 hyperlink URIs are stored per `TerminalCell` and rendered with hyperlink
styling when the host uses the stock `Terminal` component. Applications that
need browser integration can inspect cell hyperlink metadata from
`TerminalModel::lines`.
Terminal-controlled titles and hyperlink URIs preserve semicolons while being
bounded to safe storage limits. Combining sequences are also bounded per cell
so hostile PTY output cannot grow a single grid cell without limit.

`TerminalOptions::measured_font` uses GPUI's text system to derive the terminal
cell width from the configured monospace font. Set `font_family` and
`font_size` to align the rendered grid with the application's chosen terminal
face.

For IME-aware hosts, use `terminal_key_down_event_bytes` for GPUI key-down
events and `terminal_text_input_bytes` for committed text from a platform input
handler. Attach a `TerminalInputState` through `Terminal::input_state` to enable
native GPUI text input handling, keep uncommitted marked text out of the PTY,
render the active composition at the cursor, and provide candidate-window
bounds from the rendered terminal cell metrics. When `TerminalInputState` is
attached, the stock component sends printable characters only from the native
text-input commit path; key-down handling remains responsible for control,
navigation, function, and modified terminal keys.

## Notes

`guic-terminal` is a dedicated subsystem crate. Enable it through the
`terminal` feature on `guic`, or depend on `guic-terminal` directly when the
application owns terminal transport setup.
See [Terminal Conformance Matrix](../terminal-conformance.md) for the automated
behavior inventory and benchmark command.
