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
days across month boundaries. Home and End move to the first and last enabled
day of the displayed month.
Selecting a date calls `on_change` and requests closure through `on_open_change`.

PageUp/PageDown browses months; Shift+PageUp/PageDown browses years. Escape
requests closure. Labels and weekdays use the application translation catalog.

Set `.first_weekday(CalendarWeekday::Sun)` to start the grid on Sunday. Monday
is the default. Use `.date_enabled(|date: CalendarDate| ...)` for constraints
such as a booking window or excluded holidays. Disabled dates remain visible
and cannot be selected. Arrow navigation searches up to 31 days for an enabled
destination. Month and year navigation search within the destination month.

For an inclusive interval, pass `.range(selection)` and
`.on_range_change(handler)`. Store the emitted `DateRangeSelection` and
render it on the next frame. The first click or keyboard activation sets
`Start(date)` and leaves the calendar open. The second emits
`Complete { start, end }` in chronological order and requests closure.
To preselect a range, use `DateRangeSelection::between(a, b)`. Both endpoints
must pass the date predicate. The interval can span disabled days between them.
