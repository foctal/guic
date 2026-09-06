use gpui::{Pixels, px};
use guic_tokens::Theme;

/// Shared external dimensions for single-line controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlMetrics {
    /// Outer height, including borders.
    pub height: Pixels,
    /// Text size.
    pub font_size: Pixels,
    /// Horizontal content padding.
    pub horizontal_padding: Pixels,
    /// Icon width and height.
    pub icon_size: Pixels,
    /// Space between content items.
    pub gap: Pixels,
}

/// Shared control sizes: 28, 34, and 42 pixel outer heights.
/// Multiline surfaces use a separate height contract.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ComponentSize {
    /// Compact size.
    Small,
    /// Default size.
    #[default]
    Medium,
    /// Large size.
    Large,
}

impl ComponentSize {
    /// Resolves control geometry and theme-dependent typography and spacing.
    #[must_use]
    pub fn control_metrics(self, theme: &Theme) -> ControlMetrics {
        let (height, font_size, padding, icon_size) = match self {
            Self::Small => (28., theme.typography.text_sm, theme.spacing.x3, 12.),
            Self::Medium => (34., theme.typography.text_md, theme.spacing.x4, 14.),
            Self::Large => (42., theme.typography.text_lg, theme.spacing.x5, 16.),
        };
        ControlMetrics {
            height: px(height),
            font_size: px(font_size),
            horizontal_padding: px(padding),
            icon_size: px(icon_size),
            gap: px(theme.spacing.x2),
        }
    }
}
