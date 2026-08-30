# Select

`Select` is a controlled dropdown component with inline menu rendering.

Highlights:

- Stable `SelectItem` metadata
- Controlled expanded state and selected index
- Token-driven menu, hover, and selected styling
- Accessible trigger, listbox, option, expanded, and selected semantics
- Visible keyboard focus treatment

Keyboard behavior:

- `Enter` or `Space` toggles the menu.
- `Escape` closes an expanded menu.
- `Up` and `Down` select the previous or next enabled option.
- `Home` and `End` select the first or last enabled option.
- Typing a printable prefix selects the next matching enabled option and wraps
  once through the list.
- Disabled controls and options cannot be activated.

The trigger participates in tab navigation automatically. Use
`.focusable(focus_handle)` when the host needs programmatic focus control.
Use `.accessible_label(...)` when the visible surrounding label is not part of
the control. Empty option lists render a configurable `empty_message`.

Story coverage:

- `crates/guic-story/src/main.rs`
