use gpui::{
    AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, WindowBounds, WindowOptions, div, px, size,
};
use guic_webview::WebView;

struct AppView {
    webview: Entity<WebView>,
}

impl AppView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let webview = cx.new(|cx| {
            let builder = wry::WebViewBuilder::new();

            #[cfg(not(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "ios",
                target_os = "android"
            )))]
            let webview = {
                use gtk::prelude::*;
                use wry::WebViewBuilderExtUnix;

                let fixed = gtk::Fixed::builder().build();
                fixed.show_all();
                builder
                    .build_gtk(&fixed)
                    .expect("failed to create linux webview")
            };

            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "ios",
                target_os = "android"
            ))]
            let webview = {
                use raw_window_handle::HasWindowHandle;

                let window_handle = window.window_handle().expect("no window handle");
                builder
                    .build_as_child(&window_handle)
                    .expect("failed to create child webview")
            };

            WebView::new(webview, cx)
        });

        webview
            .update(cx, |view, _| view.load_url("https://example.com"))
            .expect("failed to load example url");

        Self { webview }
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.webview.clone())
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    // SAFETY: This process-level environment variable is set before the GPUI
    // application is initialized, which avoids concurrent mutation hazards.
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");
    }

    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(800.)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC WebView Example".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let app = cx.new(|cx| AppView::new(window, cx));
            cx.new(|cx| guic::core::Root::new(app, window, cx))
        })
        .expect("failed to open webview example window");
    })
}
