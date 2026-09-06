# Component Behavior and Host Integration

This guide describes shared layout, identity, focus, Dock, and terminal contracts
for application authors. See the [platform smoke checklist](platform-smoke.md)
for native integration validation requirements.

## Control geometry

`ComponentSize::control_metrics(&theme)` is the single source of truth for
single-line external control heights: Small 28px, Medium 34px, Large 42px.
`ControlMetrics` also provides text size, horizontal padding, icon size, and gap.
Button, IconButton, TextInput, Select, InputNumber, MultiSelect, Tabs, and TabMenu
use the shared height. Tabs and TabMenu include their outer borders/padding in
that height. TextArea retains its independent multiline sizing model.

MultiSelect has a single-line trigger. Excess selected chips are clipped
instead of increasing its height. Applications needing a wrapping selection
summary should render that summary separately.

## Stable identity

Button, IconButton, and Tooltip accept `.id(...)`. Their compatibility constructors
use stable Rust call-site identity instead of changing counters or visible text.
A call site is not unique across repeated siblings: **always supply IDs in loops,
shared factory functions, and repeated component instances without a parent ID**.
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
fallback. Explicit Button/IconButton IDs also determine their debug selectors;
legacy constructors retain label/icon selectors for compatibility.

## Wrapper geometry and Select

Tooltip, ContextMenu, and Popover share an internal behavioral trigger. It forwards
the child's layout ID rather than inserting a new layout box. Percentage sizing,
flex growth, and intrinsic constraints therefore reach the original parent.
The wrapper does not add a tab stop or override the child's accessibility role.
Overlay layout runs separately in viewport coordinates.

Select uses the same mechanism: only the trigger participates in normal layout.
The options menu flips upward when needed, clamps to the viewport, and scrolls
within its height cap. After mounting or selection changes, it scrolls the selected
option into view. Selects share a window-scoped exclusive group by default;
`.exclusive_group(None)` opts out. A newly opened member requests closure of the
previous member through its controlled callback. Outside mouse-down and selection request closure through
`on_toggle`; the host must apply controlled state updates. Existing trigger
keyboard navigation remains available. When supplied, the trigger focus handle
is restored after mouse selection.

```rust
use gpui::px;
use guic_components::{Select, SelectMenuWidth};

let select = Select::new("time-mode")
    .width(px(160.))
    .menu_width(SelectMenuWidth::MinTriggerWidth)
    .max_menu_height(px(240.));
```

The default trigger fills available width. Menu policies are MatchTrigger,
Content, Fixed(Pixels), and MinTriggerWidth. Non-finite explicit dimensions are
ignored. Content width remains constrained by the viewport.

## Focus lifecycle

`guic_core::OverlayFocus` provides focus after mounting and restoration on a
controlled open-to-closed transition. It forwards child geometry unchanged and
cancels pending focus when the lifecycle generation changes. Keep the wrapper
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
This is a keyboard boundary, not a restriction on explicit application `focus()`
calls. Nested portal and accessibility behavior still require native validation.

Menu supports `.open(bool)`, `.autofocus(bool)`, and `.restore_focus(bool)`.
Keep the closed Menu mounted for restoration. ContextMenu automatically focuses
its supplied menu handle after mounting and restores previous focus on closure.
Root keeps its release subscription alive and does not re-autofocus an already
mounted lower overlay when a higher overlay closes.

## TreeView

`max_height` limits the whole TreeView, including title and decoration.
Small trees remain intrinsic. Larger trees scroll in the remaining body space.
Remove application-side header subtraction only after checking the application's
layout on the target platforms.

## Dock

- New runtime splits default to 50:50. Set
  `DockLayout::initial_split_ratio(ratio)` to configure future splits.
- `Dock::split_limits(DockSplitLimits)` configures pointer and rendered split limits.
  The default minimum is 80px per child with a 1..999 sanity range. Handle width
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
  preserves the previous meaning of pinning as ordering only. All close commands
  applied to the layout obey this policy. Moving a tab is not a close operation.

