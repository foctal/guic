# DatePicker

## Purpose

Render a controlled date-selection trigger.

## Import

```rust
use guic::prelude::DatePicker;
```

## Basic Usage

```rust
DatePicker::new("due-date")
    .value("2026-06-27")
    .placeholder("Due date")
    .on_request_open(|current, _, _| {
        let _ = current;
    })
```

## Notes

`DatePicker` is host-managed. It does not own a calendar popover yet; instead it
emits an open request so applications can connect the trigger to their own
calendar or platform date-selection workflow.
