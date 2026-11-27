//! Vector snapshots of Cartesian and proportional charts.

use super::*;
use std::fmt::Write as _;

fn coordinates(points: &[Point<Pixels>]) -> String {
    let mut out = String::new();
    for p in points {
        let _ = write!(out, "{:.2},{:.2} ", f32::from(p.x), f32::from(p.y));
    }
    out
}

pub(super) fn snapshot(series: &ChartSeries, width: u32, height: u32) -> String {
    let title = escape_xml(series.options.title.as_deref().unwrap_or("Chart"));
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img"><title>{title}</title>"#
    );
    let canvas = Bounds::new(
        Point::default(),
        gpui::size(px(width as f32), px(height as f32)),
    );
    let bounds = plot_bounds_for(canvas, series);
    if bounds.size.width > px(0.) && bounds.size.height > px(0.) {
        if series.is_radial() {
            let cutout = if series.kind == ChartKind::Doughnut {
                series.options.doughnut_cutout
            } else {
                0.
            };
            for (index, start, end) in series.radial_slices() {
                let points = coordinates(&sector_vertices(bounds, cutout, start, end));
                let color = svg_palette(index);
                let label = escape_xml(&series.datasets[0].points[index].display_label());
                let _ = write!(
                    svg,
                    r#"<polygon aria-label="{label}" points="{points}" fill="{color}"/>"#
                );
            }
        } else if let Some(visible) = series.visible_index_range() {
            let count = visible.clone().count();
            for (index, dataset) in series.datasets.iter().enumerate() {
                let color = dataset.color.map_or_else(
                    || svg_palette(index).to_owned(),
                    |c| format!("#{:08x}", u32::from(c.to_rgb())),
                );
                let name = escape_xml(&dataset.label);
                let _ = write!(svg, r#"<g aria-label="{name}" fill="{color}">"#);
                let kind = dataset.kind.unwrap_or(series.kind);
                let axis = series.rendered_axis(index);
                let baseline = |value| series.scale_rendered_value(index, value);
                let mut positions = Vec::new();
                let mut base_points = Vec::new();
                for i in visible.clone().take_while(|i| *i < dataset.points.len()) {
                    let Some(p) = series.plot_position(index, i, bounds) else {
                        continue;
                    };
                    let value = dataset.points[i].value;
                    let base = if series.is_stacked() {
                        series.stacked_value_at(index, i) - value
                    } else {
                        0.
                    };
                    match kind {
                        ChartKind::Bar | ChartKind::HorizontalBar => {
                            let horizontal = kind == ChartKind::HorizontalBar;
                            let band = if horizontal {
                                bounds.size.height
                            } else {
                                bounds.size.width
                            } / count as f32;
                            let lanes = if series.is_stacked() {
                                1
                            } else {
                                series.bar_lane_count()
                            };
                            let thickness = (band * 0.72 / lanes as f32).max(px(2.));
                            let (x, y, w, h) = if horizontal {
                                let x = x_for_scaled_value(baseline(base), axis, bounds);
                                (
                                    x.min(p.x),
                                    p.y - thickness / 2.,
                                    (p.x - x).abs().max(px(1.)),
                                    thickness,
                                )
                            } else {
                                let y = y_for_scaled_value(baseline(base), axis, bounds);
                                (
                                    p.x - thickness / 2.,
                                    p.y.min(y),
                                    thickness,
                                    (p.y - y).abs().max(px(1.)),
                                )
                            };
                            let _ = write!(
                                svg,
                                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/>"#,
                                f32::from(x),
                                f32::from(y),
                                f32::from(w),
                                f32::from(h)
                            );
                        }
                        ChartKind::Scatter | ChartKind::Bubble => {
                            let r = if kind == ChartKind::Bubble {
                                dataset.points[i]
                                    .bubble_radius()
                                    .unwrap_or(4.)
                                    .clamp(1., 64.)
                            } else {
                                4.
                            };
                            let _ = write!(
                                svg,
                                r#"<circle cx="{:.2}" cy="{:.2}" r="{r}"/>"#,
                                f32::from(p.x),
                                f32::from(p.y)
                            );
                        }
                        ChartKind::Line | ChartKind::Area => {
                            positions.push(p);
                            if kind == ChartKind::Area {
                                let y = if series.is_stacked() {
                                    y_for_scaled_value(baseline(base), axis, bounds)
                                } else {
                                    bounds.bottom()
                                };
                                base_points.push(point(p.x, y));
                            }
                        }
                        _ => {}
                    }
                }
                if !positions.is_empty() {
                    let line = coordinates(&positions);
                    if kind == ChartKind::Area {
                        base_points.reverse();
                        let base = coordinates(&base_points);
                        let _ = write!(
                            svg,
                            r#"<polygon points="{line}{base}" fill-opacity="0.28"/>"#
                        );
                    }
                    let _ = write!(
                        svg,
                        r#"<polyline points="{line}" stroke="{color}" fill="none" stroke-width="2"/>"#
                    );
                    for p in positions {
                        let _ = write!(
                            svg,
                            r#"<circle cx="{:.2}" cy="{:.2}" r="2.5"/>"#,
                            f32::from(p.x),
                            f32::from(p.y)
                        );
                    }
                }
                svg.push_str("</g>");
            }
        }
    }
    svg.push_str("</svg>");
    svg
}
