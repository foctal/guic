use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, StyledText,
    Window, div,
};
use guic_tokens::Theme;

/// A simple text label.
#[derive(gpui::IntoElement)]
pub struct Label {
    text: SharedString,
    secondary: Option<SharedString>,
    muted: bool,
}

impl Label {
    /// Creates a new label.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            secondary: None,
            muted: false,
        }
    }

    /// Appends a secondary text fragment.
    pub fn secondary(mut self, text: impl Into<SharedString>) -> Self {
        self.secondary = Some(text.into());
        self
    }

    /// Applies muted foreground styling to the label.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let color = if self.muted {
            theme.muted_foreground()
        } else {
            theme.foreground()
        };

        let mut root = div().text_color(color).child(StyledText::new(self.text));
        if let Some(secondary) = self.secondary {
            root = root.child(
                div()
                    .ml_1()
                    .text_color(theme.muted_foreground())
                    .child(StyledText::new(secondary)),
            );
        }

        root
    }
}
