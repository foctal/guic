# TreeSelect

## Purpose

Pick one value from a hierarchical tree while keeping the selected value visible
in a compact select trigger.

## Import

```rust
use guic::prelude::{TreeSelect, TreeSelectNode};
```

## Basic Usage

```rust
TreeSelect::new("project-file")
    .expanded(true)
    .selected("main")
    .nodes(vec![TreeSelectNode::new("src", "src").expanded(true).children(vec![
        TreeSelectNode::new("main", "main.rs"),
        TreeSelectNode::new("lib", "lib.rs"),
    ])])
    .on_toggle(|expanded, _, _| {
        let _ = expanded;
    })
    .on_select(|id, _, _| {
        let _ = id;
    })
```

## Notes

`TreeSelect` is controlled by the host. It renders expanded branches from the
node model and emits selected node ids, but it does not mutate the tree itself.
