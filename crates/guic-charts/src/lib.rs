//! Native chart primitives for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod candle;
mod radar;
mod sankey;
mod svg;
pub use candle::ChartOhlc;
pub use sankey::{SankeyChart, SankeyDiagram, SankeyError, SankeyHit, SankeyLink};

use gpui::{
    App, AppContext as _, Bounds, ClickEvent, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, Point, Render,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window,
    canvas, div, fill, point, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::{cell::RefCell, rc::Rc, time::Duration};

type HoverHandler = Rc<dyn Fn(&Option<ChartHit>, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(&Option<ChartHit>, &mut Window, &mut App)>;
type InteractionHandler = Rc<dyn Fn(&ChartInteractionCommand, &mut Window, &mut App)>;
type OverlayRenderer = Rc<dyn Fn(&ChartSeries, &mut Window, &mut App) -> gpui::AnyElement>;

type TooltipRenderer =
    Rc<dyn Fn(&ChartSeries, &ChartHit, &mut Window, &mut App) -> gpui::AnyElement>;

type PlotLayer = Rc<dyn Fn(&ChartSeries, Bounds<Pixels>, &mut Window, &mut App)>;

struct ChartHandlers {
    hover: Option<HoverHandler>,
    select: Option<SelectHandler>,
    interaction: Option<InteractionHandler>,
    overlay: Option<OverlayRenderer>,
    tooltip: Option<TooltipRenderer>,
    layer: Option<PlotLayer>,
}

#[derive(Default)]
struct ChartTooltipRuntime {
    hit: Option<ChartHit>,
    view: Option<WeakEntity<ChartTooltip>>,
}

/// Supported chart renderers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChartKind {
    /// A connected series chart.
    Line,
    /// A vertical grouped bar chart.
    Bar,
    /// A horizontal grouped bar chart.
    HorizontalBar,
    /// A filled area chart.
    Area,
    /// A point-only cartesian chart.
    Scatter,
    /// A cartesian chart whose point radius represents a third value.
    Bubble,
    /// A price chart with open, high, low and close values per interval.
    Candlestick,
    /// Multiple measurements on equally spaced radial axes.
    Radar,
    /// A proportional radial chart.
    Pie,
    /// A proportional radial chart with an inner cutout.
    Doughnut,
}

/// Domain coordinate for a chart data point.
#[derive(Clone, Debug, PartialEq)]
pub enum ChartDomainValue {
    /// An ordered category rendered by point order.
    Category(SharedString),
    /// A numeric cartesian coordinate.
    Number(f64),
    /// A Unix timestamp in milliseconds.
    Timestamp(i64),
}

/// A single chart data point.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartPoint {
    domain: ChartDomainValue,
    label: Option<SharedString>,
    value: f64,
    radius: Option<f32>,
}

impl ChartPoint {
    /// Creates a category data point.
    ///
    /// Non-finite values are normalized to zero so malformed telemetry cannot
    /// propagate `NaN` or infinity into layout and hit-testing calculations.
    #[must_use]
    pub fn category(label: impl Into<SharedString>, value: f64) -> Self {
        let label = label.into();
        Self {
            domain: ChartDomainValue::Category(label.clone()),
            label: Some(label),
            value: if value.is_finite() { value } else { 0.0 },
            radius: None,
        }
    }

    /// Creates a point on a numeric cartesian domain.
    #[must_use]
    pub fn numeric(x: f64, y: f64) -> Self {
        Self {
            domain: ChartDomainValue::Number(if x.is_finite() { x } else { 0.0 }),
            label: None,
            value: if y.is_finite() { y } else { 0.0 },
            radius: None,
        }
    }

    /// Creates a point on a Unix-millisecond time domain.
    #[must_use]
    pub fn timestamp(unix_millis: i64, value: f64) -> Self {
        Self {
            domain: ChartDomainValue::Timestamp(unix_millis),
            label: None,
            value: if value.is_finite() { value } else { 0.0 },
            radius: None,
        }
    }

    /// Sets an optional accessible/display label independent of the domain.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the radius used by bubble renderers.
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(if radius.is_finite() {
            radius.max(0.0)
        } else {
            0.0
        });
        self
    }

    /// Returns the display label.
    #[must_use]
    pub fn display_label(&self) -> SharedString {
        self.label.clone().unwrap_or_else(|| match &self.domain {
            ChartDomainValue::Category(label) => label.clone(),
            ChartDomainValue::Number(value) => SharedString::from(format_value(*value)),
            ChartDomainValue::Timestamp(value) => SharedString::from(value.to_string()),
        })
    }

    /// Returns the point domain coordinate.
    #[must_use]
    pub fn domain(&self) -> &ChartDomainValue {
        &self.domain
    }

    /// Returns the numeric value.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Returns the optional bubble radius.
    #[must_use]
    pub fn bubble_radius(&self) -> Option<f32> {
        self.radius
    }
}

/// A named dataset rendered by a chart.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartDataset {
    id: SharedString,
    label: SharedString,
    points: Vec<ChartPoint>,
    candles: Vec<ChartOhlc>,
    color: Option<Hsla>,
    kind: Option<ChartKind>,
    axis_id: Option<SharedString>,
}

impl ChartDataset {
    /// Creates an empty dataset.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            points: Vec::new(),
            candles: Vec::new(),
            color: None,
            kind: None,
            axis_id: None,
        }
    }

    /// Replaces data points.
    #[must_use]
    pub fn points(mut self, points: Vec<ChartPoint>) -> Self {
        self.points = points;
        self.candles.clear();
        self
    }

    /// Replaces the dataset with labelled OHLC intervals in trading order.
    /// Other cartesian renderers use the closing price as their point value.
    #[must_use]
    pub fn candles(
        mut self,
        values: impl IntoIterator<Item = (ChartDomainValue, ChartOhlc)>,
    ) -> Self {
        (self.points, self.candles) = values
            .into_iter()
            .map(|(domain, prices)| {
                let point = match domain {
                    ChartDomainValue::Category(label) => {
                        ChartPoint::category(label, prices.close())
                    }
                    ChartDomainValue::Number(x) => ChartPoint::numeric(x, prices.close()),
                    ChartDomainValue::Timestamp(t) => ChartPoint::timestamp(t, prices.close()),
                };
                (point, prices)
            })
            .unzip();
        self
    }

    /// Returns price records in the same order as the closing-price points.
    #[must_use]
    pub fn ohlc(&self) -> &[ChartOhlc] {
        &self.candles
    }

    /// Sets an explicit series color.
    #[must_use]
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Overrides the renderer for this dataset in a mixed cartesian chart.
    /// Mixed layers support lines, areas, vertical bars, scatter points, and bubbles.
    #[must_use]
    pub fn kind(mut self, kind: ChartKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Returns the optional dataset-specific renderer.
    #[must_use]
    pub fn chart_kind(&self) -> Option<ChartKind> {
        self.kind
    }

    /// Assigns this dataset to a configured value axis.
    #[must_use]
    pub fn axis(mut self, axis_id: impl Into<SharedString>) -> Self {
        self.axis_id = Some(axis_id.into());
        self
    }

    /// Returns the configured value-axis identifier.
    #[must_use]
    pub fn axis_id(&self) -> Option<&SharedString> {
        self.axis_id.as_ref()
    }

    /// Returns the dataset id.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// Returns the data points.
    #[must_use]
    pub fn points_ref(&self) -> &[ChartPoint] {
        &self.points
    }
}

/// Computed bounds for chart scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartAxis {
    /// Smallest visible value.
    pub min: f64,
    /// Largest visible value.
    pub max: f64,
}

impl ChartAxis {
    /// Creates an axis range, widening equal values into a visible span.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        if !min.is_finite() || !max.is_finite() {
            return Self { min: 0.0, max: 1.0 };
        }
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        if (max - min).abs() < f64::EPSILON {
            Self {
                min: min.min(0.0),
                max: max.max(1.0),
            }
        } else {
            Self { min, max }
        }
    }

    fn normalize(self, value: f64) -> f64 {
        let span = self.max - self.min;
        let normalized = if span.is_infinite() {
            (value / 2.0 - self.min / 2.0) / (self.max / 2.0 - self.min / 2.0)
        } else {
            (value - self.min) / span
        };
        normalized.clamp(0.0, 1.0)
    }

    /// Returns the axis span.
    #[must_use]
    pub fn span(self) -> f64 {
        self.max - self.min
    }

    /// Returns a panned axis.
    #[must_use]
    pub fn pan(self, delta: f64) -> Self {
        if !delta.is_finite() {
            return self;
        }
        Self::new(self.min + delta, self.max + delta)
    }

    /// Returns an axis zoomed around a center value.
    #[must_use]
    pub fn zoom_around(self, center: f64, factor: f64) -> Self {
        if !center.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return self;
        }
        let half_span = self.span() / factor / 2.0;
        Self::new(center - half_span, center + half_span)
    }
}

/// Reusable viewport and keyboard-selection state for interactive charts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChartInteractionState {
    domain: Option<ChartAxis>,
    selected_index: Option<usize>,
}

/// Standard command set for chart viewport and keyboard interaction controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChartInteractionCommand {
    /// Pan by a signed point/domain delta.
    Pan(f64),
    /// Zoom around a domain coordinate by a positive factor.
    Zoom {
        /// Domain coordinate kept stationary while zooming.
        center: f64,
        /// Positive magnification factor.
        factor: f64,
    },
    /// Restore the complete domain.
    ResetView,
    /// Move selection to the previous point.
    PreviousPoint,
    /// Move selection to the next point.
    NextPoint,
    /// Select the first visible point.
    FirstVisiblePoint,
    /// Select the final available point.
    LastPoint,
}

impl ChartInteractionState {
    /// Creates interaction state showing the complete chart domain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current visible point-index domain.
    #[must_use]
    pub fn domain(self) -> Option<ChartAxis> {
        self.domain
    }

    /// Returns the keyboard-selected point index.
    #[must_use]
    pub fn selected_index(self) -> Option<usize> {
        self.selected_index
    }

    /// Shows the complete domain and clears any previous zoom.
    pub fn reset_view(&mut self) {
        self.domain = None;
    }

    /// Sets and clamps a visible point-index domain.
    pub fn set_domain(&mut self, domain: ChartAxis, point_count: usize) {
        self.domain = clamp_domain(domain, point_count);
    }

    /// Pans the visible domain by a point-index delta.
    pub fn pan(&mut self, delta: f64, point_count: usize) {
        let Some(domain) = self.domain else {
            return;
        };
        self.domain = clamp_domain(domain.pan(delta), point_count);
    }

    /// Zooms the visible domain around a point-index center.
    pub fn zoom(&mut self, center: f64, factor: f64, point_count: usize) {
        if point_count == 0 {
            self.domain = None;
            return;
        }
        let full = ChartAxis::new(0.0, point_count.saturating_sub(1) as f64);
        let domain = self.domain.unwrap_or(full);
        self.domain = clamp_domain(domain.zoom_around(center, factor), point_count);
    }

    /// Moves keyboard selection by a signed number of points.
    pub fn move_selection(&mut self, delta: isize, point_count: usize) {
        if point_count == 0 {
            self.selected_index = None;
            return;
        }
        let current = self.selected_index.unwrap_or(0);
        self.selected_index = Some(
            current
                .saturating_add_signed(delta)
                .min(point_count.saturating_sub(1)),
        );
    }

    /// Selects the first visible point, or clears selection for empty data.
    pub fn select_first_visible(&mut self, point_count: usize) {
        self.selected_index = self
            .domain
            .and_then(|domain| (point_count > 0).then_some(domain.min.floor().max(0.0) as usize));
        if self.domain.is_none() && point_count > 0 {
            self.selected_index = Some(0);
        }
    }

    /// Applies a standard interaction command.
    pub fn apply(&mut self, command: ChartInteractionCommand, point_count: usize) {
        match command {
            ChartInteractionCommand::Pan(delta) => self.pan(delta, point_count),
            ChartInteractionCommand::Zoom { center, factor } => {
                self.zoom(center, factor, point_count);
            }
            ChartInteractionCommand::ResetView => self.reset_view(),
            ChartInteractionCommand::PreviousPoint => self.move_selection(-1, point_count),
            ChartInteractionCommand::NextPoint => self.move_selection(1, point_count),
            ChartInteractionCommand::FirstVisiblePoint => self.select_first_visible(point_count),
            ChartInteractionCommand::LastPoint => {
                self.selected_index = point_count.checked_sub(1);
            }
        }
    }
}

fn clamp_domain(domain: ChartAxis, point_count: usize) -> Option<ChartAxis> {
    if point_count == 0 {
        return None;
    }
    let maximum = point_count.saturating_sub(1) as f64;
    if maximum == 0.0 {
        return Some(ChartAxis::new(0.0, 1.0));
    }
    let span = domain.span().clamp(1.0, maximum);
    let min = domain.min.clamp(0.0, maximum - span);
    Some(ChartAxis::new(min, min + span))
}

/// Value-scale behavior for cartesian charts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChartScale {
    /// Linear numeric scaling.
    Linear,
    /// Base-10 logarithmic scaling. Non-positive values are clamped to the
    /// visible positive minimum.
    Log10,
}

/// Side on which a value axis is presented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChartAxisSide {
    /// Leading/left plot edge.
    Leading,
    /// Trailing/right plot edge.
    Trailing,
}

/// Named value-axis configuration for multi-axis charts.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartValueAxis {
    id: SharedString,
    range: Option<ChartAxis>,
    scale: ChartScale,
    side: ChartAxisSide,
}

impl ChartValueAxis {
    /// Creates an automatically ranged linear value axis.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            range: None,
            scale: ChartScale::Linear,
            side: ChartAxisSide::Leading,
        }
    }

    /// Sets an explicit axis range.
    #[must_use]
    pub fn range(mut self, range: ChartAxis) -> Self {
        self.range = Some(range);
        self
    }

    /// Sets the axis scale.
    #[must_use]
    pub fn scale(mut self, scale: ChartScale) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the presentation side.
    #[must_use]
    pub fn side(mut self, side: ChartAxisSide) -> Self {
        self.side = side;
        self
    }
}

/// Formatting strategy for values shown by chart summaries and tooltips.
#[derive(Clone, Debug, Default)]
pub enum ChartValueFormatter {
    /// Use compact default numeric formatting.
    #[default]
    Default,
    /// Format as a percentage.
    Percent,
    /// Prefix values with a currency or unit symbol.
    Prefix(SharedString),
    /// Suffix values with a unit label.
    Suffix(SharedString),
    /// Format values with an application-defined function.
    Custom(fn(f64) -> String),
}

/// Formatting strategy for category, numeric, and time domain ticks.
#[derive(Clone, Copy, Debug, Default)]
pub enum ChartDomainFormatter {
    /// Categories use their label, numeric values use compact formatting, and
    /// timestamps use an ISO UTC date-time.
    #[default]
    Auto,
    /// Render raw Unix milliseconds.
    UnixMillis,
    /// Render Unix seconds.
    UnixSeconds,
    /// Render timestamps as `YYYY-MM-DD` in UTC.
    IsoDate,
    /// Render timestamps as `YYYY-MM-DD HH:MM:SS` in UTC.
    IsoDateTime,
    /// Format any domain value with an application-defined function.
    Custom(fn(&ChartDomainValue) -> SharedString),
}

/// Policy used to bound dense domain labels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChartLabelCollisionPolicy {
    /// Sample labels uniformly while retaining the first and final label.
    #[default]
    Sample,
    /// Truncate labels before sampling them.
    Truncate {
        /// Maximum Unicode scalar count retained per label.
        max_chars: usize,
    },
    /// Hide domain labels while retaining accessible summaries and tooltips.
    Hide,
}

/// Determines which values are included in a chart tooltip after hit testing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChartTooltipMode {
    /// Show only the nearest data element.
    #[default]
    Nearest,
    /// Show every dataset value at the intersected point index.
    Index,
    /// Show visible values from the intersected dataset.
    Dataset,
}

impl ChartValueFormatter {
    fn format(&self, value: f64) -> String {
        match self {
            Self::Default => format_value(value),
            Self::Percent => format!("{}%", format_value(value)),
            Self::Prefix(prefix) => format!("{prefix}{}", format_value(value)),
            Self::Suffix(suffix) => format!("{} {suffix}", format_value(value)),
            Self::Custom(formatter) => formatter(value),
        }
    }
}

/// Chart layout and chrome options.
#[derive(Clone, Debug)]
pub struct ChartOptions {
    title: Option<SharedString>,
    height: f32,
    show_axes: bool,
    show_legend: bool,
    show_grid: bool,
    show_values: bool,
    show_tooltip: bool,
    tooltip_mode: ChartTooltipMode,
    tooltip_intersect: bool,
    tooltip_hit_radius: f32,
    tooltip_max_rows: usize,
    stacked: bool,
    scale: ChartScale,
    value_axis: Option<ChartAxis>,
    value_formatter: ChartValueFormatter,
    crosshair_index: Option<usize>,
    active_hit: Option<ChartHit>,
    domain: Option<ChartAxis>,
    doughnut_cutout: f32,
    empty_message: Option<SharedString>,
    value_axes: Vec<ChartValueAxis>,
    domain_formatter: ChartDomainFormatter,
    max_domain_labels: usize,
    label_collision: ChartLabelCollisionPolicy,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self {
            title: None,
            height: 280.0,
            show_axes: true,
            show_legend: true,
            show_grid: true,
            show_values: false,
            show_tooltip: true,
            tooltip_mode: ChartTooltipMode::Nearest,
            tooltip_intersect: true,
            tooltip_hit_radius: 18.0,
            tooltip_max_rows: 16,
            stacked: false,
            scale: ChartScale::Linear,
            value_axis: None,
            value_formatter: ChartValueFormatter::Default,
            crosshair_index: None,
            active_hit: None,
            domain: None,
            doughnut_cutout: 0.58,
            empty_message: None,
            value_axes: Vec::new(),
            domain_formatter: ChartDomainFormatter::Auto,
            max_domain_labels: 8,
            label_collision: ChartLabelCollisionPolicy::Sample,
        }
    }
}

