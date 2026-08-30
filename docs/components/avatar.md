# Avatar

## Purpose

Render a compact identity surface from a display name, with deterministic
accent colors and an optional presence indicator.

## Import

```rust
use guic::prelude::{Avatar, AvatarShape, AvatarStatus};
```

## Basic Usage

```rust
Avatar::new("Ada Lovelace").status(AvatarStatus::Online)
```

Initials are derived from the name (up to two letters). Override them with
`.initials("AL")`, choose `.shape(AvatarShape::Rounded)`, and size with
`.size(ComponentSize::Small)`.
