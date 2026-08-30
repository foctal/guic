use gpui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use guic::prelude::{
    AreaChart, BarChart, ChartAxis, ChartDataset, ChartOptions, ChartPoint, ChartScale,
    ChartValueFormatter, DoughnutChart, HorizontalBarChart, Label, LineChart, PieChart, Root,
    ScatterChart, ScrollArea,
};

struct ChartsDashboard;

impl ChartsDashboard {
    fn monthly_actual() -> ChartDataset {
        ChartDataset::new("actual", "Actual").points(vec![
            ChartPoint::category("Jan", 12.0),
            ChartPoint::category("Feb", 18.0),
            ChartPoint::category("Mar", 14.0),
            ChartPoint::category("Apr", 26.0),
            ChartPoint::category("May", 32.0),
            ChartPoint::category("Jun", 28.0),
        ])
    }

    fn monthly_forecast() -> ChartDataset {
        ChartDataset::new("forecast", "Forecast").points(vec![
            ChartPoint::category("Jan", 10.0),
            ChartPoint::category("Feb", 16.0),
            ChartPoint::category("Mar", 20.0),
            ChartPoint::category("Apr", 30.0),
            ChartPoint::category("May", 34.0),
            ChartPoint::category("Jun", 38.0),
        ])
    }
}

impl Render for ChartsDashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = guic::tokens::Theme::global(cx);
        let base_options = ChartOptions::default()
            .height(220.0)
            .values(true)
            .tooltip(true);
        let stacked_options = base_options
            .clone()
            .title("Build Throughput")
            .stacked(true)
            .value_formatter(ChartValueFormatter::Suffix("jobs".into()))
            .crosshair_index(Some(3));
        let actual = Self::monthly_actual();
        let forecast = Self::monthly_forecast();

        let content = div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .bg(theme.background())
            .child(Label::new("GUIC Charts Dashboard").secondary("Runnable guic-charts example"))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_4()
                    .child(
                        LineChart::new("dashboard-line")
                            .options(base_options.clone().title("Revenue Trend"))
                            .datasets(vec![actual.clone(), forecast.clone()]),
                    )
                    .child(
                        BarChart::new("dashboard-bar")
                            .options(stacked_options)
                            .datasets(vec![actual.clone(), forecast.clone()]),
                    )
                    .child(
                        AreaChart::new("dashboard-area")
                            .options(
                                base_options
                                    .clone()
                                    .title("Coverage")
                                    .scale(ChartScale::Log10)
                                    .value_formatter(ChartValueFormatter::Percent)
                                    .crosshair_index(Some(4)),
                            )
                            .datasets(vec![forecast]),
                    )
                    .child(
                        PieChart::new("dashboard-pie")
                            .options(base_options.title("Component Mix").axes(false))
                            .datasets(vec![ChartDataset::new("mix", "Mix").points(vec![
                                ChartPoint::category("Components", 52.0),
                                ChartPoint::category("Charts", 18.0),
                                ChartPoint::category("Editor", 16.0),
                                ChartPoint::category("Terminal", 14.0),
                            ])]),
                    ),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap_4()
                    .child(
                        HorizontalBarChart::new("dashboard-horizontal")
                            .options(
                                ChartOptions::default()
                                    .height(180.0)
                                    .title("Issue Mix")
                                    .value_formatter(ChartValueFormatter::Suffix("items".into())),
                            )
                            .datasets(vec![ChartDataset::new("issues", "Issues").points(vec![
                                ChartPoint::category("Bug", 42.0),
                                ChartPoint::category("Feature", 28.0),
                                ChartPoint::category("Docs", 18.0),
                            ])]),
                    )
                    .child(
                        ScatterChart::new("dashboard-scatter")
                            .options(
                                ChartOptions::default()
                                    .height(180.0)
                                    .title("Latency Samples")
                                    .domain(ChartAxis::new(1.0, 4.0))
                                    .value_formatter(ChartValueFormatter::Suffix("ms".into())),
                            )
                            .datasets(vec![ChartDataset::new("samples", "Samples").points(vec![
                                ChartPoint::category("P1", 18.0),
                                ChartPoint::category("P2", 22.0),
                                ChartPoint::category("P3", 17.0),
                                ChartPoint::category("P4", 29.0),
                                ChartPoint::category("P5", 24.0),
                            ])]),
                    )
                    .child(
                        DoughnutChart::new("dashboard-doughnut")
                            .options(
                                ChartOptions::default()
                                    .height(180.0)
                                    .title("Runtime Split")
                                    .axes(false)
                                    .doughnut_cutout(0.62),
                            )
                            .datasets(vec![ChartDataset::new("runtime", "Runtime").points(vec![
                                ChartPoint::category("Core", 46.0),
                                ChartPoint::category("Components", 34.0),
                                ChartPoint::category("Subsystems", 20.0),
                            ])]),
                    ),
            );

        ScrollArea::new("charts-dashboard-scroll", content)
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1120.), px(760.)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC Charts Dashboard".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let app = cx.new(|_| ChartsDashboard);
            cx.new(|cx| Root::new(app, window, cx))
        })
        .expect("failed to open charts dashboard window");
    })
}
