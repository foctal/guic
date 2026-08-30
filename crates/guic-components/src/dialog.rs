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

/// A controlled modal dialog surface.
#[derive(gpui::IntoElement)]
pub struct Dialog {
    id: SharedString,
    open: bool,
    title: Option<SharedString>,
    description: Option<SharedString>,
    content: Option<gpui::AnyElement>,
    primary_label: Option<SharedString>,
    secondary_label: Option<SharedString>,
    dismissible: bool,
    on_confirm: Option<ClickHandler>,
    on_cancel: Option<ClickHandler>,
}

impl Dialog {
    /// Creates a new dialog.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            title: None,
            description: None,
            content: None,
            primary_label: None,
            secondary_label: None,
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
        self.title = Some(title.into());
        self
    }

    /// Sets the dialog description.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the dialog body content.
    #[must_use]
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Sets the primary action label.
    #[must_use]
    pub fn primary_label(mut self, label: impl Into<SharedString>) -> Self {
        self.primary_label = Some(label.into());
        self
    }

    /// Sets the secondary action label.
    #[must_use]
    pub fn secondary_label(mut self, label: impl Into<SharedString>) -> Self {
        self.secondary_label = Some(label.into());
        self
    }

    /// Sets whether outside dismissal is allowed.
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

    /// Registers a cancel handler.
    #[must_use]
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(on_cancel));
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.open {
            return Empty.into_any_element();
        }

        let theme = Theme::global(cx);
        let mut footer = div().flex().flex_wrap().justify_end().gap_3();
        if let Some(label) = self.secondary_label {
            let button = Button::new(label)
                .variant(ButtonVariant::Secondary)
                .size(ComponentSize::Small);
            footer = if let Some(on_cancel) = self.on_cancel.clone() {
                footer
                    .child(button.on_click(move |event, window, cx| (on_cancel)(event, window, cx)))
            } else {
                footer.child(button)
            };
        }

        if let Some(label) = self.primary_label {
            let button = Button::new(label).primary().size(ComponentSize::Small);
            footer = if let Some(on_confirm) = self.on_confirm.clone() {
                footer.child(
                    button.on_click(move |event, window, cx| (on_confirm)(event, window, cx)),
                )
            } else {
                footer.child(button)
            };
        }

        let scrim_id = format!("{}-scrim", self.id);
        let dialog_label = self.title.clone().unwrap_or_else(|| self.id.clone());
        let mut card = div()
            .id(self.id)
            .accessibility(AccessibilityProps::new(Role::Dialog).label(dialog_label))
            .debug_selector(|| "guic-dialog-card".to_owned())
            .w(px(480.0))
            .max_w_full()
            .p_5()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .shadow_xl()
            .flex()
            .flex_col()
            .gap_4();

        if let Some(title) = self.title {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(theme.typography.text_lg))
                            .text_color(theme.foreground())
                            .child(title),
                    )
                    .child(self.description.map_or_else(
                        || Empty.into_any_element(),
                        |description| {
                            div()
                                .text_size(px(theme.typography.text_sm))
                                .text_color(theme.muted_foreground())
                                .child(description)
                                .into_any_element()
                        },
                    )),
            );
        }

        if let Some(content) = self.content {
            card = card.child(content);
        }

        card = card.child(footer);

        let scrim = div()
            .id(scrim_id)
            .debug_selector(|| "guic-dialog-scrim".to_owned())
            .absolute()
            .inset_0()
            .bg(theme.foreground().opacity(0.22));

        let scrim = if self.dismissible {
            if let Some(on_cancel) = self.on_cancel {
                scrim.on_click(move |event: &ClickEvent, window, cx| (on_cancel)(event, window, cx))
            } else {
                scrim
            }
        } else {
            scrim
        };

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
