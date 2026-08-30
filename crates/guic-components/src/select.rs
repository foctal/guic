use crate::{BoolHandler, ComponentSize, IndexHandler};
use gpui::{
    App, ClickEvent, Empty, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// Immutable option metadata for [`Select`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectItem {
    /// Stable option identifier.
    pub id: SharedString,
    /// User-facing option label.
    pub label: SharedString,
    /// Whether the option is disabled.
    pub disabled: bool,
}

impl SelectItem {
    /// Creates a new option item.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the option as disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A controlled select component with an inline dropdown menu.
#[derive(gpui::IntoElement)]
pub struct Select {
    id: SharedString,
    items: Vec<SelectItem>,
    selected: Option<usize>,
    placeholder: SharedString,
    accessible_label: Option<SharedString>,
    empty_message: SharedString,
    expanded: bool,
    disabled: bool,
    size: ComponentSize,
    focus_handle: Option<FocusHandle>,
    on_toggle: Option<BoolHandler>,
    on_select: Option<IndexHandler>,
}

impl Select {
    /// Creates a new select component.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: None,
            placeholder: "Select an option".into(),
            accessible_label: None,
            empty_message: "No options available".into(),
            expanded: false,
            disabled: false,
            size: ComponentSize::Medium,
            focus_handle: None,
            on_toggle: None,
            on_select: None,
        }
    }

    /// Replaces the option list.
    #[must_use]
    pub fn items(mut self, items: Vec<SelectItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets the selected option index.
    #[must_use]
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the placeholder label.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the control name announced by assistive technologies.
    #[must_use]
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Sets the message rendered when the option list is empty.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Sets the expanded state.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the component size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets an application-owned focus handle for programmatic focus control.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers an expanded-state toggle handler.
    #[must_use]
    pub fn on_toggle(mut self, on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
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

fn next_enabled_index(
    items: &[SelectItem],
    selected: Option<usize>,
    direction: isize,
) -> Option<usize> {
    if items.is_empty() || direction == 0 {
        return selected.filter(|index| items.get(*index).is_some_and(|item| !item.disabled));
    }

    if direction > 0 {
        let start = match selected {
            Some(index) => index.checked_add(1).unwrap_or(items.len()),
            None => 0,
        }
        .min(items.len());
        (start..items.len()).find(|index| !items[*index].disabled)
    } else {
        let start = match selected {
            Some(index) => index.checked_sub(1)?.min(items.len() - 1),
            None => items.len() - 1,
        };
        (0..=start).rev().find(|index| !items[*index].disabled)
    }
}

fn typeahead_index(items: &[SelectItem], selected: Option<usize>, query: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || items.is_empty() {
        return None;
    }
    let start = selected
        .and_then(|index| index.checked_add(1))
        .unwrap_or(0)
        .min(items.len());
    (start..items.len()).chain(0..start).find(|index| {
        let item = &items[*index];
        !item.disabled && item.label.to_lowercase().starts_with(&query)
    })
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (height, text_size) = match self.size {
            ComponentSize::Small => (px(30.0), px(theme.typography.text_sm)),
            ComponentSize::Medium => (px(36.0), px(theme.typography.text_md)),
            ComponentSize::Large => (px(44.0), px(theme.typography.text_lg)),
        };
        let selected_label = self
            .selected
            .and_then(|index| self.items.get(index))
            .map(|item| item.label.clone())
            .unwrap_or_else(|| self.placeholder.clone());
        let accessible_label = self
            .accessible_label
            .clone()
            .unwrap_or_else(|| self.id.clone());

        let mut root = div().w_full().flex().flex_col().gap_2();
        let mut trigger = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(accessible_label)
                    .expanded(self.expanded)
                    .disabled(self.disabled),
            )
            .debug_selector(|| format!("guic-select-trigger-{}", self.id))
            .w_full()
            .h(height)
            .px(px(theme.spacing.x4))
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .text_size(text_size)
            .text_color(if self.selected.is_some() {
                theme.foreground()
            } else {
                theme.muted_foreground()
            })
            .flex()
            .items_center()
            .justify_between()
            .focus_visible({
                let ring = theme.ring();
                move |style| style.border_color(ring)
            })
            .child(selected_label)
            .child(
                Icon::new(if self.expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .color(theme.muted_foreground()),
            );

        let interactive = self.on_toggle.is_some() || self.on_select.is_some();
        if self.disabled {
            trigger = trigger.opacity(0.55);
        } else if interactive {
            let items = self.items.clone();
            let selected = self.selected;
            let expanded = self.expanded;
            let on_toggle = self.on_toggle.clone();
            let on_select = self.on_select.clone();
            trigger = trigger.key_context("GuicSelect").on_key_down(
                move |event: &KeyDownEvent, window, cx| {
                    let handled = match event.keystroke.key.as_str() {
                        "enter" | "space" => {
                            if let Some(handler) = on_toggle.as_ref() {
                                handler(&!expanded, window, cx);
                            }
                            true
                        }
                        "escape" if expanded => {
                            if let Some(handler) = on_toggle.as_ref() {
                                handler(&false, window, cx);
                            }
                            true
                        }
                        "down" | "up" => {
                            let direction = if event.keystroke.key == "down" { 1 } else { -1 };
                            if let (Some(index), Some(handler)) = (
                                next_enabled_index(&items, selected, direction),
                                on_select.as_ref(),
                            ) {
                                handler(&index, window, cx);
                            }
                            true
                        }
                        "home" | "end" => {
                            let direction = if event.keystroke.key == "home" { 1 } else { -1 };
                            if let (Some(index), Some(handler)) = (
                                next_enabled_index(&items, None, direction),
                                on_select.as_ref(),
                            ) {
                                handler(&index, window, cx);
                            }
                            true
                        }
                        _ if !event.keystroke.modifiers.control
                            && !event.keystroke.modifiers.alt
                            && !event.keystroke.modifiers.platform =>
                        {
                            let query = event
                                .keystroke
                                .key_char
                                .as_deref()
                                .unwrap_or(&event.keystroke.key);
                            if let (Some(index), Some(handler)) =
                                (typeahead_index(&items, selected, query), on_select.as_ref())
                            {
                                handler(&index, window, cx);
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    if handled {
                        cx.stop_propagation();
                    }
                },
            );

            trigger = if let Some(handle) = &self.focus_handle {
                trigger.track_focus(handle)
            } else {
                trigger.tab_index(0)
            };

            if let Some(on_toggle) = self.on_toggle.clone() {
                let next = !self.expanded;
                trigger = trigger
                    .cursor_pointer()
                    .hover({
                        let hover = theme.secondary().opacity(0.22);
                        move |style: gpui::StyleRefinement| style.bg(hover)
                    })
                    .on_click(move |_event: &ClickEvent, window, cx| {
                        (on_toggle)(&next, window, cx)
                    });
            }
        }
        root = root.child(trigger);

        if self.expanded {
            let mut menu = div()
                .id(format!("{}-menu", self.id))
                .accessibility(
                    AccessibilityProps::new(Role::ListBox).label(format!("{} options", self.id)),
                )
                .w_full()
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .bg(theme.background())
                .shadow_lg()
                .flex()
                .flex_col();

            if self.items.is_empty() {
                menu = menu.child(
                    div()
                        .px(px(theme.spacing.x4))
                        .py(px(theme.spacing.x3))
                        .text_color(theme.muted_foreground())
                        .child(self.empty_message),
                );
            }

            for (index, item) in self.items.into_iter().enumerate() {
                let row = div()
                    .id(item.id.clone())
                    .accessibility(
                        AccessibilityProps::new(Role::Option)
                            .label(item.label.clone())
                            .selected(Some(index) == self.selected)
                            .disabled(item.disabled),
                    )
                    .debug_selector(|| format!("guic-select-item-{}", index))
                    .px(px(theme.spacing.x4))
                    .py(px(theme.spacing.x3))
                    .text_color(if item.disabled {
                        theme.muted_foreground()
                    } else if Some(index) == self.selected {
                        theme.primary()
                    } else {
                        theme.foreground()
                    })
                    .bg(if Some(index) == self.selected {
                        theme.secondary().opacity(0.35)
                    } else {
                        theme.background()
                    })
                    .child(item.label);

                menu = if item.disabled {
                    menu.child(row.opacity(0.5))
                } else if let Some(on_select) = self.on_select.clone() {
                    menu.child(
                        row.cursor_pointer()
                            .hover({
                                let hover = theme.secondary().opacity(0.24);
                                move |style: gpui::StyleRefinement| style.bg(hover)
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                (on_select)(&index, window, cx)
                            }),
                    )
                } else {
                    menu.child(row)
                };
            }

            root = root.child(menu);
        } else {
            root = root.child(Empty);
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::{SelectItem, next_enabled_index, typeahead_index};

    #[test]
    fn select_item_supports_disabled_state() {
        let item = SelectItem::new("alpha", "Alpha").disabled(true);
        assert!(item.disabled);
    }

    #[test]
    fn keyboard_navigation_skips_disabled_items() {
        let items = vec![
            SelectItem::new("a", "Alpha"),
            SelectItem::new("b", "Beta").disabled(true),
            SelectItem::new("c", "Charlie"),
        ];
        assert_eq!(next_enabled_index(&items, Some(0), 1), Some(2));
        assert_eq!(next_enabled_index(&items, Some(2), -1), Some(0));
        assert_eq!(next_enabled_index(&items, None, 1), Some(0));
        assert_eq!(next_enabled_index(&items, None, -1), Some(2));
        assert_eq!(next_enabled_index(&items, Some(2), 1), None);
        assert_eq!(next_enabled_index(&items, Some(0), -1), None);
        assert_eq!(next_enabled_index(&items, Some(usize::MAX), 1), None);
    }

    #[test]
    fn typeahead_wraps_and_skips_disabled_items() {
        let items = vec![
            SelectItem::new("alpha", "Alpha"),
            SelectItem::new("beta", "Beta").disabled(true),
            SelectItem::new("bravo", "Bravo"),
            SelectItem::new("charlie", "Charlie"),
        ];

        assert_eq!(typeahead_index(&items, None, "b"), Some(2));
        assert_eq!(typeahead_index(&items, Some(2), "a"), Some(0));
        assert_eq!(typeahead_index(&items, Some(0), "  CH"), Some(3));
        assert_eq!(typeahead_index(&items, Some(3), "missing"), None);
        assert_eq!(typeahead_index(&items, None, " "), None);
    }
}
