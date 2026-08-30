use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role, ValueChange};
use guic_tokens::Theme;

use crate::{TextInput, text_input::Newline};

type CommandHandler = Rc<dyn Fn(&CommandPaletteItem, &mut Window, &mut App)>;
type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A command displayed by [`CommandPalette`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandPaletteItem {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    shortcut: Option<SharedString>,
    keywords: Vec<SharedString>,
    disabled: bool,
}

impl CommandPaletteItem {
    /// Creates a command.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            shortcut: None,
            keywords: Vec::new(),
            disabled: false,
        }
    }

    /// Sets supporting text.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the displayed keyboard shortcut.
    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Replaces additional searchable keywords.
    #[must_use]
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Sets whether the command is unavailable.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the stable command identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the displayed command title.
    #[must_use]
    pub fn title(&self) -> &SharedString {
        &self.title
    }
}

/// A searchable, keyboard-operated command launcher.
pub struct CommandPalette {
    id: SharedString,
    input: Entity<TextInput>,
    focus_handle: FocusHandle,
    query: String,
    items: Vec<CommandPaletteItem>,
    active_index: usize,
    max_results: usize,
    empty_message: SharedString,
    on_activate: Option<CommandHandler>,
    on_dismiss: Option<DismissHandler>,
    _subscription: Subscription,
}

