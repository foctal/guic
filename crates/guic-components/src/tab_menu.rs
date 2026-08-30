use crate::{ComponentSize, IndexHandler, TabItem};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// A compact navigation menu rendered as a pill-shaped tab strip.
///
/// `TabMenu` is controlled: applications supply `selected` and update it in
/// response to [`TabMenu::on_select`]. It is intended for switching views,
/// whereas [`crate::Tabs`] provides the conventional underline tab treatment.
#[derive(gpui::IntoElement)]
pub struct TabMenu {
    id: SharedString,
    items: Vec<TabItem>,
    selected: usize,
    size: ComponentSize,
    on_select: Option<IndexHandler>,
}

impl TabMenu {
    /// Creates an empty tab menu.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: 0,
            size: ComponentSize::Medium,
            on_select: None,
        }
    }
    /// Replaces the navigation items.
    #[must_use]
    pub fn items(mut self, items: Vec<TabItem>) -> Self {
        self.items = items;
        self
    }
    /// Sets the selected item index.
    #[must_use]
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }
    /// Sets the menu size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }
    /// Registers a selection handler.
    #[must_use]
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TabMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let height = match self.size {
            ComponentSize::Small => px(28.),
            ComponentSize::Medium => px(34.),
            ComponentSize::Large => px(40.),
        };
        let mut row = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .rounded(px(theme.radius.md))
            .bg(theme.secondary().opacity(0.35));
        let enabled_count = self.items.iter().filter(|item| !item.disabled).count();
        let mut enabled_position = 0;
        for (index, item) in self.items.into_iter().enumerate() {
            let selected = index == self.selected;
            let cell = div()
                .id(item.id)
                .accessibility(
                    AccessibilityProps::new(Role::Tab)
                        .label(item.label.clone())
                        .selected(selected)
                        .disabled(item.disabled),
                )
                .h(height)
                .px_3()
                .rounded(px(theme.radius.sm))
                .flex()
                .items_center()
                .justify_center()
                .text_color(if item.disabled {
                    theme.muted_foreground()
                } else if selected {
                    theme.foreground()
                } else {
                    theme.muted_foreground()
                })
                .bg(if selected {
                    theme.background()
                } else {
                    theme.background().opacity(0.)
                })
                .child(item.label);
            row = if item.disabled {
                row.child(cell.opacity(0.5))
            } else if let Some(handler) = self.on_select.clone() {
                let position = enabled_position;
                enabled_position += 1;
                row.child(
                    cell.tab_index(0)
                        .key_context("GuicTabMenu")
                        .cursor_pointer()
                        .hover(|style: gpui::StyleRefinement| {
                            style.bg(theme.background().opacity(0.65))
                        })
                        .on_key_down({
                            let handler = handler.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                let handled =
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        (handler)(&index, window, cx);
                                        true
                                    } else {
                                        crate::handle_roving_focus_key(
                                            event,
                                            position,
                                            enabled_count,
                                            window,
                                            cx,
                                        )
                                    };
                                if handled {
                                    cx.stop_propagation();
                                }
                            }
                        })
                        .on_click(move |_event: &ClickEvent, window, cx| {
                            (handler)(&index, window, cx)
                        }),
                )
            } else {
                row.child(cell)
            };
        }
        row
    }
}
