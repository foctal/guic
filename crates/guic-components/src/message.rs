use gpui::{
    App, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div,
    px,
};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;

/// Severity levels for an inline [`Message`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MessageVariant {
    /// Neutral informational note.
    #[default]
    Info,
    /// Positive confirmation note.
    Success,
    /// Cautionary note.
    Warning,
    /// Error note (for example, form validation).
    Danger,
}

/// A compact, inline severity note.
///
/// `Message` is lighter than [`Alert`](crate::Alert): it is a single-line,
/// accent-bordered row intended to sit beside form fields and inline content
/// rather than act as a prominent callout. It carries no title or dismiss
/// affordance.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Message, MessageVariant};
///
/// Message::new("Password must be at least 8 characters")
///     .variant(MessageVariant::Danger);
/// ```
#[derive(gpui::IntoElement)]
pub struct Message {
    text: SharedString,
    variant: MessageVariant,
}

impl Message {
    /// Creates a new informational message.
    #[must_use]
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            variant: MessageVariant::Info,
        }
    }

    /// Sets the message variant.
    #[must_use]
    pub fn variant(mut self, variant: MessageVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Applies the success variant.
    #[must_use]
    pub fn success(mut self) -> Self {
        self.variant = MessageVariant::Success;
        self
    }

    /// Applies the warning variant.
    #[must_use]
    pub fn warning(mut self) -> Self {
        self.variant = MessageVariant::Warning;
        self
    }

    /// Applies the danger variant.
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.variant = MessageVariant::Danger;
        self
    }

    fn accent(&self, theme: &Theme) -> Hsla {
        match self.variant {
            MessageVariant::Info => theme.info(),
            MessageVariant::Success => theme.success(),
            MessageVariant::Warning => theme.warning(),
            MessageVariant::Danger => theme.danger(),
        }
    }

    fn icon_name(&self) -> IconName {
        match self.variant {
            MessageVariant::Info => IconName::Info,
            MessageVariant::Success => IconName::CheckCircle,
            MessageVariant::Warning => IconName::AlertTriangle,
            MessageVariant::Danger => IconName::XCircle,
        }
    }
}

impl RenderOnce for Message {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let accent = self.accent(theme);
        let label = match self.variant {
            MessageVariant::Info => "Information",
            MessageVariant::Success => "Success",
            MessageVariant::Warning => "Warning",
            MessageVariant::Danger => "Error",
        };

        div()
            .flex()
            .items_center()
            .gap_2()
            .pl(px(theme.spacing.x3))
            .pr(px(theme.spacing.x3))
            .py(px(theme.spacing.x2))
            .rounded(px(theme.radius.sm))
            .border_l_2()
            .border_color(accent)
            .bg(accent.opacity(0.1))
            .text_size(px(theme.typography.text_sm))
            .text_color(accent)
            .child(
                Icon::new(self.icon_name())
                    .size(14.0)
                    .color(accent)
                    .label(label),
            )
            .child(div().text_color(theme.foreground()).child(self.text))
    }
}