impl CommandPalette {
    /// Creates a command palette and its search input.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let id = id.into();
        let input = cx.new(|cx| {
            TextInput::search(format!("{id}-search"), cx)
                .placeholder("Type a command")
                .accessible_label("Command search")
        });
        let focus_handle = input.read(cx).focus_handle(cx);
        let subscription = cx.subscribe(&input, |this, _, event: &ValueChange<String>, cx| {
            this.query = event.next.clone();
            this.active_index = 0;
            cx.notify();
        });
        Self {
            id,
            input,
            focus_handle,
            query: String::new(),
            items: Vec::new(),
            active_index: 0,
            max_results: 20,
            empty_message: "No matching commands".into(),
            on_activate: None,
            on_dismiss: None,
            _subscription: subscription,
        }
    }

    /// Replaces available commands.
    #[must_use]
    pub fn items(mut self, items: Vec<CommandPaletteItem>) -> Self {
        self.items = items;
        self
    }

    /// Limits visible matches.
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = max_results.max(1);
        self
    }

    /// Sets the empty-result message.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Registers a command activation handler.
    #[must_use]
    pub fn on_activate(
        mut self,
        handler: impl Fn(&CommandPaletteItem, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Registers an `Escape` dismissal handler.
    #[must_use]
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    /// Returns the search field focus handle.
    #[must_use]
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn matching_indices(&self) -> Vec<usize> {
        ranked_command_indices(&self.items, &self.query, self.max_results)
    }

    fn move_active(&mut self, direction: isize, cx: &mut Context<Self>) {
        let len = self.matching_indices().len();
        if len == 0 {
            self.active_index = 0;
        } else if direction > 0 {
            self.active_index = (self.active_index + 1) % len;
        } else {
            self.active_index = (self.active_index + len - 1) % len;
        }
        cx.notify();
    }

    fn activate(&self, window: &mut Window, cx: &mut Context<Self>) {
        let matches = self.matching_indices();
        let Some(item) = matches
            .get(self.active_index)
            .and_then(|index| self.items.get(*index))
        else {
            return;
        };
        if let Some(handler) = self.on_activate.as_ref() {
            handler(item, window, cx);
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let handled = match event.keystroke.key.as_str() {
            "down" => {
                self.move_active(1, cx);
                true
            }
            "up" => {
                self.move_active(-1, cx);
                true
            }
            "enter" => {
                self.activate(window, cx);
                true
            }
            "escape" => {
                if let Some(handler) = self.on_dismiss.as_ref() {
                    handler(window, cx);
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
    }

    fn handle_submit(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.activate(window, cx);
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let matches = self.matching_indices();
        if self.active_index >= matches.len() {
            self.active_index = 0;
        }

        let mut results = div()
            .id(format!("{}-results", self.id))
            .accessibility(AccessibilityProps::new(Role::ListBox).label("Matching commands"))
            .w_full()
            .flex()
            .flex_col()
            .gap_1();
        if matches.is_empty() {
            results = results.child(
                div()
                    .px_3()
                    .py_4()
                    .text_color(theme.muted_foreground())
                    .child(self.empty_message.clone()),
            );
        }
        for (result_index, item_index) in matches.into_iter().enumerate() {
            let item = self.items[item_index].clone();
            let active = result_index == self.active_index;
            let handler = self.on_activate.clone();
            let callback_item = item.clone();
            let mut leading = div().flex().flex_col().gap_1().child(item.title.clone());
            if let Some(description) = item.description {
                leading = leading.child(
                    div()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(description),
                );
            }
            let mut row = div()
                .id(format!("{}-command-{}", self.id, item.id))
                .debug_selector(move || format!("guic-command-palette-item-{result_index}"))
                .accessibility(
                    AccessibilityProps::new(Role::Option)
                        .label(item.title.clone())
                        .selected(active)
                        .disabled(item.disabled),
                )
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_3()
                .py_2()
                .rounded(px(theme.radius.sm))
                .bg(if active {
                    theme.secondary().opacity(0.35)
                } else {
                    theme.background()
                })
                .text_color(if item.disabled {
                    theme.muted_foreground()
                } else {
                    theme.foreground()
                })
                .child(leading);
            if let Some(shortcut) = item.shortcut {
                row = row.child(
                    div()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(shortcut),
                );
            }
            if !item.disabled {
                row = row.cursor_pointer().on_click(move |_, window, cx| {
                    if let Some(handler) = handler.as_ref() {
                        handler(&callback_item, window, cx);
                    }
                });
            }
            results = results.child(row);
        }

        div()
            .id(self.id.clone())
            .key_context("GuicCommandPalette")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key))
            .on_action(cx.listener(Self::handle_submit))
            .accessibility(AccessibilityProps::new(Role::Dialog).label("Command palette"))
            .w_full()
            .max_w(px(640.0))
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .shadow_lg()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.input.clone())
            .child(results)
    }
}

fn ranked_command_indices(items: &[CommandPaletteItem], query: &str, limit: usize) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut ranked = items
        .iter()
        .enumerate()
        .filter(|(_, item)| !item.disabled)
        .filter_map(|(index, item)| {
            if query.is_empty() {
                return Some(((false, 0, index), index));
            }
            let title = item.title.to_lowercase();
            let haystacks = std::iter::once(title.as_str())
                .chain(item.keywords.iter().map(|keyword| keyword.as_ref()));
            let score = haystacks
                .filter_map(|haystack| haystack.to_lowercase().find(&query))
                .map(|position| (position != 0, position))
                .min()?;
            Some(((score.0, score.1, index), index))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, _)| *score);
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, index)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{AppContext as _, Keystroke, SharedString, TestAppContext, VisualContext as _};

    use super::{CommandPalette, CommandPaletteItem, ranked_command_indices};

    #[test]
    fn ranking_prefers_prefixes_and_keywords() {
        let items = vec![
            CommandPaletteItem::new("settings", "Open settings"),
            CommandPaletteItem::new("project", "Project settings"),
            CommandPaletteItem::new("theme", "Change theme").keywords(["appearance", "settings"]),
            CommandPaletteItem::new("disabled", "Settings disabled").disabled(true),
        ];

        assert_eq!(
            ranked_command_indices(&items, "settings", 10),
            vec![2, 0, 1]
        );
    }

    #[test]
    fn blank_query_preserves_order_and_limit() {
        let items = (0..10)
            .map(|index| CommandPaletteItem::new(index.to_string(), format!("Command {index}")))
            .collect::<Vec<_>>();
        assert_eq!(ranked_command_indices(&items, "", 3), vec![0, 1, 2]);
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    fn arrow_keys_and_enter_activate_the_highlighted_command(cx: &mut TestAppContext) {
        init(cx);
        let activated = Rc::new(RefCell::new(Vec::<SharedString>::new()));
        let callback = activated.clone();
        let (palette, cx) = cx.add_window_view(|_, cx| {
            CommandPalette::new("commands", cx)
                .items(vec![
                    CommandPaletteItem::new("open", "Open"),
                    CommandPaletteItem::new("save", "Save"),
                ])
                .on_activate(move |item, _, _| {
                    callback.borrow_mut().push(item.id().clone());
                })
        });
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| {
            palette.update(cx, |palette, cx| {
                palette.focus_handle().focus(window, cx);
            });
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("keystroke parses"));
        cx.dispatch_keystroke(window, Keystroke::parse("enter").expect("keystroke parses"));

        assert_eq!(activated.borrow().as_slice(), ["save"]);
    }

    #[gpui::test]
    fn escape_requests_dismissal(cx: &mut TestAppContext) {
        init(cx);
        let dismissed = Rc::new(RefCell::new(false));
        let callback = dismissed.clone();
        let (palette, cx) = cx.add_window_view(|_, cx| {
            CommandPalette::new("commands", cx).on_dismiss(move |_, _| {
                *callback.borrow_mut() = true;
            })
        });
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| {
            palette.update(cx, |palette, cx| {
                palette.focus_handle().focus(window, cx);
            });
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("escape").expect("keystroke parses"),
        );

        assert!(*dismissed.borrow());
    }
}
