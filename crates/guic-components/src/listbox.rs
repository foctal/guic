use crate::{ComponentSize, SelectItem};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// Selection behavior for a [`Listbox`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ListboxSelectionMode {
    /// Exactly one item may be selected at a time.
    #[default]
    Single,
    /// Multiple items may be selected.
    Multiple,
}

/// A controlled list selection surface.
///
/// The application supplies the selected item indices and updates them from
/// [`Listbox::on_selection_change`]. Disabled items are rendered but never emit
/// selection requests.
#[derive(gpui::IntoElement)]
pub struct Listbox {
    id: SharedString,
    items: Vec<SelectItem>,
    selected: Vec<usize>,
    selection_mode: ListboxSelectionMode,
    disabled: bool,
    size: ComponentSize,
    on_selection_change: Option<SelectionHandler>,
}

type SelectionHandler = Rc<dyn Fn(&Vec<usize>, &mut Window, &mut App)>;

impl Listbox {
    /// Creates an empty listbox.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: Vec::new(),
            selection_mode: ListboxSelectionMode::Single,
            disabled: false,
            size: ComponentSize::Medium,
            on_selection_change: None,
        }
    }

    /// Replaces the listbox items.
    #[must_use]
    pub fn items(mut self, items: Vec<SelectItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets the selected item indices.
    ///
    /// Indices are normalized when rendered, so this builder can be called
    /// before or after [`Listbox::items`] and [`Listbox::selection_mode`].
    #[must_use]
    pub fn selected(mut self, selected: Vec<usize>) -> Self {
        selected.into_iter().for_each(|index| {
            if !self.selected.contains(&index) {
                self.selected.push(index);
            }
        });
        self
    }

    /// Sets the selection behavior.
    #[must_use]
    pub fn selection_mode(mut self, selection_mode: ListboxSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Sets whether the entire listbox is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the row size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Registers a handler for a requested selection update.
    #[must_use]
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(&Vec<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Listbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut selected_indices = self
            .selected
            .iter()
            .copied()
            .filter(|index| *index < self.items.len())
            .collect::<Vec<_>>();
        if self.selection_mode == ListboxSelectionMode::Single {
            selected_indices.truncate(1);
        }
        let selection_mode = self.selection_mode;
        let (row_padding, text_size) = match self.size {
            ComponentSize::Small => (px(theme.spacing.x2), px(theme.typography.text_sm)),
            ComponentSize::Medium => (px(theme.spacing.x3), px(theme.typography.text_md)),
            ComponentSize::Large => (px(theme.spacing.x4), px(theme.typography.text_lg)),
        };
        let mut root = div()
            .id(self.id)
            .accessibility(AccessibilityProps::new(Role::ListBox))
            .w_full()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .overflow_hidden()
            .flex()
            .flex_col();

        let enabled_count = self
            .items
            .iter()
            .filter(|item| !self.disabled && !item.disabled)
            .count();
        let mut enabled_position = 0;
        for (index, item) in self.items.into_iter().enumerate() {
            let selected = selected_indices.contains(&index);
            let label = item.label.clone();
            let row = div()
                .id(format!("guic-listbox-item-{index}"))
                .accessibility(
                    AccessibilityProps::new(Role::Option)
                        .label(label)
                        .selected(selected),
                )
                .debug_selector(|| format!("guic-listbox-item-{index}"))
                .px(px(theme.spacing.x4))
                .py(row_padding)
                .text_size(text_size)
                .text_color(if item.disabled {
                    theme.muted_foreground()
                } else if selected {
                    theme.primary()
                } else {
                    theme.foreground()
                })
                .bg(if selected {
                    theme.primary().opacity(0.12)
                } else {
                    theme.background()
                })
                .hover({
                    let hover = theme.secondary().opacity(0.28);
                    move |style: gpui::StyleRefinement| style.bg(hover)
                })
                .child(item.label);
            root = if self.disabled || item.disabled {
                root.child(row.opacity(0.5))
            } else if let Some(handler) = self.on_selection_change.clone() {
                let position = enabled_position;
                enabled_position += 1;
                let selection = match selection_mode {
                    ListboxSelectionMode::Single => vec![index],
                    ListboxSelectionMode::Multiple => {
                        let mut selection = selected_indices.clone();
                        if let Some(position) = selection
                            .iter()
                            .position(|selected_index| *selected_index == index)
                        {
                            selection.remove(position);
                        } else {
                            selection.push(index);
                            selection.sort_unstable();
                        }
                        selection
                    }
                };
                let keyboard_handler = handler.clone();
                let keyboard_selection = selection.clone();
                root.child(
                    row.tab_index(0)
                        .key_context("GuicListboxOption")
                        .on_key_down(move |event: &KeyDownEvent, window, cx| {
                            let handled =
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    (keyboard_handler)(&keyboard_selection, window, cx);
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
                            (handler)(&selection, window, cx)
                        }),
                )
            } else {
                root.child(row)
            };
        }
        root.opacity(if self.disabled { 0.55 } else { 1.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::{Listbox, ListboxSelectionMode};
    use crate::SelectItem;

    #[test]
    fn single_selection_is_normalized() {
        let listbox = Listbox::new("languages")
            .items(vec![
                SelectItem::new("rust", "Rust"),
                SelectItem::new("go", "Go"),
            ])
            .selection_mode(ListboxSelectionMode::Single)
            .selected(vec![1, 0]);
        assert_eq!(listbox.selected, vec![1, 0]);
    }

    #[test]
    fn builder_order_does_not_discard_selection() {
        let selected_first = Listbox::new("languages").selected(vec![1]).items(vec![
            SelectItem::new("rust", "Rust"),
            SelectItem::new("go", "Go"),
        ]);
        assert_eq!(selected_first.selected, vec![1]);

        let mode_last = Listbox::new("languages")
            .selected(vec![0, 1])
            .selection_mode(ListboxSelectionMode::Multiple);
        assert_eq!(mode_last.selected, vec![0, 1]);
    }
}
