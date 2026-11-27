//! Shared radial projection for rendering, export and pointer selection.

use super::*;

struct RadarPlot {
    center: Point<Pixels>,
    radius: Pixels,
    first: usize,
    count: usize,
}

impl RadarPlot {
    fn new(bounds: Bounds<Pixels>, series: &ChartSeries) -> Option<Self> {
        let visible = series.visible_index_range()?;
        let first = *visible.start();
        let count = visible.count();
        if count < 3 {
            return None;
        }
        let margin = if series.options.show_axes {
            px(24.)
        } else {
            px(6.)
        };
        let radius = bounds.size.width.min(bounds.size.height) / 2. - margin;
        (radius > px(0.)).then_some(Self {
            center: bounds.center(),
            radius,
            first,
            count,
        })
    }

    fn point(&self, index: usize, fraction: f32) -> Point<Pixels> {
        let angle = std::f32::consts::TAU * (index - self.first) as f32 / self.count as f32
            - std::f32::consts::FRAC_PI_2;
        point_at_angle(self.center, self.radius * fraction, angle)
    }

    fn vertices<'a>(
        &'a self,
        series: &'a ChartSeries,
        dataset: usize,
    ) -> impl Iterator<Item = (usize, Point<Pixels>)> + 'a {
        let axis = series.rendered_axis(dataset);
        series.datasets[dataset]
            .points
            .iter()
            .enumerate()
            .skip(self.first)
            .take(self.count)
            .map(move |(index, p)| {
                (
                    index,
                    self.point(
                        index,
                        axis.normalize(series.scale_rendered_value(dataset, p.value)) as f32,
                    ),
                )
            })
    }
}

pub(super) fn hit_test(
    series: &ChartSeries,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<ChartHit> {
    let plot = RadarPlot::new(
        Bounds::new(Point::default(), gpui::size(px(width), px(height))),
        series,
    )?;
    let mut best = None;
    let mut distance = f32::INFINITY;
    for dataset in 0..series.datasets.len() {
        for (index, p) in plot.vertices(series, dataset) {
            let d = (f32::from(p.x) - x).hypot(f32::from(p.y) - y);
            if d < distance
                && (!series.options.tooltip_intersect || d <= series.options.tooltip_hit_radius)
            {
                distance = d;
                best = Some((dataset, index));
            }
        }
    }
    best.map(|(dataset_index, point_index)| ChartHit {
        dataset_index,
        point_index,
        label: series.datasets[dataset_index].points[point_index].display_label(),
    })
}

fn path(
    points: impl IntoIterator<Item = Point<Pixels>>,
    mut builder: PathBuilder,
    closed: bool,
    color: Hsla,
    window: &mut Window,
) {
    let mut points = points.into_iter();
    let Some(first) = points.next() else { return };
    builder.move_to(first);
    for p in points {
        builder.line_to(p);
    }
    if closed {
        builder.close();
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

pub(super) fn paint(
    bounds: Bounds<Pixels>,
    series: &ChartSeries,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(plot) = RadarPlot::new(bounds, series) else {
        return;
    };
    let theme = Theme::global(cx);
    let label_color = theme.muted_foreground();
    let font_size = px(theme.typography.text_sm);
    let axes = plot.first..plot.first + plot.count;
    if series.options.show_grid {
        for level in 1..=4 {
            path(
                axes.clone().map(|i| plot.point(i, level as f32 / 4.)),
                PathBuilder::stroke(px(1.)),
                true,
                theme.border(),
                window,
            );
        }
    }
    if series.options.show_axes {
        for i in axes.clone() {
            path(
                [plot.center, plot.point(i, 1.)],
                PathBuilder::stroke(px(1.)),
                false,
                theme.border(),
                window,
            );
        }
    }
    for (dataset, data) in series.datasets.iter().enumerate() {
        let vertices: Vec<_> = plot.vertices(series, dataset).map(|(_, p)| p).collect();
        let closed = vertices.len() == plot.count;
        let color = data.color.unwrap_or_else(|| palette(dataset, theme));
        if closed {
            path(
                vertices.iter().copied(),
                PathBuilder::fill(),
                true,
                color.opacity(0.16),
                window,
            );
        }
        path(
            vertices.iter().copied(),
            PathBuilder::stroke(px(2.)),
            closed,
            color,
            window,
        );
        for p in vertices {
            window.paint_quad(
                fill(
                    Bounds::new(p - point(px(3.), px(3.)), gpui::size(px(6.), px(6.))),
                    color,
                )
                .corner_radii(px(3.)),
            );
        }
    }
    if series.options.show_axes {
        for tick in series.layout_domain_labels() {
            let anchor = plot.point(tick.point_index, 1. + 12. / f32::from(plot.radius));
            let mut run = window.text_style().to_run(tick.label.len());
            run.color = label_color;
            let line = window
                .text_system()
                .shape_line(tick.label, font_size, &[run], None);
            let x = (anchor.x - line.width / 2.).clamp(
                bounds.left(),
                (bounds.right() - line.width).max(bounds.left()),
            );
            let _ = line.paint(
                point(x, anchor.y - font_size / 2.),
                font_size,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}

pub(super) fn svg(series: &ChartSeries, width: u32, height: u32) -> String {
    use std::fmt::Write as _;
    let title = escape_xml(series.options.title.as_deref().unwrap_or("Chart"));
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img"><title>{title}</title>"#
    );
    if let Some(plot) = RadarPlot::new(
        Bounds::new(
            Point::default(),
            gpui::size(px(width as f32), px(height as f32)),
        ),
        series,
    ) {
        for (i, dataset) in series.datasets.iter().enumerate() {
            let points: Vec<_> = plot
                .vertices(series, i)
                .map(|(_, p)| format!("{:.2},{:.2}", f32::from(p.x), f32::from(p.y)))
                .collect();
            let name = escape_xml(&dataset.label);
            let joined = points.join(" ");
            let color = dataset.color.map_or_else(
                || svg_palette(i).to_owned(),
                |color| format!("#{:06x}", u32::from(color.to_rgb()) >> 8),
            );
            if points.len() == plot.count {
                let _ = write!(
                    svg,
                    r#"<polygon aria-label="{name}" points="{joined}" fill="{color}" fill-opacity="0.16" stroke="{color}"/>"#
                );
            } else {
                let _ = write!(
                    svg,
                    r#"<polyline aria-label="{name}" points="{joined}" fill="none" stroke="{color}"/>"#
                );
            }
        }
    }
    svg.push_str("</svg>");
    svg
}
