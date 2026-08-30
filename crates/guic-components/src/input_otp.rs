use gpui::{
    App, ClickEvent, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_tokens::Theme;
use std::rc::Rc;

type OtpChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A controlled one-time-code input display.
///
/// `InputOtp` intentionally leaves text capture to the host for now. It renders
/// a stable sequence of slots, normalizes incoming values to the configured
/// length, and emits clear/backspace intents from its affordances.
#[derive(gpui::IntoElement)]
pub struct InputOtp {
    id: SharedString,
    value: SharedString,
    length: usize,
    disabled: bool,
    masked: bool,
    focus_handle: Option<FocusHandle>,
    on_change: Option<OtpChangeHandler>,
}

impl InputOtp {
    /// Creates an OTP input with six slots.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: SharedString::default(),
            length: 6,
            disabled: false,
            masked: false,
            focus_handle: None,
            on_change: None,
        }
    }

    /// Sets the current value. Extra characters are ignored.
    #[must_use]
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = normalize_otp_value(value.into().as_ref(), self.length).into();
        self
    }

    /// Sets the number of visible slots.
    #[must_use]
    pub fn length(mut self, length: usize) -> Self {
        self.length = length.clamp(1, 12);
        self.value = normalize_otp_value(self.value.as_ref(), self.length).into();
        self
    }

    /// Sets whether the input is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets whether filled slots render as bullets.
    #[must_use]
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// Sets a focus handle so the OTP surface can receive keyboard input.
    #[must_use]
    pub fn focusable(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Registers a handler for requested value changes.
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for InputOtp {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut root = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap(px(theme.spacing.x2))
            .opacity(if self.disabled { 0.55 } else { 1.0 });
        if let Some(handle) = &self.focus_handle {
            root = root.key_context("GuicInputOtp").track_focus(handle);
        }

        for index in 0..self.length {
            let text = chars.get(index).map_or(String::new(), |ch| {
                if self.masked {
                    "•".to_owned()
                } else {
                    ch.to_string()
                }
            });
            root = root.child(
                div()
                    .w(px(40.0))
                    .h(px(44.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme.radius.md))
                    .border_1()
                    .border_color(theme.border())
                    .bg(if index == chars.len().min(self.length.saturating_sub(1)) {
                        theme.secondary().opacity(0.18)
                    } else {
                        theme.background()
                    })
                    .text_size(px(theme.typography.text_lg))
                    .child(text),
            );
        }

        if !self.disabled && self.on_change.is_some() {
            if let (Some(handle), Some(handler)) =
                (self.focus_handle.clone(), self.on_change.clone())
            {
                let current_value = self.value.clone();
                let length = self.length;
                root = root
                    .cursor_text()
                    .on_click(move |event, window, cx| {
                        let _ = event;
                        window.focus(&handle, cx);
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let mut next = current_value.to_string();
                        match event.keystroke.key.as_str() {
                            "backspace" => {
                                next.pop();
                            }
                            "delete" | "escape" => {
                                next.clear();
                            }
                            _ => {
                                if let Some(text) = event.keystroke.key_char.as_deref() {
                                    for ch in text.chars().filter(|ch| !ch.is_whitespace()) {
                                        if next.chars().count() < length {
                                            next.push(ch);
                                        }
                                    }
                                }
                            }
                        }
                        handler(
                            &SharedString::from(normalize_otp_value(&next, length)),
                            window,
                            cx,
                        );
                    });
            }
            let backspace_value = self
                .value
                .chars()
                .take(self.value.chars().count().saturating_sub(1))
                .collect::<String>();
            let clear_value = SharedString::default();
            let backspace_handler = self.on_change.clone();
            let clear_handler = self.on_change.clone();
            let action_style = |id: String, label: &'static str, theme: &Theme| {
                div()
                    .id(id)
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radius.sm))
                    .text_size(px(theme.typography.text_sm))
                    .text_color(theme.muted_foreground())
                    .cursor_pointer()
                    .child(label)
            };
            root = root
                .child(
                    action_style(format!("{}-backspace", self.id), "Backspace", theme).on_click(
                        move |event: &ClickEvent, window, cx| {
                            let _ = event;
                            if let Some(handler) = backspace_handler.as_ref() {
                                handler(&SharedString::from(backspace_value.clone()), window, cx);
                            }
                        },
                    ),
                )
                .child(
                    action_style(format!("{}-clear", self.id), "Clear", theme).on_click(
                        move |event: &ClickEvent, window, cx| {
                            let _ = event;
                            if let Some(handler) = clear_handler.as_ref() {
                                handler(&clear_value, window, cx);
                            }
                        },
                    ),
                );
        }

        root
    }
}

fn normalize_otp_value(value: &str, length: usize) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .take(length)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{InputOtp, normalize_otp_value};

    #[test]
    fn otp_value_is_normalized_to_length() {
        assert_eq!(normalize_otp_value("12 34 56", 4), "1234");
        let input = InputOtp::new("otp").length(4).value("123456").masked(true);
        assert_eq!(input.value, "1234");
        assert_eq!(input.length, 4);
        assert!(input.masked);
    }
}