impl ChartOptions {
    /// Sets the chart title.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets plot height in logical pixels.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.height = height.max(120.0);
        }
        self
    }

    /// Sets whether axes are rendered.
    #[must_use]
    pub fn axes(mut self, show_axes: bool) -> Self {
        self.show_axes = show_axes;
        self
    }

    /// Sets whether the legend is rendered.
    #[must_use]
    pub fn legend(mut self, show_legend: bool) -> Self {
        self.show_legend = show_legend;
        self
    }

    /// Sets whether grid lines are rendered.
    #[must_use]
    pub fn grid(mut self, show_grid: bool) -> Self {
        self.show_grid = show_grid;
        self
    }

    /// Sets whether a compact value summary is rendered below the chart.
    #[must_use]
    pub fn values(mut self, show_values: bool) -> Self {
        self.show_values = show_values;
        self
    }

    /// Sets whether the plot surface exposes a hover tooltip with point values.
    #[must_use]
    pub fn tooltip(mut self, show_tooltip: bool) -> Self {
        self.show_tooltip = show_tooltip;
        self
    }

    /// Sets how values are grouped after pointer hit testing selects chart data.
    #[must_use]
    pub fn tooltip_mode(mut self, mode: ChartTooltipMode) -> Self {
        self.tooltip_mode = mode;
        self
    }

    /// Sets whether cartesian tooltips require direct geometry intersection.
    ///
    /// The default is `true`, so tooltips appear only when the pointer is over
    /// painted data geometry. Set this to `false` for continuous nearest-datum
    /// interaction while the pointer remains inside the plot. Radial charts
    /// always require slice intersection.
    #[must_use]
    pub fn tooltip_intersect(mut self, intersect: bool) -> Self {
        self.tooltip_intersect = intersect;
        self
    }

    /// Sets the maximum point distance used by intersecting line and point hit testing.
    ///
    /// This value is used when `tooltip_intersect(true)` is enabled. Bar and
    /// radial charts use their painted geometry instead of this radius.
    #[must_use]
    pub fn tooltip_hit_radius(mut self, radius: f32) -> Self {
        if radius.is_finite() {
            self.tooltip_hit_radius = radius.clamp(0.0, 96.0);
        }
        self
    }

    /// Sets the maximum rows rendered by grouped tooltip modes.
    #[must_use]
    pub fn tooltip_max_rows(mut self, max_rows: usize) -> Self {
        self.tooltip_max_rows = max_rows.clamp(1, 100);
        self
    }

    /// Sets whether bar and area datasets are stacked by point index.
    #[must_use]
    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }

    /// Sets the value scale used by cartesian charts.
    #[must_use]
    pub fn scale(mut self, scale: ChartScale) -> Self {
        self.scale = scale;
        self
    }

    /// Sets an explicit value axis range.
    #[must_use]
    pub fn value_axis(mut self, axis: ChartAxis) -> Self {
        self.value_axis = Some(axis);
        self
    }

    /// Sets the formatter used by summaries and tooltips.
    #[must_use]
    pub fn value_formatter(mut self, formatter: ChartValueFormatter) -> Self {
        self.value_formatter = formatter;
        self
    }

    /// Sets a host-managed point index to emphasize with a crosshair.
    #[must_use]
    pub fn crosshair_index(mut self, index: Option<usize>) -> Self {
        self.crosshair_index = index;
        self
    }

    /// Sets the host-managed active hit used by tooltips and emphasis.
    #[must_use]
    pub fn active_hit(mut self, hit: Option<ChartHit>) -> Self {
        self.active_hit = hit;
        self
    }

    /// Sets the visible point-index domain for zoomed or panned charts.
    #[must_use]
    pub fn domain(mut self, domain: ChartAxis) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Sets the inner radius fraction used by doughnut charts.
    #[must_use]
    pub fn doughnut_cutout(mut self, cutout: f32) -> Self {
        if cutout.is_finite() {
            self.doughnut_cutout = cutout.clamp(0.0, 0.9);
        }
        self
    }

    /// Sets text shown when a chart has no data.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = Some(message.into());
        self
    }

    /// Replaces named value axes used by assigned datasets.
    #[must_use]
    pub fn value_axes(mut self, axes: Vec<ChartValueAxis>) -> Self {
        self.value_axes = axes;
        self
    }

    /// Sets formatting for domain-axis ticks.
    #[must_use]
    pub fn domain_formatter(mut self, formatter: ChartDomainFormatter) -> Self {
        self.domain_formatter = formatter;
        self
    }

    /// Sets the maximum domain labels rendered below the plot.
    #[must_use]
    pub fn max_domain_labels(mut self, max_labels: usize) -> Self {
        self.max_domain_labels = max_labels;
        self
    }

    /// Sets the dense-label collision policy.
    #[must_use]
    pub fn label_collision(mut self, policy: ChartLabelCollisionPolicy) -> Self {
        self.label_collision = policy;
        self
    }
}

/// A hit-test result for a chart point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChartHit {
    /// Dataset index.
    pub dataset_index: usize,
    /// Point index within the dataset.
    pub point_index: usize,
    /// Point label.
    pub label: SharedString,
}

/// A category-axis tick selected for rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChartCategoryTick {
    /// Point index represented by the tick.
    pub point_index: usize,
    /// Category label displayed at the tick.
    pub label: SharedString,
}

/// A density-limited domain-axis tick.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartDomainTick {
    /// Dataset point index represented by the tick.
    pub point_index: usize,
    /// Numeric coordinate used for positioning.
    pub coordinate: f64,
    /// Formatted tick label.
    pub label: SharedString,
}

/// Shared chart model used by concrete chart components.
#[derive(Clone, Debug)]
pub struct ChartSeries {
    kind: ChartKind,
    datasets: Vec<ChartDataset>,
    options: ChartOptions,
    value_axis_cache: ChartAxis,
    stacked_axis_cache: ChartAxis,
    dataset_axis_cache: Vec<ChartAxis>,
    mixed: bool,
    mixed_bar_indices: Vec<usize>,
    domain_order: std::sync::OnceLock<Vec<DomainOrder>>,
    visible_axis: std::sync::OnceLock<ChartAxis>,
    smallest_positive_cache: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DomainOrder {
    Categories,
    Ascending,
    Unordered,
}

/// Easing curve used by a chart data transition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChartEasing {
    /// Constant-rate interpolation.
    Linear,
    /// Smooth cubic acceleration and deceleration.
    #[default]
    EaseInOut,
}

impl ChartEasing {
    fn apply(self, progress: f32) -> f32 {
        let progress = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self {
            Self::Linear => progress,
            Self::EaseInOut => progress * progress * (3.0 - 2.0 * progress),
        }
    }
}

/// Deterministic transition between two chart series snapshots.
#[derive(Clone, Debug)]
pub struct ChartTransition {
    from: ChartSeries,
    to: ChartSeries,
    easing: ChartEasing,
}

impl ChartTransition {
    /// Creates a transition between immutable series snapshots.
    #[must_use]
    pub fn new(from: ChartSeries, to: ChartSeries) -> Self {
        Self {
            from,
            to,
            easing: ChartEasing::default(),
        }
    }

    /// Sets the easing curve.
    #[must_use]
    pub fn easing(mut self, easing: ChartEasing) -> Self {
        self.easing = easing;
        self
    }

    /// Produces a renderable series at normalized progress `0..=1`.
    #[must_use]
    pub fn series_at(&self, progress: f32) -> ChartSeries {
        let progress = f64::from(self.easing.apply(progress));
        let mut result = self.to.clone();
        for target_dataset in &mut result.datasets {
            let Some(source_dataset) = self
                .from
                .datasets
                .iter()
                .find(|dataset| dataset.id == target_dataset.id)
            else {
                continue;
            };
            for (index, prices) in target_dataset
                .candles
                .iter_mut()
                .take(source_dataset.points.len())
                .enumerate()
            {
                let from = source_dataset
                    .candles
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| ChartOhlc::flat(source_dataset.points[index].value));
                *prices = from.interpolate(*prices, progress);
            }
            for (target_point, source_point) in
                target_dataset.points.iter_mut().zip(&source_dataset.points)
            {
                target_point.value = interpolate(source_point.value, target_point.value, progress);
                if source_point.domain != target_point.domain {
                    target_point.domain =
                        interpolate_domain(&source_point.domain, &target_point.domain, progress);
                }
                target_point.radius = match (source_point.radius, target_point.radius) {
                    (Some(from), Some(to)) => {
                        Some(interpolate(f64::from(from), f64::from(to), progress) as f32)
                    }
                    _ => target_point.radius,
                };
            }
        }
        result.rebuild_derived_data();
        result
    }
}

fn interpolate(from: f64, to: f64, progress: f64) -> f64 {
    let delta = to - from;
    if delta.is_finite() {
        if progress == 1. {
            to
        } else {
            from + delta * progress
        }
    } else {
        from * (1. - progress) + to * progress
    }
}

fn interpolate_domain(
    from: &ChartDomainValue,
    to: &ChartDomainValue,
    progress: f64,
) -> ChartDomainValue {
    match (from, to) {
        (ChartDomainValue::Number(from), ChartDomainValue::Number(to)) => {
            ChartDomainValue::Number(interpolate(*from, *to, progress))
        }
        (ChartDomainValue::Timestamp(from), ChartDomainValue::Timestamp(to)) => {
            ChartDomainValue::Timestamp(
                interpolate(*from as f64, *to as f64, progress).round() as i64
            )
        }
        _ => to.clone(),
    }
}

impl ChartSeries {
    /// Creates an empty chart model.
    #[must_use]
    pub fn new(kind: ChartKind) -> Self {
        Self {
            kind,
            datasets: Vec::new(),
            options: ChartOptions::default(),
            value_axis_cache: ChartAxis::new(0.0, 1.0),
            stacked_axis_cache: ChartAxis::new(0.0, 1.0),
            dataset_axis_cache: Vec::new(),
            mixed: false,
            mixed_bar_indices: Vec::new(),
            domain_order: Default::default(),
            visible_axis: Default::default(),
            smallest_positive_cache: 1.0,
        }
    }

    /// Replaces datasets.
    #[must_use]
    pub fn datasets(mut self, datasets: Vec<ChartDataset>) -> Self {
        self.datasets = datasets;
        self.rebuild_derived_data();
        self
    }

    /// Returns the datasets in legend and hit-test order.
    #[must_use]
    pub fn datasets_ref(&self) -> &[ChartDataset] {
        &self.datasets
    }

    /// Replaces chart options.
    #[must_use]
    pub fn options(mut self, options: ChartOptions) -> Self {
        if self.options.domain != options.domain {
            self.visible_axis.take();
        }
        self.options = options;
        self
    }

    /// Returns the data axis for all datasets.
    #[must_use]
    pub fn value_axis(&self) -> ChartAxis {
        if let Some(axis) = self.options.value_axis {
            return axis;
        }
        if self.is_stacked()
            && (self.kind == ChartKind::Bar
                || self.kind == ChartKind::HorizontalBar
                || self.kind == ChartKind::Area)
            && !self.datasets.is_empty()
        {
            return self.stacked_axis_cache;
        }
        self.value_axis_cache
    }

