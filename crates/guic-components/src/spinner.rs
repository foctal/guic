use crate::ComponentSize;
use gpui::{
    Animation, AnimationExt as _, App, Hsla, IntoElement, RenderOnce, Styled as _, Window, div,
    ease_in_out, px,
};
use guic_tokens::Theme;
use std::time::Duration;

/// A lightweight animated loading spinner.
#[derive(gpui::IntoElement)]
pub struct Spinner {
    size: ComponentSize,
    color: Option<Hsla>,
}

impl Spinner {
    /// Creates a new spinner.
    pub fn new() -> Self {
        Self {
            size: ComponentSize::Medium,
            color: None,
        }
    }

    /// Sets the spinner size.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets the spinner color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = match self.size {
            ComponentSize::Small => px(12.),
            ComponentSize::Medium => px(16.),
            ComponentSize::Large => px(20.),
        };
        let color = self.color.unwrap_or_else(|| Theme::global(cx).primary());

        div()
            .w(size)
            .h(size)
            .rounded_full()
            .border_2()
            .border_color(color.opacity(0.25))
            .bg(color.opacity(0.08))
            .with_animation(
                "spinner-pulse",
                Animation::new(Duration::from_millis(900))
                    .repeat()
                    .with_easing(ease_in_out),
                |this: gpui::Div, delta| this.opacity(0.45 + (0.55 * delta)),
            )
    }
}
