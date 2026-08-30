use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};
use guic_tokens::Theme;

/// A surface container that groups related content with an elevated panel.
///
/// A `Card` provides an optional title/subtitle header, a body that accepts
/// arbitrary children, and an optional footer (typically actions). It is the
/// canonical building block for dashboards and settings layouts.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Button, Card, Label};
///
/// Card::new()
///     .title("Usage")
///     .subtitle("Last 30 days")
///     .child(Label::new("1,204 requests"))
///     .footer(Button::new("View report"));
/// ```
#[derive(gpui::IntoElement)]
pub struct Card {
    title: Option<SharedString>,
    subtitle: Option<SharedString>,
    header_actions: Option<AnyElement>,
    body: Vec<AnyElement>,
    footer: Option<AnyElement>,
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    /// Creates a new, empty card.
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            subtitle: None,
            header_actions: None,
            body: Vec::new(),
            footer: None,
        }
    }

    /// Sets the card title.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the card subtitle, rendered beneath the title.
    #[must_use]
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Sets a trailing element in the header (for example, an action button).
    #[must_use]
    pub fn header_actions(mut self, actions: impl IntoElement) -> Self {
        self.header_actions = Some(actions.into_any_element());
        self
    }

    /// Appends a child element to the card body.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// Sets the card footer, typically containing actions.
    #[must_use]
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let has_header = self.title.is_some() || self.header_actions.is_some();

        let mut root = div()
            .flex()
            .flex_col()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .shadow_sm()
            .overflow_hidden();

        if has_header {
            let mut heading = div().flex().flex_col().gap_0p5();
            if let Some(title) = self.title {
                heading = heading.child(
                    div()
                        .text_size(px(theme.typography.text_md))
                        .text_color(theme.foreground())
                        .child(title),
                );
            }
            if let Some(subtitle) = self.subtitle {
                heading = heading.child(
                    div()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(subtitle),
                );
            }

            let mut header = div()
                .flex()
                .items_start()
                .justify_between()
                .gap_3()
                .px(px(theme.spacing.x4))
                .py(px(theme.spacing.x3))
                .border_b_1()
                .border_color(theme.border())
                .child(heading);
            if let Some(actions) = self.header_actions {
                header = header.child(actions);
            }
            root = root.child(header);
        }

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(theme.spacing.x3))
            .px(px(theme.spacing.x4))
            .py(px(theme.spacing.x4));
        for child in self.body {
            body = body.child(child);
        }
        root = root.child(body);

        if let Some(footer) = self.footer {
            root = root.child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(theme.spacing.x2))
                    .px(px(theme.spacing.x4))
                    .py(px(theme.spacing.x3))
                    .border_t_1()
                    .border_color(theme.border())
                    .child(footer),
            );
        }

        root
    }
}
