use crate::{Button, ButtonVariant, ComponentSize, Label, Tag, TagVariant};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px,
};
use guic_tokens::Theme;
use std::rc::Rc;

type ItemHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Layout mode for [`DataView`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DataViewLayout {
    /// Dense vertical list layout.
    #[default]
    List,
    /// Card-based responsive grid layout.
    Grid,
}

/// A display item for [`DataView`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataViewItem {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    metadata: Option<SharedString>,
    badge: Option<SharedString>,
    disabled: bool,
}

impl DataViewItem {
    /// Creates a new data-view item.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            metadata: None,
            badge: None,
            disabled: false,
        }
    }

    /// Sets supporting description text.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets compact metadata text.
    #[must_use]
    pub fn metadata(mut self, metadata: impl Into<SharedString>) -> Self {
        self.metadata = Some(metadata.into());
        self
    }

    /// Sets a trailing badge.
    #[must_use]
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Marks the item as disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the stable item identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// A controlled collection view for card or list presentation.
///
/// `DataView` is useful when records need a richer presentation than a table
/// row but still need selection, empty states, and repeatable item structure.
#[derive(gpui::IntoElement)]
pub struct DataView {
    id: SharedString,
    items: Vec<DataViewItem>,
    layout: DataViewLayout,
    selected_id: Option<SharedString>,
    empty_message: SharedString,
    on_select: Option<ItemHandler>,
}

impl DataView {
    /// Creates an empty data view.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            layout: DataViewLayout::List,
            selected_id: None,
            empty_message: SharedString::from("No items available"),
            on_select: None,
        }
    }

    /// Replaces the items rendered by the view.
    #[must_use]
    pub fn items(mut self, items: Vec<DataViewItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets the presentation layout.
    #[must_use]
    pub fn layout(mut self, layout: DataViewLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Sets the selected item id.
    #[must_use]
    pub fn selected(mut self, selected_id: impl Into<SharedString>) -> Self {
        self.selected_id = Some(selected_id.into());
        self
    }

    /// Sets the empty-state message.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Registers a handler for requested item selection.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DataView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let selected_id = self.selected_id.clone();
        let root = div()
            .id(self.id)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(theme.spacing.x3));

        if self.items.is_empty() {
            return root
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .bg(theme.secondary().opacity(0.08))
                .p_4()
                .child(Label::new(self.empty_message).muted(true))
                .into_any_element();
        }

        let mut collection = match self.layout {
            DataViewLayout::List => div().w_full().flex().flex_col().gap_2(),
            DataViewLayout::Grid => div().w_full().grid().grid_cols(2).gap(px(theme.spacing.x3)),
        };

        for item in self.items {
            let selected = selected_id.as_ref() == Some(&item.id);
            collection = collection.child(render_data_view_item(
                item,
                selected,
                self.on_select.clone(),
                theme,
            ));
        }

        root.child(collection).into_any_element()
    }
}

fn render_data_view_item(
    item: DataViewItem,
    selected: bool,
    on_select: Option<ItemHandler>,
    theme: &Theme,
) -> gpui::AnyElement {
    let foreground = if item.disabled {
        theme.muted_foreground()
    } else {
        theme.foreground()
    };
    let id = item.id.clone();
    let mut surface = div()
        .id(item.id.clone())
        .debug_selector({
            let id = item.id.clone();
            move || format!("guic-data-view-item-{id}")
        })
        .w_full()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(if selected {
            theme.primary()
        } else {
            theme.border()
        })
        .bg(if selected {
            theme.primary().opacity(0.08)
        } else {
            theme.background()
        })
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .text_color(foreground);

    let mut title_row = div().flex().items_start().justify_between().gap_3().child(
        div()
            .text_size(px(theme.typography.text_md))
            .child(item.title),
    );
    if let Some(badge) = item.badge {
        title_row = title_row.child(Tag::new(badge).variant(TagVariant::Info));
    }
    surface = surface.child(title_row);

    if let Some(description) = item.description {
        surface = surface.child(Label::new(description).muted(true));
    }
    if let Some(metadata) = item.metadata {
        surface = surface.child(
            div()
                .text_size(px(theme.typography.text_sm))
                .text_color(theme.muted_foreground())
                .child(metadata),
        );
    }

    if item.disabled {
        surface.opacity(0.55).into_any_element()
    } else if let Some(on_select) = on_select {
        surface
            .cursor_pointer()
            .hover({
                let hover = theme.secondary().opacity(0.28);
                move |style: gpui::StyleRefinement| style.bg(hover)
            })
            .child(
                Button::new("Select")
                    .variant(ButtonVariant::Ghost)
                    .size(ComponentSize::Small)
                    .on_click(move |event: &ClickEvent, window, cx| {
                        let _ = event;
                        on_select(&id, window, cx);
                    }),
            )
            .into_any_element()
    } else {
        surface.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{DataView, DataViewItem, DataViewLayout};

    #[test]
    fn data_view_item_builder_preserves_metadata() {
        let item = DataViewItem::new("runtime", "Runtime")
            .description("Core systems")
            .metadata("Updated today")
            .badge("Preview")
            .disabled(true);

        assert_eq!(item.id(), "runtime");
        assert_eq!(item.description.as_deref(), Some("Core systems"));
        assert_eq!(item.metadata.as_deref(), Some("Updated today"));
        assert_eq!(item.badge.as_deref(), Some("Preview"));
        assert!(item.disabled);
    }

    #[test]
    fn data_view_builder_tracks_layout_and_selection() {
        let view = DataView::new("catalog")
            .items(vec![DataViewItem::new("one", "One")])
            .layout(DataViewLayout::Grid)
            .selected("one")
            .empty_message("Nothing here");

        assert_eq!(view.layout, DataViewLayout::Grid);
        assert_eq!(view.selected_id.as_deref(), Some("one"));
        assert_eq!(view.empty_message, "Nothing here");
    }
}
