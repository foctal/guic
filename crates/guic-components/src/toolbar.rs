use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, Styled as _, Window, div, px,
};
use guic_tokens::Theme;

enum ToolbarItem {
    Element(AnyElement),
    Separator,
    Spacer,
}

/// A horizontal container for grouping actions and controls.
///
/// `Toolbar` lays out children in a bordered row. Use [`Toolbar::separator`] to
/// divide logical groups and [`Toolbar::spacer`] to push subsequent items to
/// the trailing edge.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Button, Toolbar};
///
/// Toolbar::new()
///     .child(Button::new("New"))
///     .child(Button::new("Open"))
///     .separator()
///     .child(Button::new("Save"))
///     .spacer()
///     .child(Button::new("Settings"));
/// ```
#[derive(gpui::IntoElement)]
pub struct Toolbar {
    items: Vec<ToolbarItem>,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Toolbar {
    /// Creates a new, empty toolbar.
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Appends a child element.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.items
            .push(ToolbarItem::Element(child.into_any_element()));
        self
    }

    /// Appends a vertical separator between logical groups.
    #[must_use]
    pub fn separator(mut self) -> Self {
        self.items.push(ToolbarItem::Separator);
        self
    }

    /// Appends a flexible spacer that pushes following items to the trailing edge.
    #[must_use]
    pub fn spacer(mut self) -> Self {
        self.items.push(ToolbarItem::Spacer);
        self
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(theme.spacing.x2))
            .px(px(theme.spacing.x2))
            .py(px(theme.spacing.x1_5))
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background());

        for item in self.items {
            row = match item {
                ToolbarItem::Element(element) => row.child(element),
                ToolbarItem::Separator => {
                    row.child(div().w(px(1.)).h(px(20.)).mx_1().bg(theme.border()))
                }
                ToolbarItem::Spacer => row.child(div().flex_1()),
            };
        }

        row
    }
}
