# Terminal Conformance Matrix

This matrix maps terminal behavior to automated coverage. Physical shell and
IME validation remains in [Platform Smoke Record](platform-smoke.md).

| Area | Automated status | Coverage |
| --- | --- | --- |
| Printable text and control characters | Covered | UTF-8 text, combining marks, wide cells, malformed/truncated input |
| CSI cursor/edit/erase operations | Covered | cursor movement, line clearing, scroll regions, insert/delete/repeat |
| SGR | Covered | standard, bright, indexed, RGB, colon-delimited colors, text attributes |
| DEC modes | Covered | origin, wrap, cursor visibility/style, alternate screen, saved private modes |
| Alternate screen | Covered | enter/leave, reset, cursor and mode restoration |
| Scrollback and viewport | Covered | bounded history, navigation, visible selection |
| Resize and reflow | Covered | hard breaks and soft-wrapped lines, narrow/wide resize |
| Tab stops and charsets | Covered | configurable stops and DEC special graphics |
| OSC | Covered | title and bounded OSC 8 hyperlinks |
| DCS and unsupported strings | Covered | safely ignored payloads without following-text corruption |
| Query replies | Covered | DA, secondary DA, DSR, cursor report, DEC private status |
| Keyboard encoding | Covered | control, navigation, modified keys, single printable-input path |
| Paste and clipboard model | Covered | bracketed paste and bounded large payloads |
| Mouse protocols | Covered | SGR and legacy event encoding, alternate-scroll helpers |
| Selection | Covered | live grid, scrollback viewport, soft-wrap newline behavior |
| Lifecycle | Covered | graceful close, escalation, force close, restart state |
| PTY integration | Platform-gated | Windows ConPTY automated; Unix shell execution requires release smoke |
| IME candidate UI | Platform-gated | marked/committed model covered; native candidate placement requires release smoke |
| Screen reader behavior | Platform-gated | semantic surface present; native audit requires release smoke |

Run the model suite with `cargo test -p guic-terminal --all-features` and the
repeatable stress benchmark with:

```bash
cargo bench -p guic-terminal --bench terminal_models
```

The benchmark covers dense SGR output, long scrollback navigation, wrapped-line
resize, and 24 concurrently updated pane models. Record hardware and build
revision when comparing results.
