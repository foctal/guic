use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use guic_tokens::Theme;

use crate::{Badge, BadgeVariant, Label, Separator};

/// A key/value item displayed by [`PropertyList`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyItem {
    label: SharedString,
    value: SharedString,
    badge: Option<(SharedString, BadgeVariant)>,
}

impl PropertyItem {
    /// Creates a property item with a label and value.
    #[must_use]
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            badge: None,
        }
    }

    /// Attaches an optional badge to the property row.
    #[must_use]
    pub fn badge(mut self, label: impl Into<SharedString>, variant: BadgeVariant) -> Self {
        self.badge = Some((label.into(), variant));
        self
    }
}

/// A grouped key/value widget for runtime diagnostics and inspector-style layouts.
///
/// # Example
///
/// ```no_run
/// use guic_components::{PropertyItem, PropertyList};
///
/// let widget = PropertyList::new("Runtime").items(vec![
///     PropertyItem::new("Theme", "DefaultDark"),
///     PropertyItem::new("Platform", "macOS"),
/// ]);
/// ```
#[derive(gpui::IntoElement)]
pub struct PropertyList {
    title: SharedString,
    items: Vec<PropertyItem>,
}

impl PropertyList {
    /// Creates a new property list.
    #[must_use]
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
        }
    }

    /// Replaces the property items.
    #[must_use]
    pub fn items(mut self, items: Vec<PropertyItem>) -> Self {
        self.items = items;
        self
    }
}

impl RenderOnce for PropertyList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut content = div()
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.secondary().opacity(0.18))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new(self.title).muted(true))
            .child(Separator::new());

        for item in self.items {
            let mut row = div()
                .flex()
                .justify_between()
                .items_center()
                .gap_4()
                .child(Label::new(item.label).muted(true))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(Label::new(item.value)),
                );

            if let Some((label, variant)) = item.badge {
                row = row.child(Badge::new(label).variant(variant));
            }

            content = content.child(row);
        }

        content
    }
}

#[cfg(test)]
mod tests {
    use super::{PropertyItem, PropertyList};
    use crate::BadgeVariant;

    #[test]
    fn property_item_supports_badges() {
        let item = PropertyItem::new("Phase", "Stable").badge("ready", BadgeVariant::Success);
        assert!(item.badge.is_some());
    }

    #[test]
    fn property_list_builds() {
        let _ = PropertyList::new("Runtime").items(vec![PropertyItem::new("Theme", "Dark")]);
    }
}
