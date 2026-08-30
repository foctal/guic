# Fieldset

`Fieldset` groups related form controls under a visible legend and optional
supporting description. Use it to give a form a meaningful visual and semantic
structure instead of relying on spacing alone.

```rust,ignore
Fieldset::new("Connection")
    .description("Settings used by the remote service")
    .child(TextInput::new(input_state))
```

The fieldset itself is not interactive. Keyboard and disabled behavior belong
to its child controls. Its border, typography, spacing, and colors come from
the active `Theme`.
