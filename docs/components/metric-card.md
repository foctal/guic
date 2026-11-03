# Metric Card

`MetricCard` presents a compact label, primary value, and optional detail for
dashboards and status pages.

```rust,ignore
MetricCard::new("Requests", "12,480").detail("+8.2% from last month")
```

The card is read-only.
