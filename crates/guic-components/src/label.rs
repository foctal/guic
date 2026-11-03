use gpui::{
    App, IntoElement, ParentElement as _, Pixels, RenderOnce, SharedString, Styled as _,
    StyledText, Window, div,
};
use guic_tokens::Theme;

/// A simple text label.
#[derive(gpui::IntoElement)]
pub struct Label {
    text: SharedString,
    secondary: Option<SharedString>,
    muted: bool,
    secondary_text_size: Option<Pixels>,
}

impl Label {
    /// Creates a new label.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            secondary: None,
            muted: false,
            secondary_text_size: None,
        }
    }

    /// Appends a secondary text fragment.
    pub fn secondary(mut self, text: impl Into<SharedString>) -> Self {
        self.secondary = Some(text.into());
        self
    }

    /// Sets secondary text size independently. By default it inherits the main size.
    #[must_use]
    pub fn secondary_text_size(mut self, size: Pixels) -> Self {
        if f32::from(size).is_finite() && size > gpui::px(0.) {
            self.secondary_text_size = Some(size);
        }
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
            let mut secondary = div()
                .ml_1()
                .text_color(theme.muted_foreground())
                .child(StyledText::new(secondary));
            if let Some(size) = self.secondary_text_size {
                secondary = secondary.text_size(size);
            }
            root = root.child(secondary);
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, InteractiveElement as _, Render, TestAppContext, px};

    struct Harness;
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .text_size(px(32.))
                .line_height(gpui::relative(1.5))
                .flex()
                .flex_col()
                .child(
                    div()
                        .debug_selector(|| "inherited-label".into())
                        .child(Label::new("Main").secondary("Detail")),
                )
                .child(
                    div().debug_selector(|| "compact-label".into()).child(
                        Label::new("Main")
                            .secondary("Detail")
                            .secondary_text_size(px(12.)),
                    ),
                )
        }
    }
    #[gpui::test]
    fn secondary_typography_can_change_without_shrinking_primary_text(cx: &mut TestAppContext) {
        cx.update(guic_tokens::init);
        let (_, cx) = cx.add_window_view(|_, _| Harness);
        let inherited = cx.debug_bounds("inherited-label").expect("inherited label");
        let compact = cx.debug_bounds("compact-label").expect("compact label");
        assert_eq!(inherited.size.height, px(96.));
        assert_eq!(compact.size.height, px(66.));
    }
}
