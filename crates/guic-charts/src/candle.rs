//! OHLC geometry shared by painting, hit testing, and export.

use super::*;

/// A finite price interval whose high and low enclose both open and close.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartOhlc {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

impl ChartOhlc {
    /// Returns a price interval, or `None` for non-finite or inconsistent prices.
    #[must_use]
    pub fn new(open: f64, high: f64, low: f64, close: f64) -> Option<Self> {
        ([open, high, low, close].into_iter().all(f64::is_finite)
            && low <= open.min(close)
            && high >= open.max(close))
        .then_some(Self {
            open,
            high,
            low,
            close,
        })
    }
    /// Opening price.
    pub fn open(self) -> f64 {
        self.open
    }
    /// Highest price.
    pub fn high(self) -> f64 {
        self.high
    }
    /// Lowest price.
    pub fn low(self) -> f64 {
        self.low
    }
    /// Closing price.
    pub fn close(self) -> f64 {
        self.close
    }

    pub(super) fn flat(value: f64) -> Self {
        Self {
            open: value,
            high: value,
            low: value,
            close: value,
        }
    }
    pub(super) fn interpolate(self, target: Self, t: f64) -> Self {
        Self {
            open: interpolate(self.open, target.open, t),
            high: interpolate(self.high, target.high, t),
            low: interpolate(self.low, target.low, t),
            close: interpolate(self.close, target.close, t),
        }
    }
    pub(super) fn fields(self) -> [(&'static str, f64); 4] {
        [
            ("Open", self.open),
            ("High", self.high),
            ("Low", self.low),
            ("Close", self.close),
        ]
    }
}

pub(super) fn prices(dataset: &ChartDataset, index: usize) -> ChartOhlc {
    dataset
        .candles
        .get(index)
        .copied()
        .unwrap_or_else(|| ChartOhlc::flat(dataset.points[index].value))
}
pub(super) fn extent(dataset: &ChartDataset) -> Option<(f64, f64)> {
    (0..dataset.points.len())
        .map(|i| {
            let p = prices(dataset, i);
            (p.low, p.high)
        })
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
}

struct Candle {
    wick: Bounds<Pixels>,
    body: Bounds<Pixels>,
    rising: bool,
}

fn geometry(
    series: &ChartSeries,
    bounds: Bounds<Pixels>,
    dataset: usize,
    index: usize,
    first: usize,
    count: usize,
) -> Candle {
    let prices = prices(&series.datasets[dataset], index);
    let axis = series.rendered_axis(dataset);
    let y = |value| y_for_scaled_value(series.scale_rendered_value(dataset, value), axis, bounds);
    let band = bounds.size.width / count as f32;
    let slot = band * 0.8 / series.datasets.len() as f32;
    let center =
        bounds.left() + band * ((index - first) as f32 + 0.1) + slot * (dataset as f32 + 0.5);
    let body_width = (slot * 0.85).min(px(36.)).max(px(1.));
    let open = y(prices.open);
    let close = y(prices.close);
    let high = y(prices.high);
    let low = y(prices.low);
    Candle {
        wick: Bounds::new(
            point(center - px(0.5), high),
            gpui::size(px(1.), (low - high).max(px(1.))),
        ),
        body: Bounds::new(
            point(center - body_width / 2., open.min(close)),
            gpui::size(body_width, (open - close).abs().max(px(1.))),
        ),
        rising: prices.close >= prices.open,
    }
}

fn each(series: &ChartSeries, bounds: Bounds<Pixels>, mut f: impl FnMut(usize, Candle)) {
    let Some(range) = series.visible_index_range() else {
        return;
    };
    let first = *range.start();
    let count = range.clone().count();
    for (d, dataset) in series.datasets.iter().enumerate() {
        for i in range.clone().take_while(|&i| i < dataset.points.len()) {
            f(d, geometry(series, bounds, d, i, first, count));
        }
    }
}

pub(super) fn hit_test(
    series: &ChartSeries,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<ChartHit> {
    let range = series.visible_index_range()?;
    let first = *range.start();
    let count = range.count();
    let i = first + ((x / width * count as f32) as usize).min(count - 1);
    let bounds = Bounds::new(Point::default(), gpui::size(px(width), px(height)));
    let distance = |r: Bounds<Pixels>| {
        distance_to_interval(x, r.left().into(), r.right().into()).hypot(distance_to_interval(
            y,
            r.top().into(),
            r.bottom().into(),
        ))
    };
    let mut best = None;
    let mut closest = f32::INFINITY;
    for (d, dataset) in series.datasets.iter().enumerate() {
        let Some(p) = dataset.points.get(i) else {
            continue;
        };
        let candle = geometry(series, bounds, d, i, first, count);
        let dist = distance(candle.body).min(distance(candle.wick));
        if dist < closest && (!series.options.tooltip_intersect || dist <= 1.) {
            closest = dist;
            best = Some(ChartHit {
                dataset_index: d,
                point_index: i,
                label: p.display_label(),
            });
        }
    }
    best
}

pub(super) fn color(dataset: &ChartDataset, rising: bool, theme: &Theme) -> Hsla {
    dataset.color.unwrap_or_else(|| {
        if rising {
            theme.success()
        } else {
            theme.danger()
        }
    })
}

pub(super) fn paint(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    theme: &Theme,
) {
    each(series, bounds, |d, candle| {
        let color = color(&series.datasets[d], candle.rising, theme);
        window.paint_quad(fill(candle.wick, color));
        window.paint_quad(fill(candle.body, color));
    });
}

pub(super) fn csv(series: &ChartSeries) -> String {
    let mut rows = vec!["dataset,label,open,high,low,close".to_string()];
    if let Some(range) = series.visible_index_range() {
        for dataset in &series.datasets {
            for i in range.clone().take_while(|&i| i < dataset.points.len()) {
                let p = prices(dataset, i);
                rows.push(format!(
                    "{},{},{},{},{},{}",
                    escape_csv(&dataset.label),
                    escape_csv(&dataset.points[i].display_label()),
                    p.open,
                    p.high,
                    p.low,
                    p.close
                ));
            }
        }
    }
    rows.join("\n")
}

pub(super) fn svg(series: &ChartSeries, width: u32, height: u32) -> String {
    use std::fmt::Write as _;
    let title = escape_xml(series.options.title.as_deref().unwrap_or("Chart"));
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" role="img"><title>{title}</title>"#
    );
    let bounds = Bounds::new(
        Point::default(),
        gpui::size(px(width as f32), px(height as f32)),
    );
    each(series, bounds, |d, candle| {
        let color = series.datasets[d].color.map_or_else(
            || {
                if candle.rising {
                    "#16a34a".to_owned()
                } else {
                    "#dc2626".to_owned()
                }
            },
            |c| format!("#{:06x}", u32::from(c.to_rgb()) >> 8),
        );
        for r in [candle.wick, candle.body] {
            let _ = write!(
                svg,
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{color}"/>"#,
                f32::from(r.left()),
                f32::from(r.top()),
                f32::from(r.size.width),
                f32::from(r.size.height)
            );
        }
    });
    svg.push_str("</svg>");
    svg
}
