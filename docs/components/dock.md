# Dock

Enable the `dock` feature on `guic`.

Render split-panel workspace layouts with stacked tabs and serializable layout
state.

```rust
use guic::prelude::{Dock, DockCommand, DockLayout, DockNode, DockPlacement, DockTab, DockTabs};

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

## Moving tabs

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

## Closing tabs

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

The host owns persistence and applies commands. Keep live terminal, editor, and
chart entities outside the serialized layout, using stable stack and tab IDs.

## Layout and presentation

- New runtime splits default to 50:50. Set
  `DockLayout::initial_split_ratio(ratio)` to configure future splits.
- `Dock::split_limits(DockSplitLimits)` configures pointer and rendered split limits.
  The default minimum is 80 logical pixels per child. Handle width
  is excluded from available space. The renderer recomputes limits from actual
  bounds on every layout, including window resizing. When both minimums cannot fit,
  use 50:50 without changing the persisted preferred ratio.
- Each normalized layout split has a persistent `id`. `DockLayout::new` and
  `from_json` assign missing IDs and resolve duplicates without taking IDs from
  existing splits. ID allocation state is serialized.
- Renderer resize commands include the exact split ID through
  `DockSplitResize::split_id`. The legacy stack-based constructor and resize
  methods remain available; use exact IDs for nested trees.
- `DockDensity::Compact` reduces root/pane padding and tab padding.
  `show_pin_actions(false)` hides tab and stack pin controls.
  `render_tab` replaces label content while retaining selection and drag behavior.
  `show_stack_actions(false)` independently hides built-in stack actions.
  `CloseButtonVisibility` supports Always, Hover, Selected, HoverOrSelected, and
  Never. Hover policies reserve the action space to avoid shifting labels.
  `render_tab_actions` replaces built-in tab actions; `render_stack_header`
  replaces the full header, including its tab strip. Hosts own behavior in custom
  action/header elements.
- `DockLayout::close_policy(DockClosePolicy::ProtectPinned)` prevents pinned tabs
  from being closed directly or through a stack close. The default AllowPinned
  allows pinned tabs to close. All close commands
  applied to the layout obey this policy. Moving a tab is not a close operation.

Serialized layouts include split IDs, an allocation counter, a default split
ratio, and a close policy. `from_json` accepts older JSON with defaults for these
fields. Direct `DockNode::Split` construction must include `id` (an empty ID is assigned during
layout normalization). Pattern matches can use `..` for new fields.

### Transfer ownership

Drag payloads carry `source_dock_id` and `source_window_id`. Direct struct
literals must initialize these optional fields; `None` denotes a local
command. Cross-Dock/window UI drops emit `DockCommand::TransferTab` containing
source ownership, destination ownership, and the drop target.

`DockLayout::apply` returns false for `TransferTab`. The host must
validate source liveness and destination availability, approve the transfer, move
application resources, rebind resource notifications, and then update both
layouts atomically. A rejected transfer leaves the source untouched.

`DockLayout::transfer_tab_to` updates two layouts together, rejecting stale sources/destinations and duplicate tab IDs. It preserves
pinned identity. Apply it to layout clones while preparing resource transfer, then
install both clones only after resource movement succeeds. Roll back application
resources if their transfer fails; layout validation alone cannot roll them back.

## Compact panes

```rust
use gpui::px;
use guic::prelude::{DockDensity, DockFocusIndicator};

Dock::new("workspace", layout)
    .density(DockDensity::Compact)
    .focus_indicator(DockFocusIndicator::ActiveTab)
    .leaf_padding(px(0.))
    .leaf_border(false)
```

The default focus indicator is a pane border. `ActiveTab` uses a primary
underline for the focused stack and a muted underline for other stacks' selected
tabs. Borderless leaves also use tab indication. `leaf_padding` overrides the
density's default padding; drag targets remain visible independently of borders.
These presentation settings do not change serialized layouts.