    /// Returns the numeric/time domain axis, or the point-index axis for
    /// category data.
    #[must_use]
    pub fn domain_axis(&self) -> ChartAxis {
        let mut values = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.points.iter())
            .filter_map(|point| match point.domain {
                ChartDomainValue::Number(value) => Some(value),
                ChartDomainValue::Timestamp(value) => Some(value as f64),
                ChartDomainValue::Category(_) => None,
            });
        let Some(first) = values.next() else {
            return ChartAxis::new(0.0, self.max_points().saturating_sub(1) as f64);
        };
        let (mut min, mut max) = (first, first);
        for value in values {
            min = min.min(value);
            max = max.max(value);
        }
        ChartAxis::new(min, max)
    }

    /// Returns the domain axis used to render the current viewport.
    ///
    /// Viewports are expressed as point indices so the same interaction state
    /// works for category, numeric, and time series. Numeric and time series
    /// are rescaled to the domain coordinates of the visible points.
    #[must_use]
    pub fn visible_domain_axis(&self) -> ChartAxis {
        *self
            .visible_axis
            .get_or_init(|| self.compute_visible_domain_axis())
    }

    fn domain_orders(&self) -> &[DomainOrder] {
        self.domain_order.get_or_init(|| {
            self.datasets
                .iter()
                .map(|d| {
                    if d.points
                        .first()
                        .is_none_or(|p| matches!(p.domain, ChartDomainValue::Category(_)))
                    {
                        if d.points
                            .iter()
                            .all(|p| matches!(p.domain, ChartDomainValue::Category(_)))
                        {
                            return DomainOrder::Categories;
                        }
                    } else {
                        let mut previous = f64::NEG_INFINITY;
                        let sorted = d.points.iter().all(|p| {
                            let x = match p.domain {
                                ChartDomainValue::Number(x) => x,
                                ChartDomainValue::Timestamp(t) => t as f64,
                                ChartDomainValue::Category(_) => return false,
                            };
                            let ascending = x >= previous;
                            previous = x;
                            ascending
                        });
                        if sorted {
                            return DomainOrder::Ascending;
                        }
                    }
                    DomainOrder::Unordered
                })
                .collect()
        })
    }

    fn compute_visible_domain_axis(&self) -> ChartAxis {
        let Some(indices) = self.visible_index_range() else {
            return ChartAxis::new(0.0, 1.0);
        };
        if self
            .domain_orders()
            .iter()
            .all(|order| *order == DomainOrder::Categories)
        {
            return ChartAxis::new(*indices.start() as f64, *indices.end() as f64);
        }
        let mut values = self
            .datasets
            .iter()
            .flat_map(|dataset| {
                indices
                    .clone()
                    .filter_map(|index| dataset.points.get(index))
            })
            .filter_map(|point| match point.domain {
                ChartDomainValue::Number(value) => Some(value),
                ChartDomainValue::Timestamp(value) => Some(value as f64),
                ChartDomainValue::Category(_) => None,
            });
        let Some(first) = values.next() else {
            return ChartAxis::new(*indices.start() as f64, *indices.end() as f64);
        };
        let (mut min, mut max) = (first, first);
        for value in values {
            min = min.min(value);
            max = max.max(value);
        }
        if (max - min).abs() < f64::EPSILON {
            let padding = if min.abs() >= 1_000.0 {
                1.0
            } else {
                (min.abs() * 0.01).max(0.5)
            };
            ChartAxis::new(min - padding, max + padding)
        } else {
            ChartAxis::new(min, max)
        }
    }

    /// Returns the effective value axis for a dataset.
    #[must_use]
    pub fn dataset_value_axis(&self, dataset_index: usize) -> ChartAxis {
        let Some(dataset) = self.datasets.get(dataset_index) else {
            return self.value_axis();
        };
        if let Some(config) = self.axis_config(dataset)
            && let Some(range) = config.range
        {
            return range;
        }
        self.dataset_axis_cache
            .get(dataset_index)
            .copied()
            .unwrap_or(self.value_axis_cache)
    }

    fn axis_config(&self, dataset: &ChartDataset) -> Option<&ChartValueAxis> {
        let id = dataset.axis_id.as_ref()?;
        self.options.value_axes.iter().find(|axis| &axis.id == id)
    }

    fn dataset_scale(&self, dataset_index: usize) -> ChartScale {
        self.datasets
            .get(dataset_index)
            .and_then(|dataset| self.axis_config(dataset))
            .map_or(self.options.scale, |axis| axis.scale)
    }

    fn scaled_dataset_axis(&self, dataset_index: usize) -> ChartAxis {
        let axis = self.dataset_value_axis(dataset_index);
        match self.dataset_scale(dataset_index) {
            ChartScale::Linear => axis,
            ChartScale::Log10 => {
                let min = positive_log_min(axis.min, self);
                ChartAxis::new(min.log10(), axis.max.max(min).log10())
            }
        }
    }

    fn scale_dataset_value(&self, dataset_index: usize, value: f64) -> f64 {
        match self.dataset_scale(dataset_index) {
            ChartScale::Linear => value,
            ChartScale::Log10 => value
                .max(positive_log_min(
                    self.dataset_value_axis(dataset_index).min,
                    self,
                ))
                .log10(),
        }
    }

    fn rendered_axis(&self, dataset_index: usize) -> ChartAxis {
        if self
            .datasets
            .get(dataset_index)
            .and_then(|dataset| self.axis_config(dataset))
            .is_some()
        {
            self.scaled_dataset_axis(dataset_index)
        } else {
            self.scaled_axis()
        }
    }

    fn scale_rendered_value(&self, dataset_index: usize, value: f64) -> f64 {
        if self
            .datasets
            .get(dataset_index)
            .and_then(|dataset| self.axis_config(dataset))
            .is_some()
        {
            self.scale_dataset_value(dataset_index, value)
        } else {
            self.scale_value(value)
        }
    }

    /// Projects a visible Cartesian point into plot coordinates for a custom layer.
    /// Bar coordinates identify the value end at the center of the bar.
    #[must_use]
    pub fn plot_position(
        &self,
        dataset_index: usize,
        point_index: usize,
        bounds: Bounds<Pixels>,
    ) -> Option<Point<Pixels>> {
        let visible = self.visible_index_range()?;
        if !visible.contains(&point_index) {
            return None;
        }
        let dataset = self.datasets.get(dataset_index)?;
        let datum = dataset.points.get(point_index)?;
        let kind = dataset.kind.unwrap_or(self.kind);
        let slot = point_index - visible.start();
        let count = visible.count();
        let axis = self.rendered_axis(dataset_index);
        let stacked = self.is_stacked()
            && matches!(
                kind,
                ChartKind::Bar | ChartKind::HorizontalBar | ChartKind::Area
            );
        let value = if stacked {
            self.stacked_value_at(dataset_index, point_index)
        } else {
            datum.value
        };
        let scaled = self.scale_rendered_value(dataset_index, value);
        match kind {
            ChartKind::Bar | ChartKind::HorizontalBar => {
                let horizontal = kind == ChartKind::HorizontalBar;
                let band = if horizontal {
                    bounds.size.height
                } else {
                    bounds.size.width
                } / count as f32;
                let lanes = if stacked { 1 } else { self.bar_lane_count() };
                let width = (band * 0.72 / lanes as f32).max(px(2.));
                let lane = if stacked {
                    0
                } else {
                    self.bar_lane(dataset_index)
                };
                let mid = band * (slot as f32 + 0.14) + width * (lane as f32 + 0.5);
                Some(if horizontal {
                    point(x_for_scaled_value(scaled, axis, bounds), bounds.top() + mid)
                } else {
                    point(
                        bounds.left() + mid,
                        y_for_scaled_value(scaled, axis, bounds),
                    )
                })
            }
            ChartKind::Pie | ChartKind::Doughnut | ChartKind::Radar | ChartKind::Candlestick => {
                None
            }
            _ => Some(point_for_data(
                datum,
                slot,
                count,
                self.visible_domain_axis(),
                scaled,
                axis,
                bounds,
            )),
        }
    }

    /// Returns the nearest rendered point for a plot-space coordinate.
    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32, width: f32, height: f32) -> Option<ChartHit> {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || x < 0.0
            || y < 0.0
            || x > width
            || y > height
        {
            return None;
        }
        if self.kind == ChartKind::Candlestick {
            return candle::hit_test(self, x, y, width, height);
        }
        if self.kind == ChartKind::Radar {
            return radar::hit_test(self, x, y, width, height);
        }
        if self.kind == ChartKind::Pie || self.kind == ChartKind::Doughnut {
            return self.pie_hit_test(x, y, width, height);
        }
        if !self.mixed && matches!(self.kind, ChartKind::Bar | ChartKind::HorizontalBar) {
            return self.bar_hit_test(x, y, width, height).map(|(hit, _)| hit);
        }
        let points = self.visible_index_range()?;
        let max_points = points.clone().count();
        let first = *points.start();
        let domain_axis = self.visible_domain_axis();
        let category_index = first + (x / width * (max_points - 1) as f32).round() as usize;
        let bar = if self.mixed && !self.mixed_bar_indices.is_empty() {
            self.bar_hit_test(x, y, width, height)
        } else {
            None
        };
        let mut best = bar
            .as_ref()
            .map(|(hit, _)| (hit.dataset_index, hit.point_index));
        let mut best_distance = bar.map_or(f32::MAX, |(_, distance)| distance);
        for (dataset_index, dataset) in self.datasets.iter().enumerate() {
            let kind = dataset.kind.unwrap_or(self.kind);
            if self.mixed
                && !matches!(
                    kind,
                    ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble
                )
            {
                continue;
            }
            let axis = self.rendered_axis(dataset_index);
            let stop = (*points.end() + 1).min(dataset.points.len());
            if first >= stop {
                continue;
            }
            let mut candidates = first..stop;
            match self.domain_orders()[dataset_index] {
                DomainOrder::Categories => {
                    if category_index >= stop {
                        continue;
                    }
                    candidates = category_index..category_index + 1;
                }
                DomainOrder::Ascending if self.options.tooltip_intersect => {
                    let radius = if kind == ChartKind::Bubble {
                        self.options.tooltip_hit_radius.max(64.)
                    } else {
                        self.options.tooltip_hit_radius
                    };
                    let visible = &dataset.points[first..stop];
                    let screen_x = |point: &ChartPoint| {
                        domain_axis.normalize(point_domain_value(point, 0)) as f32 * width
                    };
                    let left = visible.partition_point(|p| screen_x(p) < x - radius);
                    let right = visible.partition_point(|p| screen_x(p) <= x + radius);
                    candidates = first + left..first + right;
                }
                _ => {}
            }
            for point_index in candidates {
                let point = &dataset.points[point_index];
                if !point.value.is_finite() {
                    continue;
                }
                let value = if self.is_stacked()
                    && (self.kind == ChartKind::Bar
                        || self.kind == ChartKind::HorizontalBar
                        || self.kind == ChartKind::Area)
                {
                    self.stacked_value_at(dataset_index, point_index)
                } else {
                    point.value
                };
                let visible_index = point_index.saturating_sub(*points.start());
                let category = if matches!(point.domain, ChartDomainValue::Category(_)) {
                    category_position(visible_index, max_points)
                } else {
                    domain_axis.normalize(point_domain_value(point, point_index)) as f32
                };
                let normalized =
                    axis.normalize(self.scale_rendered_value(dataset_index, value)) as f32;
                let (px, py) = if self.kind == ChartKind::HorizontalBar {
                    (normalized * width, category * height)
                } else {
                    (category * width, height - normalized * height)
                };
                let distance = (px - x).hypot(py - y);
                let radius = if kind == ChartKind::Bubble {
                    point
                        .radius
                        .unwrap_or(4.)
                        .clamp(1., 64.)
                        .max(self.options.tooltip_hit_radius)
                } else {
                    self.options.tooltip_hit_radius
                };
                if distance < best_distance
                    && (!self.options.tooltip_intersect || distance <= radius)
                {
                    best_distance = distance;
                    best = Some((dataset_index, point_index));
                }
            }
        }
        best.map(|(dataset_index, point_index)| ChartHit {
            dataset_index,
            point_index,
            label: self.datasets[dataset_index].points[point_index].display_label(),
        })
    }

    fn bar_hit_test(&self, x: f32, y: f32, width: f32, height: f32) -> Option<(ChartHit, f32)> {
        let points = self.visible_index_range()?;
        let max_points = points.clone().count();
        let dataset_count = self.bar_lane_count();
        let horizontal = self.kind == ChartKind::HorizontalBar;
        let group_extent = if horizontal {
            height / max_points as f32
        } else {
            width / max_points as f32
        };
        let category_coordinate = if horizontal { y } else { x };
        let visible_index = (category_coordinate / group_extent).floor() as usize;
        let visible_index = visible_index.min(max_points - 1);
        let point_index = points.clone().nth(visible_index)?;
        let bar_extent = if self.is_stacked() {
            (group_extent * 0.72).max(2.0)
        } else {
            (group_extent / dataset_count as f32 * 0.72).max(2.0)
        };
        let mut positive_offset = 0.0;
        let mut negative_offset = 0.0;
        let mut nearest = None;
        let mut nearest_distance = f32::MAX;

        for (dataset_index, dataset) in self.datasets.iter().enumerate() {
            if self.mixed && dataset.kind.unwrap_or(self.kind) != ChartKind::Bar {
                continue;
            }
            let axis = if self.is_stacked() {
                self.scaled_axis()
            } else {
                self.rendered_axis(dataset_index)
            };
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            if !data_point.value.is_finite() {
                continue;
            }
            let (start, end) = if self.is_stacked() {
                let offset = if data_point.value >= 0.0 {
                    &mut positive_offset
                } else {
                    &mut negative_offset
                };
                let start = *offset;
                *offset += data_point.value;
                (start, *offset)
            } else {
                (
                    match self.dataset_scale(dataset_index) {
                        ChartScale::Linear => 0.0,
                        ChartScale::Log10 => {
                            positive_log_min(self.dataset_value_axis(dataset_index).min, self)
                        }
                    },
                    data_point.value,
                )
            };
            let start = if self.is_stacked() {
                axis.normalize(self.scale_value(start)) as f32
            } else {
                axis.normalize(self.scale_rendered_value(dataset_index, start)) as f32
            };
            let end = if self.is_stacked() {
                axis.normalize(self.scale_value(end)) as f32
            } else {
                axis.normalize(self.scale_rendered_value(dataset_index, end)) as f32
            };
            let dataset_offset = if self.is_stacked() {
                0.0
            } else {
                self.bar_lane(dataset_index) as f32 * bar_extent
            };
            let category_start =
                visible_index as f32 * group_extent + group_extent * 0.14 + dataset_offset;
            let intersects = if horizontal {
                let value_start = start.min(end) * width;
                let value_end = start.max(end) * width;
                x >= value_start
                    && x <= value_end.max(value_start + 1.0)
                    && y >= category_start
                    && y <= category_start + bar_extent
            } else {
                let value_start = height - start.max(end) * height;
                let value_end = height - start.min(end) * height;
                x >= category_start
                    && x <= category_start + bar_extent
                    && y >= value_start
                    && y <= value_end.max(value_start + 1.0)
            };
            if intersects {
                return Some((
                    ChartHit {
                        dataset_index,
                        point_index,
                        label: data_point.display_label(),
                    },
                    0.,
                ));
            }
            if !self.options.tooltip_intersect {
                let (distance_x, distance_y) = if horizontal {
                    let value_start = start.min(end) * width;
                    let value_end = start.max(end) * width;
                    (
                        distance_to_interval(x, value_start, value_end.max(value_start + 1.0)),
                        distance_to_interval(y, category_start, category_start + bar_extent),
                    )
                } else {
                    let value_start = height - start.max(end) * height;
                    let value_end = height - start.min(end) * height;
                    (
                        distance_to_interval(x, category_start, category_start + bar_extent),
                        distance_to_interval(y, value_start, value_end.max(value_start + 1.0)),
                    )
                };
                let distance = distance_x.hypot(distance_y);
                if distance < nearest_distance {
                    nearest_distance = distance;
                    nearest = Some(ChartHit {
                        dataset_index,
                        point_index,
                        label: data_point.display_label(),
                    });
                }
            }
        }
        nearest.map(|hit| (hit, nearest_distance))
    }

    /// Returns the nearest point index for a plot-space x coordinate.
    #[must_use]
    pub fn nearest_point_index(&self, x: f32, width: f32) -> Option<usize> {
        if !x.is_finite() || !width.is_finite() || width <= 0.0 {
            return None;
        }
        let range = self.visible_index_range()?;
        let first = *range.start();
        let last = *range.end();
        if self
            .domain_orders()
            .iter()
            .all(|o| *o == DomainOrder::Categories)
        {
            return Some(nearest_uniform(x, width, first, last));
        }
        let axis = self.visible_domain_axis();
        let project = |p: &ChartPoint, i| match p.domain {
            ChartDomainValue::Category(_) => category_position(i - first, last - first + 1) * width,
            _ => axis.normalize(point_domain_value(p, i)) as f32 * width,
        };
        // The first available dataset defines the public index lookup at each position.
        if self.domain_orders().first() == Some(&DomainOrder::Ascending)
            && let Some(visible) = self.datasets[0].points.get(range.clone())
        {
            let at = visible.partition_point(|p| project(p, first) < x);
            let right = at.min(visible.len() - 1);
            let left = at.saturating_sub(1);
            let a = project(&visible[left], first);
            let b = project(&visible[right], first);
            let target = if (x - a).abs() <= (x - b).abs() { a } else { b };
            return Some(first + visible.partition_point(|p| project(p, first) < target));
        }
        range.min_by(|&a, &b| {
            let distance = |i| {
                self.datasets
                    .iter()
                    .find_map(|d| d.points.get(i))
                    .map_or(f32::INFINITY, |p| (x - project(p, i)).abs())
            };
            distance(a).total_cmp(&distance(b))
        })
    }

    /// Returns the nearest point index for a plot-space y coordinate.
    #[must_use]
    pub fn nearest_point_index_y(&self, y: f32, height: f32) -> Option<usize> {
        if !y.is_finite() || !height.is_finite() || height <= 0. {
            return None;
        }
        let range = self.visible_index_range()?;
        Some(nearest_uniform(y, height, *range.start(), *range.end()))
    }

    /// Returns rows suitable for accessibility summaries or external legends.
    #[must_use]
    pub fn accessible_summary(&self) -> Vec<String> {
        chart_value_rows(self)
            .into_iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect()
    }

    /// Returns labels visible under the current point-index domain.
    #[must_use]
    pub fn visible_labels(&self) -> Vec<SharedString> {
        let Some(dataset) = self.datasets.first() else {
            return Vec::new();
        };
        self.visible_point_indices()
            .into_iter()
            .filter_map(|index| dataset.points.get(index).map(ChartPoint::display_label))
            .collect()
    }

    /// Returns density-limited category ticks for the visible domain.
    ///
    /// The first and last visible labels are retained whenever at least two
    /// ticks are requested. Hosts can use this to avoid overlapping labels on
    /// dense category axes.
    #[must_use]
    pub fn category_ticks(&self, max_ticks: usize) -> Vec<ChartCategoryTick> {
        let Some(dataset) = self.datasets.first() else {
            return Vec::new();
        };
        self.sampled_ticks(dataset, max_ticks)
            .into_iter()
            .filter_map(|point_index| {
                dataset
                    .points
                    .get(point_index)
                    .map(|point| ChartCategoryTick {
                        point_index,
                        label: point.display_label(),
                    })
            })
            .collect()
    }

    /// Returns density-limited ticks for category, numeric, or time domains.
    #[must_use]
    pub fn domain_ticks(&self, max_ticks: usize) -> Vec<ChartDomainTick> {
        let Some(dataset) = self.datasets.first() else {
            return Vec::new();
        };
        let indices = self.sampled_ticks(dataset, max_ticks);
        indices
            .into_iter()
            .filter_map(|point_index| {
                dataset
                    .points
                    .get(point_index)
                    .map(|point| ChartDomainTick {
                        point_index,
                        coordinate: point_domain_value(point, point_index),
                        label: format_domain_value(&point.domain, self.options.domain_formatter),
                    })
            })
            .collect()
    }

    fn sampled_ticks(&self, dataset: &ChartDataset, limit: usize) -> Vec<usize> {
        let Some(range) = self.visible_index_range() else {
            return Vec::new();
        };
        let start = *range.start();
        let available = (*range.end() + 1)
            .min(dataset.points.len())
            .saturating_sub(start);
        let count = limit.min(available);
        (0..count)
            .map(|tick| {
                start
                    + if count <= 1 {
                        0
                    } else {
                        (tick as u128 * (available - 1) as u128 / (count - 1) as u128) as usize
                    }
            })
            .collect()
    }

    /// Returns domain labels after applying the configured collision policy.
    #[must_use]
    pub fn layout_domain_labels(&self) -> Vec<ChartDomainTick> {
        if self.options.label_collision == ChartLabelCollisionPolicy::Hide {
            return Vec::new();
        }
        let mut ticks = self.domain_ticks(self.options.max_domain_labels);
        if let ChartLabelCollisionPolicy::Truncate { max_chars } = self.options.label_collision {
            for tick in &mut ticks {
                tick.label = SharedString::from(truncate_label(&tick.label, max_chars));
            }
        }
        ticks
    }

    /// Exports the chart data as comma-separated values.
    #[must_use]
    pub fn to_csv(&self) -> String {
        if self.kind == ChartKind::Candlestick {
            return candle::csv(self);
        }
        let mut rows = Vec::new();
        let mut header = Vec::with_capacity(self.datasets.len() + 1);
        header.push("label".to_string());
        header.extend(
            self.datasets
                .iter()
                .map(|dataset| escape_csv(&dataset.label)),
        );
        rows.push(header.join(","));

        for point_index in self.visible_point_indices() {
            let label = self
                .datasets
                .iter()
                .find_map(|dataset| dataset.points.get(point_index))
                .map(|point| escape_csv(&point.display_label()))
                .unwrap_or_default();
            let mut row = Vec::with_capacity(self.datasets.len() + 1);
            row.push(label);
            row.extend(self.datasets.iter().map(|dataset| {
                dataset
                    .points
                    .get(point_index)
                    .map(|point| point.value.to_string())
                    .unwrap_or_default()
            }));
            rows.push(row.join(","));
        }
        rows.join("\n")
    }

    /// Exports a dependency-free SVG snapshot of the current visible domain.
    ///
    /// The SVG contains an accessible title and the visible chart geometry. Applications can write the returned UTF-8 string directly to
    /// an `.svg` file or hand it to an image conversion pipeline.
    #[must_use]
    pub fn to_svg(&self, width: u32, height: u32) -> String {
        let width = width.max(1);
        let height = height.max(1);
        if self.kind == ChartKind::Candlestick {
            return candle::svg(self, width, height);
        }
        if self.kind == ChartKind::Radar {
            return radar::svg(self, width, height);
        }
        svg::snapshot(self, width, height)
    }

    fn max_points(&self) -> usize {
        self.datasets
            .iter()
            .map(|dataset| dataset.points.len())
            .max()
            .unwrap_or(0)
    }

    fn visible_point_indices(&self) -> Vec<usize> {
        self.visible_index_range()
            .map(|range| range.collect())
            .unwrap_or_default()
    }

    fn visible_index_range(&self) -> Option<std::ops::RangeInclusive<usize>> {
        let max_points = self.max_points();
        if max_points == 0 {
            return None;
        }
        let (start, end) = self.options.domain.map_or((0, max_points - 1), |domain| {
            (
                domain.min.floor().max(0.0) as usize,
                domain.max.ceil().max(domain.min).max(0.0) as usize,
            )
        });
        let end = end.min(max_points - 1);
        (start <= end).then_some(start..=end)
    }

    fn pie_hit_test(&self, x: f32, y: f32, width: f32, height: f32) -> Option<ChartHit> {
        let cx = width / 2.0;
        let cy = height / 2.0;
        let dx = x - cx;
        let dy = y - cy;
        let radius = width.min(height) / 2.0;
        let distance = dx.hypot(dy);
        let inner_radius = if self.kind == ChartKind::Doughnut {
            radius * self.options.doughnut_cutout
        } else {
            0.0
        };
        if distance > radius || distance < inner_radius {
            return None;
        }
        let mut angle = dy.atan2(dx);
        if angle < -std::f32::consts::FRAC_PI_2 {
            angle += std::f32::consts::TAU;
        }
        let normalized = (angle + std::f32::consts::FRAC_PI_2) / std::f32::consts::TAU;
        self.radial_slices()
            .find(|(_, _, end)| normalized <= *end)
            .map(|(index, _, _)| ChartHit {
                dataset_index: 0,
                point_index: index,
                label: self.datasets[0].points[index].display_label(),
            })
    }

    fn radial_slices(&self) -> impl Iterator<Item = (usize, f32, f32)> + '_ {
        let points = self
            .datasets
            .first()
            .map(|d| d.points.as_slice())
            .unwrap_or_default();
        let positive = points
            .iter()
            .enumerate()
            .filter(|(_, p)| p.value.is_finite() && p.value > 0.);
        let largest = positive.clone().map(|(_, p)| p.value).fold(0., f64::max);
        let total: f64 = positive.clone().map(|(_, p)| p.value / largest).sum();
        let mut sum = 0.;
        positive.map(move |(i, p)| {
            let start = sum / total;
            sum += p.value / largest;
            (i, start as f32, (sum / total) as f32)
        })
    }

    fn rebuild_derived_data(&mut self) {
        self.mixed = matches!(
            self.kind,
            ChartKind::Line
                | ChartKind::Bar
                | ChartKind::Area
                | ChartKind::Bubble
                | ChartKind::Scatter
        ) && self
            .datasets
            .iter()
            .any(|dataset| dataset.kind.is_some_and(|kind| kind != self.kind));
        self.mixed_bar_indices.clear();
        if self.mixed {
            self.mixed_bar_indices.extend(
                self.datasets.iter().enumerate().filter_map(|(i, d)| {
                    (d.kind.unwrap_or(self.kind) == ChartKind::Bar).then_some(i)
                }),
            );
        }
        let mut value_min = 0.0_f64;
        let mut value_max = 1.0_f64;
        let mut smallest_positive = f64::INFINITY;
        self.visible_axis.take();
        self.domain_order.take();
        self.dataset_axis_cache = self
            .datasets
            .iter()
            .map(|dataset| {
                let mut min = 0.0_f64;
                let mut max = 1.0_f64;
                for point in &dataset.points {
                    if !point.value.is_finite() {
                        continue;
                    }
                    min = min.min(point.value);
                    max = max.max(point.value);
                    value_min = value_min.min(point.value);
                    value_max = value_max.max(point.value);
                    if point.value > 0.0 {
                        smallest_positive = smallest_positive.min(point.value);
                    }
                }
                ChartAxis::new(min, max)
            })
            .collect();
        self.value_axis_cache = if self.kind == ChartKind::Candlestick {
            let mut combined: Option<(f64, f64)> = None;
            for (i, dataset) in self.datasets.iter().enumerate() {
                if let Some((low, high)) = candle::extent(dataset) {
                    self.dataset_axis_cache[i] = ChartAxis::new(low, high);
                    combined =
                        Some(combined.map_or((low, high), |(a, b)| (low.min(a), high.max(b))));
                }
            }
            combined.map_or(ChartAxis::new(0., 1.), |(a, b)| ChartAxis::new(a, b))
        } else {
            ChartAxis::new(value_min, value_max)
        };
        self.smallest_positive_cache = if smallest_positive.is_finite() {
            smallest_positive.min(1.0)
        } else {
            1.0
        };

        if self.datasets.len() <= 1 {
            self.stacked_axis_cache = ChartAxis::new(value_min, value_max);
            return;
        }

        let max_points = self.max_points();
        let mut min = 0.0;
        let mut max = 1.0;
        for point_index in 0..max_points {
            let mut positive = 0.0;
            let mut negative = 0.0;
            for dataset in &self.datasets {
                let value = dataset
                    .points
                    .get(point_index)
                    .map(ChartPoint::value)
                    .unwrap_or_default();
                if !value.is_finite() {
                    continue;
                }
                if value >= 0.0 {
                    positive += value;
                } else {
                    negative += value;
                }
            }
            min = f64::min(min, negative);
            max = f64::max(max, positive);
        }
        self.stacked_axis_cache = ChartAxis::new(min, max);
    }

    fn stacked_value_at(&self, dataset_index: usize, point_index: usize) -> f64 {
        let Some(point) = self
            .datasets
            .get(dataset_index)
            .and_then(|dataset| dataset.points.get(point_index))
        else {
            return 0.0;
        };
        let mut total = 0.0;
        for dataset in self.datasets.iter().take(dataset_index + 1) {
            let value = dataset
                .points
                .get(point_index)
                .map(ChartPoint::value)
                .unwrap_or_default();
            if !value.is_finite() {
                continue;
            }
            if (value >= 0.0) == (point.value >= 0.0) {
                total += value;
            }
        }
        total
    }

    fn scaled_axis(&self) -> ChartAxis {
        let axis = self.value_axis();
        match self.options.scale {
            ChartScale::Linear => axis,
            ChartScale::Log10 => {
                let min = positive_log_min(axis.min, self);
                ChartAxis::new(min.log10(), axis.max.max(min).log10())
            }
        }
    }

    fn scale_value(&self, value: f64) -> f64 {
        match self.options.scale {
            ChartScale::Linear => value,
            ChartScale::Log10 => value
                .max(positive_log_min(self.value_axis().min, self))
                .log10(),
        }
    }

    fn is_stacked(&self) -> bool {
        self.options.stacked && !self.mixed
    }

    fn bar_lane(&self, dataset: usize) -> usize {
        if self.mixed {
            self.mixed_bar_indices.partition_point(|i| *i < dataset)
        } else {
            dataset
        }
    }

    fn bar_lane_count(&self) -> usize {
        if self.mixed {
            self.mixed_bar_indices.len().max(1)
        } else {
            self.datasets.len().max(1)
        }
    }

    fn is_radial(&self) -> bool {
        self.kind == ChartKind::Pie || self.kind == ChartKind::Doughnut
    }
}

