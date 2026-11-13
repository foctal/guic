# Macros

The `guic` crate exports `shared_string!`, `shared_format!`, and `theme_name!`.
They resolve their types through GUIC, including when the dependency is renamed.

```rust
let title = guic::shared_string!("Settings");
let count = guic::shared_format!("{count} items", count = 3);
let theme = guic::theme_name!("Custom");
```

The standalone `guic-macros` crate provides the same convenience macros for
applications with direct `gpui` and `guic_tokens` dependencies. Its string macros
require the runtime under the dependency name `gpui`; its `theme_name!` macro
requires `guic_tokens`.
