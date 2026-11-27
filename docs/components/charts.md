# Charts

Render native data visualizations from reusable chart datasets and options.

```rust
use guic::prelude::{
    BarChart, ChartAxis, ChartDataset, ChartOptions, ChartPoint, ChartScale,
    ChartHit, ChartTooltipMode, ChartValueFormatter, LineChart,
};

LineChart::new("revenue")
    .options(
        ChartOptions::default()
            .title("Revenue")
            .value_formatter(ChartValueFormatter::Prefix("$".into()))
            .crosshair_index(Some(1)),
    )
    .datasets(vec![ChartDataset::new("actual", "Actual").points(vec![
        ChartPoint::category("Jan", 12.0),
        ChartPoint::category("Feb", 18.0),
        ChartPoint::category("Mar", 14.0),
    ])])
```

## Options

`ChartOptions` supports:

- title, height, axes, grid, legend, tooltip, and value-summary visibility
- pointer-following tooltips with strict X/Y geometry intersection by default,
  opt-in continuous nearest selection through `tooltip_intersect(false)`,
  shared-index or dataset grouping through `tooltip_mode`, and bounded grouped
  rows through `tooltip_max_rows`
- explicit value-axis ranges through `value_axis`
- linear and base-10 logarithmic value scales through `scale`
- value formatting through `ChartValueFormatter`
- stacked bar and area rendering through `stacked`
- host-managed crosshair emphasis through `crosshair_index`
- host-managed active tooltip rows through `active_hit`
- point-index domains for zoomed or panned views through `domain`
- doughnut cutout sizing through `doughnut_cutout`
- empty-state text through `empty_message`
- point-index viewport helpers through `ChartAxis::pan` and
  `ChartAxis::zoom_around`

```rust
BarChart::new("throughput")
    .options(
        ChartOptions::default()
            .title("Throughput")
            .stacked(true)
            .scale(ChartScale::Linear)
            .tooltip_mode(ChartTooltipMode::Index)
            .value_formatter(ChartValueFormatter::Suffix("jobs".into()))
            .crosshair_index(Some(2)),
    )
    .datasets(vec![queued, completed])
```

## Chart Types

The chart crate provides:

- `LineChart`
- `AreaChart`
- `BarChart`
- `HorizontalBarChart`
- `ScatterChart`
- `BubbleChart`
- `PieChart`
- `DoughnutChart`
- `RadarChart` with multiple profiles, radial axes and per-vertex selection
- `CandlestickChart` with grouped price intervals, wick and body hit testing
- `SankeyChart` with weighted ribbons, node and link selection
- `MixedChart` with per-dataset line, area, bar, or scatter rendering

## Interaction

Charts follow GUIC's host-managed interaction style. Use `on_hover` to receive
the nearest `ChartHit`, store that in application state, then pass it back
through `ChartOptions::active_hit` and `ChartOptions::crosshair_index`.
The built-in tooltip follows the pointer even within the same datum. It opens
12 logical pixels to the right and below the pointer when space permits. Near
a plot edge it flips sides. Small or partially clipped plots can use the rest
of the window. In short windows, a `+N` in the heading counts the configured
rows that do not fit. Leaving the plot dismisses the tooltip. By default, the
tooltip appears only over a datum. Set `tooltip_intersect(false)` for continuous
nearest-datum selection.

```rust
LineChart::new("latency")
    .options(
        ChartOptions::default()
            .title("Latency")
            .domain(ChartAxis::new(10.0, 30.0))
            .active_hit(active_hit.clone())
            .crosshair_index(active_hit.as_ref().map(|hit| hit.point_index)),
    )
    .datasets(vec![samples])
    .on_hover(cx.listener(|this, hit: &Option<ChartHit>, _window, cx| {
        this.active_hit = hit.clone();
        cx.notify();
    }))
```

Use `ChartSeries::nearest_point_index`, `nearest_point_index_y`, and
`hit_test` when building custom pointer, keyboard, or viewport controls around a
chart. `ChartSeries::visible_labels` returns labels in the active domain.
`ChartInteractionState` provides bounded pan, zoom, reset, and keyboard point
selection. `ChartSeries::category_ticks` limits dense category labels while
retaining the visible endpoints.
Use `ChartInteractionState::apply` with `ChartInteractionCommand` to share the
same behavior between toolbar buttons, menus, and keyboard shortcuts.
The viewport is always expressed as point indices. Numeric and timestamp
series rescale the coordinates of the visible points to the plot bounds, so a
zoomed viewport fills the available chart surface without changing the source
data.

Mixed cartesian charts use `ChartDataset::kind`:

