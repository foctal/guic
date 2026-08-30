# InputOtp

## Purpose

Render a controlled one-time-code slot input.

## Import

```rust
use guic::prelude::InputOtp;
```

## Basic Usage

```rust
InputOtp::new("login-code")
    .length(6)
    .value("123456")
    .masked(false)
    .on_change(|value, _, _| {
        let _ = value;
    })
```

## Notes

`InputOtp` is host-managed. The application owns the value and updates it after
`on_change` fires. The current surface provides stable slot rendering plus
clear/backspace intents; direct text capture can be layered on top by the host.