Serialized layouts gain split IDs, an allocation counter, a default split ratio,
and a close policy. Older JSON remains accepted through `from_json`. Direct Rust
`DockNode::Split` construction must include `id` (an empty ID is assigned during
layout normalization). Pattern matches can use `..` for new fields.

### Transfer ownership

Native drag payloads carry `source_dock_id` and `source_window_id`. Direct struct
literals must initialize these new optional fields; None denotes a legacy local
command. Cross-Dock/window UI drops emit `DockCommand::TransferTab` containing
source ownership, destination ownership, and the drop target.

`DockLayout::apply` deliberately returns false for TransferTab. The host must
validate source liveness and destination availability, approve the transfer, move
application resources, rebind terminal notifications, and then update both
layouts atomically. A rejected transfer leaves the source untouched. This change
provides transfer intent; native cross-window drag transport still needs smoke
validation. `DockLayout::transfer_tab_to` atomically updates two host-resolved
layouts, rejecting stale sources/destinations and duplicate tab IDs. It preserves
pinned identity. Apply it to layout clones while preparing resource transfer, then
install both clones only after resource movement succeeds. Roll back application
resources if their transfer fails; layout validation alone cannot roll them back.

## Terminal

`Terminal::from_shared` accepts either `Arc<TerminalModel>` immutable snapshots
or `SharedTerminalModel` (`Rc<RefCell<TerminalModel>>`) on the UI thread. Both retain
shared storage instead of cloning scrollback per render. Mutable borrows must be
released before rendering or dispatching input. Snapshot creation costs still
belong to the model owner.

`LocalPtySession::spawn(PtySpawnConfig, columns, rows)` passes executable,
arguments, environment overrides, and working directory directly to the process
builder. The configuration survives restart. Output notifications can be rebound;
callbacks execute outside the notifier lock. Unread PTY output is bounded to about
1 MiB through channel backpressure. Reader state distinguishes Running, Closed,
and Failed. `take_reader_error` retains the diagnostic error API.
`try_outcome` distinguishes a real child exit (with a termination-request flag)
from a failed wait. Check reader state independently: an output-read failure does
not prove that the child exited. The termination-request flag records a successful
request, not proof of the cause of exit. Legacy `try_exit_status` still represents wait failure with
code -1 for compatibility; new hosts should use the explicit outcome API.

Terminal rendering includes measured ascent/descent, font weight, minimum pixel
line height and percentage line height, optional ligatures, and a configurable
ANSI palette. Existing tuple palettes and array palettes are both accepted.
Cursor/text blink runs at 500 ms only while blinking content is present; the task
is canceled when static or unmounted. `.blink_visible(...)` selects host-managed
animation, including reduced-motion policies.

The muxt-terminal model and rendering improvements were adapted under Apache-2.0:

- OSC 7 local file URI decoding; OSC 8 hyperlinks; bounded OSC 9 and 777
  notifications; OSC 133 C/D command-output regions and reported exit status.
  OSC 9 progress subcommands are ignored rather than emitted as notifications.
- Conservative URL/path detection, hover feedback, and modifier-click callbacks.
  The host decides whether and how to open the target; GUIC does not launch it.
- Plain/regex/case/whole-word searches, selection and command-output filtering,
  result highlighting and reveal, bounded regex compilation and match counts.
  `Arc<TerminalModel>::search_async` uses the supplied GPUI background executor
  without copying the snapshot. Hosts discard stale results using their query
  generation and model history/reset metadata. Search matches address physical
  rows; multi-row query matching is not provided.
- Grapheme and emoji cluster handling, configurable ambiguous-width policy,
  reflow/selection preservation, bounded paste and protocol-response buffers,
  mouse/focus reporting, and command-output invalidation when reflow changes rows.
