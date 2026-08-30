use gpui::{App, IntoElement, RenderOnce, Styled as _, Window, div, px};
use guic_tokens::Theme;

/// The axis of a separator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SeparatorAxis {
    /// A horizontal separator.
    #[default]
    Horizontal,
    /// A vertical separator.
    Vertical,
}

/// A simple line separator.
#[derive(gpui::IntoElement)]
pub struct Separator {
    axis: SeparatorAxis,
}

impl Separator {
    /// Creates a new horizontal separator.
    pub fn new() -> Self {
        Self {
            axis: SeparatorAxis::Horizontal,
        }
    }

    /// Creates a horizontal separator.
    pub fn horizontal() -> Self {
        Self::new()
    }

    /// Creates a vertical separator.
    pub fn vertical() -> Self {
        Self {
            axis: SeparatorAxis::Vertical,
        }
    }
}

impl Default for Separator {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);

        match self.axis {
            SeparatorAxis::Horizontal => div().w_full().h(px(1.)).bg(theme.border()),
            SeparatorAxis::Vertical => div().h_full().w(px(1.)).bg(theme.border()),
        }
    }
}
