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

Opening focuses the cancel action when it has a handler, or the dialog card
otherwise. Tab and Shift-Tab cycle within the modal. Keep the component rendered
with `.open(false)` after closing so focus returns to the previously focused control.

The modal is centered in the window independently of its parent's layout. Its
420px card is constrained to the viewport with a 16px margin on each side, and
overflowing content can scroll vertically.