macro_rules! chart_component {
    ($name:ident, $kind:expr) => {
        #[doc = concat!("A ", stringify!($name), " component.")]
        #[derive(gpui::IntoElement)]
        pub struct $name {
            id: SharedString,
            series: ChartSeries,
            on_hover: Option<HoverHandler>,
            on_select: Option<SelectHandler>,
            on_interaction: Option<InteractionHandler>,
            overlay: Option<OverlayRenderer>,
    tooltip: Option<TooltipRenderer>,
    layer: Option<PlotLayer>,
        }

        impl $name {
            /// Creates an empty chart.
            #[must_use]
            pub fn new(id: impl Into<SharedString>) -> Self {
                Self {
                    id: id.into(),
                    series: ChartSeries::new($kind),
                    on_hover: None,
                    on_select: None,
                    on_interaction: None,
                    overlay: None,
                    tooltip: None,
                    layer: None,
                }
            }

            /// Replaces datasets.
            #[must_use]
            pub fn datasets(mut self, datasets: Vec<ChartDataset>) -> Self {
                self.series.datasets = datasets;
                self.series.rebuild_derived_data();
                self
            }

            /// Replaces chart options.
            #[must_use]
            pub fn options(mut self, options: ChartOptions) -> Self {
                self.series = self.series.options(options);
                self
            }

            /// Returns the underlying chart model.
            #[must_use]
            pub fn series(&self) -> &ChartSeries {
                &self.series
            }

            /// Registers a host-managed hover hit callback.
            ///
            /// The callback receives `Some(hit)` while the pointer is over a
            /// chart datum and `None` when the pointer leaves the chart.
            #[must_use]
            pub fn on_hover(
                mut self,
                handler: impl Fn(&Option<ChartHit>, &mut Window, &mut App) + 'static,
            ) -> Self {
                self.on_hover = Some(Rc::new(handler));
                self
            }

            /// Registers a selection callback distinct from transient hover.
            /// Pointer activation reports the nearest datum, or `None` when
            /// the plot background was activated.
            #[must_use]
            pub fn on_select(
                mut self,
                handler: impl Fn(&Option<ChartHit>, &mut Window, &mut App) + 'static,
            ) -> Self {
                self.on_select = Some(Rc::new(handler));
                self
            }

            /// Registers a controlled viewport/selection command callback and
            /// enables the built-in chart controls.
            #[must_use]
            pub fn on_interaction(
                mut self,
                handler: impl Fn(&ChartInteractionCommand, &mut Window, &mut App) + 'static,
            ) -> Self {
                self.on_interaction = Some(Rc::new(handler));
                self
            }

            /// Paints custom annotations above the series in the plot's coordinate system.
            #[must_use]
            pub fn plot_layer(mut self, paint: impl Fn(&ChartSeries, Bounds<Pixels>, &mut Window, &mut App) + 'static) -> Self {
                self.layer = Some(Rc::new(paint));
                self
            }

            /// Replaces the default hover panel with an application-defined element.
            /// The chart positions the element near the pointer and bounds it to the window.
            #[must_use]
            pub fn tooltip_content<E: IntoElement>(
                mut self,
                renderer: impl Fn(&ChartSeries, &ChartHit, &mut Window, &mut App) -> E + 'static,
            ) -> Self {
                self.tooltip = Some(Rc::new(move |series, hit, window, cx| {
                    renderer(series, hit, window, cx).into_any_element()
                }));
                self
            }

            /// Adds application-defined content above the chart chrome.
            ///
            /// This hook supports annotations, branded controls, and plugin
            /// surfaces without giving extensions access to renderer internals.
            #[must_use]
            pub fn overlay(
                mut self,
                renderer: impl Fn(&ChartSeries, &mut Window, &mut App) -> gpui::AnyElement
                + 'static,
            ) -> Self {
                self.overlay = Some(Rc::new(renderer));
                self
            }
        }

        impl RenderOnce for $name {
            fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
                render_chart(
                    self.id,
                    self.series,
                    ChartHandlers {
                        hover: self.on_hover,
                        select: self.on_select,
                        interaction: self.on_interaction,
                        overlay: self.overlay,
                        tooltip: self.tooltip,
                        layer: self.layer,
                    },
                    window,
                    cx,
                )
            }
        }
    };
}

chart_component!(LineChart, ChartKind::Line);
chart_component!(BarChart, ChartKind::Bar);
chart_component!(HorizontalBarChart, ChartKind::HorizontalBar);
chart_component!(AreaChart, ChartKind::Area);
chart_component!(ScatterChart, ChartKind::Scatter);
chart_component!(BubbleChart, ChartKind::Bubble);
chart_component!(CandlestickChart, ChartKind::Candlestick);
chart_component!(RadarChart, ChartKind::Radar);
chart_component!(PieChart, ChartKind::Pie);
chart_component!(DoughnutChart, ChartKind::Doughnut);
chart_component!(MixedChart, ChartKind::Line);

fn render_chart(
    id: SharedString,
    series: ChartSeries,
    handlers: ChartHandlers,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = Theme::global(cx).clone();
    let accessible_label = series
        .options
        .title
        .clone()
        .unwrap_or_else(|| SharedString::from("Chart"));
    let accessible_description = bounded_accessible_description(&series, 20);
    let mut root = div()
        .id(id.clone())
        .accessible(
            AccessibilityProps::new(Role::Group)
                .label(accessible_label)
                .description(accessible_description),
        )
        .w_full()
        .rounded(px(theme.radius.lg))
        .border_1()
        .border_color(theme.border())
        .bg(theme.background())
        .p_3()
        .flex()
        .flex_col()
        .gap_3();

    if let Some(title) = series.options.title.clone() {
        root = root.child(
            div()
                .text_color(theme.foreground())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        );
    }

    if let Some(overlay) = handlers.overlay.clone() {
        root = root.child(overlay(&series, window, cx));
    }

    if let Some(handler) = handlers.interaction.clone() {
        root = root.child(render_interaction_controls(&id, &series, handler, &theme));
    }

    if series
        .datasets
        .iter()
        .all(|dataset| dataset.points.is_empty())
    {
        return root
            .child(render_empty_state(&series, &theme))
            .into_any_element();
    }

    let series = Rc::new(series);
    let plot_series = series.clone();
    let plot_bounds = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
    let runtime = window.use_keyed_state(format!("{id}-hover"), cx, |_, _| {
        ChartTooltipRuntime::default()
    });
    let tooltip_series = series.clone();
    let builder_runtime = runtime.clone();
    let tooltip_enabled = series.options.show_tooltip;
    let layout_tooltip = handlers.tooltip.clone();
    let tooltip_builder = move |_window: &mut Window, cx: &mut App| {
        let active_hit = builder_runtime.read(cx).hit.clone();
        let view = cx.new(|_| ChartTooltip {
            series: tooltip_series.clone(),
            content: handlers.tooltip.clone(),
            active_hit,
        });
        builder_runtime.update(cx, |state, _| state.view = Some(view.downgrade()));
        view.into()
    };
    let layout_runtime = runtime.clone();
    let layout_series = series.clone();
    let canvas_plot_bounds = plot_bounds.clone();
    let mut plot = div()
        .id(format!("{id}-plot"))
        .debug_selector(|| "guic-chart-plot".to_owned())
        .relative()
        .w_full()
        .h(px(series.options.height))
        .flex_shrink_0()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(theme.border())
        .bg(theme.secondary().opacity(0.12))
        .overflow_hidden()
        .child(
            canvas(
                move |bounds, window, cx| {
                    let bounds = plot_bounds_for(bounds, &layout_series);
                    let previous = canvas_plot_bounds.replace(Some(bounds));
                    let view = layout_runtime
                        .read(cx)
                        .view
                        .as_ref()
                        .and_then(WeakEntity::upgrade);
                    if let Some(view) = view
                        && (previous != Some(bounds)
                            || !Rc::ptr_eq(&view.read(cx).series, &layout_series))
                    {
                        let p = window.mouse_position() - bounds.origin;
                        let hit = layout_series.hit_test(
                            p.x.into(),
                            p.y.into(),
                            bounds.size.width.into(),
                            bounds.size.height.into(),
                        );
                        layout_runtime.update(cx, |state, _| state.hit = hit.clone());
                        view.update(cx, |tooltip, _| {
                            tooltip.series = layout_series.clone();
                            tooltip.content = layout_tooltip.clone();
                            tooltip.active_hit = hit;
                        });
                    }
                },
                move |bounds, _, window, cx| {
                    paint_chart(bounds, &plot_series, window, cx);
                    if let Some(layer) = &handlers.layer {
                        let plot = plot_bounds_for(bounds, &plot_series);
                        if plot.size.width > px(0.) && plot.size.height > px(0.) {
                            window.with_content_mask(
                                Some(gpui::ContentMask { bounds: plot }),
                                |window| {
                                    layer(&plot_series, plot, window, cx);
                                },
                            );
                        }
                    }
                },
            )
            .absolute()
            .size_full(),
        );
    if let Some(handler) = handlers.interaction {
        let domain = series
            .options
            .domain
            .unwrap_or_else(|| ChartAxis::new(0.0, series.max_points().saturating_sub(1) as f64));
        plot = plot.tab_index(0).key_context("GuicChart").on_key_down(
            move |event: &KeyDownEvent, window, cx| {
                if let Some(command) = chart_key_command(&event.keystroke.key, domain) {
                    handler(&command, window, cx);
                    cx.stop_propagation();
                }
            },
        );
    }
    if handlers.hover.is_some() || series.options.show_tooltip {
        let hover_series = series.clone();
        let hover_bounds = plot_bounds.clone();
        let hover_handler = handlers.hover.clone();
        let leave_hover_handler = handlers.hover.clone();
        let move_runtime = runtime.clone();
        let leave_runtime = runtime;
        plot = plot
            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                let hit = hover_bounds.borrow().and_then(|bounds| {
                    let local_x = f32::from(event.position.x - bounds.origin.x);
                    let local_y = f32::from(event.position.y - bounds.origin.y);
                    hover_series.hit_test(
                        local_x,
                        local_y,
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    )
                });
                let (changed, tooltip_view) = move_runtime.update(cx, |state, _| {
                    let changed = state.hit != hit;
                    state.hit = hit.clone();
                    (changed, state.view.as_ref().and_then(WeakEntity::upgrade))
                });
                if let Some(tooltip) = tooltip_view {
                    tooltip.update(cx, |tooltip, cx| {
                        if tooltip.active_hit != hit {
                            tooltip.active_hit = hit.clone();
                            cx.notify();
                        }
                    });
                }
                if changed && let Some(handler) = &hover_handler {
                    handler(&hit, window, cx);
                }
            })
            .on_mouse_exit(move |_, window, cx| {
                let tooltip_view = leave_runtime.update(cx, |state, _| {
                    state.hit = None;
                    state.view.as_ref().and_then(WeakEntity::upgrade)
                });
                if let Some(tooltip) = tooltip_view {
                    tooltip.update(cx, |tooltip, cx| {
                        if tooltip.active_hit.take().is_some() {
                            cx.notify();
                        }
                    });
                }
                if let Some(handler) = &leave_hover_handler {
                    handler(&None, window, cx);
                }
            });
    }
    if tooltip_enabled {
        plot = plot
            .tooltip_show_delay(Duration::ZERO)
            .cursor_tooltip(tooltip_builder);
    }
    if let Some(on_select) = handlers.select {
        let select_series = series.clone();
        let select_bounds = plot_bounds.clone();
        plot = plot.on_click(move |event: &ClickEvent, window, cx| {
            let hit = select_bounds.borrow().and_then(|bounds| {
                let position = event.position();
                let local_x = f32::from(position.x - bounds.origin.x);
                let local_y = f32::from(position.y - bounds.origin.y);
                select_series.hit_test(
                    local_x,
                    local_y,
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                )
            });
            on_select(&hit, window, cx);
        });
    }
    root = root.child(plot);

    if series.options.show_legend {
        root = root.child(render_legend(&series, &theme));
    }
    if series.options.show_values {
        root = root.child(render_value_summary(&series, &theme));
    }

    root.into_any_element()
}

fn render_interaction_controls(
    id: &SharedString,
    series: &ChartSeries,
    handler: InteractionHandler,
    theme: &Theme,
) -> gpui::AnyElement {
    let domain = series
        .options
        .domain
        .unwrap_or_else(|| ChartAxis::new(0.0, series.max_points().saturating_sub(1) as f64));
    let center = domain.min + domain.span() / 2.0;
    let pan_step = (domain.span() * 0.1).max(1.0);
    let commands = [
        ("Pan left", ChartInteractionCommand::Pan(-pan_step)),
        (
            "Zoom in",
            ChartInteractionCommand::Zoom {
                center,
                factor: 1.25,
            },
        ),
        (
            "Zoom out",
            ChartInteractionCommand::Zoom {
                center,
                factor: 0.8,
            },
        ),
        ("Pan right", ChartInteractionCommand::Pan(pan_step)),
        ("Reset", ChartInteractionCommand::ResetView),
    ];

    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(
            commands
                .into_iter()
                .enumerate()
                .map(|(index, (label, command))| {
                    let handler = handler.clone();
                    div()
                        .id(format!("{id}-control-{index}"))
                        .accessible(AccessibilityProps::new(Role::Button).label(label))
                        .tab_index(0)
                        .key_context("GuicChartControl")
                        .px_2()
                        .py_1()
                        .rounded(px(theme.radius.sm))
                        .border_1()
                        .border_color(theme.border())
                        .bg(theme.secondary().opacity(0.18))
                        .text_color(theme.foreground())
                        .text_size(px(theme.typography.text_sm))
                        .cursor_pointer()
                        .on_key_down({
                            let handler = handler.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    handler(&command, window, cx);
                                    cx.stop_propagation();
                                }
                            }
                        })
                        .on_click(move |_, window, cx| handler(&command, window, cx))
                        .child(label)
                }),
        )
        .into_any_element()
}

fn chart_key_command(key: &str, domain: ChartAxis) -> Option<ChartInteractionCommand> {
    let center = domain.min + domain.span() / 2.0;
    match key {
        "left" => Some(ChartInteractionCommand::PreviousPoint),
        "right" => Some(ChartInteractionCommand::NextPoint),
        "home" => Some(ChartInteractionCommand::FirstVisiblePoint),
        "end" => Some(ChartInteractionCommand::LastPoint),
        "+" | "=" => Some(ChartInteractionCommand::Zoom {
            center,
            factor: 1.25,
        }),
        "-" => Some(ChartInteractionCommand::Zoom {
            center,
            factor: 0.8,
        }),
        "0" => Some(ChartInteractionCommand::ResetView),
        _ => None,
    }
}

fn bounded_accessible_description(series: &ChartSeries, limit: usize) -> String {
    let total = if series.kind == ChartKind::Pie || series.kind == ChartKind::Doughnut {
        series
            .datasets
            .first()
            .map_or(0, |dataset| dataset.points.len())
    } else {
        series
            .datasets
            .iter()
            .map(|dataset| dataset.points.len())
            .sum()
    };
    let mut description = chart_value_rows_bounded(series, limit)
        .into_iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("; ");
    if total > limit {
        description.push_str(&format!("; and {} more values", total - limit));
    }
    if description.is_empty() {
        description.push_str("No data");
    }
    description
}

struct ChartTooltip {
    content: Option<TooltipRenderer>,
    series: Rc<ChartSeries>,
    active_hit: Option<ChartHit>,
}

impl Render for ChartTooltip {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        if let Some(content) = &self.content {
            let Some(hit) = &self.active_hit else {
                return div().hidden();
            };
            let viewport = window.viewport_size();
            return div()
                .debug_selector(|| "guic-chart-tooltip".to_owned())
                .max_w((viewport.width - px(12.)).max(px(0.)))
                .max_h((viewport.height - px(12.)).max(px(0.)))
                .overflow_hidden()
                .child(content(&self.series, hit, window, cx));
        }
        let theme = Theme::global(cx);
        let rows = chart_tooltip_rows_for_hit(&self.series, self.active_hit.as_ref());
        if rows.is_empty() {
            return div().hidden();
        }
        let candle_color = self
            .active_hit
            .as_ref()
            .filter(|_| self.series.kind == ChartKind::Candlestick)
            .map(|hit| {
                let dataset = &self.series.datasets[hit.dataset_index];
                let prices = candle::prices(dataset, hit.point_index);
                candle::color(dataset, prices.close() >= prices.open(), theme)
            });
        let title = chart_tooltip_title(&self.series, self.active_hit.as_ref());
        let viewport = window.viewport_size();
        let width = px(220.).min((viewport.width - px(12.)).max(px(0.)));
        let pointer = window.mouse_position();
        let mut max_height = (viewport.height - px(12.)).max(px(0.));
        let row_height = px((theme.typography.text_sm * 1.4).ceil());
        let gap = px(4.);
        // Reserve a clear side of the pointer on narrow windows whenever the
        // heading and at least one value can be shown there.
        let clearance = px(18.); // 12px pointer gap plus 6px window inset.
        if pointer.x.max(viewport.width - pointer.x) < width + clearance {
            let available = pointer.y.max(viewport.height - pointer.y) - clearance;
            if available >= px(18.) + row_height * 2. + gap {
                max_height = max_height.min(available);
            }
        }
        // The panel has two 1px borders, 8px vertical padding, and one title row.
        let capacity = ((max_height - px(18.) - row_height) / (row_height + gap))
            .floor()
            .max(0.) as usize;
        let mut heading = div()
            .flex()
            .items_center()
            .gap_2()
            .h(row_height)
            .flex_shrink_0()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(div().min_w_0().flex_1().truncate().child(title));
        if rows.len() > capacity {
            heading = heading.child(
                div()
                    .debug_selector(|| "guic-chart-tooltip-more".to_owned())
                    .flex_shrink_0()
                    .text_color(theme.muted_foreground())
                    .child(format!("+{}", rows.len() - capacity)),
            );
        }
        div()
            .debug_selector(|| "guic-chart-tooltip".to_owned())
            .w(width)
            .max_h(max_height)
            .overflow_hidden()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background().opacity(0.98))
            .shadow_lg()
            .text_color(theme.foreground())
            .px(px(10.))
            .py(px(8.))
            .flex()
            .flex_col()
            .gap(gap)
            .text_size(px(theme.typography.text_sm))
            .line_height(row_height)
            .child(heading)
            .children(rows.into_iter().take(capacity).map(|row| {
                div()
                    .flex()
                    .h(row_height)
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .flex_shrink_0()
                            .rounded(px(4.))
                            .bg(row
                                .color
                                .or(candle_color)
                                .unwrap_or_else(|| palette(row.color_index, theme))),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(row.label))
                    .child(
                        div()
                            .min_w_0()
                            .max_w(gpui::relative(0.6))
                            .truncate()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(row.value),
                    )
            }))
    }
}

