# Components

GUIC includes native controls for common desktop application layouts, input,
navigation, data display, and feedback. The component gallery exercises the
complete preview catalog.

Optional integrations such as WebView support live outside the core component
crate so that native-first applications do not pay for them by default.

Core components:

- `Label`
- `Badge`
- `Button`
- `IconButton`
- `Alert`
- `Progress`
- `Checkbox`
- `Radio`
- `Switch`
- `Separator`
- `Spinner`
- `Tabs`
- `ScrollArea`
- `VirtualList`
- `TextInput`
- `SearchInput`
- `PasswordInput`
- `TextArea`
- `Select`
- `AutoComplete`
- `Dialog`
- `Popover`
- `Tooltip`
- `Form` / `FormField` / `FormSummary`
- `CommandPalette`

Always-available presentation widgets:

- `PropertyList`
- `MetricCard`
- `Avatar` — identity initials with optional presence status
- `Tag` — removable, tinted categorization label
- `Card` — titled surface with header/body/footer slots
- `Panel` — titled, optionally collapsible content region
- `Accordion` — stacked collapsible sections (host-managed expansion)
- `Breadcrumb` — navigation trail with host-managed selection
- `Stepper` — multi-step workflow progress indicator
- `Toolbar` — horizontal action grouping with separators and spacers
- `Menu` — reusable menu surface (items, separators, headers, shortcuts)
- `Menubar` — horizontal application menu bar (host-managed open state)
- `TieredMenu` — nested command menu for hierarchical action groups
- `ContextMenu` — right-click menu with host-managed position and open state
- `PanelMenu` — persistent, controlled vertical navigation with grouping
- `Drawer` — edge-anchored slide-in panel with dismiss scrim
- `ConfirmDialog` — focused confirm/cancel modal with optional danger styling
- `ConfirmPopup` — contextual, anchored confirm/cancel surface
- `Message` — compact inline severity note for forms
- `Toast` / `ToastStack` — transient corner notifications (host-managed list)
- `Image` — framed native image surface with loading and fallback states
- `Slider` — draggable, keyboard-operable value slider (stateful entity)
- `DatePicker` — controlled date-selection trigger
- `InputNumber` — numeric stepper with keyboard support (host-managed)
- `InputOtp` — controlled one-time-code slot display
- `ColorPicker` — controlled swatch picker
- `CascadeSelect` — controlled multi-column hierarchical select
- `TreeSelect` — controlled tree-backed select trigger
- `MultiSelect` — multi-selection dropdown with chips (host-managed)
- `Paginator` — page navigation with ellipsis truncation (host-managed)
- `DataView` — controlled list/grid collection presentation
- `TreeTable` — controlled hierarchical table with expandable rows
- `FilePicker` — controlled file-import workflow surface
- `Chip` — selectable, optionally removable compact value for filters and choices
- `Listbox` — controlled single- or multiple-selection list surface
- `PickList` — controlled two-pane subset picker
- `TabMenu` — compact pill-style, host-managed navigation menu
- `Timeline` — read-only vertical activity and event history

Optional subsystem widgets (gated behind `guic-components` feature flags):

- `Dock` — `dock`
- `DataTable` — `data-table`
- `TreeView` — `tree`
- `Markdown` / `HtmlFragment` — `markdown`

Optional subsystem crates:

- `LineChart` / `BarChart` / `AreaChart` / `PieChart` — `guic-charts` via the
  `charts` feature on `guic`
- `CodeEditor` — `guic-editor` via the `editor` feature on `guic`
- `Terminal` — `guic-terminal` via the `terminal` feature on `guic`

All reusable components live in the single `guic-components` crate. The optional
subsystems above are compiled only when their feature flag is enabled, either
directly on `guic-components` or via the matching re-export feature on the
`guic` umbrella crate (`data-table`, `tree`, `dock`, `markdown`). Dedicated
subsystem crates such as `guic-charts` stay outside `guic-components` and are
exposed by their own umbrella features when they ship real implementations.
There is no separate "advanced" component crate.

See the per-component pages in `docs/components/` for basic usage and API
guidance. For a runnable end-to-end showcase, launch
`samples/component-gallery`.

Release evidence and known interaction gaps are tracked in
[`component-readiness.md`](component-readiness.md). An implemented module is not
automatically a stable or physically validated component.
