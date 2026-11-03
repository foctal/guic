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

Dates use `YYYY-MM-DD`. The calendar shows the selected month; an absent or
invalid date falls back to June 2026. It has no month-switching controls.
Set the date from application state to show another month, or use
`on_request_open` to open a custom date-selection interface.

Enter/Space activates the trigger or a focused day. Arrow keys move through
days within the displayed month; Home/End moves to its first/last day.
Selecting a date calls `on_change` and requests closure through `on_open_change`.
