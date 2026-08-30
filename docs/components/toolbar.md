# Toolbar

## Purpose

Group actions and controls in a bordered horizontal row, with separators
between logical groups and a flexible spacer for trailing-edge alignment.

## Import

```rust
use guic::prelude::Toolbar;
```

## Basic Usage

```rust
Toolbar::new()
    .child(Button::new("New"))
    .child(Button::new("Open"))
    .separator()
    .child(Button::new("Save"))
    .spacer()
    .child(Button::new("Settings"))
```
