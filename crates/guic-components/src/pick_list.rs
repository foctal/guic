use crate::SelectItem;
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

type SelectionHandler = Rc<dyn Fn(&Vec<usize>, &mut Window, &mut App)>;

/// A controlled two-pane picker for building an ordered subset of options.
///
/// Supply all options and the selected indices. Clicking a row moves that item
/// between the available and selected panes by emitting the next selection.
#[derive(gpui::IntoElement)]
pub struct PickList {
    id: SharedString,
    items: Vec<SelectItem>,
    selected: Vec<usize>,
    available_label: SharedString,
    selected_label: SharedString,
    on_change: Option<SelectionHandler>,
}

impl PickList {
    /// Creates an empty pick list.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: Vec::new(),
            available_label: "Available".into(),
            selected_label: "Selected".into(),
            on_change: None,
        }
    }
    /// Replaces the candidate items.
    #[must_use]
    pub fn items(mut self, items: Vec<SelectItem>) -> Self {
        self.items = items;
        self
    }
    /// Sets the selected item indices, preserving their supplied order.
    #[must_use]
    pub fn selected(mut self, selected: Vec<usize>) -> Self {
        self.selected = selected
            .into_iter()
            .filter(|index| *index < self.items.len())
            .fold(Vec::new(), |mut values, index| {
                if !values.contains(&index) {
                    values.push(index);
                }
                values
            });
        self
    }
    /// Sets the available-pane label.
    #[must_use]
    pub fn available_label(mut self, label: impl Into<SharedString>) -> Self {
        self.available_label = label.into();
        self
    }
    /// Sets the selected-pane label.
    #[must_use]
    pub fn selected_label(mut self, label: impl Into<SharedString>) -> Self {
        self.selected_label = label.into();
        self
    }
    /// Registers a callback receiving the next selected indices.
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(&Vec<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PickList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let selected = self.selected.clone();
        let root_id = self.id.clone();
        let render_pane = |label: SharedString, indices: Vec<usize>, selected_pane: bool| {
            let mut pane = div()
                .id(format!(
                    "{}-{}-pane",
                    self.id,
                    if selected_pane {
                        "selected"
                    } else {
                        "available"
                    }
                ))
                .accessibility(AccessibilityProps::new(Role::ListBox).label(label.clone()))
                .flex_1()
                .min_w(px(160.))
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .bg(theme.secondary().opacity(0.25))
                        .child(label),
                );
            if indices.is_empty() {
                pane = pane.child(
                    div()
                        .px_3()
                        .py_4()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child("No items"),
                );
            }
            let enabled_count = indices
                .iter()
                .filter(|index| !self.items[**index].disabled)
                .count();
            let mut enabled_position = 0;
            for index in indices {
                let item = &self.items[index];
                let mut next = selected.clone();
                if selected_pane {
                    next.retain(|value| *value != index);
                } else {
                    next.push(index);
                }
                let row = div()
                    .id(format!(
                        "{}-{}-{index}",
                        self.id,
                        if selected_pane {
                            "selected"
                        } else {
                            "available"
                        }
                    ))
                    .accessibility(
                        AccessibilityProps::new(Role::Option)
                            .label(item.label.clone())
                            .selected(selected_pane)
                            .disabled(item.disabled),
                    )
                    .debug_selector({
                        let selector = format!(
                            "guic-pick-list-{}-{index}",
                            if selected_pane {
                                "selected"
                            } else {
                                "available"
                            }
                        );
                        move || selector.clone()
                    })
                    .px_3()
                    .py_2()
                    .text_color(if item.disabled {
                        theme.muted_foreground()
                    } else {
                        theme.foreground()
                    })
                    .hover({
                        let hover = theme.secondary().opacity(0.3);
                        move |style: gpui::StyleRefinement| style.bg(hover)
                    })
                    .child(item.label.clone());
                pane = if item.disabled {
                    pane.child(row.opacity(0.55))
                } else if let Some(handler) = self.on_change.clone() {
                    let position = enabled_position;
                    enabled_position += 1;
                    let keyboard_handler = handler.clone();
                    let keyboard_next = next.clone();
                    pane.child(
                        row.tab_index(0)
                            .key_context("GuicPickListOption")
                            .cursor_pointer()
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let handled =
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        keyboard_handler(&keyboard_next, window, cx);
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
                                handler(&next, window, cx)
                            }),
                    )
                } else {
                    pane.child(row)
                };
            }
            pane
        };
        let available = (0..self.items.len())
            .filter(|index| !selected.contains(index))
            .collect();
        div()
            .id(root_id)
            .w_full()
            .flex()
            .gap_3()
            .flex_wrap()
            .child(render_pane(self.available_label, available, false))
            .child(render_pane(self.selected_label, selected.clone(), true))
    }
}

#[cfg(test)]
mod tests {
    use super::PickList;
    use crate::SelectItem;
    #[test]
    fn selection_is_deduplicated_and_bounded() {
        let picker = PickList::new("members")
            .items(vec![SelectItem::new("a", "Ada")])
            .selected(vec![0, 0, 3]);
        assert_eq!(picker.selected, vec![0]);
    }
}
