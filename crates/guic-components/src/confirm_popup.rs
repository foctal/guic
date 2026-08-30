use crate::{Button, ButtonVariant, ClickHandler, ComponentSize};
use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, Styled as _, Window, div, px,
};
use guic_core::{
    AccessibilityElementExt as _, AccessibilityProps, OverlayPriority, Role, overlay_portal,
};
use guic_tokens::Theme;
use std::rc::Rc;

/// An inline confirmation surface anchored below an application-provided trigger.
///
/// The host owns `open` state. Use [`crate::ConfirmDialog`] when the decision
/// must block the rest of the window; use this component for contextual actions.
#[derive(gpui::IntoElement)]
pub struct ConfirmPopup {
    id: SharedString,
    trigger: AnyElement,
    open: bool,
    message: SharedString,
    confirm_label: SharedString,
    cancel_label: SharedString,
    danger: bool,
    on_confirm: Option<ClickHandler>,
    on_cancel: Option<ClickHandler>,
}

impl ConfirmPopup {
    /// Creates a closed confirmation popup associated with `trigger`.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, trigger: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into_any_element(),
            open: false,
            message: SharedString::from("Are you sure?"),
            confirm_label: SharedString::from("Confirm"),
            cancel_label: SharedString::from("Cancel"),
            danger: false,
            on_confirm: None,
            on_cancel: None,
        }
    }
    /// Sets whether the popup is visible.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// Sets the confirmation message.
    #[must_use]
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = message.into();
        self
    }
    /// Sets the confirm button label.
    #[must_use]
    pub fn confirm_label(mut self, label: impl Into<SharedString>) -> Self {
        self.confirm_label = label.into();
        self
    }
    /// Sets the cancel button label.
    #[must_use]
    pub fn cancel_label(mut self, label: impl Into<SharedString>) -> Self {
        self.cancel_label = label.into();
        self
    }
    /// Styles the confirm button destructively.
    #[must_use]
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }
    /// Registers the confirmation callback.
    #[must_use]
    pub fn on_confirm(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }
    /// Registers the cancellation callback.
    #[must_use]
    pub fn on_cancel(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ConfirmPopup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div().id(self.id.clone()).relative().child(self.trigger);
        if self.open {
            let cancel = Button::new(self.cancel_label)
                .variant(ButtonVariant::Secondary)
                .size(ComponentSize::Small);
            let cancel = if let Some(on_cancel) = self.on_cancel {
                cancel.on_click(move |event, window, cx| (on_cancel)(event, window, cx))
            } else {
                cancel
            };
            let variant = if self.danger {
                ButtonVariant::Danger
            } else {
                ButtonVariant::Primary
            };
            let confirm = Button::new(self.confirm_label)
                .variant(variant)
                .size(ComponentSize::Small);
            let confirm = if let Some(on_confirm) = self.on_confirm {
                confirm.on_click(move |event, window, cx| (on_confirm)(event, window, cx))
            } else {
                confirm
            };
            root = root.child(overlay_portal(
                div()
                    .id(format!("{}-panel", self.id))
                    .accessibility(
                        AccessibilityProps::new(Role::Dialog).label(self.message.clone()),
                    )
                    .debug_selector(|| "guic-confirm-popup-panel".to_owned())
                    .absolute()
                    .top_full()
                    .left_0()
                    .mt_2()
                    .w(px(280.))
                    .max_w_full()
                    .p_3()
                    .rounded(px(theme.radius.md))
                    .border_1()
                    .border_color(theme.border())
                    .bg(theme.background())
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(theme.typography.text_sm))
                            .text_color(theme.foreground())
                            .child(self.message),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .justify_end()
                            .gap_2()
                            .child(cancel)
                            .child(confirm),
                    ),
                OverlayPriority::MODAL,
            ));
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::ConfirmPopup;
    use gpui::div;

    #[test]
    fn popup_builder_tracks_configuration() {
        let popup = ConfirmPopup::new("archive", div())
            .open(true)
            .message("Archive this project?")
            .confirm_label("Archive")
            .cancel_label("Keep")
            .danger(true);
        assert!(popup.open);
        assert!(popup.danger);
        assert_eq!(popup.confirm_label, "Archive");
        assert_eq!(popup.cancel_label, "Keep");
    }
}
