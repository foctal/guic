use gpui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use guic::prelude::{Alert, Badge, Button, Label, Progress, Separator, Spinner};

struct AppView;

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_6()
            .gap_4()
            .flex()
            .flex_col()
            .bg(guic::tokens::Theme::global(cx).background())
            .child(Label::new("Welcome to GUIC").secondary("Native. Fast. Solid."))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(Badge::new("alpha").primary())
                    .child(Badge::new("native-first")),
            )
            .child(Separator::new())
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(Spinner::new())
                    .child(Label::new("Loading foundation components...").muted(true)),
            )
            .child(
                Alert::new("The application is using GUIC components and theme tokens.")
                    .title("Status")
                    .info(),
            )
            .child(
                div()
                    .w(px(240.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Progress::new(64.0).id("hello-world-progress"))
                    .child(Button::new("Continue").primary()),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(800.), px(600.)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC Hello World".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let app = cx.new(|_| AppView);
            cx.new(|cx| guic::core::Root::new(app, window, cx))
        })
        .expect("failed to open hello world window");
    })
}
