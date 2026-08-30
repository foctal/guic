# ConfirmPopup

`ConfirmPopup` presents a contextual confirmation surface directly below a
trigger. The host owns the open state and closes it after either callback.

```rust,ignore
ConfirmPopup::new("archive-confirm", Button::new("Archive"))
    .open(confirming_archive)
    .message("Archive this project?")
    .confirm_label("Archive")
    .on_confirm(cx.listener(|view, _, _, cx| {
        view.confirming_archive = false;
        cx.notify();
    }));
```

Use `danger(true)` for destructive operations. Use `ConfirmDialog` instead when
the user must make a modal decision before continuing.
