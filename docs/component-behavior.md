# Component Behavior and Host Integration

This guide describes shared layout, identity, and focus behavior
for application authors. [Dock](components/dock.md) documents workspace layout
and tab-transfer behavior.

## Control geometry

`ComponentSize::control_metrics(&theme)` is the shared source for
single-line external control heights: Small 28px, Medium 34px, Large 42px.
`ControlMetrics` also provides text size, horizontal padding, icon size, and gap.
Button, IconButton, TextInput, Select, InputNumber, MultiSelect, Tabs, and TabMenu
use the shared height. Tabs and TabMenu include their outer borders/padding in
that height. TextArea retains its independent multiline sizing model.

MultiSelect has a single-line trigger. Excess selected chips are clipped
instead of increasing its height. Applications needing a wrapping selection
summary should render that summary separately.

## Stable identity

Button, IconButton, and Tooltip accept `.id(...)`. Their default constructors
use stable Rust call-site identity.
A call site is not unique across repeated siblings: supply IDs in loops,
shared factory functions, and repeated component instances without a parent ID.
Changing a label does not change an explicitly assigned logical ID.

```rust
use guic_components::{Button, ComponentSize};

let button = Button::new("+")
    .id("toolbar-new-terminal")
    .accessible_label("New terminal")
    .size(ComponentSize::Small)
    .tooltip("New terminal");
```

IconButton retains `.label(...)` for its accessible name and also supports
`.id(...)` and `.tooltip(...)`. Prefer a meaningful label over the icon-name
fallback.

## Wrapper geometry and Select

Tooltip, ContextMenu, and Popover preserve the child's layout, including
percentage sizing, flex growth, and intrinsic constraints.
The wrapper does not add a tab stop or override the child's accessibility role.
Overlay layout runs separately in viewport coordinates.

Select also keeps its menu outside normal layout. See [Select](components/select.md)
for menu sizing, placement, and controlled open state.

## Focus lifecycle

`guic_core::OverlayFocus` provides focus after mounting and restoration on a
controlled open-to-closed transition. Keep the wrapper
rendered with `open(false)` on close; simply removing it cannot run restoration.

Dialog and Popover expose `.autofocus_target(handle)` and `.restore_focus(bool)`.
Keep the controlled component in the render tree when closed. CommandPalette
exposes `.autofocus(true)`, `.restore_focus(bool)`, and
`set_open(open, cx)`; retain its entity when closing through that method.
Alternatively, use the palette as Dialog content and set the dialog's autofocus
target to the palette's search handle.

Managed overlays can use `OverlayOptions::autofocus(handle)`.
`OverlayManager::open_rendered_in_window` records the window and captures previous
focus; invoke it through an application global update to access manager and App
together. Root renders only entries belonging to its window (plus unscoped
entries) and applies autofocus after rendering. Existing focus-trap targets retain
their autofocus behavior. Use the manager's close-and-restore methods on closure.
A non-dismissible top overlay prevents generic dismissal of lower overlays.

`FocusTrap` cycles Tab and Shift-Tab within a full-size modal surface, including
wrapping at both ends and a fallback when there are no tab stops. Dialog and
managed overlays with `traps_focus` use it. Child handles must be tab stops;
TextInput and the core controls register interactive handles accordingly.
Explicit application `focus()` calls can move focus outside the trap.

Menu supports `.open(bool)`, `.autofocus(bool)`, and `.restore_focus(bool)`.
Keep the closed Menu mounted for restoration. ContextMenu automatically focuses
its supplied menu handle after mounting and restores previous focus on closure.
Closing a higher overlay does not re-autofocus an already mounted lower overlay.
