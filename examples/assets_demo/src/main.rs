use gpui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowBounds, WindowOptions, div, px, size, svg,
};
use guic::prelude::{Label, MetricCard, PropertyItem, PropertyList, Root};
use guic_assets::{AssetManifest, FileAssetSource};
use std::path::PathBuf;

struct AssetsDemo;

impl Render for AssetsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = guic::tokens::Theme::global(cx);
        let manifest = AssetManifest::global(cx);
        let count = manifest.len().to_string();

        div()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .bg(theme.background())
            .child(Label::new("GUIC Assets Demo").secondary("FileAssetSource integration sample"))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .flex_wrap()
                    .child(MetricCard::new("Registered Assets", count).detail("Manifest-backed"))
                    .child(MetricCard::new("Source", "Filesystem").detail("GPUI AssetSource")),
            )
            .child(
                div()
                    .flex()
                    .gap_6()
                    .items_center()
                    .child(
                        svg()
                            .path("logo.svg")
                            .w(px(96.0))
                            .h(px(96.0))
                            .text_color(theme.primary()),
                    )
                    .child(PropertyList::new("Manifest Entries").items(vec![
                        PropertyItem::new("demo/logo.svg", "vector/logo.svg"),
                        PropertyItem::new("demo/guide.txt", "data/guide.txt"),
                    ])),
            )
    }
}

fn main() {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    gpui_platform::application()
        .with_assets(FileAssetSource::new(&asset_root))
        .run(move |cx: &mut gpui::App| {
            guic::init(cx);
            let manifest = AssetManifest::global_mut(cx);
            manifest
                .register_directory_inferred("demo", &asset_root)
                .expect("assets should register from directory");

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(900.), px(640.)),
                    cx,
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("GUIC Assets Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                let app = cx.new(|_| AssetsDemo);
                cx.new(|cx| Root::new(app, window, cx))
            })
            .expect("failed to open assets demo window");
        });
}
