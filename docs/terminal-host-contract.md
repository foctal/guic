# Terminal Host Contract

`guic-terminal` separates terminal emulation from process ownership. A host may
use `TerminalModel` with any transport, or use `LocalPtySession` for a local
shell. Production hosts are responsible for the policies described below.

## Ownership and data flow

- Keep one `TerminalModel` and one transport per live pane. Models are not
  shared between panes or threads.
- Drain transport output whenever the notifier fires, feed it to
  `TerminalModel::write`, then immediately write
  `TerminalModel::take_response_bytes()` back to the transport. Query response
  bytes must not be displayed as user input.
- Route committed text, key bytes, paste bytes, focus reports, and mouse reports
  to the transport in event order. Marked IME text is presentation state and
  must not be written until committed.
- Treat transport read failure and child exit as terminal lifecycle events.
  Preserve the final model contents until the user closes or restarts the pane.

## Resize policy

- Derive rows and columns from rendered content bounds and measured cell
  metrics. Ignore duplicate sizes to avoid unnecessary PTY system calls.
- Resize both the model and transport from the same normalized grid size.
  `TerminalModel` accepts a minimum of one cell; `LocalPtySession` clamps each
  PTY dimension to the portable `u16` range.
- Debouncing resize events is allowed, but the final size after a layout change
  must always reach both the model and transport.

## Lifecycle policy

- A normal user close should call `request_graceful_close` first. Hosts should
  keep the pane alive for their chosen grace period and call `force_close` if
  the child does not exit.
- `TerminalLifecycleSupervisor` implements this escalation as a deterministic,
  timer-independent state machine. Feed it elapsed time and the current process
  status, then pass its action to `LocalPtySession::apply_lifecycle_action`.
- Application shutdown must force-close remaining children after the grace
  period. Dropping a view alone is not a process supervision policy.
- `restart` preserves the command, initial working directory, notifier, and
  current PTY size. It intentionally starts a new process and does not restore
  shell memory or process state.
- Persist command/profile identifiers and initial working directories, never
  PTY handles. Workspace restoration recreates sessions.

## Scrollback and resource limits

- Set `TerminalModel::max_scrollback` explicitly for the product's memory
  budget. The default is 10,000 rows and the model evicts the oldest rows.
- Keep host-side output queues bounded by draining on notifier wakeups. Do not
  retain duplicate copies of long scrollback in view state.
- Limit paste payloads at the application boundary when accepting untrusted or
  remote clipboard content. Bracketed paste changes framing, not trust.

## Failure UX

- Surface unavailable shell profiles using
  `TerminalShellProfile::unavailable_message` before spawning.
- Show spawn, read, write, and resize failures in the pane without destroying
  the last rendered screen.
- Distinguish a successful exit from a non-zero exit and from forced shutdown.
  Provide restart and close actions for every terminal failure state.

## Platform verification

The automated model suite covers parser behavior, bounded scrollback, reflow,
selection, large output, many isolated pane models, and malformed sequences.
Release validation must still exercise real shells, IME candidate placement,
clipboard integration, mouse modes, and process shutdown on macOS, Windows, and
Linux. Windows validation must include ConPTY through `portable-pty`.
