# InputNumber

An editable numeric field with decrement/increment controls. Host-managed: supply the
value and react to `on_change`.

```rust
use guic::prelude::InputNumber;

InputNumber::new("quantity")
    .value(3.0)
    .range(0.0, 99.0)
    .step(1.0)
    .suffix("items")
    .focusable(handle)
    .on_change(|value, _window, _cx| { /* store */ })
```

`on_change` reports the next clamped value from direct text entry, the step buttons, or the
`Up`/`Down`/`Home`/`End` keys. The step buttons disable automatically at the
range bounds or when the step would not change the value.

Each builder call clamps the value to the range configured at that point.
Call `.range(...)` before `.value(...)` for values outside the default 0–100 range. Non-finite values, bounds, and steps are ignored.

Interactive inputs participate in keyboard traversal by default. A host-owned
focus handle is optional and enables programmatic focus. Arrow keys and Home/End
consume the handled event; unchanged values do not emit change callbacks.

Incomplete text such as a sign or exponent remains editable and does not emit
a number until it parses to a finite value. Host changes synchronize the editor
without discarding equivalent numeric text. Accessible Increment, Decrement,
and SetValue actions use the same range and callbacks.
