# DataTable

## Purpose

Render structured row and column data with stable sizing, row selection
styling, and explicit loading, empty, and error states.

## Import

```rust
use guic::prelude::{
    DataColumn, DataColumnPin, DataRow, DataTable, DataTableColumnViewport,
    DataTableViewport,
};
```

## Basic Usage

```rust
DataTable::new("builds")
    .columns(vec![
        DataColumn::new("name", "Name"),
        DataColumn::new("status", "Status"),
    ])
    .rows(vec![
        DataRow::new("stable", vec!["stable", "Ready"]).selected(true),
        DataRow::new("nightly", vec!["nightly", "Running"]),
    ])
```

## Virtualized Rows

```rust
DataTable::new("large-builds")
    .columns(vec![
        DataColumn::new("name", "Name"),
        DataColumn::new("status", "Status"),
    ])
    .rows((0..500).map(|index| {
        DataRow::new(
            format!("build-{index}"),
            vec![format!("build-{index}"), "Ready".to_string()],
        )
    }).collect())
    .row_height(36.0)
    .viewport(DataTableViewport::new(720.0, 240.0).overscan(3))
```

## Host-Managed Navigation

```rust
use guic::prelude::{DataTableNavigation, DataTableNavigationOutcome};

let table = DataTable::new("builds").rows(vec![
    DataRow::new("stable", vec!["stable", "Ready"]).selected(true),
    DataRow::new("nightly", vec!["nightly", "Running"]),
]);

match table.navigation_outcome("stable", DataTableNavigation::Down) {
    DataTableNavigationOutcome::Select(next) => {
        assert_eq!(next.as_ref(), "nightly");
    }
    DataTableNavigationOutcome::Noop => {}
}
```

## In-Widget Keyboard Navigation

Provide a host-owned `FocusHandle` through `focusable` to turn the host-managed
navigation helpers into real keyboard interaction. When the table body is
focused, `Up`/`Down`, `Home`/`End`, and `PageUp`/`PageDown` move the selection
through the `on_row_select` callback or the richer `on_row_selection` callback.
With no active selection, the first key press lands on an edge row.

```rust
DataTable::new("builds")
    .columns(vec![DataColumn::new("name", "Name")])
    .rows(rows)
    .focusable(focus_handle) // created once with cx.focus_handle()
    .on_row_select(|row_id, _window, _cx| {
        // update the selected row in host state
        let _ = row_id;
    })
```

Keyboard navigation is wired only when both a focus handle and an
`on_row_select` or `on_row_selection` handler are present.

## Multi-Select and Range Selection

Use `selection_mode(DataTableSelectionMode::Multiple)` with `on_row_selection`
when the host needs multi-select or range-select behavior. Normal row clicks
replace selection, Cmd/Ctrl-click toggles a row, and Shift-click or
Shift-navigation selects an inclusive range from the current anchor row.

```rust
use guic::prelude::{DataTableSelection, DataTableSelectionMode};

DataTable::new("builds")
    .columns(vec![DataColumn::new("name", "Name")])
    .rows(rows)
    .selection_mode(DataTableSelectionMode::Multiple)
    .focusable(focus_handle)
    .on_row_selection(|selection: &DataTableSelection, _window, _cx| {
        let selected_ids = selection.selected_ids();
        let focused_id = selection.focused_id();
        let _ = (selected_ids, focused_id);
    })
```

For host-managed selection outside the widget event path, `selected_row_ids`,
`row_range_ids`, and `selection_change` expose the same table-ordered selection
math used by the widget.

## Host-Managed Column Resizing

```rust
let table = DataTable::new("builds").columns(vec![
    DataColumn::new("name", "Name").width(180.0).min_width(120.0),
    DataColumn::new("status", "Status").width(140.0),
]);

let resized = table
    .resized_column("status", 220.0)
    .map(|resize| table.apply_column_resize(resize));
```

Providing `on_column_resize` also adds an eight-pixel resize handle to every
header. Dragging emits the current pointer-derived width; clicking the handle
emits a 24-pixel keyboard/test-friendly increment. The host stores the emitted
width and supplies it on the next render.

```rust
DataTable::new("builds")
    .columns(columns)
    .rows(rows)
    .on_column_resize(|resize, _window, _cx| {
        let column_id = resize.column_id();
        let width = resize.width();
        // Persist column_id and width in host state.
        let _ = (column_id, width);
    })
```

## Virtualized and Pinned Columns

Column virtualization uses externally managed horizontal viewport metadata,
matching the row virtualization ownership model. Columns pinned to either edge
are always rendered and are ordered outside the virtualized middle region.
Specify widths for virtualized columns so offsets remain stable.

```rust
DataTable::new("wide-builds")
    .columns(vec![
        DataColumn::new("name", "Name")
            .width(180.0)
            .pin(DataColumnPin::Start),
        DataColumn::new("duration", "Duration").width(120.0),
        DataColumn::new("agent", "Agent").width(180.0),
        DataColumn::new("actions", "Actions")
            .width(120.0)
            .pin(DataColumnPin::End),
    ])
    .rows(rows)
    .column_viewport(
        DataTableColumnViewport::new(horizontal_offset, viewport_width)
            .overscan(120.0),
    )
```

## Cell Formatting and Row Actions

`render_cell` receives stable row and column identifiers, source indices, the
cell value, and selection state. `render_row_actions` adds a fixed trailing
region; customize its allocation with `row_actions_width`.

```rust
DataTable::new("builds")
    .columns(columns)
    .rows(rows)
    .render_cell(|cell| {
        Label::new(cell.value().clone()).into_any_element()
    })
    .render_row_actions(|row| {
        Button::new(format!("Open {}", row.id())).into_any_element()
    })
    .row_actions_width(144.0)
```

## Notes

The current implementation emphasizes predictable presentation and integration
with application-managed state. Row virtualization is supported through
externally supplied viewport metadata, which keeps scroll ownership in the host
application while reducing the rendered row count for large datasets. Column
virtualization follows the same contract and always retains pinned edge columns.
`selected_row_id`, `visible_row_models`, and `navigation_outcome` provide a
host-managed foundation for keyboard traversal and selection workflows.
`visible_column_models`, `resized_column`, and `apply_column_resize` provide
the same kind of host-managed foundation for column sizing and resize flows.
Custom renderers are pure callbacks; application state changes remain explicit
through component event handlers owned by the host.
