use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use guic_tokens::Theme;

/// Immutable content for one event in a [`Timeline`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEvent {
    /// The event heading.
    pub title: SharedString,
    /// Optional supporting text.
    pub description: Option<SharedString>,
    /// Optional timestamp or other compact event metadata.
    pub timestamp: Option<SharedString>,
}

impl TimelineEvent {
    /// Creates an event with a title.
    #[must_use]
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            timestamp: None,
        }
    }

    /// Adds supporting text beneath the title.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds compact event metadata, commonly a timestamp.
    #[must_use]
    pub fn timestamp(mut self, timestamp: impl Into<SharedString>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }
}

/// A vertical, read-only sequence of events.
///
/// The application supplies events in display order. Use this surface for
/// activity history, deployment progress, and audit-style summaries.
#[derive(gpui::IntoElement)]
pub struct Timeline {
    events: Vec<TimelineEvent>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    /// Creates an empty timeline.
    #[must_use]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Replaces the displayed events.
    #[must_use]
    pub fn events(mut self, events: Vec<TimelineEvent>) -> Self {
        self.events = events;
        self
    }
}

impl RenderOnce for Timeline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let total = self.events.len();
        let mut root = div().w_full().flex().flex_col();
        for (index, event) in self.events.into_iter().enumerate() {
            let marker = div()
                .mt(px(5.))
                .size(px(10.))
                .rounded_full()
                .bg(theme.primary());
            let rail = if index + 1 < total {
                div().w(px(2.)).flex_1().my_1().bg(theme.border())
            } else {
                div().w(px(2.)).flex_1()
            };
            let mut heading = div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_color(theme.foreground()).child(event.title));
            if let Some(timestamp) = event.timestamp {
                heading = heading.child(
                    div()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(timestamp),
                );
            }
            let mut content = div()
                .flex_1()
                .pb_5()
                .flex()
                .flex_col()
                .gap_1()
                .child(heading);
            if let Some(description) = event.description {
                content = content.child(
                    div()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(description),
                );
            }
            root = root.child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .w(px(10.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(marker)
                            .child(rail),
                    )
                    .child(content),
            );
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::{Timeline, TimelineEvent};

    #[test]
    fn event_builder_preserves_optional_content() {
        let event = TimelineEvent::new("Deployed")
            .description("Production")
            .timestamp("10:30");
        assert_eq!(event.description.as_deref(), Some("Production"));
        assert_eq!(event.timestamp.as_deref(), Some("10:30"));
        assert_eq!(Timeline::new().events(Vec::new()).events.len(), 0);
    }
}
