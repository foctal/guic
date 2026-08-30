use guic_charts::{
    ChartAxis, ChartDataset, ChartKind, ChartOptions, ChartPoint, ChartSeries, ChartTransition,
};
use std::{hint::black_box, time::Instant};

fn main() {
    benchmark_large_dataset();
    benchmark_pointer_hit_testing();
    benchmark_frequent_updates();
    benchmark_dashboard_models();
}

fn benchmark_pointer_hit_testing() {
    let points: Vec<_> = (0..100_000)
        .map(|index| ChartPoint::category(index.to_string(), (index % 1_000) as f64))
        .collect();
    let line = ChartSeries::new(ChartKind::Line).datasets(vec![
        ChartDataset::new("large", "Large").points(points.clone()),
    ]);
    let bars = ChartSeries::new(ChartKind::Bar)
        .datasets(vec![ChartDataset::new("large", "Large").points(points)]);
    let started = Instant::now();
    for index in 0..10_000 {
        let x = (index % 800) as f32;
        black_box(line.hit_test(x, 200.0, 800.0, 400.0));
        black_box(bars.hit_test(x, 300.0, 800.0, 400.0));
    }
    report("indexed_pointer_hit_test", started, 20_000);
}

fn benchmark_large_dataset() {
    let points = (0..100_000)
        .map(|index| ChartPoint::numeric(index as f64, (index % 1_000) as f64))
        .collect();
    let series = ChartSeries::new(ChartKind::Line)
        .options(ChartOptions::default().domain(ChartAxis::new(40_000.0, 60_000.0)))
        .datasets(vec![ChartDataset::new("large", "Large").points(points)]);
    let started = Instant::now();
    for index in 0..100 {
        black_box(series.hit_test((index % 800) as f32, (index % 400) as f32, 800.0, 400.0));
        black_box(series.domain_ticks(12));
    }
    report("large_dataset_hit_test_and_ticks", started, 100);
}

fn benchmark_frequent_updates() {
    let from = benchmark_series(10_000, 0.0);
    let to = benchmark_series(10_000, 100.0);
    let transition = ChartTransition::new(from, to);
    let started = Instant::now();
    for frame in 0..120 {
        black_box(transition.series_at((frame % 60) as f32 / 59.0));
    }
    report("frequent_update_transition", started, 120);
}

fn benchmark_dashboard_models() {
    let charts = (0..12)
        .map(|chart| benchmark_series(1_000, chart as f64))
        .collect::<Vec<_>>();
    let started = Instant::now();
    for _ in 0..10 {
        for chart in &charts {
            black_box(chart.value_axis());
            black_box(chart.domain_axis());
            black_box(chart.accessible_summary());
        }
    }
    report("dashboard_model_pass", started, 120);
}

fn benchmark_series(point_count: usize, offset: f64) -> ChartSeries {
    ChartSeries::new(ChartKind::Line).datasets(vec![
        ChartDataset::new("series", "Series").points(
            (0..point_count)
                .map(|index| ChartPoint::numeric(index as f64, offset + (index % 100) as f64))
                .collect(),
        ),
    ])
}

fn report(name: &str, started: Instant, iterations: usize) {
    let elapsed = started.elapsed();
    println!(
        "{name}: {iterations} iterations in {elapsed:?} ({:?}/iteration)",
        elapsed / iterations as u32
    );
}
