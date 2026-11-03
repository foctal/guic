# Dialog

`Dialog` displays controlled modal content with optional title, description,
and confirm/cancel actions.

```rust
use guic::prelude::{Dialog, Label};

Dialog::new("save-confirm")
    .open(is_open)
    .title("Save changes?")
    .content(Label::new("Save before closing this document."))
    .primary_label("Save")
    .secondary_label("Cancel")
    .on_confirm(|_event, _window, _cx| { /* save and close */ })
    .on_cancel(|_event, _window, _cx| { /* close */ })
```

The card is centered in the window, including when mounted inside a small
parent. Its default width is 480 logical pixels, constrained by a 16-pixel
window margin. Overflowing content scrolls vertically so actions remain
reachable. Card and scrim clicks do not reach controls behind the modal.
A scrim click invokes `on_cancel` unless `dismissible(false)` is set.

Tab and Shift-Tab stay within the modal. Use `autofocus_target(handle)` to choose
the initial focus. Keep the component rendered with `open(false)` after closing
so it can restore focus. See [focus lifecycle](../component-behavior.md#focus-lifecycle).
