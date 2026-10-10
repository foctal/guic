use gpui::{
    AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, WindowBounds, WindowOptions, div, px, size,
};
use guic_webview::WebView;

#[cfg(target_os = "linux")]
mod linux;

struct AppView {
    webview: Entity<WebView>,
}

impl AppView {
    fn new(window: &mut Window, url: &str, cx: &mut Context<Self>) -> Self {
        let webview = cx.new(|cx| {
            let builder = wry::WebViewBuilder::new().with_on_page_load_handler(|event, url| {
                let event = match event {
                    wry::PageLoadEvent::Started => "started",
                    wry::PageLoadEvent::Finished => "finished",
                };
                eprintln!("WebView {event}: {url}");
            });

            #[cfg(target_os = "linux")]
            let webview =
                linux::build_child(builder, window).expect("failed to create Linux child webview");

            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "ios",
                target_os = "android"
            ))]
            let webview = {
                use raw_window_handle::HasWindowHandle;

                let window_handle =
                    HasWindowHandle::window_handle(window).expect("no window handle");
                builder
                    .build_as_child(&window_handle)
                    .expect("failed to create child webview")
            };

            WebView::new(webview, cx)
        });

        webview
            .update(cx, |view, _| view.load_url(url))
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
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".into());

    #[cfg(target_os = "linux")]
    if let Err(error) = linux::init() {
        eprintln!("GUIC WebView Example: {error}");
        std::process::exit(1);
    }

    #[cfg(target_os = "windows")]
    // SAFETY: This process-level environment variable is set before the GPUI
    // application is initialized, which avoids concurrent mutation hazards.
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");
    }

    gpui_platform::application().run(move |cx: &mut gpui::App| {
        guic::init(cx);

        #[cfg(target_os = "linux")]
        linux::run_gtk_loop(cx);

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
            let app = cx.new(|cx| AppView::new(window, &url, cx));
            cx.new(|cx| guic::core::Root::new(app, window, cx))
        })
        .expect("failed to open webview example window");
    })
}
