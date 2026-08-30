use gpui::{
    AppContext as _, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement as _, Render,
    Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
};
use guic::prelude::{
    Label, Root, Terminal, TerminalGridSize, TerminalInputState, TerminalModel, TerminalOptions,
    TerminalSelection,
};
use guic::terminal::{LocalPtySession, TerminalTransport as _};
use std::time::Duration;

struct TerminalDemo {
    model: TerminalModel,
    session: Option<LocalPtySession>,
    input_state: Entity<TerminalInputState>,
    focus: FocusHandle,
    status: String,
}

impl TerminalDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut model = TerminalModel::new(88, 22);
        let (session, status) = match LocalPtySession::spawn_shell(88, 22) {
            Ok(session) => (Some(session), "PTY session running".to_string()),
            Err(error) => {
                model.write(&format!("failed to start PTY: {error}\n"));
                (None, "PTY unavailable".to_string())
            }
        };
        let terminal = Self {
            model,
            session,
            input_state: cx.new(|_| TerminalInputState::new()),
            focus: cx.focus_handle(),
            status,
        };
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this.update(cx, |this, cx| this.refresh_output(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        terminal
    }

    fn refresh_output(&mut self, cx: &mut Context<Self>) {
        let output = self
            .session
            .as_mut()
            .map(LocalPtySession::drain_output)
            .unwrap_or_default();
        if !output.is_empty() {
            self.feed_output(&output);
            cx.notify();
        }
        if let Some(session) = &mut self.session
            && let Some(code) = session.try_exit_code()
        {
            self.status = format!("PTY exited with status {code}");
            cx.notify();
        }
    }

    fn write_input(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            if let Err(error) = session.write(bytes) {
                self.status = format!("PTY write failed: {error}");
            }
            let output = session.drain_output();
            if !output.is_empty() {
                self.feed_output(&output);
            }
            cx.notify();
        }
    }

    fn feed_output(&mut self, output: &[u8]) {
        self.model.write(&String::from_utf8_lossy(output));
        self.flush_terminal_responses();
    }

    fn flush_terminal_responses(&mut self) {
        let responses = self.model.take_response_bytes();
        if responses.is_empty() {
            return;
        }

        if let Some(session) = &mut self.session
            && let Err(error) = session.write(&responses)
        {
            self.status = format!("PTY response write failed: {error}");
        }
    }

    fn update_selection(&mut self, selection: &TerminalSelection, cx: &mut Context<Self>) {
        self.model.set_selection(*selection);
        cx.notify();
    }

    fn scroll_viewport(&mut self, delta: &isize, cx: &mut Context<Self>) {
        if *delta > 0 {
            self.model.scroll_up(delta.unsigned_abs());
        } else {
            self.model.scroll_down(delta.unsigned_abs());
        }
        cx.notify();
    }

    fn resize_terminal(&mut self, size: &TerminalGridSize, cx: &mut Context<Self>) {
        if self.model.columns() == size.columns && self.model.rows() == size.rows {
            return;
        }
        self.model.resize(size.columns, size.rows);
        if let Some(session) = &mut self.session
            && let Err(error) = session.resize(size.columns, size.rows)
        {
            self.status = format!("PTY resize failed: {error}");
        }
        cx.notify();
    }
}

impl Render for TerminalDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = guic::tokens::Theme::global(cx);

        div()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .bg(theme.background())
            .child(Label::new("GUIC Terminal Demo").secondary(self.status.clone()))
            .child(
                div().flex_1().min_h_0().child(
                    Terminal::new("terminal-demo", self.model.clone())
                        .focusable(self.focus.clone())
                        .input_state(self.input_state.clone())
                        .options(
                            TerminalOptions::default()
                                .visible_scrollback(0)
                                .measured_font(),
                        )
                        .on_input(cx.listener(|this, bytes: &[u8], _, cx| {
                            this.write_input(bytes, cx);
                        }))
                        .on_selection(cx.listener(|this, selection: &TerminalSelection, _, cx| {
                            this.update_selection(selection, cx);
                        }))
                        .on_viewport_scroll(cx.listener(|this, delta: &isize, _, cx| {
                            this.scroll_viewport(delta, cx);
                        }))
                        .on_resize(cx.listener(|this, size: &TerminalGridSize, _, cx| {
                            this.resize_terminal(size, cx);
                        })),
                ),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(960.), px(560.)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC Terminal Demo".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let app = cx.new(TerminalDemo::new);
            cx.new(|cx| Root::new(app, window, cx))
        })
        .expect("failed to open terminal demo window");
    })
}
