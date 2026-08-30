use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};
use guic_tokens::Theme;

/// A labelled grouping surface for related form controls.
#[derive(gpui::IntoElement)]
pub struct Fieldset {
    legend: Option<SharedString>,
    description: Option<SharedString>,
    children: Vec<AnyElement>,
}
impl Default for Fieldset {
    fn default() -> Self {
        Self::new()
    }
}
impl Fieldset {
    /// Creates an empty fieldset.
    #[must_use]
    pub fn new() -> Self {
        Self {
            legend: None,
            description: None,
            children: Vec::new(),
        }
    }
    /// Sets the fieldset legend.
    #[must_use]
    pub fn legend(mut self, legend: impl Into<SharedString>) -> Self {
        self.legend = Some(legend.into());
        self
    }
    /// Sets supporting description text.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
    /// Appends a grouped control.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}
impl RenderOnce for Fieldset {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div()
            .w_full()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .p_4()
            .flex()
            .flex_col()
            .gap_3();
        if let Some(legend) = self.legend {
            root = root.child(
                div()
                    .text_size(px(theme.typography.text_md))
                    .text_color(theme.foreground())
                    .child(legend),
            );
        }
        if let Some(description) = self.description {
            root = root.child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(theme.muted_foreground())
                    .child(description),
            );
        }
        root.children(self.children)
    }
}
#[cfg(test)]
mod tests {
    use super::Fieldset;
    #[test]
    fn fieldset_accepts_metadata() {
        let fieldset = Fieldset::new()
            .legend("Profile")
            .description("Public details");
        assert_eq!(fieldset.legend.as_deref(), Some("Profile"));
        assert_eq!(fieldset.description.as_deref(), Some("Public details"));
    }
}
