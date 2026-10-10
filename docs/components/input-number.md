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

`on_change` reports the next clamped value from the step buttons or the
`Up`/`Down`/`Home`/`End` keys. The step buttons disable automatically at the
range bounds or when the step would not change the value.

Values are clamped regardless of whether `.value(...)` or `.range(...)` is
called first. Non-finite values, bounds, and steps are ignored.

Interactive inputs participate in keyboard traversal by default. A host-owned
focus handle is optional and enables programmatic focus. Arrow keys and Home/End
consume the handled event; unchanged values do not emit change callbacks.
