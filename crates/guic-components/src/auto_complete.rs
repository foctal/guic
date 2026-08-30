use crate::{SelectItem, TextInput};
use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role, ValueChange};
use guic_tokens::Theme;
use std::rc::Rc;

type SelectHandler = Rc<dyn Fn(&SelectItem, &mut Window, &mut App)>;

/// A searchable suggestion input with a filtered result list.
pub struct AutoComplete {
    id: SharedString,
    input: Entity<TextInput>,
    items: Vec<SelectItem>,
    query: String,
    max_results: usize,
    empty_message: SharedString,
    on_select: Option<SelectHandler>,
    _subscription: Subscription,
}

impl AutoComplete {
    /// Creates an autocomplete input.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let id = id.into();
        let input_id = id.clone();
        let input = cx.new(|cx| TextInput::search(input_id, cx));
        let subscription = cx.subscribe(&input, |this, _, event: &ValueChange<String>, cx| {
            this.query = event.next.clone();
            cx.notify();
        });
        Self {
            id,
            input,
            items: Vec::new(),
            query: String::new(),
            max_results: 20,
            empty_message: "No matching suggestions".into(),
            on_select: None,
            _subscription: subscription,
        }
    }
    /// Replaces available suggestions.
    #[must_use]
    pub fn items(mut self, items: Vec<SelectItem>) -> Self {
        self.items = items;
        self
    }

    /// Limits the number of rendered suggestions.
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = max_results.max(1);
        self
    }

    /// Sets the result-list empty-state message.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Registers a selection callback.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&SelectItem, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

fn ranked_matches(items: &[SelectItem], query: &str, limit: usize) -> Vec<SelectItem> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut matches = items
        .iter()
        .enumerate()
        .filter(|(_, item)| !item.disabled)
        .filter_map(|(index, item)| {
            let label = item.label.to_lowercase();
            let position = label.find(&query)?;
            Some(((position > 0, position, index), item.clone()))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(score, _)| *score);
    matches
        .into_iter()
        .take(limit)
        .map(|(_, item)| item)
        .collect()
}

impl Render for AutoComplete {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let query = self.query.trim();
        let matches = ranked_matches(&self.items, query, self.max_results);
        let match_count = matches.len();
        let mut root = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.input.clone());
        if !query.is_empty() {
            let mut menu = div()
                .id(format!("{}-suggestions", self.id))
                .accessibility(
                    AccessibilityProps::new(Role::ListBox).label("Autocomplete suggestions"),
                )
                .w_full()
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .bg(theme.background())
                .shadow_lg()
                .flex()
                .flex_col();
            if matches.is_empty() {
                menu = menu.child(
                    div()
                        .px(px(theme.spacing.x4))
                        .py(px(theme.spacing.x3))
                        .text_color(theme.muted_foreground())
                        .child(self.empty_message.clone()),
                );
            }
            for (index, item) in matches.into_iter().enumerate() {
                let handler = self.on_select.clone();
                let callback_item = item.clone();
                menu = menu.child(
                    div()
                        .id(item.id.clone())
                        .accessibility(
                            AccessibilityProps::new(Role::Option).label(item.label.clone()),
                        )
                        .debug_selector(move || format!("guic-autocomplete-result-{index}"))
                        .tab_index(0)
                        .key_context("GuicAutoCompleteOption")
                        .px(px(theme.spacing.x4))
                        .py(px(theme.spacing.x3))
                        .text_color(theme.foreground())
                        .cursor_pointer()
                        .hover({
                            let hover = theme.secondary().opacity(0.25);
                            move |style: gpui::StyleRefinement| style.bg(hover)
                        })
                        .child(item.label)
                        .on_key_down({
                            let handler = handler.clone();
                            let callback_item = callback_item.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                let handled =
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        if let Some(handler) = handler.as_ref() {
                                            handler(&callback_item, window, cx);
                                        }
                                        true
                                    } else {
                                        crate::handle_roving_focus_key(
                                            event,
                                            index,
                                            match_count,
                                            window,
                                            cx,
                                        )
                                    };
                                if handled {
                                    cx.stop_propagation();
                                }
                            }
                        })
                        .on_click(move |_, window, cx| {
                            if let Some(handler) = handler.as_ref() {
                                handler(&callback_item, window, cx);
                            }
                        }),
                );
            }
            root = root.child(menu);
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::{SelectItem, ranked_matches};

    #[test]
    fn suggestions_prioritize_prefixes_and_preserve_stable_order() {
        let items = vec![
            SelectItem::new("one", "Create project"),
            SelectItem::new("two", "Project settings"),
            SelectItem::new("three", "Project files"),
            SelectItem::new("disabled", "Project disabled").disabled(true),
        ];

        let matches = ranked_matches(&items, "project", 10);
        assert_eq!(
            matches
                .iter()
                .map(|item| item.id.as_ref())
                .collect::<Vec<_>>(),
            vec!["two", "three", "one"]
        );
    }

    #[test]
    fn suggestions_are_bounded_and_ignore_blank_queries() {
        let items = (0..100)
            .map(|index| SelectItem::new(index.to_string(), format!("Item {index}")))
            .collect::<Vec<_>>();
        assert_eq!(ranked_matches(&items, "item", 12).len(), 12);
        assert!(ranked_matches(&items, " ", 12).is_empty());
        assert!(ranked_matches(&items, "item", 0).is_empty());
    }
}
