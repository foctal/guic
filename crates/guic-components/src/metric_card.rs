use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use guic_tokens::Theme;

use crate::Label;

/// A summarized metric card for dashboards and gallery layouts.
///
/// # Example
///
/// ```no_run
/// use guic_components::MetricCard;
///
/// let card = MetricCard::new("Components", "22").detail("v0.1 baseline");
/// ```
#[derive(gpui::IntoElement)]
pub struct MetricCard {
    title: SharedString,
    value: SharedString,
    detail: Option<SharedString>,
}

impl MetricCard {
    /// Creates a new metric card.
    #[must_use]
    pub fn new(title: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            detail: None,
        }
    }

    /// Sets the supporting detail line.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for MetricCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        div()
            .min_w(px(180.0))
            .p_4()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new(self.title).muted(true))
            .child(
                div()
                    .text_size(px(theme.typography.text_lg))
                    .text_color(theme.foreground())
                    .child(self.value),
            )
            .child(self.detail.map_or_else(
                || div().into_any_element(),
                |detail| Label::new(detail).muted(true).into_any_element(),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::MetricCard;

    #[test]
    fn metric_card_builds() {
        let _ = MetricCard::new("Components", "22").detail("v0.1 baseline");
    }
}
