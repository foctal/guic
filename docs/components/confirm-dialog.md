# ConfirmDialog

## Purpose

A focused confirmation modal for yes/no decisions. Unlike the general `Dialog`,
it always renders confirm and cancel actions and styles the confirm button
destructively when `danger` is set.

## Import

```rust
use guic::prelude::ConfirmDialog;
```

## Basic Usage

```rust
ConfirmDialog::new("delete-confirm")
    .open(is_open)
    .title("Delete project?")
    .message("This action cannot be undone.")
    .confirm_label("Delete")
    .cancel_label("Keep")
    .danger(true)
    .on_confirm(|_event, _window, _cx| { /* perform deletion */ })
    .on_cancel(|_event, _window, _cx| { /* dismiss */ })
```

The dismiss scrim also fires `on_cancel` unless `.dismissible(false)`.
