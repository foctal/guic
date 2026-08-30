use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px, relative,
};
use guic_tokens::Theme;

/// The orientation of a [`Splitter`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SplitterAxis {
    /// Places panes next to each other.
    #[default]
    Horizontal,
    /// Places panes above and below each other.
    Vertical,
}

/// A controlled two-pane layout with a visible divider.
#[derive(gpui::IntoElement)]
pub struct Splitter {
    id: SharedString,
    axis: SplitterAxis,
    fraction: f32,
    first: AnyElement,
    second: AnyElement,
}

impl Splitter {
    /// Creates a horizontal splitter with an equal initial division.
    #[must_use]
    pub fn new(
        id: impl Into<SharedString>,
        first: impl IntoElement,
        second: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            axis: SplitterAxis::Horizontal,
            fraction: 0.5,
            first: first.into_any_element(),
            second: second.into_any_element(),
        }
    }

    /// Sets the layout orientation.
    #[must_use]
    pub fn axis(mut self, axis: SplitterAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Sets the first pane fraction, clamped to keep both panes usable.
    ///
    /// Non-finite fractions are ignored.
    #[must_use]
    pub fn fraction(mut self, fraction: f32) -> Self {
        if fraction.is_finite() {
            self.fraction = fraction.clamp(0.1, 0.9);
        }
        self
    }
}

impl RenderOnce for Splitter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let divider = match self.axis {
            SplitterAxis::Horizontal => div().w(px(1.)).h_full(),
            SplitterAxis::Vertical => div().h(px(1.)).w_full(),
        }
        .bg(theme.border());
        let first = match self.axis {
            SplitterAxis::Horizontal => div().w(relative(self.fraction)).h_full(),
            SplitterAxis::Vertical => div().h(relative(self.fraction)).w_full(),
        }
        .overflow_hidden()
        .child(self.first);
        let second = match self.axis {
            SplitterAxis::Horizontal => div().flex_1().h_full(),
            SplitterAxis::Vertical => div().flex_1().w_full(),
        }
        .overflow_hidden()
        .child(self.second);
        let root = div().id(self.id).size_full().flex();
        match self.axis {
            SplitterAxis::Horizontal => root.flex_row().child(first).child(divider).child(second),
            SplitterAxis::Vertical => root.flex_col().child(first).child(divider).child(second),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Splitter;
    use gpui::div;

    #[test]
    fn clamps_pane_fraction() {
        assert_eq!(
            Splitter::new("split", div(), div()).fraction(2.).fraction,
            0.9
        );
        assert_eq!(
            Splitter::new("split", div(), div())
                .fraction(f32::NAN)
                .fraction,
            0.5
        );
    }
}
