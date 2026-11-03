# Fieldset

`Fieldset` groups related form controls under a visible legend and optional
supporting description.

```rust,ignore
Fieldset::new()
    .legend("Connection")
    .description("Settings used by the remote service")
    .child(input.clone())
```

`input` is a retained `Entity<TextInput>`. Keyboard interaction and disabled
state belong to the child controls.
