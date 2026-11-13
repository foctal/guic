# DatePicker

`DatePicker` displays a controlled date trigger and an optional calendar grid.
The application owns both the date string and open state.

```rust
use guic::prelude::DatePicker;

DatePicker::new("due-date")
    .value("2026-06-27")
    .open(is_open)
    .on_open_change(|_open, _window, _cx| { /* update open state */ })
    .on_change(|_date, _window, _cx| { /* store the selected date */ })
```

Dates use `YYYY-MM-DD` with years from 1 through 9999. The calendar initially
shows the selected month, or the current local month when the date is absent or
invalid. Previous/next controls browse months without changing the selected date.
An authoritative selected-date change resets the displayed month.
`on_request_open` remains available for custom date-selection interfaces.

Enter/Space activates the trigger or a focused day. Arrow keys move through
days within the displayed month; Home/End moves to its first/last day.
Selecting a date calls `on_change` and requests closure through `on_open_change`.

PageUp/PageDown browses months; Shift+PageUp/PageDown browses years. Escape
requests closure. Labels and weekdays use the application translation catalog.