fn chart_tooltip_title(series: &ChartSeries, hit: Option<&ChartHit>) -> SharedString {
    let Some(hit) = hit else {
        return SharedString::default();
    };
    if series.kind == ChartKind::Candlestick
        && series.datasets.len() > 1
        && let Some(dataset) = series.datasets.get(hit.dataset_index)
    {
        return format!("{} · {}", dataset.label, hit.label).into();
    }
    if series.options.tooltip_mode == ChartTooltipMode::Dataset && !series.is_radial() {
        return series
            .datasets
            .get(hit.dataset_index)
            .map(|dataset| dataset.label.clone())
            .unwrap_or_else(|| hit.label.clone());
    }
    hit.label.clone()
}

fn render_value_summary(series: &ChartSeries, theme: &Theme) -> gpui::AnyElement {
    let mut rows = div()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(theme.border())
        .bg(theme.secondary().opacity(0.08))
        .p_2()
        .grid()
        .grid_cols(2)
        .gap_2()
        .text_size(px(theme.typography.text_sm));
    for (label, value) in chart_value_rows(series) {
        rows = rows.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .text_color(theme.muted_foreground())
                .child(label)
                .child(div().text_color(theme.foreground()).child(value)),
        );
    }
    rows.into_any_element()
}

fn render_empty_state(series: &ChartSeries, theme: &Theme) -> gpui::AnyElement {
    div()
        .h(px(series.options.height))
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(theme.border())
        .bg(theme.secondary().opacity(0.08))
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.muted_foreground())
        .child(
            series
                .options
                .empty_message
                .clone()
                .unwrap_or_else(|| SharedString::from("No data")),
        )
        .into_any_element()
}

#[cfg(test)]
fn chart_tooltip_rows(series: &ChartSeries) -> Vec<(String, String)> {
    chart_tooltip_rows_for_hit(series, series.options.active_hit.as_ref())
        .into_iter()
        .map(|row| (row.label.to_string(), row.value))
        .collect()
}

struct ChartTooltipRow {
    label: SharedString,
    value: String,
    color_index: usize,
    color: Option<Hsla>,
}

fn chart_tooltip_rows_for_hit(
    series: &ChartSeries,
    hit: Option<&ChartHit>,
) -> Vec<ChartTooltipRow> {
    let Some(hit) = hit else { return Vec::new() };
    let Some(dataset) = series.datasets.get(hit.dataset_index) else {
        return Vec::new();
    };
    let Some(point) = dataset.points.get(hit.point_index) else {
        return Vec::new();
    };
    let make_row = |label, value, color_index, color| ChartTooltipRow {
        label,
        value: series.options.value_formatter.format(value),
        color_index,
        color,
    };
    if series.kind == ChartKind::Candlestick {
        return candle::prices(dataset, hit.point_index)
            .fields()
            .into_iter()
            .map(|(name, value)| make_row(name.into(), value, hit.dataset_index, dataset.color))
            .collect();
    }
    if series.is_radial() {
        return vec![make_row(
            dataset.label.clone(),
            point.value,
            hit.point_index,
            None,
        )];
    }
    match series.options.tooltip_mode {
        ChartTooltipMode::Nearest => vec![make_row(
            dataset.label.clone(),
            point.value,
            hit.dataset_index,
            dataset.color,
        )],
        ChartTooltipMode::Index => series
            .datasets
            .iter()
            .enumerate()
            .filter_map(|(index, dataset)| {
                let point = dataset.points.get(hit.point_index)?;
                Some(make_row(
                    dataset.label.clone(),
                    point.value,
                    index,
                    dataset.color,
                ))
            })
            .take(series.options.tooltip_max_rows)
            .collect(),
        ChartTooltipMode::Dataset => series
            .visible_index_range()
            .into_iter()
            .flatten()
            .take_while(|&index| index < dataset.points.len())
            .take(series.options.tooltip_max_rows)
            .map(|index| {
                let point = &dataset.points[index];
                make_row(
                    point.display_label(),
                    point.value,
                    hit.dataset_index,
                    dataset.color,
                )
            })
            .collect(),
    }
}

