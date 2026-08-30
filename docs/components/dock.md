# Dock

## Purpose

Render split-panel workspace layouts with stacked tabs and serializable layout
state.

## Import

```rust
use guic::prelude::{Dock, DockCommand, DockLayout, DockNode, DockPlacement, DockTab, DockTabs};
```

## Basic Usage

```rust
let layout = DockLayout::new(DockNode::horizontal(
    DockNode::Tabs(DockTabs::new(
        "sidebar",
        vec![DockTab::new("files", "Files", "Project files")],
    )),
    DockNode::Tabs(DockTabs::new(
        "editor",
        vec![DockTab::new("main", "main.rs", "fn main() {}")],
    )),
    280,
));

Dock::new("workspace-dock", layout)
```

## Live Tab Bodies

Use `Dock::render_tab_body` when tabs need to host live surfaces such as
terminals, editors, charts, or inspectors instead of static text.

```rust
Dock::new("workspace-dock", layout).render_tab_body(|selection, tab| {
    // Return the active pane element for selection.tab_id().
    let _ = (selection, tab);
    gpui::div().into_any_element()
})
```

## Persistence

```rust
let json = layout.to_json()?;
let restored = DockLayout::from_json(&json)?;
```

## Host-Managed Movement

```rust
layout.insert_tab("editor", DockTab::new("terminal", "Terminal", "zsh"));
layout.split_stack_with_tab(
    "editor",
    DockPlacement::Right,
    "terminal-right",
    DockTab::new("terminal-2", "Terminal 2", "zsh"),
);
layout.move_tab_to_stack("sidebar", "files", "editor");
layout.move_tab_within_stack("editor", "terminal", 0);
layout.pin_tab("editor", "terminal", true);
layout.select_adjacent_tab("editor", 1);
layout.split_stack_with_moved_tab(
    "editor",
    "main",
    "sidebar",
    DockPlacement::Right,
    "preview",
);
```

`DockLayout::stack_ids` returns stack ids in layout order for command palettes,
focus traversal, and persistence tooling.

## Host-Managed Close Operations

```rust
layout.close_tab("editor", "preview");
layout.close_stack("sidebar");
```

## Interaction Commands

`Dock::on_command` enables selection, tab and stack close controls, split resize
handles, and native pointer drag-and-drop. All interactions use the same
`DockCommand` stream, so a controlled host only needs one state update path.
`DockLayout::apply` normalizes empty stacks and split branches after every
successful structural command.

```rust,ignore
Dock::new("workspace-dock", self.layout.clone()).on_command(
    cx.listener(|this, command: &DockCommand, _window, cx| {
        if this.layout.apply(command) {
            cx.notify();
        }
    }),
)
```

Dragging a tab exposes center, left, right, top, and bottom drop zones on every
stack. Center drops move into the target stack. Edge drops create a split and
use a collision-safe stack identifier derived from `DockDropTarget`.

## Keyboard Routing

Pass a host-owned `FocusHandle` to `Dock::focusable`. `DockLayout` persists the
focused stack and updates it after pointer and structural commands. Use
`Dock::keyboard_stack` only when the host needs to override that target.

- `Control`/`Command` + `Tab`: select the next tab.
- `Control`/`Command` + `Shift` + `Tab`: select the previous tab.
- `Control`/`Command` + `Shift` + arrow: move the active tab within its stack.
- `Control`/`Command` + `Alt` + arrow: focus the adjacent stack.
- `Control`/`Command` + `Alt` + `Shift` + arrow: move the active tab to the
  adjacent stack.
- `Control`/`Command` + `Shift` + `Enter`: split the active tab to the right.
- `Control`/`Command` + `Alt` + `Shift` + `Enter`: split the active tab below.
- `Control`/`Command` + `W`: close the active tab.
- `Control`/`Command` + `Shift` + `W`: close the focused stack.

The keyboard path emits the same `DockCommand` values as pointer interaction.

## Tab Overflow

Tab strips scroll horizontally and keep the active tab visible. For controlled
surfaces that re-render frequently, retain a `gpui::ScrollHandle` per stack and
pass it through `Dock::track_tab_scroll` so the user's scroll position survives
renders. Stack-level controls remain fixed outside the scrolling strip.

## Pinning

Every tab exposes a pin toggle. Pinned tabs remain before unpinned tabs while
preserving active-tab identity. Every stack also exposes a panel pin toggle.
A pinned stack rejects close commands and remains in the split tree when its
last tab is closed or moved away. Unpinning an empty stack immediately runs
normal layout collapse.

Pinning is serializable and can also be controlled directly:

```rust
layout.pin_tab("editor", "main", true);
layout.pin_stack("editor", true);
```

## Notes

The dock is a controlled component: the host owns layout persistence and applies
commands. This keeps live terminal, editor, and chart entities outside the
serializable layout model while preserving stable stack and tab identifiers.
The serializable layout includes focused-stack state.
