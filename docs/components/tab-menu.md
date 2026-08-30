# TabMenu

`TabMenu` is a compact, pill-style navigation control. It is controlled: the
application supplies the selected index and updates it from `on_select`.

```rust,ignore
TabMenu::new("section-menu")
    .items(vec![
        TabItem::new("overview", "Overview"),
        TabItem::new("activity", "Activity"),
    ])
    .selected(active_section)
    .on_select(cx.listener(|view, index, _, cx| {
        view.active_section = *index;
        cx.notify();
    }));
```

Disabled `TabItem`s remain visible but do not emit a selection request. Use
`Tabs` for the conventional underline treatment of document tabs.