fn chart_value_rows_bounded(series: &ChartSeries, limit: usize) -> Vec<(String, String)> {
    if limit == 0 {
        return Vec::new();
    }
    if series.kind == ChartKind::Candlestick {
        return series
            .datasets
            .iter()
            .flat_map(|dataset| {
                dataset.points.iter().enumerate().map(move |(index, p)| {
                    let values = candle::prices(dataset, index)
                        .fields()
                        .map(|(name, value)| {
                            format!("{name} {}", series.options.value_formatter.format(value))
                        });
                    (
                        format!("{} · {}", dataset.label, p.display_label()),
                        values.join(", "),
                    )
                })
            })
            .take(limit)
            .collect();
    }
    if series.kind == ChartKind::Pie || series.kind == ChartKind::Doughnut {
        return series
            .datasets
            .first()
            .map(|dataset| {
                dataset
                    .points
                    .iter()
                    .take(limit)
                    .map(|point| {
                        (
                            point.display_label().to_string(),
                            series.options.value_formatter.format(point.value),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    series
        .datasets
        .iter()
        .flat_map(|dataset| {
            dataset.points.iter().map(move |point| {
                (
                    format!("{} · {}", dataset.label, point.display_label()),
                    series.options.value_formatter.format(point.value),
                )
            })
        })
        .take(limit)
        .collect()
}

fn chart_value_rows(series: &ChartSeries) -> Vec<(String, String)> {
    chart_value_rows_bounded(series, usize::MAX)
}

fn format_value(value: f64) -> String {
    if value.fract() == 0.
        && value.abs() <= (1u64 << 53) as f64
        && value.to_bits() != (-0.0f64).to_bits()
    {
        return (value as i64).to_string();
    }
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn format_domain_value(value: &ChartDomainValue, formatter: ChartDomainFormatter) -> SharedString {
    if let ChartDomainFormatter::Custom(formatter) = formatter {
        return formatter(value);
    }
    match value {
        ChartDomainValue::Category(label) => label.clone(),
        ChartDomainValue::Number(value) => SharedString::from(format_value(*value)),
        ChartDomainValue::Timestamp(millis) => SharedString::from(match formatter {
            ChartDomainFormatter::UnixMillis => millis.to_string(),
            ChartDomainFormatter::UnixSeconds => millis.div_euclid(1_000).to_string(),
            ChartDomainFormatter::IsoDate => format_unix_millis(*millis, false),
            ChartDomainFormatter::Auto | ChartDomainFormatter::IsoDateTime => {
                format_unix_millis(*millis, true)
            }
            ChartDomainFormatter::Custom(_) => unreachable!("custom formatter returned above"),
        }),
    }
}

fn format_unix_millis(unix_millis: i64, include_time: bool) -> String {
    let seconds = unix_millis.div_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_unix_days(days);
    if !include_time {
        return format!("{year:04}-{month:02}-{day:02}");
    }
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn civil_date_from_unix_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn render_legend(series: &ChartSeries, theme: &Theme) -> gpui::AnyElement {
    let mut legend = div().flex().flex_wrap().gap_3();
    let labels: Vec<(SharedString, Hsla)> = if series.kind == ChartKind::Candlestick
        && series.datasets.iter().all(|d| d.color.is_none())
    {
        vec![
            ("Close ≥ open".into(), theme.success()),
            ("Close < open".into(), theme.danger()),
        ]
    } else if series.kind == ChartKind::Pie || series.kind == ChartKind::Doughnut {
        series
            .datasets
            .first()
            .map(|dataset| {
                dataset
                    .points
                    .iter()
                    .enumerate()
                    .map(|(index, point)| (point.display_label(), palette(index, theme)))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        series
            .datasets
            .iter()
            .enumerate()
            .map(|(index, dataset)| {
                (
                    dataset.label.clone(),
                    dataset.color.unwrap_or_else(|| palette(index, theme)),
                )
            })
            .collect()
    };

    for (label, color) in labels {
        legend = legend.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(theme.muted_foreground())
                .child(div().w(px(10.0)).h(px(10.0)).rounded(px(5.0)).bg(color))
                .child(label),
        );
    }
    legend.into_any_element()
}

fn paint_chart(bounds: Bounds<Pixels>, series: &ChartSeries, window: &mut Window, cx: &mut App) {
    let theme = Theme::global(cx);
    let plot = plot_bounds_for(bounds, series);
    if plot.size.width <= px(0.) || plot.size.height <= px(0.) {
        return;
    }

    if series.kind == ChartKind::Radar {
        radar::paint(plot, series, window, cx);
        return;
    }
    if series.options.show_grid && !series.is_radial() {
        paint_grid(plot, window, theme.border().opacity(0.6));
    }
    if series.options.show_axes && !series.is_radial() {
        paint_axes(plot, window, theme.border());
    }

    if series.mixed {
        for (index, dataset) in series.datasets.iter().enumerate() {
            let kind = dataset.kind.unwrap_or(series.kind);
            paint_cartesian(plot, series, kind, index..index + 1, window, theme);
        }
    } else {
        paint_chart_kind(plot, series, window, theme);
    }
    if !series.is_radial() && series.kind != ChartKind::Candlestick {
        paint_crosshair(plot, series, window, theme);
    }
    if series.options.show_axes && !series.is_radial() {
        paint_domain_labels(bounds, plot, series, window, cx);
    }
}

fn paint_domain_labels(
    bounds: Bounds<Pixels>,
    plot: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(range) = series.visible_index_range() else {
        return;
    };
    let first = *range.start();
    let count = range.count();
    let theme = Theme::global(cx);
    let color = theme.muted_foreground();
    let font = px(theme.typography.text_sm);
    let horizontal = series.kind == ChartKind::HorizontalBar;
    if !horizontal && series.options.value_axes.is_empty() {
        let axis = series.scaled_axis();
        let levels = (f32::from(plot.size.height) / 38.).clamp(1., 4.) as usize;
        for level in 0..=levels {
            let t = level as f64 / levels as f64;
            let v = axis.min * (1. - t) + axis.max * t;
            let v = if series.options.scale == ChartScale::Log10 {
                10f64.powf(v)
            } else {
                v
            };
            let label: SharedString = series.options.value_formatter.format(v).into();
            let mut run = window.text_style().to_run(label.len());
            run.color = color;
            let line = window.text_system().shape_line(label, font, &[run], None);
            let x = plot.left() - line.width - px(4.);
            if x < bounds.left() {
                continue;
            }
            let _ = line.paint(
                point(x, plot.bottom() - plot.size.height * t as f32 - font / 2.),
                font,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
    let mut labels = series.layout_domain_labels();
    // User-supplied numeric coordinates can arrive out of order.
    let position = |tick: &ChartDomainTick| {
        let index = tick.point_index - first;
        match series.kind {
            ChartKind::Bar | ChartKind::Candlestick | ChartKind::HorizontalBar => {
                (index as f32 + 0.5) / count as f32
            }
            _ if matches!(
                series.datasets[0].points[tick.point_index].domain,
                ChartDomainValue::Category(_)
            ) =>
            {
                category_position(index, count)
            }
            _ => series.visible_domain_axis().normalize(tick.coordinate) as f32,
        }
    };
    labels.sort_by(|a, b| position(a).total_cmp(&position(b)));
    let mut previous = None;
    for tick in labels {
        let t = position(&tick);
        let mut run = window.text_style().to_run(tick.label.len());
        run.color = color;
        let line = window
            .text_system()
            .shape_line(tick.label, font, &[run], None);
        let (x, y) = if horizontal {
            (
                plot.left() - px(4.) - line.width,
                plot.top() + plot.size.height * t - font / 2.,
            )
        } else {
            (
                (plot.left() + plot.size.width * t - line.width / 2.).clamp(
                    bounds.left(),
                    (bounds.right() - line.width).max(bounds.left()),
                ),
                plot.bottom() + px(4.),
            )
        };
        let (start, end) = if horizontal {
            (y, y + font)
        } else {
            (x, x + line.width)
        };
        if previous.is_some_and(|edge| start < edge + px(3.)) {
            continue;
        }
        previous = Some(end);
        let _ = line.paint(point(x, y), font, gpui::TextAlign::Left, None, window, cx);
    }
}

fn paint_chart_kind(
    plot: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
) {
    match series.kind {
        ChartKind::Line
        | ChartKind::Area
        | ChartKind::Bar
        | ChartKind::Scatter
        | ChartKind::Bubble => paint_cartesian(
            plot,
            series,
            series.kind,
            0..series.datasets.len(),
            window,
            theme,
        ),
        ChartKind::HorizontalBar => paint_horizontal_bars(plot, series, window, theme),
        ChartKind::Candlestick => candle::paint(plot, series, window, theme),
        ChartKind::Radar => unreachable!("radar axes and series are painted together"),
        ChartKind::Pie | ChartKind::Doughnut => paint_pie(plot, series, window, theme),
    }
}

fn paint_cartesian(
    plot: Bounds<Pixels>,
    series: &ChartSeries,
    kind: ChartKind,
    indices: std::ops::Range<usize>,
    window: &mut Window,
    theme: &Theme,
) {
    match kind {
        ChartKind::Line => paint_lines(plot, series, indices, window, theme),
        ChartKind::Area => paint_areas(plot, series, indices, window, theme),
        ChartKind::Bar => paint_bars(plot, series, indices, window, theme),
        ChartKind::Scatter | ChartKind::Bubble => {
            paint_scatter(plot, series, indices, window, theme)
        }
        _ => {}
    }
}

fn plot_bounds_for(bounds: Bounds<Pixels>, series: &ChartSeries) -> Bounds<Pixels> {
    if series.kind == ChartKind::Radar {
        return bounds;
    }
    let (inset_x, inset_y) = plot_content_inset(series);
    let inset = px(inset_x);
    Bounds::new(
        point(bounds.origin.x + inset, bounds.origin.y + px(inset_y)),
        gpui::size(
            bounds.size.width - inset - px(14.0),
            bounds.size.height - px(44.0),
        ),
    )
}

fn plot_content_inset(series: &ChartSeries) -> (f32, f32) {
    let x = if series.is_radial() {
        18.0
    } else if series.kind == ChartKind::HorizontalBar {
        72.0
    } else {
        36.0
    };
    (x, 14.0)
}

fn paint_grid(bounds: Bounds<Pixels>, window: &mut Window, color: Hsla) {
    for step in 1..4 {
        let y = bounds.origin.y + bounds.size.height * (step as f32 / 4.0);
        let mut builder = PathBuilder::stroke(px(1.0));
        builder.move_to(point(bounds.origin.x, y));
        builder.line_to(point(bounds.origin.x + bounds.size.width, y));
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

fn paint_axes(bounds: Bounds<Pixels>, window: &mut Window, color: Hsla) {
    let mut builder = PathBuilder::stroke(px(1.0));
    builder.move_to(point(bounds.origin.x, bounds.origin.y));
    builder.line_to(point(bounds.origin.x, bounds.origin.y + bounds.size.height));
    builder.line_to(point(
        bounds.origin.x + bounds.size.width,
        bounds.origin.y + bounds.size.height,
    ));
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn paint_lines(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    indices: std::ops::Range<usize>,
    window: &mut Window,
    theme: &Theme,
) {
    let domain_axis = series.visible_domain_axis();
    let points = series.visible_point_indices();
    let max_points = points.len().max(1);
    for dataset_index in indices.clone() {
        let dataset = &series.datasets[dataset_index];
        let axis = series.rendered_axis(dataset_index);
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        let mut builder = PathBuilder::stroke(px(2.0));
        let mut drawn = 0usize;
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(point) = dataset.points.get(point_index) else {
                continue;
            };
            let p = point_for_data(
                point,
                visible_index,
                max_points,
                domain_axis,
                series.scale_rendered_value(dataset_index, point.value),
                axis,
                bounds,
            );
            if drawn == 0 {
                builder.move_to(p);
            } else {
                builder.line_to(p);
            }
            drawn += 1;
        }
        if drawn > 0
            && let Ok(path) = builder.build()
        {
            window.paint_path(path, color);
        }
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let p = point_for_data(
                data_point,
                visible_index,
                max_points,
                domain_axis,
                series.scale_rendered_value(dataset_index, data_point.value),
                axis,
                bounds,
            );
            window.paint_quad(fill(
                Bounds::new(
                    point(p.x - px(3.0), p.y - px(3.0)),
                    gpui::size(px(6.0), px(6.0)),
                ),
                color,
            ));
        }
    }
}

fn paint_areas(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    indices: std::ops::Range<usize>,
    window: &mut Window,
    theme: &Theme,
) {
    if series.is_stacked() {
        paint_stacked_areas(bounds, series, window, theme);
        return;
    }
    let domain_axis = series.visible_domain_axis();
    let points = series.visible_point_indices();
    let max_points = points.len().max(1);
    for dataset_index in indices.clone() {
        let dataset = &series.datasets[dataset_index];
        let axis = series.rendered_axis(dataset_index);
        if dataset.points.is_empty() {
            continue;
        }
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        let mut fill_builder = PathBuilder::fill();
        let baseline = bounds.origin.y + bounds.size.height;
        let mut end_x = None;
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let p = point_for_data(
                data_point,
                visible_index,
                max_points,
                domain_axis,
                series.scale_rendered_value(dataset_index, data_point.value),
                axis,
                bounds,
            );
            if end_x.is_none() {
                fill_builder.move_to(point(p.x, baseline));
                fill_builder.line_to(p);
            } else {
                fill_builder.line_to(p);
            }
            end_x = Some(p.x);
        }
        let Some(end_x) = end_x else { continue };
        fill_builder.line_to(point(end_x, baseline));
        fill_builder.close();
        if let Ok(path) = fill_builder.build() {
            window.paint_path(path, color.opacity(0.28));
        }
    }
    paint_lines(bounds, series, indices, window, theme);
}

fn paint_bars(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    indices: std::ops::Range<usize>,
    window: &mut Window,
    theme: &Theme,
) {
    let points = series.visible_point_indices();
    let max_points = points.len().max(1);
    if series.is_stacked() {
        paint_stacked_bars(
            bounds,
            series,
            window,
            theme,
            series.scaled_axis(),
            max_points,
        );
        return;
    }
    let dataset_count = series.bar_lane_count();
    let group_width = f32::from(bounds.size.width) / max_points as f32;
    let bar_width = (group_width / dataset_count as f32 * 0.72).max(2.0);

    for dataset_index in indices.clone() {
        let dataset = &series.datasets[dataset_index];
        let axis = series.rendered_axis(dataset_index);
        let lane = series.bar_lane(dataset_index);
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let baseline = match series.dataset_scale(dataset_index) {
                ChartScale::Linear => {
                    axis.normalize(series.scale_rendered_value(dataset_index, 0.0)) as f32
                }
                ChartScale::Log10 => 0.0,
            };
            let normalized =
                axis.normalize(series.scale_rendered_value(dataset_index, data_point.value)) as f32;
            let value_start = baseline.min(normalized);
            let value_end = baseline.max(normalized);
            let height = f32::from(bounds.size.height) * (value_end - value_start);
            let x = f32::from(bounds.origin.x)
                + visible_index as f32 * group_width
                + lane as f32 * bar_width
                + group_width * 0.14;
            let y = f32::from(bounds.origin.y) + f32::from(bounds.size.height) * (1.0 - value_end);
            window.paint_quad(fill(
                Bounds::new(
                    point(px(x), px(y)),
                    gpui::size(px(bar_width), px(height.max(1.0))),
                ),
                color,
            ));
        }
    }
}

fn paint_horizontal_bars(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
) {
    let points = series.visible_point_indices();
    let max_points = points.len().max(1);
    if series.is_stacked() {
        paint_stacked_horizontal_bars(bounds, series, window, theme, series.scaled_axis(), &points);
        return;
    }
    let dataset_count = series.datasets.len().max(1);
    let group_height = f32::from(bounds.size.height) / max_points as f32;
    let bar_height = (group_height / dataset_count as f32 * 0.72).max(2.0);

    for (dataset_index, dataset) in series.datasets.iter().enumerate() {
        let axis = series.rendered_axis(dataset_index);
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let baseline = match series.options.scale {
                ChartScale::Linear => {
                    axis.normalize(series.scale_rendered_value(dataset_index, 0.0)) as f32
                }
                ChartScale::Log10 => 0.0,
            };
            let normalized =
                axis.normalize(series.scale_rendered_value(dataset_index, data_point.value)) as f32;
            let value_start = baseline.min(normalized);
            let value_end = baseline.max(normalized);
            let width = f32::from(bounds.size.width) * (value_end - value_start);
            let x = f32::from(bounds.origin.x) + f32::from(bounds.size.width) * value_start;
            let y = f32::from(bounds.origin.y)
                + visible_index as f32 * group_height
                + dataset_index as f32 * bar_height
                + group_height * 0.14;
            window.paint_quad(fill(
                Bounds::new(
                    point(px(x), px(y)),
                    gpui::size(px(width.max(1.0)), px(bar_height)),
                ),
                color,
            ));
        }
    }
}

fn paint_stacked_horizontal_bars(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
    axis: ChartAxis,
    points: &[usize],
) {
    let max_points = points.len().max(1);
    let group_height = f32::from(bounds.size.height) / max_points as f32;
    let bar_height = (group_height * 0.72).max(2.0);
    let mut positive_offsets = vec![0.0; max_points];
    let mut negative_offsets = vec![0.0; max_points];

    for (dataset_index, dataset) in series.datasets.iter().enumerate() {
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let offset = if data_point.value >= 0.0 {
                &mut positive_offsets[visible_index]
            } else {
                &mut negative_offsets[visible_index]
            };
            let start = *offset;
            *offset += data_point.value;
            let x0 = x_for_scaled_value(series.scale_value(start), axis, bounds);
            let x1 = x_for_scaled_value(series.scale_value(*offset), axis, bounds);
            let x = x0.min(x1);
            let width = f32::from((x1 - x0).abs()).max(1.0);
            let y = f32::from(bounds.origin.y)
                + visible_index as f32 * group_height
                + group_height * 0.14;
            window.paint_quad(fill(
                Bounds::new(point(x, px(y)), gpui::size(px(width), px(bar_height))),
                color,
            ));
        }
    }
}

fn paint_scatter(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    indices: std::ops::Range<usize>,
    window: &mut Window,
    theme: &Theme,
) {
    let domain_axis = series.visible_domain_axis();
    let points = series.visible_point_indices();
    let max_points = points.len().max(1);
    for dataset_index in indices.clone() {
        let dataset = &series.datasets[dataset_index];
        let axis = series.rendered_axis(dataset_index);
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let p = point_for_data(
                data_point,
                visible_index,
                max_points,
                domain_axis,
                series.scale_rendered_value(dataset_index, data_point.value),
                axis,
                bounds,
            );
            let radius = if dataset.kind.unwrap_or(series.kind) == ChartKind::Bubble {
                data_point.bubble_radius().unwrap_or(4.0).clamp(1.0, 64.0)
            } else {
                4.0
            };
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(p.x - px(radius), p.y - px(radius)),
                        gpui::size(px(radius * 2.0), px(radius * 2.0)),
                    ),
                    color,
                )
                .corner_radii(px(radius)),
            );
        }
    }
}

fn paint_stacked_areas(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
) {
    let axis = series.scaled_axis();
    let points = series.visible_point_indices();
    let max_points = points.len().max(1);
    for (dataset_index, dataset) in series.datasets.iter().enumerate() {
        if dataset.points.is_empty() {
            continue;
        }
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(datum) = dataset.points.get(point_index) else {
                continue;
            };
            let current = series.stacked_value_at(dataset_index, point_index);
            let previous = current - datum.value;
            top.push(point_for_data(
                datum,
                visible_index,
                max_points,
                series.visible_domain_axis(),
                series.scale_value(current),
                axis,
                bounds,
            ));
            bottom.push(point_for_data(
                datum,
                visible_index,
                max_points,
                series.visible_domain_axis(),
                series.scale_value(previous),
                axis,
                bounds,
            ));
        }

        let mut fill_builder = PathBuilder::fill();
        for (index, point) in top.iter().enumerate() {
            if index == 0 {
                fill_builder.move_to(*point);
            } else {
                fill_builder.line_to(*point);
            }
        }
        for point in bottom.iter().rev() {
            fill_builder.line_to(*point);
        }
        fill_builder.close();
        if let Ok(path) = fill_builder.build() {
            window.paint_path(path, color.opacity(0.28));
        }

        let mut line_builder = PathBuilder::stroke(px(2.0));
        for (index, point) in top.iter().enumerate() {
            if index == 0 {
                line_builder.move_to(*point);
            } else {
                line_builder.line_to(*point);
            }
        }
        if let Ok(path) = line_builder.build() {
            window.paint_path(path, color);
        }
    }
}

fn paint_stacked_bars(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
    axis: ChartAxis,
    max_points: usize,
) {
    let points = series.visible_point_indices();
    let group_width = f32::from(bounds.size.width) / max_points as f32;
    let bar_width = (group_width * 0.72).max(2.0);
    let zero_y = y_for_scaled_value(series.scale_value(0.0), axis, bounds);
    let mut positive_offsets = vec![0.0; max_points];
    let mut negative_offsets = vec![0.0; max_points];

    for (dataset_index, dataset) in series.datasets.iter().enumerate() {
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        for (visible_index, point_index) in points.iter().copied().enumerate() {
            let Some(data_point) = dataset.points.get(point_index) else {
                continue;
            };
            let offset = if data_point.value >= 0.0 {
                &mut positive_offsets[visible_index]
            } else {
                &mut negative_offsets[visible_index]
            };
            let start = *offset;
            *offset += data_point.value;
            let y0 = y_for_scaled_value(series.scale_value(start), axis, bounds);
            let y1 = y_for_scaled_value(series.scale_value(*offset), axis, bounds);
            let y = y0.min(y1);
            let height = f32::from((y1 - y0).abs()).max(1.0);
            let x = f32::from(bounds.origin.x)
                + visible_index as f32 * group_width
                + group_width * 0.14;
            window.paint_quad(fill(
                Bounds::new(point(px(x), y), gpui::size(px(bar_width), px(height))),
                color,
            ));
            if start == 0.0 && data_point.value == 0.0 {
                window.paint_quad(fill(
                    Bounds::new(
                        point(px(x), zero_y - px(0.5)),
                        gpui::size(px(bar_width), px(1.0)),
                    ),
                    color,
                ));
            }
        }
    }
}

fn paint_crosshair(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
) {
    let Some(point_index) = series.options.crosshair_index else {
        return;
    };
    let horizontal = series.kind == ChartKind::HorizontalBar;
    let mut guide_drawn = false;
    for (dataset_index, dataset) in series.datasets.iter().enumerate() {
        let Some(p) = series.plot_position(dataset_index, point_index, bounds) else {
            continue;
        };
        if !guide_drawn {
            let mut builder = PathBuilder::stroke(px(1.0));
            if horizontal {
                builder.move_to(point(bounds.left(), p.y));
                builder.line_to(point(bounds.right(), p.y));
            } else {
                builder.move_to(point(p.x, bounds.top()));
                builder.line_to(point(p.x, bounds.bottom()));
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, theme.primary().opacity(0.55));
            }
            guide_drawn = true;
        }
        let color = dataset
            .color
            .unwrap_or_else(|| palette(dataset_index, theme));
        window.paint_quad(fill(
            Bounds::new(
                point(p.x - px(4.0), p.y - px(4.0)),
                gpui::size(px(8.0), px(8.0)),
            ),
            color,
        ));
    }
}

fn sector_vertices(
    bounds: Bounds<Pixels>,
    cutout: f32,
    start: f32,
    end: f32,
) -> Vec<Point<Pixels>> {
    let center = bounds.center();
    let radius = bounds.size.width.min(bounds.size.height) / 2.;
    let sweep = (end - start) * std::f32::consts::TAU;
    let start = start * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let steps = ((sweep.abs() / 0.12).ceil() as usize).max(2);
    let angle = |step: usize| start + sweep * step as f32 / steps as f32;
    let mut vertices = Vec::with_capacity((steps + 1) * 2);
    vertices.extend((0..=steps).map(|i| point_at_angle(center, radius, angle(i))));
    if cutout > 0. {
        vertices.extend(
            (0..=steps)
                .rev()
                .map(|i| point_at_angle(center, radius * cutout, angle(i))),
        );
    } else {
        vertices.push(center);
    }
    vertices
}

fn paint_pie(bounds: Bounds<Pixels>, series: &ChartSeries, window: &mut Window, theme: &Theme) {
    let cutout = if series.kind == ChartKind::Doughnut {
        series.options.doughnut_cutout
    } else {
        0.
    };
    for (index, start, end) in series.radial_slices() {
        let vertices = sector_vertices(bounds, cutout, start, end);
        let mut builder = PathBuilder::fill();
        builder.move_to(vertices[0]);
        for p in &vertices[1..] {
            builder.line_to(*p);
        }
        builder.close();
        if let Ok(path) = builder.build() {
            window.paint_path(path, palette(index, theme));
        }
    }
}

fn point_for_data(
    data_point: &ChartPoint,
    visible_index: usize,
    max_points: usize,
    domain_axis: ChartAxis,
    value: f64,
    axis: ChartAxis,
    bounds: Bounds<Pixels>,
) -> Point<Pixels> {
    let x_fraction = match data_point.domain {
        ChartDomainValue::Category(_) => category_position(visible_index, max_points),
        ChartDomainValue::Number(x) => domain_axis.normalize(x) as f32,
        ChartDomainValue::Timestamp(x) => domain_axis.normalize(x as f64) as f32,
    };
    let x = bounds.origin.x + px(x_fraction * f32::from(bounds.size.width));
    let y = y_for_scaled_value(value, axis, bounds);
    point(x, y)
}

fn point_domain_value(point: &ChartPoint, fallback_index: usize) -> f64 {
    match point.domain {
        ChartDomainValue::Category(_) => fallback_index as f64,
        ChartDomainValue::Number(value) => value,
        ChartDomainValue::Timestamp(value) => value as f64,
    }
}

fn y_for_scaled_value(value: f64, axis: ChartAxis, bounds: Bounds<Pixels>) -> Pixels {
    bounds.origin.y + bounds.size.height
        - px((axis.normalize(value) as f32) * f32::from(bounds.size.height))
}

fn x_for_scaled_value(value: f64, axis: ChartAxis, bounds: Bounds<Pixels>) -> Pixels {
    bounds.origin.x + px((axis.normalize(value) as f32) * f32::from(bounds.size.width))
}

fn nearest_uniform(position: f32, extent: f32, first: usize, last: usize) -> usize {
    // Match a forward scan: an exact midpoint belongs to the earlier point.
    let fraction = f64::from(position).clamp(0., f64::from(extent)) / f64::from(extent);
    first + ((fraction * (last - first) as f64 - 0.5).ceil() as usize).min(last - first)
}

fn category_position(index: usize, len: usize) -> f32 {
    if len <= 1 {
        0.5
    } else {
        index as f32 / (len - 1) as f32
    }
}

fn distance_to_interval(value: f32, start: f32, end: f32) -> f32 {
    if value < start {
        start - value
    } else if value > end {
        value - end
    } else {
        0.0
    }
}

fn point_at_angle(center: Point<Pixels>, radius: Pixels, angle: f32) -> Point<Pixels> {
    point(
        center.x + px(f32::from(radius) * angle.cos()),
        center.y + px(f32::from(radius) * angle.sin()),
    )
}

fn palette(index: usize, theme: &Theme) -> Hsla {
    match index % 6 {
        0 => theme.primary(),
        1 => theme.success(),
        2 => theme.warning(),
        3 => theme.danger(),
        4 => theme.info(),
        _ => theme.muted_foreground(),
    }
}

fn positive_log_min(axis_min: f64, series: &ChartSeries) -> f64 {
    if axis_min > 0.0 {
        axis_min
    } else {
        series.smallest_positive_cache
    }
}

fn escape_csv(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    let mut characters = value.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn svg_palette(index: usize) -> &'static str {
    const COLORS: [&str; 8] = [
        "#2563eb", "#dc2626", "#16a34a", "#9333ea", "#ea580c", "#0891b2", "#ca8a04", "#4f46e5",
    ];
    COLORS[index % COLORS.len()]
}

#[cfg(test)]
mod tests {
    #[test]
    fn candle_wicks_bodies_and_flat_intervals_share_export_and_hit_geometry() {
        use super::{ChartDomainValue, ChartOhlc};
        assert!(ChartOhlc::new(10., 9., 8., 10.).is_none());
        assert!(ChartOhlc::new(0., 1., f64::NAN, 0.).is_none());
        let series = ChartSeries::new(ChartKind::Candlestick).datasets(vec![
            ChartDataset::new("q", "Q").candles(
                [
                    (
                        "One",
                        ChartOhlc::new(20., 40., 10., 30.).expect("valid test fixture"),
                    ),
                    (
                        "Two",
                        ChartOhlc::new(20., 30., 15., 20.).expect("valid test fixture"),
                    ),
                ]
                .map(|(name, v)| (ChartDomainValue::Category(name.into()), v)),
            ),
        ]);
        assert_eq!(series.value_axis(), ChartAxis::new(10., 40.));
        for (x, y, i) in [(25., 5., 0), (35., 50., 0), (75., 67., 1)] {
            assert_eq!(
                series
                    .hit_test(x, y, 100., 100.)
                    .expect("valid test fixture")
                    .point_index,
                i
            );
        }
        assert!(series.hit_test(45., 4., 100., 100.).is_none());
        let hit = series
            .hit_test(25., 5., 100., 100.)
            .expect("valid test fixture");
        assert_eq!(
            super::chart_tooltip_rows_for_hit(&series, Some(&hit)).len(),
            4
        );
        assert!(series.to_csv().contains("Q,One,20,40,10,30"));
        assert_eq!(series.to_svg(300, 200).matches("<rect").count(), 4);
        assert!(series.accessible_summary()[0].contains("High 40"));
    }

    #[test]
    fn radar_vertices_use_the_same_axis_for_selection_and_export() {
        let data = ChartDataset::new("s", "S").points(
            ["North", "East", "South", "West"]
                .map(|name| ChartPoint::category(name, 10.))
                .to_vec(),
        );
        let chart = ChartSeries::new(ChartKind::Radar).datasets(vec![data]);
        for (i, (x, y)) in [(100., 24.), (176., 100.), (100., 176.), (24., 100.)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                chart
                    .hit_test(x, y, 200., 200.)
                    .expect("valid test fixture")
                    .point_index,
                i
            );
        }
        assert!(chart.hit_test(100., 100., 200., 200.).is_none());
        assert!(
            chart
                .to_svg(200, 200)
                .contains("100.00,24.00 176.00,100.00")
        );
    }

    #[test]
    fn sorted_hit_candidates_match_exhaustive_search_for_mixed_and_unsorted_domains() {
        let data = ChartDataset::new("s", "S").points(
            (0..800)
                .map(|i| ChartPoint::numeric((i / 3) as f64, (i % 19) as f64))
                .collect(),
        );
        let mut chart = ChartSeries::new(ChartKind::Bubble).datasets(vec![
            data,
            ChartDataset::new("t", "T").points(vec![
                ChartPoint::numeric(20., 9.).radius(24.),
                ChartPoint::category("C", 14.),
                ChartPoint::numeric(2., 18.),
            ]),
        ]);
        let mut exhaustive = chart.clone();
        exhaustive.domain_order = std::sync::OnceLock::from(vec![
            super::DomainOrder::Unordered;
            exhaustive.datasets.len()
        ]);
        for x in (0..200).step_by(7) {
            for y in (0..120).step_by(9) {
                assert_eq!(
                    chart.hit_test(x as f32, y as f32, 200., 120.),
                    exhaustive.hit_test(x as f32, y as f32, 200., 120.)
                );
            }
        }
        let _ = chart.visible_domain_axis();
        chart = chart.options(ChartOptions::default().domain(ChartAxis::new(300., 360.)));
        assert_eq!(chart.visible_domain_axis(), ChartAxis::new(100., 120.));
    }

    #[gpui::test]
    fn stacked_bars_draw_a_late_viewport_in_both_orientations(cx: &mut TestAppContext) {
        struct Bars;
        impl Render for Bars {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let data = ChartDataset::new("a", "A").points(
                    (0..160)
                        .map(|i| ChartPoint::numeric(i as f64, i as f64 - 150.))
                        .collect(),
                );
                let opts = ChartOptions::default()
                    .stacked(true)
                    .domain(ChartAxis::new(145., 155.));
                div()
                    .flex()
                    .flex_col()
                    .child(
                        super::BarChart::new("b")
                            .options(opts.clone())
                            .datasets(vec![data.clone()]),
                    )
                    .child(
                        HorizontalBarChart::new("h")
                            .options(opts)
                            .datasets(vec![data]),
                    )
            }
        }
        init_visual_test(cx);
        let (_, cx) = cx.add_window_view(|_, _| Bars);
        assert!(cx.debug_bounds("guic-chart-plot").is_some());
    }

    #[test]
    fn extreme_finite_axis_preserves_endpoints_and_midpoint() {
        let axis = super::ChartAxis::new(-f64::MAX, f64::MAX);
        assert_eq!(axis.normalize(-f64::MAX), 0.0);
        assert_eq!(axis.normalize(0.0), 0.5);
        assert_eq!(axis.normalize(f64::MAX), 1.0);
        assert_eq!(super::interpolate(-f64::MAX, f64::MAX, 0.), -f64::MAX);
        assert_eq!(super::interpolate(-f64::MAX, f64::MAX, 0.5), 0.);
        assert_eq!(super::interpolate(f64::MAX, 1., 1.), 1.);
    }

    use super::{
        BubbleChart, ChartAxis, ChartAxisSide, ChartDataset, ChartDomainFormatter,
        ChartDomainValue, ChartEasing, ChartHit, ChartInteractionCommand, ChartInteractionState,
        ChartKind, ChartLabelCollisionPolicy, ChartOptions, ChartPoint, ChartScale, ChartSeries,
        ChartTransition, ChartValueAxis, ChartValueFormatter, DoughnutChart, HorizontalBarChart,
        LineChart, MixedChart, ScatterChart, bounded_accessible_description, chart_key_command,
    };
    use gpui::{
        Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton, ParentElement as _,
        Render, Styled as _, TestAppContext, Window, div, point,
    };
    use std::sync::{Arc, Mutex};

    struct TooltipHarness {
        hit: Arc<Mutex<Option<ChartHit>>>,
    }

    impl Render for TooltipHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let hit = self.hit.clone();
            div().size_full().child(
                LineChart::new("tooltip-test")
                    .options(
                        ChartOptions::default()
                            .height(220.0)
                            .tooltip_intersect(false),
                    )
                    .datasets(vec![ChartDataset::new("actual", "Actual").points(vec![
                        ChartPoint::category("Jan", 12.0),
                        ChartPoint::category("Feb", 18.0),
                        ChartPoint::category("Mar", 14.0),
                    ])])
                    .on_hover(move |next, _, _| {
                        *hit.lock().expect("hit state lock should be available") = next.clone();
                    }),
            )
        }
    }

    pub(super) fn init_visual_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
    }

    #[gpui::test]
    fn tooltip_flips_at_plot_edges_inside_a_larger_window(cx: &mut TestAppContext) {
        use gpui::{px, size};
        struct InsetChart;
        impl Render for InsetChart {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .absolute()
                    .left(px(80.))
                    .top(px(40.))
                    .w(px(420.))
                    .child(
                        LineChart::new("inset")
                            .options(
                                ChartOptions::default()
                                    .height(200.)
                                    .tooltip_intersect(false),
                            )
                            .datasets(vec![ChartDataset::new("series", "Series").points(vec![
                                ChartPoint::category("A", 1.),
                                ChartPoint::category("B", 3.),
                            ])]),
                    )
            }
        }
        init_visual_test(cx);
        let (_, cx) = cx.add_window_view(|_, _| InsetChart);
        cx.simulate_resize(size(px(800.), px(520.)));
        let plot = cx.debug_bounds("guic-chart-plot").expect("plot");
        for (x, right) in [
            (plot.left() + px(50.), false),
            (plot.right() - px(20.), true),
        ] {
            for (y, bottom) in [
                (plot.top() + px(24.), false),
                (plot.bottom() - px(36.), true),
            ] {
                let pointer = point(x, y);
                cx.simulate_mouse_move(pointer, Option::<MouseButton>::None, Modifiers::none());
                let panel = cx.debug_bounds("guic-chart-tooltip").expect("tooltip");
                assert!(panel.left() >= plot.left() && panel.right() <= plot.right());
                assert!(panel.top() >= plot.top() && panel.bottom() <= plot.bottom());
                assert_eq!(
                    if right {
                        x - panel.right()
                    } else {
                        panel.left() - x
                    },
                    px(12.)
                );
                assert_eq!(
                    if bottom {
                        y - panel.bottom()
                    } else {
                        panel.top() - y
                    },
                    px(12.)
                );
            }
        }
    }

    #[gpui::test]
    fn grouped_tooltip_fits_a_narrow_short_window(cx: &mut TestAppContext) {
        use gpui::{px, size};
        struct Grouped;
        impl Render for Grouped {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                LineChart::new("grouped")
                    .options(
                        ChartOptions::default()
                            .height(130.)
                            .tooltip_intersect(false)
                            .tooltip_mode(super::ChartTooltipMode::Index),
                    )
                    .datasets(
                        (0..16)
                            .map(|i| {
                                ChartDataset::new(
                                    format!("s{i}"),
                                    "An exceptionally long series label",
                                )
                                .points(vec![ChartPoint::category("A", i as f64)])
                            })
                            .collect(),
                    )
            }
        }
        init_visual_test(cx);
        let (_, cx) = cx.add_window_view(|_, _| Grouped);
        cx.simulate_resize(size(px(180.), px(148.)));
        let plot = cx.debug_bounds("guic-chart-plot").expect("plot");
        let pointer = point(plot.center().x, plot.top() + px(30.));
        cx.simulate_mouse_move(pointer, Option::<MouseButton>::None, Modifiers::none());
        let panel = cx
            .debug_bounds("guic-chart-tooltip")
            .unwrap_or_else(|| panic!("expected grouped tooltip for plot {plot:?}"));
        assert_eq!(panel.size.width, px(168.));
        assert!(panel.top() >= pointer.y + px(12.));
        assert!(panel.left() >= px(6.) && panel.right() <= px(174.));
        assert!(panel.top() >= px(6.) && panel.bottom() <= px(142.));
        assert!(cx.debug_bounds("guic-chart-tooltip-more").is_some());
    }

    #[test]
    fn grouped_tooltip_keeps_series_colors_when_a_dataset_has_no_matching_point() {
        let color = gpui::rgb(0x223344).into();
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().tooltip_mode(super::ChartTooltipMode::Index))
            .datasets(vec![
                ChartDataset::new("empty", "Empty"),
                ChartDataset::new("present", "Present")
                    .color(color)
                    .points(vec![ChartPoint::category("A", 1.)]),
            ]);
        let hit = ChartHit {
            dataset_index: 1,
            point_index: 0,
            label: "A".into(),
        };
        let rows = super::chart_tooltip_rows_for_hit(&chart, Some(&hit));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].color_index, 1);
        assert_eq!(rows[0].color, Some(color));
    }

    #[gpui::test]
    fn stationary_hover_uses_the_new_dataset_after_a_host_update(cx: &mut TestAppContext) {
        struct Feed(bool);
        impl Render for Feed {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                LineChart::new("feed")
                    .tooltip_content(|series, hit, _, _| {
                        let value = series.datasets_ref()[hit.dataset_index].points_ref()
                            [hit.point_index]
                            .value();
                        div()
                            .debug_selector(|| "live-quote".to_owned())
                            .child(value.to_string())
                    })
                    .options(
                        ChartOptions::default()
                            .height(220.)
                            .value_axis(ChartAxis::new(0., 10.))
                            .legend(false),
                    )
                    .datasets(vec![ChartDataset::new("feed", "Feed").points(vec![
                        ChartPoint::category("Latest", if self.0 { 8. } else { 2. }),
                    ])])
            }
        }
        init_visual_test(cx);
        let (view, cx) = cx.add_window_view(|_, _| Feed(false));
        let rect = cx.debug_bounds("guic-chart-plot").expect("plot");
        let bounds = super::plot_bounds_for(rect, &ChartSeries::new(ChartKind::Line));
        let pointer = point(
            bounds.center().x,
            bounds.bottom() - bounds.size.height * 0.2,
        );
        cx.simulate_mouse_move(pointer, None, Modifiers::none());
        assert!(cx.debug_bounds("live-quote").is_some());
        view.update(cx, |chart, cx| {
            chart.0 = true;
            cx.notify();
        });
        assert!(cx.debug_bounds("guic-chart-tooltip").is_none());
        view.update(cx, |chart, cx| {
            chart.0 = false;
            cx.notify();
        });
        assert!(cx.debug_bounds("guic-chart-tooltip").is_some());
    }

    #[gpui::test]
    fn tooltip_mounts_tracks_pointer_and_dismisses_without_clicks(cx: &mut TestAppContext) {
        init_visual_test(cx);
        let hit = Arc::new(Mutex::new(None));
        let harness_hit = hit.clone();
        let (_, cx) = cx.add_window_view(move |_, _| TooltipHarness { hit: harness_hit });
        let plot = cx
            .debug_bounds("guic-chart-plot")
            .expect("chart plot should be rendered");
        assert!(cx.debug_bounds("guic-chart-tooltip").is_none());

        let first = point(plot.origin.x + plot.size.width * 0.25, plot.center().y);
        cx.simulate_mouse_move(first, Option::<MouseButton>::None, Modifiers::none());
        assert!(
            hit.lock()
                .expect("hit state lock should be available")
                .is_some(),
            "chart hover callback should receive a hit"
        );
        let first_tooltip = cx
            .debug_bounds("guic-chart-tooltip")
            .expect("tooltip should appear on the first pointer move");
        assert_eq!(
            hit.lock()
                .expect("hit state lock should be available")
                .as_ref()
                .map(|hit| hit.label.as_ref()),
            Some("Jan")
        );
        assert!(f32::from(first_tooltip.origin.y - first.y).abs() < 80.0);

        let second = point(plot.origin.x + plot.size.width * 0.75, plot.center().y);
        cx.simulate_mouse_move(second, Option::<MouseButton>::None, Modifiers::none());
        let second_tooltip = cx
            .debug_bounds("guic-chart-tooltip")
            .expect("tooltip should remain visible while the pointer moves");
        assert_eq!(
            hit.lock()
                .expect("hit state lock should be available")
                .as_ref()
                .map(|hit| hit.label.as_ref()),
            Some("Mar")
        );
        assert_eq!(first_tooltip.size, second_tooltip.size);
        assert!(first_tooltip.left() > first.x);
        assert!(second_tooltip.left() > first_tooltip.left());
        let nearby = point(second.x - gpui::px(3.0), second.y);
        cx.simulate_mouse_move(nearby, Option::<MouseButton>::None, Modifiers::none());
        let nearby_tooltip = cx.debug_bounds("guic-chart-tooltip").expect("same datum");
        assert_eq!(nearby_tooltip.left(), second_tooltip.left() - gpui::px(3.0));

        cx.simulate_mouse_move(
            point(
                plot.origin.x - gpui::px(20.0),
                plot.origin.y - gpui::px(20.0),
            ),
            Option::<MouseButton>::None,
            Modifiers::none(),
        );
        assert!(cx.debug_bounds("guic-chart-tooltip").is_none());
    }

    #[test]
    fn chart_keyboard_commands_cover_navigation_and_viewport_controls() {
        let domain = ChartAxis::new(10.0, 20.0);
        assert_eq!(
            chart_key_command("left", domain),
            Some(ChartInteractionCommand::PreviousPoint)
        );
        assert_eq!(
            chart_key_command("end", domain),
            Some(ChartInteractionCommand::LastPoint)
        );
        assert_eq!(
            chart_key_command("0", domain),
            Some(ChartInteractionCommand::ResetView)
        );
        assert!(matches!(
            chart_key_command("+", domain),
            Some(ChartInteractionCommand::Zoom { center: 15.0, .. })
        ));
        assert_eq!(chart_key_command("escape", domain), None);
    }

    #[test]
    fn rendered_accessibility_description_is_bounded() {
        let points = (0..100)
            .map(|index| ChartPoint::category(index.to_string(), index as f64))
            .collect();
        let series = ChartSeries::new(ChartKind::Line)
            .datasets(vec![ChartDataset::new("data", "Data").points(points)]);
        let description = bounded_accessible_description(&series, 3);
        assert!(description.contains("and 97 more values"));
        assert!(description.len() < 100);
        assert!(super::chart_tooltip_rows(&series).is_empty());
        assert!(super::chart_value_rows_bounded(&series, 0).is_empty());
    }

    #[test]
    fn value_axis_includes_zero_and_dataset_values() {
        let chart =
            ChartSeries::new(ChartKind::Line).datasets(vec![ChartDataset::new("a", "A").points(
                vec![
                    ChartPoint::category("Jan", 4.0),
                    ChartPoint::category("Feb", -2.0),
                ],
            )]);

        assert_eq!(chart.value_axis(), ChartAxis::new(-2.0, 4.0));
    }

    #[test]
    fn line_chart_exposes_underlying_series() {
        let chart = LineChart::new("revenue")
            .options(ChartOptions::default().title("Revenue"))
            .datasets(vec![
                ChartDataset::new("actual", "Actual")
                    .points(vec![ChartPoint::category("Jan", 10.0)]),
            ]);

        assert_eq!(chart.series().value_axis(), ChartAxis::new(0.0, 10.0));
    }

    #[test]
    fn hit_test_reports_nearest_point() {
        let chart =
            ChartSeries::new(ChartKind::Line).datasets(vec![ChartDataset::new("a", "A").points(
                vec![
                    ChartPoint::category("Jan", 0.0),
                    ChartPoint::category("Feb", 10.0),
                ],
            )]);

        let hit = chart.hit_test(100.0, 0.0, 100.0, 100.0);
        assert_eq!(hit.map(|hit| hit.point_index), Some(1));
    }

    #[test]
    fn default_line_hit_testing_requires_matching_x_and_y() {
        let chart = ChartSeries::new(ChartKind::Line).datasets(vec![
            ChartDataset::new("actual", "Actual").points(vec![
                ChartPoint::category("Jan", 12.0),
                ChartPoint::category("Feb", 18.0),
                ChartPoint::category("Mar", 14.0),
            ]),
        ]);

        let on_point = chart.hit_test(100.0, 22.22, 100.0, 100.0);
        assert_eq!(on_point.map(|hit| hit.point_index), Some(2));
        assert_eq!(
            chart.hit_test(100.0, 90.0, 100.0, 100.0),
            None,
            "matching the category X must not ignore a distant pointer Y"
        );
    }

    #[test]
    fn multi_dataset_line_hit_testing_uses_the_rendered_shared_axis() {
        let chart = ChartSeries::new(ChartKind::Line).datasets(vec![
            ChartDataset::new("actual", "Actual").points(vec![
                ChartPoint::category("Jan", 12.0),
                ChartPoint::category("Feb", 18.0),
                ChartPoint::category("Mar", 14.0),
                ChartPoint::category("Apr", 26.0),
                ChartPoint::category("May", 32.0),
                ChartPoint::category("Jun", 28.0),
            ]),
            ChartDataset::new("forecast", "Forecast").points(vec![
                ChartPoint::category("Jan", 10.0),
                ChartPoint::category("Feb", 16.0),
                ChartPoint::category("Mar", 20.0),
                ChartPoint::category("Apr", 30.0),
                ChartPoint::category("May", 34.0),
                ChartPoint::category("Jun", 38.0),
            ]),
        ]);

        let hit = chart.hit_test(80.0, 100.0 * (1.0 - 32.0 / 38.0), 100.0, 100.0);
        assert_eq!(
            hit.map(|hit| (hit.dataset_index, hit.point_index)),
            Some((0, 4))
        );
    }

    #[test]
    fn stacked_axis_sums_positive_and_negative_values() {
        let chart = ChartSeries::new(ChartKind::Bar)
            .options(ChartOptions::default().stacked(true))
            .datasets(vec![
                ChartDataset::new("a", "A").points(vec![
                    ChartPoint::category("Jan", 4.0),
                    ChartPoint::category("Feb", -2.0),
                ]),
                ChartDataset::new("b", "B").points(vec![
                    ChartPoint::category("Jan", 6.0),
                    ChartPoint::category("Feb", -3.0),
                ]),
            ]);

        assert_eq!(chart.value_axis(), ChartAxis::new(-5.0, 10.0));
    }

    #[test]
    fn explicit_axis_overrides_data_axis() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().value_axis(ChartAxis::new(10.0, 20.0)))
            .datasets(vec![
                ChartDataset::new("a", "A").points(vec![ChartPoint::category("Jan", 4.0)]),
            ]);

        assert_eq!(chart.value_axis(), ChartAxis::new(10.0, 20.0));
    }

    #[test]
    fn named_value_axes_scale_datasets_independently() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().value_axes(vec![
                ChartValueAxis::new("temperature")
                    .range(ChartAxis::new(-20.0, 50.0))
                    .side(ChartAxisSide::Leading),
                ChartValueAxis::new("pressure")
                    .range(ChartAxis::new(900.0, 1_100.0))
                    .side(ChartAxisSide::Trailing),
            ]))
            .datasets(vec![
                ChartDataset::new("temp", "Temperature")
                    .axis("temperature")
                    .points(vec![ChartPoint::category("Now", 21.0)]),
                ChartDataset::new("pressure", "Pressure")
                    .axis("pressure")
                    .points(vec![ChartPoint::category("Now", 1_013.0)]),
            ]);

        assert_eq!(chart.dataset_value_axis(0), ChartAxis::new(-20.0, 50.0));
        assert_eq!(chart.dataset_value_axis(1), ChartAxis::new(900.0, 1_100.0));
        assert_eq!(
            chart.datasets[0].axis_id().map(AsRef::as_ref),
            Some("temperature")
        );
    }

    #[test]
    fn log_scale_hit_testing_uses_scaled_values() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().scale(ChartScale::Log10))
            .datasets(vec![ChartDataset::new("a", "A").points(vec![
                ChartPoint::category("One", 1.0),
                ChartPoint::category("Hundred", 100.0),
            ])]);

        let hit = chart.hit_test(100.0, 0.0, 100.0, 100.0);
        assert_eq!(hit.map(|hit| hit.point_index), Some(1));
    }

    #[test]
    fn nearest_point_index_uses_plot_width() {
        let chart =
            ChartSeries::new(ChartKind::Line).datasets(vec![ChartDataset::new("a", "A").points(
                vec![
                    ChartPoint::category("Jan", 1.0),
                    ChartPoint::category("Feb", 2.0),
                    ChartPoint::category("Mar", 3.0),
                ],
            )]);

        assert_eq!(chart.nearest_point_index(75.0, 100.0), Some(1));
    }

    #[test]
    fn value_formatter_supports_units() {
        assert_eq!(
            ChartValueFormatter::Suffix("ms".into()).format(12.5),
            "12.50 ms"
        );
        assert_eq!(ChartValueFormatter::Percent.format(42.0), "42%");
        assert_eq!(
            ChartValueFormatter::Custom(|value| format!("custom:{value:.1}")).format(12.5),
            "custom:12.5"
        );
    }

    #[test]
    fn domain_limits_nearest_point_lookup() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(2.0, 4.0)))
            .datasets(vec![ChartDataset::new("a", "A").points(vec![
                ChartPoint::category("Jan", 1.0),
                ChartPoint::category("Feb", 2.0),
                ChartPoint::category("Mar", 3.0),
                ChartPoint::category("Apr", 4.0),
                ChartPoint::category("May", 5.0),
            ])]);

        assert_eq!(chart.nearest_point_index(0.0, 100.0), Some(2));
        assert_eq!(chart.nearest_point_index(100.0, 100.0), Some(4));
    }

    #[test]
    fn visible_domain_rescales_numeric_points_to_the_viewport() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(1.0, 2.0)))
            .datasets(vec![ChartDataset::new("a", "A").points(vec![
                ChartPoint::numeric(10.0, 1.0),
                ChartPoint::numeric(30.0, 2.0),
                ChartPoint::numeric(90.0, 3.0),
                ChartPoint::numeric(200.0, 4.0),
            ])]);

        assert_eq!(chart.visible_domain_axis(), ChartAxis::new(30.0, 90.0));
        assert_eq!(chart.nearest_point_index(0.0, 100.0), Some(1));
        assert_eq!(chart.nearest_point_index(100.0, 100.0), Some(2));
    }

    #[test]
    fn single_visible_timestamp_uses_a_bounded_domain() {
        let timestamp = 1_700_000_000_000;
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(1.0, 1.1)))
            .datasets(vec![ChartDataset::new("a", "A").points(vec![
                ChartPoint::timestamp(timestamp - 1_000, 1.0),
                ChartPoint::timestamp(timestamp, 2.0),
            ])]);

        assert_eq!(
            chart.visible_domain_axis(),
            ChartAxis::new(timestamp as f64 - 1.0, timestamp as f64 + 1.0)
        );
    }

    #[test]
    fn horizontal_hit_testing_uses_y_axis_for_categories() {
        let chart = ChartSeries::new(ChartKind::HorizontalBar).datasets(vec![
            ChartDataset::new("a", "A").points(vec![
                ChartPoint::category("Jan", 1.0),
                ChartPoint::category("Feb", 2.0),
                ChartPoint::category("Mar", 3.0),
            ]),
        ]);

        let hit = chart.hit_test(60.0, 50.0, 100.0, 100.0);
        assert_eq!(hit.map(|hit| hit.point_index), Some(1));
    }

    #[test]
    fn bar_hit_testing_intersects_painted_rectangles_and_rejects_gaps() {
        let chart = ChartSeries::new(ChartKind::Bar)
            .options(ChartOptions::default().tooltip_intersect(true))
            .datasets(vec![ChartDataset::new("actual", "Actual").points(vec![
                ChartPoint::category("Jan", 5.0),
                ChartPoint::category("Feb", 10.0),
            ])]);

        let hit = chart.hit_test(25.0, 75.0, 100.0, 100.0);
        assert_eq!(hit.map(|hit| hit.point_index), Some(0));
        assert_eq!(
            chart.hit_test(25.0, 10.0, 100.0, 100.0),
            None,
            "matching the bar X must not ignore a pointer above its value extent"
        );
        assert_eq!(chart.hit_test(50.0, 75.0, 100.0, 100.0), None);
        assert_eq!(chart.hit_test(-1.0, 50.0, 100.0, 100.0), None);
        assert_eq!(chart.hit_test(f32::NAN, 50.0, 100.0, 100.0), None);

        let continuous = ChartSeries::new(ChartKind::Bar)
            .options(ChartOptions::default().tooltip_intersect(false))
            .datasets(vec![ChartDataset::new("actual", "Actual").points(vec![
                ChartPoint::category("Jan", 5.0),
                ChartPoint::category("Feb", 10.0),
            ])]);
        assert_eq!(
            continuous
                .hit_test(50.0, 75.0, 100.0, 100.0)
                .map(|hit| hit.point_index),
            Some(1)
        );

        let signed = ChartSeries::new(ChartKind::Bar).datasets(vec![
            ChartDataset::new("delta", "Delta").points(vec![
                ChartPoint::category("Loss", -10.0),
                ChartPoint::category("Gain", 10.0),
            ]),
        ]);
        assert_eq!(
            signed.hit_test(25.0, 75.0, 100.0, 100.0),
            Some(ChartHit {
                dataset_index: 0,
                point_index: 0,
                label: "Loss".into(),
            })
        );
        assert_eq!(
            signed.hit_test(75.0, 25.0, 100.0, 100.0),
            Some(ChartHit {
                dataset_index: 0,
                point_index: 1,
                label: "Gain".into(),
            })
        );
    }

    #[test]
    fn stacked_bar_hit_testing_uses_the_rendered_stacked_axis() {
        let chart = ChartSeries::new(ChartKind::Bar)
            .options(ChartOptions::default().stacked(true))
            .datasets(vec![
                ChartDataset::new("actual", "Actual").points(vec![
                    ChartPoint::category("Jan", 12.0),
                    ChartPoint::category("Feb", 18.0),
                    ChartPoint::category("Mar", 14.0),
                    ChartPoint::category("Apr", 26.0),
                    ChartPoint::category("May", 32.0),
                    ChartPoint::category("Jun", 28.0),
                ]),
                ChartDataset::new("forecast", "Forecast").points(vec![
                    ChartPoint::category("Jan", 10.0),
                    ChartPoint::category("Feb", 16.0),
                    ChartPoint::category("Mar", 20.0),
                    ChartPoint::category("Apr", 30.0),
                    ChartPoint::category("May", 34.0),
                    ChartPoint::category("Jun", 38.0),
                ]),
            ]);

        let hit = chart.hit_test(450.0, 25.0, 600.0, 100.0);
        assert_eq!(
            hit.map(|hit| (hit.dataset_index, hit.point_index)),
            Some((1, 4))
        );
    }

    #[test]
    fn doughnut_hit_testing_ignores_cutout() {
        let chart = ChartSeries::new(ChartKind::Doughnut)
            .options(ChartOptions::default().doughnut_cutout(0.5))
            .datasets(vec![ChartDataset::new("mix", "Mix").points(vec![
                ChartPoint::category("A", 1.0),
                ChartPoint::category("B", 1.0),
            ])]);

        assert_eq!(chart.hit_test(50.0, 50.0, 100.0, 100.0), None);
        assert!(chart.hit_test(50.0, 5.0, 100.0, 100.0).is_some());
    }

    #[test]
    fn active_hit_limits_tooltip_rows() {
        let hit = ChartHit {
            dataset_index: 0,
            point_index: 1,
            label: "Feb".into(),
        };
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().active_hit(Some(hit)))
            .datasets(vec![ChartDataset::new("a", "A").points(vec![
                ChartPoint::category("Jan", 1.0),
                ChartPoint::category("Feb", 2.0),
            ])]);

        assert_eq!(
            super::chart_tooltip_rows(&chart),
            vec![("A".into(), "2".into())]
        );
        assert_eq!(
            super::chart_tooltip_title(&chart, chart.options.active_hit.as_ref()),
            "Feb"
        );
    }

    #[test]
    fn tooltip_modes_group_only_the_intersected_index_or_dataset() {
        let hit = ChartHit {
            dataset_index: 0,
            point_index: 1,
            label: "Feb".into(),
        };
        let datasets = vec![
            ChartDataset::new("actual", "Actual").points(vec![
                ChartPoint::category("Jan", 1.0),
                ChartPoint::category("Feb", 2.0),
                ChartPoint::category("Mar", 3.0),
            ]),
            ChartDataset::new("forecast", "Forecast").points(vec![
                ChartPoint::category("Jan", 4.0),
                ChartPoint::category("Feb", 5.0),
                ChartPoint::category("Mar", 6.0),
            ]),
        ];
        let index = ChartSeries::new(ChartKind::Line)
            .options(
                ChartOptions::default()
                    .active_hit(Some(hit.clone()))
                    .tooltip_mode(super::ChartTooltipMode::Index),
            )
            .datasets(datasets.clone());
        assert_eq!(
            super::chart_tooltip_rows(&index),
            vec![
                ("Actual".into(), "2".into()),
                ("Forecast".into(), "5".into())
            ]
        );

        let dataset = ChartSeries::new(ChartKind::Line)
            .options(
                ChartOptions::default()
                    .active_hit(Some(hit))
                    .tooltip_mode(super::ChartTooltipMode::Dataset)
                    .tooltip_max_rows(2),
            )
            .datasets(datasets);
        assert_eq!(
            super::chart_tooltip_rows(&dataset),
            vec![("Jan".into(), "1".into()), ("Feb".into(), "2".into())]
        );
        assert_eq!(
            super::chart_tooltip_title(&dataset, dataset.options.active_hit.as_ref()),
            "Actual"
        );
    }

    #[test]
    fn accessible_summary_reports_values() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().value_formatter(ChartValueFormatter::Percent))
            .datasets(vec![
                ChartDataset::new("a", "A").points(vec![ChartPoint::category("Jan", 1.0)]),
            ]);

        assert_eq!(chart.accessible_summary(), vec!["A · Jan: 1%"]);
    }

    #[test]
    fn new_chart_components_expose_series() {
        assert_eq!(
            HorizontalBarChart::new("horizontal").series().value_axis(),
            ChartAxis::new(0.0, 1.0)
        );
        assert_eq!(
            ScatterChart::new("scatter").series().value_axis(),
            ChartAxis::new(0.0, 1.0)
        );
        assert_eq!(
            DoughnutChart::new("doughnut").series().value_axis(),
            ChartAxis::new(0.0, 1.0)
        );
        let mixed = MixedChart::new("mixed").datasets(vec![
            ChartDataset::new("bars", "Bars")
                .kind(ChartKind::Bar)
                .points(vec![ChartPoint::category("Jan", 4.0)]),
            ChartDataset::new("line", "Line")
                .kind(ChartKind::Line)
                .points(vec![ChartPoint::category("Jan", 8.0)]),
        ]);
        assert_eq!(mixed.series().value_axis(), ChartAxis::new(0.0, 8.0));
        assert_eq!(
            mixed.series().datasets[0].chart_kind(),
            Some(ChartKind::Bar)
        );
    }

    #[test]
    fn axis_pan_and_zoom_support_host_viewports() {
        let axis = ChartAxis::new(10.0, 20.0);

        assert_eq!(axis.pan(5.0), ChartAxis::new(15.0, 25.0));
        assert_eq!(axis.zoom_around(15.0, 2.0), ChartAxis::new(12.5, 17.5));
    }

    #[test]
    fn interaction_state_clamps_viewports_and_keyboard_selection() {
        let mut state = ChartInteractionState::new();
        state.zoom(5.0, 2.0, 10);
        assert_eq!(state.domain(), Some(ChartAxis::new(2.75, 7.25)));

        state.pan(100.0, 10);
        assert_eq!(state.domain(), Some(ChartAxis::new(4.5, 9.0)));
        state.select_first_visible(10);
        assert_eq!(state.selected_index(), Some(4));
        state.move_selection(100, 10);
        assert_eq!(state.selected_index(), Some(9));
        state.move_selection(-100, 10);
        assert_eq!(state.selected_index(), Some(0));

        state.reset_view();
        assert_eq!(state.domain(), None);
        state.move_selection(1, 0);
        assert_eq!(state.selected_index(), None);
    }

    #[test]
    fn interaction_commands_share_toolbar_and_keyboard_semantics() {
        let mut state = ChartInteractionState::new();
        state.apply(
            ChartInteractionCommand::Zoom {
                center: 50.0,
                factor: 2.0,
            },
            101,
        );
        state.apply(ChartInteractionCommand::Pan(10.0), 101);
        assert_eq!(state.domain(), Some(ChartAxis::new(35.0, 85.0)));
        state.apply(ChartInteractionCommand::FirstVisiblePoint, 101);
        assert_eq!(state.selected_index(), Some(35));
        state.apply(ChartInteractionCommand::NextPoint, 101);
        assert_eq!(state.selected_index(), Some(36));
        state.apply(ChartInteractionCommand::LastPoint, 101);
        assert_eq!(state.selected_index(), Some(100));
        state.apply(ChartInteractionCommand::ResetView, 101);
        assert_eq!(state.domain(), None);
    }

    #[test]
    fn chart_inputs_reject_non_finite_layout_values() {
        assert_eq!(ChartPoint::category("invalid", f64::NAN).value(), 0.0);
        assert_eq!(ChartPoint::category("invalid", f64::INFINITY).value(), 0.0);
        assert_eq!(ChartAxis::new(f64::NAN, 10.0), ChartAxis::new(0.0, 1.0));
        assert_eq!(ChartAxis::new(20.0, 10.0), ChartAxis::new(10.0, 20.0));

        let axis = ChartAxis::new(10.0, 20.0);
        assert_eq!(axis.pan(f64::NAN), axis);
        assert_eq!(axis.zoom_around(15.0, 0.0), axis);
        assert_eq!(axis.zoom_around(f64::INFINITY, 2.0), axis);

        let options = ChartOptions::default()
            .height(f32::INFINITY)
            .doughnut_cutout(f32::NAN);
        assert_eq!(options.height, 280.0);
        assert_eq!(options.doughnut_cutout, 0.58);

        let series = ChartSeries::new(ChartKind::Line).datasets(vec![
            ChartDataset::new("safe", "Safe").points(vec![ChartPoint::category("A", 1.0)]),
        ]);
        assert_eq!(series.nearest_point_index(f32::NAN, 100.0), None);
        assert_eq!(series.nearest_point_index_y(10.0, f32::INFINITY), None);
        let transitioned = ChartTransition::new(series.clone(), series).series_at(f32::NAN);
        assert!(transitioned.datasets[0].points[0].value().is_finite());
    }

    #[test]
    fn numeric_time_and_bubble_points_preserve_domain_semantics() {
        let numeric = ChartPoint::numeric(12.5, 42.0).label("sample").radius(9.0);
        assert_eq!(numeric.domain(), &ChartDomainValue::Number(12.5));
        assert_eq!(numeric.display_label(), "sample");
        assert_eq!(numeric.bubble_radius(), Some(9.0));

        let timestamp = ChartPoint::timestamp(1_700_000_000_000, 7.0);
        assert_eq!(
            timestamp.domain(),
            &ChartDomainValue::Timestamp(1_700_000_000_000)
        );
        assert_eq!(timestamp.value(), 7.0);

        let chart = BubbleChart::new("bubble").datasets(vec![
            ChartDataset::new("samples", "Samples").points(vec![
                ChartPoint::numeric(10.0, 1.0).radius(3.0),
                ChartPoint::numeric(30.0, 2.0).radius(8.0),
            ]),
        ]);
        assert_eq!(chart.series().domain_axis(), ChartAxis::new(10.0, 30.0));
        assert_eq!(chart.series().nearest_point_index(100.0, 100.0), Some(1));
    }

    #[test]
    fn time_domain_ticks_are_density_limited_and_utc_formatted() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain_formatter(ChartDomainFormatter::IsoDateTime))
            .datasets(vec![ChartDataset::new("time", "Time").points(vec![
                ChartPoint::timestamp(-1_000, 1.0),
                ChartPoint::timestamp(0, 2.0),
                ChartPoint::timestamp(86_400_000, 3.0),
            ])]);

        let ticks = chart.domain_ticks(2);
        assert_eq!(ticks.len(), 2);
        assert_eq!(ticks[0].label, "1969-12-31 23:59:59");
        assert_eq!(ticks[1].label, "1970-01-02 00:00:00");
        assert_eq!(ticks[0].coordinate, -1_000.0);
        assert!(chart.domain_ticks(0).is_empty());

        let custom = ChartSeries::new(ChartKind::Line)
            .options(
                ChartOptions::default()
                    .domain_formatter(ChartDomainFormatter::Custom(|_| "custom-domain".into())),
            )
            .datasets(chart.datasets.clone());
        assert_eq!(custom.domain_ticks(1)[0].label, "custom-domain");
    }

    #[test]
    fn visible_labels_follow_domain() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(1.0, 2.0)))
            .datasets(vec![ChartDataset::new("a", "A").points(vec![
                ChartPoint::category("Jan", 1.0),
                ChartPoint::category("Feb", 2.0),
                ChartPoint::category("Mar", 3.0),
            ])]);

        assert_eq!(chart.visible_labels(), vec!["Feb", "Mar"]);
    }

    #[test]
    fn category_ticks_preserve_visible_endpoints_at_bounded_density() {
        let points = (0..100)
            .map(|index| ChartPoint::category(format!("P{index}"), index as f64))
            .collect();
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(20.0, 79.0)))
            .datasets(vec![ChartDataset::new("a", "A").points(points)]);

        let ticks = chart.category_ticks(5);
        assert_eq!(ticks.len(), 5);
        assert_eq!(ticks.first().map(|tick| tick.point_index), Some(20));
        assert_eq!(ticks.last().map(|tick| tick.point_index), Some(79));
        assert!(chart.category_ticks(0).is_empty());
    }

    #[test]
    fn csv_export_escapes_labels_and_respects_domain() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(1.0, 1.1)))
            .datasets(vec![
                ChartDataset::new("a", "Actual").points(vec![
                    ChartPoint::category("Jan", 1.0),
                    ChartPoint::category("Feb, quoted", 2.0),
                ]),
                ChartDataset::new("b", "Forecast").points(vec![
                    ChartPoint::category("Jan", 3.0),
                    ChartPoint::category("Feb, quoted", 4.0),
                ]),
            ]);

        assert_eq!(chart.to_csv(), "label,Actual,Forecast\n\"Feb, quoted\",2,4");
    }

    #[test]
    fn label_layout_is_bounded_truncated_and_hideable() {
        let dataset = ChartDataset::new("a", "A").points(
            (0..20)
                .map(|index| ChartPoint::category(format!("Long label {index}"), index as f64))
                .collect(),
        );
        let truncated = ChartSeries::new(ChartKind::Line)
            .options(
                ChartOptions::default()
                    .max_domain_labels(4)
                    .label_collision(ChartLabelCollisionPolicy::Truncate { max_chars: 4 }),
            )
            .datasets(vec![dataset.clone()]);
        assert_eq!(truncated.layout_domain_labels().len(), 4);
        assert!(
            truncated
                .layout_domain_labels()
                .iter()
                .all(|tick| tick.label.ends_with('…'))
        );
        let hidden = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().label_collision(ChartLabelCollisionPolicy::Hide))
            .datasets(vec![dataset]);
        assert!(hidden.layout_domain_labels().is_empty());
    }

    #[test]
    fn mixed_layers_share_axes_palette_indices_and_rendered_hit_geometry() {
        let series = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().value_axis(ChartAxis::new(0., 100.)))
            .datasets(vec![
                ChartDataset::new("line", "Line").points(vec![ChartPoint::numeric(2., 75.)]),
                ChartDataset::new("bar", "Bar")
                    .kind(ChartKind::Bar)
                    .points(vec![
                        ChartPoint::numeric(0., 40.),
                        ChartPoint::numeric(10., 15.),
                    ]),
                ChartDataset::new("bubbles", "Bubbles")
                    .kind(ChartKind::Bubble)
                    .points(vec![ChartPoint::numeric(9., 80.).radius(24.)]),
            ]);
        let bounds = gpui::Bounds::new(
            point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(200.), gpui::px(100.)),
        );
        assert_eq!(
            series.plot_position(1, 0, bounds),
            Some(point(gpui::px(50.), gpui::px(60.)))
        );
        assert_eq!(
            series.plot_position(0, 0, bounds),
            Some(point(gpui::px(40.), gpui::px(25.)))
        );
        assert_eq!(
            series
                .hit_test(50., 85., 200., 100.)
                .expect("bar interior")
                .dataset_index,
            1
        );
        assert_eq!(
            series
                .hit_test(158., 20., 200., 100.)
                .expect("bubble rim")
                .dataset_index,
            2
        );
        let svg = series.to_svg(400, 250);
        assert_eq!(svg.matches("<rect ").count(), 2);
        assert_eq!(svg.matches("<polyline ").count(), 1);
        assert!(svg.contains(r#"r="24""#));
    }

    #[test]
    fn pie_proportions_support_extreme_and_subnormal_weights() {
        for v in [f64::MAX, f64::from_bits(1)] {
            let dataset = ChartDataset::new("a", "A").points(vec![
                ChartPoint::category("one", v),
                ChartPoint::category("zero", 0.),
                ChartPoint::category("two", v),
            ]);
            let mut chart = ChartSeries::new(ChartKind::Pie).datasets(vec![dataset]);
            assert_eq!(
                chart
                    .hit_test(75., 50., 100., 100.)
                    .expect("right half")
                    .point_index,
                0
            );
            assert_eq!(
                chart
                    .hit_test(25., 50., 100., 100.)
                    .expect("left half")
                    .point_index,
                2
            );
            assert_eq!(chart.to_svg(320, 240).matches("<polygon ").count(), 2);
            chart.kind = ChartKind::Doughnut;
            assert!(chart.hit_test(50., 50., 100., 100.).is_none());
        }
    }

    #[test]
    fn svg_snapshot_is_bounded_valid_and_escaped() {
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().title("A < B"))
            .datasets(vec![ChartDataset::new("a", "Series & one").points(vec![
                ChartPoint::category("Jan", 1.0),
                ChartPoint::category("Feb", 2.0),
            ])]);
        let svg = chart.to_svg(640, 360);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("A &lt; B"));
        assert!(svg.contains("Series &amp; one"));
        assert!(svg.contains("<polyline"));
    }

    #[test]
    fn large_dataset_model_operations_stay_linear_and_bounded() {
        let points = (0..10_000)
            .map(|index| ChartPoint::category(format!("P{index}"), index as f64))
            .collect();
        let chart = ChartSeries::new(ChartKind::Line)
            .options(ChartOptions::default().domain(ChartAxis::new(500.0, 550.0)))
            .datasets(vec![ChartDataset::new("a", "A").points(points)]);

        assert_eq!(chart.visible_labels().len(), 51);
        assert_eq!(chart.nearest_point_index(0.0, 100.0), Some(500));
        assert_eq!(chart.nearest_point_index(100.0, 100.0), Some(550));
        assert_eq!(chart.to_csv().lines().count(), 52);
    }

    #[test]
    fn chart_transition_interpolates_values_domains_and_bubbles() {
        let from = ChartSeries::new(ChartKind::Bubble).datasets(vec![
            ChartDataset::new("samples", "Samples")
                .points(vec![ChartPoint::numeric(0.0, 10.0).radius(4.0)]),
        ]);
        let to = ChartSeries::new(ChartKind::Bubble).datasets(vec![
            ChartDataset::new("samples", "Samples")
                .points(vec![ChartPoint::numeric(20.0, 30.0).radius(12.0)]),
        ]);

        let halfway = ChartTransition::new(from, to)
            .easing(ChartEasing::Linear)
            .series_at(0.5);
        let point = &halfway.datasets[0].points[0];
        assert_eq!(point.domain(), &ChartDomainValue::Number(10.0));
        assert_eq!(point.value(), 20.0);
        assert_eq!(point.bubble_radius(), Some(8.0));
    }
}
