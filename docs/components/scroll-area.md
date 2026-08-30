# ScrollArea

## Purpose

Wrap overflow content in a reusable token-friendly scroll container.

## Import

```rust
use guic::prelude::ScrollArea;
```

## Basic Usage

```rust
ScrollArea::new("logs", content).vertical(true)
```

## Notes

`ScrollArea` is a thin wrapper over GPUI scroll behavior. It is useful for
keeping higher-level GUIC examples and components consistent without exposing
raw overflow setup at every call site.
