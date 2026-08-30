# InputNumber

## Purpose

A numeric stepper with decrement/increment controls. Host-managed: supply the
value and react to `on_change`.

## Import

```rust
use guic::prelude::InputNumber;
```

## Basic Usage

```rust
InputNumber::new("quantity")
    .value(3.0)
    .range(0.0, 99.0)
    .step(1.0)
    .suffix("items")
    .focusable(handle)
    .on_change(|value, _window, _cx| { /* store */ })
```

`on_change` reports the next clamped value from the step buttons or, when
`.focusable(handle)` is set, the `Up`/`Down`/`Home`/`End` keys. The step buttons
disable automatically at the range bounds.

Values are clamped regardless of whether `.value(...)` or `.range(...)` is
called first. Non-finite values, bounds, and steps are ignored.
