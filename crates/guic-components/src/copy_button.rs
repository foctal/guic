//! A text copy action with short-lived confirmation.
use crate::{Button, ButtonVariant, ComponentSize};
use gpui::{App, ClipboardItem, IntoElement, RenderOnce, SharedString, Task, Window};
use std::time::Duration;

#[derive(Default)]
struct CopyState {
    copied: Option<SharedString>,
    timer: Option<Task<()>>,
}

/// Copies a string to the platform clipboard and briefly confirms the action.
#[derive(gpui::IntoElement)]
pub struct CopyButton {
    id: SharedString,
    text: SharedString,
    label: SharedString,
    copied_label: SharedString,
    disabled: bool,
}

impl CopyButton {
    /// Creates a copy button with stable identity and the text to copy.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            label: "Copy".into(),
            copied_label: "Copied".into(),
            disabled: false,
        }
    }
    /// Sets the idle and confirmation captions, including for accessibility.
    #[must_use]
    pub fn labels(
        mut self,
        idle: impl Into<SharedString>,
        copied: impl Into<SharedString>,
    ) -> Self {
        self.label = idle.into();
        self.copied_label = copied.into();
        self
    }
    /// Disables clipboard writes and focus.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for CopyButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(format!("{}-copy-state", self.id), cx, |_, _| {
            CopyState::default()
        });
        let label = if state.read(cx).copied.as_ref() == Some(&self.text) {
            self.copied_label
        } else {
            self.label
        };
        Button::new(label)
            .id(self.id)
            .variant(ButtonVariant::Secondary)
            .size(ComponentSize::Small)
            .disabled(self.disabled)
            .on_click(move |_, window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(self.text.to_string()));
                state.update(cx, |state, cx| {
                    state.copied = Some(self.text.clone());
                    state.timer = Some(cx.spawn_in(window, async move |entity, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(1500))
                            .await;
                        let _ = entity.update(cx, |state, cx| {
                            state.copied = None;
                            cx.notify();
                        });
                    }));
                    cx.notify();
                });
            })
    }
}
