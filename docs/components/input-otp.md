# InputOtp

Render a controlled one-time-code slot input.

```rust
use guic::prelude::InputOtp;

InputOtp::new("login-code")
    .length(6)
    .value("123456")
    .masked(false)
    .focusable(handle)
    .on_change(|value, _, _| {
        let _ = value;
    })
```

The application stores the value received through `on_change` and passes it
back on the next render. With a focus handle, typing appends non-whitespace
characters, Backspace removes the last character, and Delete/Escape clears the
value. Length is limited to 1–12 slots. This control handles key events; it does
not provide the IME and clipboard integration of `TextInput`.
