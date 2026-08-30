use crate::ComponentSize;
use gpui::{
    App, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div,
    px, white,
};
use guic_tokens::Theme;

/// Visual badge variants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BadgeVariant {
    /// Neutral badge styling.
    #[default]
    Neutral,
    /// Primary badge styling.
    Primary,
    /// Success badge styling.
    Success,
    /// Warning badge styling.
    Warning,
    /// Danger badge styling.
    Danger,
}

/// A compact badge for short status text.
#[derive(gpui::IntoElement)]
pub struct Badge {
    text: SharedString,
    variant: BadgeVariant,
    size: ComponentSize,
}

impl Badge {
    /// Creates a new badge.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            variant: BadgeVariant::Neutral,
            size: ComponentSize::Medium,
        }
    }

    /// Sets the badge variant.
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Applies the primary variant.
    pub fn primary(mut self) -> Self {
        self.variant = BadgeVariant::Primary;
        self
    }

    /// Applies the success variant.
    pub fn success(mut self) -> Self {
        self.variant = BadgeVariant::Success;
        self
    }

    /// Applies the warning variant.
    pub fn warning(mut self) -> Self {
        self.variant = BadgeVariant::Warning;
        self
    }

    /// Applies the danger variant.
    pub fn danger(mut self) -> Self {
        self.variant = BadgeVariant::Danger;
        self
    }

    /// Sets the badge size.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    fn colors(&self, theme: &Theme) -> (Hsla, Hsla) {
        match self.variant {
            BadgeVariant::Neutral => (theme.secondary(), theme.foreground()),
            BadgeVariant::Primary => (theme.primary(), white()),
            BadgeVariant::Success => (theme.success(), white()),
            BadgeVariant::Warning => (theme.warning(), white()),
            BadgeVariant::Danger => (theme.danger(), white()),
        }
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (bg, fg) = self.colors(theme);
        let (px_x, px_y, text_size) = match self.size {
            ComponentSize::Small => (px(6.), px(2.), px(10.)),
            ComponentSize::Medium => (px(8.), px(3.), px(11.)),
            ComponentSize::Large => (px(10.), px(4.), px(12.)),
        };

        div()
            .px(px_x)
            .py(px_y)
            .rounded_full()
            .bg(bg)
            .text_color(fg)
            .text_size(text_size)
            .child(self.text)
    }
}
