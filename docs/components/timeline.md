# Timeline

`Timeline` renders a vertical, read-only sequence of application events. Supply
events in the order in which they should appear.

```rust,ignore
Timeline::new().events(vec![
    TimelineEvent::new("Deployment completed")
        .description("Production is serving the new release.")
        .timestamp("10:32"),
    TimelineEvent::new("Release approved").timestamp("09:45"),
]);
```

It is intended for activity feeds, progress summaries, and audit history. For
selectable or editable data, use a collection component instead.
