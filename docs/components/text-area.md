# TextArea

`TextArea` is the multi-line member of the shared GUIC text-input family.

Highlights:

- Reuses the GPUI-backed text editing core
- Up/Down move between visual lines, including soft wraps, preserving the preferred horizontal position across short lines
- Shift+Up/Down extend selection; horizontal movement and editing reset the preferred horizontal position
- Larger surface for note-taking and longer descriptions
- Suitable as the default long-form text component while richer editor work continues in higher layers

Story coverage:

- `crates/guic-story/src/main.rs`