```rust
MixedChart::new("revenue-mixed").datasets(vec![
    ChartDataset::new("actual", "Actual")
        .kind(ChartKind::Bar)
        .points(actual),
    ChartDataset::new("trend", "Trend")
        .kind(ChartKind::Line)
        .points(trend),
])
```

Use explicit point constructors for each domain. This prevents category,
numeric, and time coordinates from being confused:

```rust
let category = ChartPoint::category("January", 12.0);
let numeric = ChartPoint::numeric(2.5, 12.0).label("Sample");
let timed = ChartPoint::timestamp(1_700_000_000_000, 12.0);
let bubble = ChartPoint::numeric(2.5, 12.0).radius(8.0);
```

`ChartOptions::domain_formatter` controls time labels. `IsoDate` and
`IsoDateTime` are deterministic UTC formats; raw Unix millisecond and second
formats are also available. `ChartSeries::domain_ticks` returns density-limited
ticks for category, numeric, and timestamp domains.
Use `ChartDomainFormatter::Custom` and `ChartValueFormatter::Custom` for
application-specific labels and units. Custom formatters are plain function
pointers and cannot capture application state.

Animate data updates with `ChartTransition::series_at`. The transition is
frame-scheduler agnostic and interpolates values, numeric/time coordinates, and
bubble radii. Hosts choose linear or smooth `ChartEasing` and render the
returned immutable series snapshot for each frame.

Assign datasets to named value axes when units or ranges differ:

```rust
let options = ChartOptions::default().value_axes(vec![
    ChartValueAxis::new("temperature")
        .range(ChartAxis::new(-20.0, 50.0))
        .side(ChartAxisSide::Leading),
    ChartValueAxis::new("pressure")
        .range(ChartAxis::new(900.0, 1_100.0))
        .side(ChartAxisSide::Trailing),
]);

let temperature = ChartDataset::new("temperature", "Temperature")
    .axis("temperature")
    .points(temperature_points);
let pressure = ChartDataset::new("pressure", "Pressure")
    .axis("pressure")
    .points(pressure_points);
```

## Accessibility

`ChartSeries::accessible_summary` returns formatted text rows that hosts can
attach to surrounding labels, details panels, or platform accessibility
metadata.

## Export

`ChartSeries::to_csv` exports the active domain as CSV.
`ChartSeries::to_svg(width, height)` exports a standalone SVG image.

`guic-charts` is a dedicated subsystem crate. Enable it through the `charts`
feature on `guic`, or depend on `guic-charts` directly when building chart-heavy
surfaces.

## Price intervals, profiles and flows

Build price data with `ChartDataset::candles`. Each entry pairs a
`ChartDomainValue` with a validated `ChartOhlc::new(open, high, low, close)`.
The constructor returns `None` for non-finite prices or a high/low that does
not enclose both endpoints. Candles use equally spaced trading intervals.
The tooltip, CSV and accessible value summary include all four prices.

`RadarChart` accepts ordinary datasets in the same category order. Supply at
least three visible categories. It shares the value axis across profiles by
default and draws an open outline for a series missing trailing observations.

Prepare weighted flows once with `SankeyDiagram::new(names, links)`, then pass
a cheap clone to `SankeyChart::new(id, diagram)`. Each `SankeyLink` specifies
source and target node indices and a positive finite value. Cycles, missing
endpoints and invalid weights return a `SankeyError`. Use `on_select` to handle
a `SankeyHit::Node` or `SankeyHit::Link`. The charts dashboard includes runnable
examples of all three views.

Use `.tooltip_content(|series, hit, window, cx| ...)` on a dataset chart to
supply a native element for the hovered point. The chart tracks the pointer,
handles window-edge placement and dismisses the panel when the hit disappears.
Read the source value through
`series.datasets_ref()[hit.dataset_index].points_ref()[hit.point_index]`.

Use `.plot_layer(|series, bounds, window, cx| ...)` to paint an annotation
in the plot after the series. The layer is clipped to the plot. Project a
visible Cartesian datum with `series.plot_position(dataset, index, bounds)`.
Mixed lines, areas, scatter points, bubbles and vertical bars share their
domain and default value axis. Assign a named axis for a separate scale.
Standalone SVG snapshots preserve the chart geometry, including grouped bars,
bubble radii, area fills, pie sectors and doughnut cutouts.


Use `SankeyChart::node_layout(width, spacing)` to control node sizes and
vertical spacing, `label_gap` for the label offset, and `link_opacity` for
ribbons. Reusing the same `SankeyDiagram` retains its layout across frames.
Changing the model or resizing a chart refreshes the flow tooltip under a
stationary pointer.
