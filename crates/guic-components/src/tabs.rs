use crate::{ComponentSize, IndexHandler};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// Immutable tab metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabItem {
    /// Stable tab identifier.
    pub id: SharedString,
    /// User-facing tab label.
    pub label: SharedString,
    /// Whether the tab is disabled.
    pub disabled: bool,
}

impl TabItem {
    /// Creates a new tab item.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the tab as disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A simple horizontal tabs component.
#[derive(gpui::IntoElement)]
pub struct Tabs {
    id: SharedString,
    items: Vec<TabItem>,
    selected: usize,
    size: ComponentSize,
    on_select: Option<IndexHandler>,
}

impl Tabs {
    /// Creates a new tabs component.
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

    /// Replaces the tab item list.
    #[must_use]
    pub fn items(mut self, items: Vec<TabItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets the selected tab index.
    #[must_use]
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the tab size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Registers a selection handler.
    #[must_use]
    pub fn on_select(
        mut self,
        on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }
}

fn enabled_tab_from(items: &[TabItem], selected: usize, key: &str) -> Option<usize> {
    match key {
        "home" => items.iter().position(|item| !item.disabled),
        "end" => items.iter().rposition(|item| !item.disabled),
        "left" | "up" => (0..selected)
            .rev()
            .find(|index| !items[*index].disabled)
            .or_else(|| items.iter().rposition(|item| !item.disabled)),
        "right" | "down" => ((selected + 1).min(items.len())..items.len())
            .find(|index| !items[*index].disabled)
            .or_else(|| items.iter().position(|item| !item.disabled)),
        _ => None,
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let height = match self.size {
            ComponentSize::Small => px(28.0),
            ComponentSize::Medium => px(34.0),
            ComponentSize::Large => px(40.0),
        };

        let mut row = div()
            .id(self.id)
            .w_full()
            .flex()
            .gap_1()
            .border_b_1()
            .border_color(theme.border());

        let items = self.items;
        for (ix, item) in items.iter().cloned().enumerate() {
            let selected = ix == self.selected;
            let label = item.label.clone();
            let base = div()
                .px_3()
                .h(height)
                .flex()
                .items_center()
                .justify_center()
                .border_b_2()
                .border_color(if selected {
                    theme.primary()
                } else {
                    theme.background().opacity(0.0)
                })
                .text_color(if item.disabled {
                    theme.muted_foreground()
                } else if selected {
                    theme.primary()
                } else {
                    theme.foreground()
                })
                .bg(if selected {
                    theme.secondary().opacity(0.35)
                } else {
                    theme.background().opacity(0.0)
                })
                .child(item.label);

            row = if item.disabled {
                row.child(base.opacity(0.5))
            } else if let Some(on_select) = self.on_select.clone() {
                row.child(
                    base.id(item.id)
                        .accessibility(
                            AccessibilityProps::new(Role::Tab)
                                .label(label)
                                .selected(selected),
                        )
                        .tab_index(0)
                        .key_context("GuicTabs")
                        .cursor_pointer()
                        .hover(|style: gpui::StyleRefinement| {
                            style.bg(theme.secondary().opacity(0.45))
                        })
                        .on_key_down({
                            let items = items.clone();
                            let on_select = on_select.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                let target = match event.keystroke.key.as_str() {
                                    "enter" | "space" => Some(ix),
                                    key => enabled_tab_from(&items, ix, key),
                                };
                                if let Some(target) = target {
                                    (on_select)(&target, window, cx);
                                    cx.stop_propagation();
                                }
                            }
                        })
                        .on_click(move |_event: &ClickEvent, window, cx| {
                            (on_select)(&ix, window, cx)
                        }),
                )
            } else {
                row.child(base)
            };
        }

        row
    }
}

#[cfg(test)]
mod tests {
    use super::{TabItem, enabled_tab_from};

    #[test]
    fn keyboard_navigation_wraps_and_skips_disabled_tabs() {
        let items = vec![
            TabItem::new("a", "A"),
            TabItem::new("b", "B").disabled(true),
            TabItem::new("c", "C"),
        ];
        assert_eq!(enabled_tab_from(&items, 0, "right"), Some(2));
        assert_eq!(enabled_tab_from(&items, 2, "right"), Some(0));
        assert_eq!(enabled_tab_from(&items, 0, "left"), Some(2));
        assert_eq!(enabled_tab_from(&items, 2, "home"), Some(0));
    }
}
