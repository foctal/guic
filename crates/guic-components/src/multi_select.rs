use crate::{BoolHandler, ComponentSize, IndexHandler, SelectItem, Tag};
use gpui::{
    App, ClickEvent, Empty, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// A controlled multi-selection dropdown.
///
/// `MultiSelect` mirrors [`Select`](crate::Select) but allows several options to
/// be active at once. Selected options render as chips in the trigger, and each
/// dropdown row toggles its membership. It is host-managed: supply the
/// [`MultiSelect::selected`] indices and the [`MultiSelect::expanded`] flag, then
/// react to [`MultiSelect::on_toggle`] and [`MultiSelect::on_select`] (which
/// reports the index whose membership should flip).
///
/// # Example
///
/// ```no_run
/// use guic_components::{MultiSelect, SelectItem};
///
/// let is_open = false;
/// MultiSelect::new("labels")
///     .items(vec![SelectItem::new("bug", "Bug"), SelectItem::new("docs", "Docs")])
///     .selected(vec![0])
///     .expanded(is_open)
///     .on_toggle(|expanded, _, _| { /* store */ })
///     .on_select(|index, _, _| { /* flip membership */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct MultiSelect {
    id: SharedString,
    items: Vec<SelectItem>,
    selected: Vec<usize>,
    placeholder: SharedString,
    expanded: bool,
    disabled: bool,
    size: ComponentSize,
    on_toggle: Option<BoolHandler>,
    on_select: Option<IndexHandler>,
}

impl MultiSelect {
    /// Creates a new multi-select component.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: Vec::new(),
            placeholder: "Select options".into(),
            expanded: false,
            disabled: false,
            size: ComponentSize::Medium,
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

    /// Sets the selected option indices.
    #[must_use]
    pub fn selected(mut self, selected: Vec<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the placeholder shown when nothing is selected.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
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

    /// Registers an expanded-state toggle handler.
    #[must_use]
    pub fn on_toggle(mut self, on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// Registers a handler invoked with the index whose membership should flip.
    #[must_use]
    pub fn on_select(
        mut self,
        on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }

    fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }
}

impl RenderOnce for MultiSelect {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (min_height, text_size) = match self.size {
            ComponentSize::Small => (px(30.0), px(theme.typography.text_sm)),
            ComponentSize::Medium => (px(36.0), px(theme.typography.text_md)),
            ComponentSize::Large => (px(44.0), px(theme.typography.text_lg)),
        };

        let mut chips = div().flex_1().flex().flex_wrap().items_center().gap_1();
        if self.selected.is_empty() {
            chips = chips.child(
                div()
                    .text_size(text_size)
                    .text_color(theme.muted_foreground())
                    .child(self.placeholder.clone()),
            );
        } else {
            for index in &self.selected {
                if let Some(item) = self.items.get(*index) {
                    chips = chips.child(Tag::new(item.label.clone()).size(ComponentSize::Small));
                }
            }
        }

        let trigger = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(self.placeholder.clone())
                    .expanded(self.expanded)
                    .disabled(self.disabled),
            )
            .debug_selector(|| format!("guic-multi-select-trigger-{}", self.id))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .min_h(min_height)
            .px(px(theme.spacing.x3))
            .py(px(theme.spacing.x1))
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .hover({
                let hover = theme.secondary().opacity(0.22);
                move |style: gpui::StyleRefinement| style.bg(hover)
            })
            .child(chips)
            .child(
                Icon::new(if self.expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .color(theme.muted_foreground()),
            );

        let mut root = div().w_full().flex().flex_col().gap_2();
        root = if self.disabled {
            root.child(trigger.opacity(0.55))
        } else if let Some(on_toggle) = self.on_toggle.clone() {
            let next = !self.expanded;
            let keyboard_handler = on_toggle.clone();
            root.child(
                trigger
                    .tab_index(0)
                    .key_context("GuicMultiSelect")
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            (keyboard_handler)(&next, window, cx);
                            cx.stop_propagation();
                        }
                    })
                    .on_click(move |_event: &ClickEvent, window, cx| {
                        (on_toggle)(&next, window, cx)
                    }),
            )
        } else {
            root.child(trigger)
        };

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
                .flex_col()
                .p_1();

            let enabled_count = self.items.iter().filter(|item| !item.disabled).count();
            let mut enabled_position = 0;
            for (index, item) in self.items.iter().enumerate() {
                let selected = self.is_selected(index);
                let row = div()
                    .id(SharedString::from(format!("{}-item-{index}", self.id)))
                    .accessibility(
                        AccessibilityProps::new(Role::Option)
                            .label(item.label.clone())
                            .selected(selected)
                            .disabled(item.disabled),
                    )
                    .debug_selector(move || format!("guic-multi-select-item-{index}"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px(px(theme.spacing.x3))
                    .py(px(theme.spacing.x2))
                    .rounded(px(theme.radius.sm))
                    .text_size(text_size)
                    .text_color(if item.disabled {
                        theme.muted_foreground()
                    } else {
                        theme.foreground()
                    })
                    .child(item.label.clone())
                    .child(if selected {
                        Icon::new(IconName::CheckCircle)
                            .size(14.0)
                            .color(theme.primary())
                            .into_any_element()
                    } else {
                        Empty.into_any_element()
                    });

                menu = if item.disabled {
                    menu.child(row.opacity(0.5))
                } else if let Some(on_select) = self.on_select.clone() {
                    let position = enabled_position;
                    enabled_position += 1;
                    let keyboard_handler = on_select.clone();
                    menu.child(
                        row.tab_index(0)
                            .key_context("GuicMultiSelectOption")
                            .hover({
                                let hover = theme.secondary().opacity(0.24);
                                move |style: gpui::StyleRefinement| style.bg(hover)
                            })
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let handled =
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        (keyboard_handler)(&index, window, cx);
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
    use super::MultiSelect;
    use crate::SelectItem;

    #[test]
    fn tracks_membership() {
        let select = MultiSelect::new("m")
            .items(vec![SelectItem::new("a", "A"), SelectItem::new("b", "B")])
            .selected(vec![1]);
        assert!(!select.is_selected(0));
        assert!(select.is_selected(1));
    }
}
