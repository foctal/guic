# Metric Card

`MetricCard` presents a compact label, primary value, and optional trend for
dashboards and status pages.

```rust,ignore
MetricCard::new("Requests", "12,480").trend("+8.2%")
```

This is a read-only presentation component and has no keyboard or disabled
state. Surface, border, value, and supporting-text colors use the active
`Theme`.
