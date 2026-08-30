use crate::{Button, ButtonVariant, ClickHandler, ComponentSize};
use gpui::{
    App, ClickEvent, Empty, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{
    AccessibilityElementExt as _, AccessibilityProps, OverlayPriority, Role, overlay_portal,
};
use guic_tokens::Theme;
use std::rc::Rc;

/// A focused confirmation modal with explicit confirm and cancel actions.
///
/// Unlike the general [`Dialog`](crate::Dialog), `ConfirmDialog` is purpose-built
/// for yes/no decisions: it always renders both actions and styles the confirm
/// button destructively when [`ConfirmDialog::danger`] is set. It is
/// host-managed via [`ConfirmDialog::open`].
///
/// # Example
///
/// ```no_run
/// use guic_components::ConfirmDialog;
///
/// ConfirmDialog::new("delete-confirm")
///     .open(true)
///     .title("Delete project?")
///     .message("This action cannot be undone.")
///     .confirm_label("Delete")
///     .danger(true)
///     .on_confirm(|_, _, _| { /* perform deletion */ })
///     .on_cancel(|_, _, _| { /* dismiss */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct ConfirmDialog {
    id: SharedString,
    open: bool,
    title: SharedString,
    message: Option<SharedString>,
    confirm_label: SharedString,
    cancel_label: SharedString,
    danger: bool,
    dismissible: bool,
    on_confirm: Option<ClickHandler>,
    on_cancel: Option<ClickHandler>,
}

impl ConfirmDialog {
    /// Creates a new, closed confirmation dialog.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            title: SharedString::from("Are you sure?"),
            message: None,
            confirm_label: SharedString::from("Confirm"),
            cancel_label: SharedString::from("Cancel"),
            danger: false,
            dismissible: true,
            on_confirm: None,
            on_cancel: None,
        }
    }

    /// Sets whether the dialog is visible.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Sets the dialog title.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the explanatory message.
    #[must_use]
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
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

    /// Styles the confirm action destructively.
    #[must_use]
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Sets whether clicking the scrim cancels the dialog.
    #[must_use]
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Registers a confirmation handler.
    #[must_use]
    pub fn on_confirm(
        mut self,
        on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Rc::new(on_confirm));
        self
    }

    /// Registers a cancel handler (also fired by the dismiss scrim).
    #[must_use]
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(on_cancel));
        self
    }
}

impl RenderOnce for ConfirmDialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.open {
            return Empty.into_any_element();
        }

        let theme = Theme::global(cx);

        let cancel_button = Button::new(self.cancel_label)
            .variant(ButtonVariant::Secondary)
            .size(ComponentSize::Small);
        let cancel_button = if let Some(on_cancel) = self.on_cancel.clone() {
            cancel_button.on_click(move |event, window, cx| (on_cancel)(event, window, cx))
        } else {
            cancel_button
        };

        let confirm_variant = if self.danger {
            ButtonVariant::Danger
        } else {
            ButtonVariant::Primary
        };
        let confirm_button = Button::new(self.confirm_label)
            .variant(confirm_variant)
            .size(ComponentSize::Small);
        let confirm_button = if let Some(on_confirm) = self.on_confirm.clone() {
            confirm_button.on_click(move |event, window, cx| (on_confirm)(event, window, cx))
        } else {
            confirm_button
        };

        let footer = div()
            .flex()
            .flex_wrap()
            .justify_end()
            .gap_3()
            .child(cancel_button)
            .child(confirm_button);

        let mut card = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::AlertDialog)
                    .label(self.title.clone())
                    .description(self.message.clone().unwrap_or_default()),
            )
            .debug_selector(|| "guic-confirm-dialog-card".to_owned())
            .w(px(420.0))
            .max_w_full()
            .p_5()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .shadow_xl()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(theme.typography.text_lg))
                    .text_color(theme.foreground())
                    .child(self.title),
            );

        if let Some(message) = self.message {
            card = card.child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(theme.muted_foreground())
                    .child(message),
            );
        }

        card = card.child(footer);

        let scrim_id = format!("{}-scrim", self.id);
        let mut scrim = div()
            .id(SharedString::from(scrim_id))
            .debug_selector(|| "guic-confirm-dialog-scrim".to_owned())
            .absolute()
            .inset_0()
            .bg(theme.foreground().opacity(0.22));
        if self.dismissible
            && let Some(on_cancel) = self.on_cancel.clone()
        {
            scrim = scrim
                .on_click(move |event: &ClickEvent, window, cx| (on_cancel)(event, window, cx));
        }

        overlay_portal(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(scrim)
                .child(card),
            OverlayPriority::MODAL,
        )
    }
}
