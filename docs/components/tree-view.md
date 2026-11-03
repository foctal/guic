# TreeView

Enable the `tree` feature on `guic`.

Render hierarchical data for navigation, inspectors, and project-style sidebars.

```rust
use guic::prelude::{TreeNode, TreeView};

TreeView::new("project-tree").nodes(vec![
    TreeNode::new("src", "src").expanded(true).children(vec![
        TreeNode::new("main", "main.rs").selected(true),
    ]),
])
```

## Navigation

```rust
use guic::prelude::{TreeNavigation, TreeNavigationOutcome};

let tree = TreeView::new("project-tree").nodes(vec![
    TreeNode::new("src", "src").expanded(true).children(vec![
        TreeNode::new("main", "main.rs").selected(true),
    ]),
]);

match tree.navigation_outcome("main", TreeNavigation::Left) {
    TreeNavigationOutcome::Select(parent) => {
        assert_eq!(parent.as_ref(), "src");
    }
    TreeNavigationOutcome::Toggle(_) | TreeNavigationOutcome::Noop => {}
}
```

## Keyboard

Provide a host-owned `FocusHandle` through `focusable` to enable keyboard
interaction. When the tree is focused, `Up`/`Down` and `Home`/`End` move the
selection through `on_select` or `on_node_selection`, while `Left`/`Right`
either move between parent/child nodes or collapse/expand a branch through
`on_toggle`.

```rust
TreeView::new("project-tree")
    .nodes(nodes)
    .focusable(focus_handle) // created once with cx.focus_handle()
    .on_select(|node_id, _window, _cx| {
        let _ = node_id; // update host selection state
    })
    .on_toggle(|node_id, _window, _cx| {
        let _ = node_id; // toggle expansion in host state
    })
```

## Virtualized Trees

Large trees can use externally managed viewport metadata. Navigation and
selection continue to operate over the complete visible depth-first sequence,
while `rendered_nodes` and the widget renderer are limited to the viewport plus
overscan rows.

```rust
use guic::prelude::TreeViewport;

TreeView::new("large-project-tree")
    .nodes(nodes)
    .row_height(36.0)
    .viewport(TreeViewport::new(scroll_offset, body_height).overscan(3))
    .height(240.0)
```

Retain a `gpui::ScrollHandle` and pass it through `track_scroll`. Read
`-handle.offset().y` for the scroll offset and use the available body height,
excluding the title and outer padding, for `TreeViewport`. Refresh the view
when this metadata changes. Rendered rows and virtual spacers use the same pitch.

## Sizing

`row_height` sets the actual row pitch, including spacing: 36 logical pixels by
default, with a minimum of 24. Long labels truncate within the fixed row height.
`height` fixes the outer height; `fill_height()` fills a parent with an allocated
height. `max_height` caps intrinsic height without stretching short trees.
The title stays fixed while the body scrolls.

`body_only(true)` removes outer padding, background, and border for use in an
application-owned panel. An explicitly supplied title is retained.

```rust
TreeView::new("inspector")
    .nodes(nodes)
    .row_height(24.0)
    .height(240.0)
    .body_only(true)
```

## Controlled Mutations

`TreeMutation` provides typed updates for nested expansion, selection,
checkbox, loading, and child-collection state. Updates return a new tree and
report missing identifiers explicitly.

```rust
use guic::prelude::TreeMutation;

let tree = TreeView::new("project-tree")
    .nodes(nodes)
    .apply_mutation(TreeMutation::expanded("src", true))?
    .apply_mutation(TreeMutation::replace_children(
        "remote-packages",
        loaded_children,
    ))?;
```

Replacing children also clears the node's loading and lazy-placeholder state.
For root insertion or removal, update the host's root collection and pass it
through `nodes` on the next render.

## Checkbox and Multi-Select

Use `selection_mode(TreeSelectionMode::Multiple)` with `on_node_selection` when
the host needs range or toggle selection. Normal node clicks replace selection,
Cmd/Ctrl-click toggles a node, and Shift-click or Shift-navigation selects an
inclusive range in visible tree order.

Use `selection_mode(TreeSelectionMode::Checkbox)` to render checkbox
controls on each visible node. Checkbox clicks and Space on the focused node
emit toggle selection updates.

```rust
use guic::prelude::{TreeSelection, TreeSelectionMode};

TreeView::new("project-tree")
    .nodes(nodes)
    .selection_mode(TreeSelectionMode::Checkbox)
    .focusable(focus_handle)
    .on_node_selection(|selection: &TreeSelection, _window, _cx| {
        let selected_ids = selection.selected_ids();
        let focused_id = selection.focused_id();
        let _ = (selected_ids, focused_id);
    })
```

For host-managed selection outside the widget event path, `selected_node_ids`,
`checked_node_ids`, `node_range_ids`, and `selection_change` expose the same
visible-tree-ordered selection math used by the widget.

## Lazy Loading

Use `lazy_children(true)` when a node should behave like a branch before its
children are available. This keeps the disclosure marker, toggle callback, and
keyboard `Right` behavior active even when `children` is still empty.

Use `loading(true)` while the host is fetching children. Loading nodes are also
treated as lazy branches, and an expanded loading node renders an inline
loading status without changing the virtual row count.

```rust
TreeView::new("project-tree")
    .nodes(vec![
        TreeNode::new("remote-workspace", "Remote workspace")
            .lazy_children(true),
        TreeNode::new("packages", "Packages")
            .expanded(true)
            .loading(true),
    ])
    .on_toggle(|node_id, _window, _cx| {
        let _ = node_id; // fetch or reveal children in host state
    })
```
