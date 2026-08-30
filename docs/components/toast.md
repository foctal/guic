# Toast

## Purpose

Transient notifications. `Toast` is a single notification card; `ToastStack`
positions and layers a list of toasts in a window corner above all other
content.

## Import

```rust
use guic::prelude::{Toast, ToastStack, ToastPlacement, ToastVariant};
```

## Basic Usage

```rust
ToastStack::new("app-toasts")
    .placement(ToastPlacement::BottomRight)
    .toasts(vec![
        Toast::new("saved", "Changes saved")
            .variant(ToastVariant::Success)
            .description("Your workspace is up to date.")
            .on_close(|_event, _window, _cx| { /* drop from the list */ }),
    ])
```

Host-managed: the host owns the list of active toasts and their lifecycle
(timeouts, dismissal), removing one in response to each `on_close`.
