use crate::{ButtonVariant, ClickHandler, ComponentSize, IconButton};
use gpui::{
    App, ClickEvent, Empty, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// Supported alert variants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AlertVariant {
    /// Neutral informational notice.
    #[default]
    Neutral,
    /// Positive success notice.
    Success,
    /// Warning notice.
    Warning,
    /// Destructive or error notice.
    Danger,
    /// Informational accent notice.
    Info,
}

/// A compact callout for messages that need emphasis.
#[derive(gpui::IntoElement)]
pub struct Alert {
    title: Option<SharedString>,
    message: SharedString,
    variant: AlertVariant,
    size: ComponentSize,
    closable: bool,
    on_close: Option<ClickHandler>,
}

impl Alert {
    /// Creates a new alert with a message.
    #[must_use]
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            title: None,
            message: message.into(),
            variant: AlertVariant::Neutral,
            size: ComponentSize::Medium,
            closable: false,
            on_close: None,
        }
    }

    /// Sets the alert title.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the alert variant.
    #[must_use]
    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Applies the informational variant.
    #[must_use]
    pub fn info(mut self) -> Self {
        self.variant = AlertVariant::Info;
        self
    }

    /// Applies the success variant.
    #[must_use]
    pub fn success(mut self) -> Self {
        self.variant = AlertVariant::Success;
        self
    }

    /// Applies the warning variant.
    #[must_use]
    pub fn warning(mut self) -> Self {
        self.variant = AlertVariant::Warning;
        self
    }

    /// Applies the danger variant.
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.variant = AlertVariant::Danger;
        self
    }

    /// Sets the alert size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Enables a close affordance.
    #[must_use]
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Registers a close handler and enables the close affordance.
    #[must_use]
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.closable = true;
        self.on_close = Some(Rc::new(on_close));
        self
    }

    fn icon_name(&self) -> IconName {
        match self.variant {
            AlertVariant::Neutral | AlertVariant::Info => IconName::Info,
            AlertVariant::Success => IconName::CheckCircle,
            AlertVariant::Warning => IconName::AlertTriangle,
            AlertVariant::Danger => IconName::XCircle,
        }
    }
}

impl RenderOnce for Alert {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let icon_name = self.icon_name();
        let has_title = self.title.is_some();
        let theme = Theme::global(cx);
        let (background, foreground, border) = match self.variant {
            AlertVariant::Neutral => (
                theme.secondary().opacity(0.45),
                theme.foreground(),
                theme.border(),
            ),
            AlertVariant::Success => (
                theme.success().opacity(0.12),
                theme.success(),
                theme.success().opacity(0.35),
            ),
            AlertVariant::Warning => (
                theme.warning().opacity(0.12),
                theme.warning(),
                theme.warning().opacity(0.35),
            ),
            AlertVariant::Danger => (
                theme.danger().opacity(0.12),
                theme.danger(),
                theme.danger().opacity(0.35),
            ),
            AlertVariant::Info => (
                theme.info().opacity(0.12),
                theme.info(),
                theme.info().opacity(0.35),
            ),
        };

        let (padding_x, padding_y, gap, title_size, body_size) = match self.size {
            ComponentSize::Small => (
                px(theme.spacing.x3),
                px(theme.spacing.x2),
                px(theme.spacing.x2),
                px(theme.typography.text_sm),
                px(theme.typography.text_sm),
            ),
            ComponentSize::Medium => (
                px(theme.spacing.x4),
                px(theme.spacing.x3),
                px(theme.spacing.x3),
                px(theme.typography.text_md),
                px(theme.typography.text_sm),
            ),
            ComponentSize::Large => (
                px(theme.spacing.x5),
                px(theme.spacing.x4),
                px(theme.spacing.x4),
                px(theme.typography.text_lg),
                px(theme.typography.text_md),
            ),
        };

        let mut body = div().flex_1().flex().flex_col().gap_1().child(
            div()
                .text_size(body_size)
                .text_color(foreground)
                .child(self.message),
        );

        if let Some(title) = self.title {
            body = div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(title_size)
                        .text_color(theme.foreground())
                        .child(title),
                )
                .child(body);
        }

        if !has_title {
            body = body.justify_center();
        }

        div()
            .w_full()
            .flex()
            .items_start()
            .gap(gap)
            .px(padding_x)
            .py(padding_y)
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(border)
            .bg(background)
            .child(
                Icon::new(icon_name)
                    .color(foreground)
                    .label(match self.variant {
                        AlertVariant::Neutral | AlertVariant::Info => "Information",
                        AlertVariant::Success => "Success",
                        AlertVariant::Warning => "Warning",
                        AlertVariant::Danger => "Error",
                    }),
            )
            .child(body)
            .child(if self.closable {
                IconButton::new(IconName::X)
                    .variant(ButtonVariant::Ghost)
                    .size(ComponentSize::Small)
                    .label("Dismiss alert")
                    .on_click_option(self.on_close)
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
    }
}
