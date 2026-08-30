use crate::SelectItem;
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

type PathHandler = Rc<dyn Fn(&Vec<usize>, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// One option in a [`CascadeSelect`] hierarchy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CascadeOption {
    item: SelectItem,
    children: Vec<CascadeOption>,
}

impl CascadeOption {
    /// Creates an option without children.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            item: SelectItem::new(id, label),
            children: Vec::new(),
        }
    }

    /// Marks the option disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.item.disabled = disabled;
        self
    }

    /// Replaces child options.
    #[must_use]
    pub fn children(mut self, children: Vec<CascadeOption>) -> Self {
        self.children = children;
        self
    }

    /// Returns whether this option has child options.
    #[must_use]
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// A controlled cascading select surface.
#[derive(gpui::IntoElement)]
pub struct CascadeSelect {
    id: SharedString,
    options: Vec<CascadeOption>,
    path: Vec<usize>,
    placeholder: SharedString,
    expanded: bool,
    disabled: bool,
    on_toggle: Option<ToggleHandler>,
    on_select: Option<PathHandler>,
}

impl CascadeSelect {
    /// Creates an empty cascading select.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            options: Vec::new(),
            path: Vec::new(),
            placeholder: SharedString::from("Select an option"),
            expanded: false,
            disabled: false,
            on_toggle: None,
            on_select: None,
        }
    }

    /// Replaces root options.
    #[must_use]
    pub fn options(mut self, options: Vec<CascadeOption>) -> Self {
        self.options = options;
        self
    }

    /// Sets the selected path.
    #[must_use]
    pub fn path(mut self, path: Vec<usize>) -> Self {
        self.path = path;
        self
    }

    /// Sets placeholder text.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets whether the panel is expanded.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets whether the control is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Registers an expanded-state toggle handler.
    #[must_use]
    pub fn on_toggle(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Registers a path selection handler.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&Vec<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns labels along the current path.
    #[must_use]
    pub fn selected_labels(&self) -> Vec<SharedString> {
        labels_for_path(&self.options, &self.path)
    }
}

impl RenderOnce for CascadeSelect {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let labels = labels_for_path(&self.options, &self.path);
        let trigger_label = if labels.is_empty() {
            self.placeholder.clone()
        } else {
            labels
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(" / ")
                .into()
        };
        let mut root = div().w_full().flex().flex_col().gap_2();
        let trigger = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(self.placeholder.clone())
                    .expanded(self.expanded)
                    .disabled(self.disabled),
            )
            .debug_selector(|| format!("guic-cascade-select-trigger-{}", self.id))
            .w_full()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .text_color(if labels.is_empty() {
                theme.muted_foreground()
            } else {
                theme.foreground()
            })
            .child(trigger_label)
            .child(Icon::new(IconName::ChevronRight).color(theme.muted_foreground()));

        root = if self.disabled {
            root.child(trigger.opacity(0.55))
        } else if let Some(on_toggle) = self.on_toggle.clone() {
            let next = !self.expanded;
            let keyboard_handler = on_toggle.clone();
            root.child(
                trigger
                    .tab_index(0)
                    .key_context("GuicCascadeSelect")
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            keyboard_handler(&next, window, cx);
                            cx.stop_propagation();
                        }
                    })
                    .on_click(move |_event: &ClickEvent, window, cx| {
                        on_toggle(&next, window, cx);
                    }),
            )
        } else {
            root.child(trigger)
        };

        if self.expanded {
            root = root.child(render_cascade_columns(
                &self.options,
                &self.path,
                self.on_select,
                theme,
            ));
        }

        root
    }
}

fn render_cascade_columns(
    options: &[CascadeOption],
    selected_path: &[usize],
    on_select: Option<PathHandler>,
    theme: &Theme,
) -> gpui::AnyElement {
    let mut columns = div()
        .w_full()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(theme.border())
        .bg(theme.background())
        .shadow_lg()
        .flex()
        .items_start()
        .overflow_hidden();

    let mut current = options;
    let mut prefix = Vec::new();
    let mut depth = 0usize;
    loop {
        let selected = selected_path.get(depth).copied();
        let enabled_count = current
            .iter()
            .filter(|option| !option.item.disabled)
            .count();
        let mut enabled_position = 0;
        let mut column = div()
            .min_w(px(180.0))
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme.border());
        for (index, option) in current.iter().enumerate() {
            let mut path = prefix.clone();
            path.push(index);
            let active = selected == Some(index);
            let mut row = div()
                .id(format!("guic-cascade-option-{}", path_to_key(&path)))
                .accessibility(
                    AccessibilityProps::new(Role::Option)
                        .label(option.item.label.clone())
                        .selected(active)
                        .disabled(option.item.disabled),
                )
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .justify_between()
                .text_color(if option.item.disabled {
                    theme.muted_foreground()
                } else if active {
                    theme.primary()
                } else {
                    theme.foreground()
                })
                .bg(if active {
                    theme.secondary().opacity(0.28)
                } else {
                    theme.background()
                })
                .child(option.item.label.clone());
            if option.has_children() {
                row = row.child(
                    Icon::new(IconName::ChevronRight)
                        .size(14.0)
                        .color(theme.muted_foreground()),
                );
            }
            column = if option.item.disabled {
                column.child(row.opacity(0.55))
            } else if let Some(handler) = on_select.clone() {
                let position = enabled_position;
                enabled_position += 1;
                let keyboard_handler = handler.clone();
                let keyboard_path = path.clone();
                column.child(
                    row.tab_index(0)
                        .key_context("GuicCascadeOption")
                        .cursor_pointer()
                        .on_key_down(move |event: &KeyDownEvent, window, cx| {
                            let handled =
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    keyboard_handler(&keyboard_path, window, cx);
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
                            handler(&path, window, cx);
                        }),
                )
            } else {
                column.child(row)
            };
        }
        columns = columns.child(column);

        let Some(next_index) = selected else {
            break;
        };
        let Some(next) = current.get(next_index) else {
            break;
        };
        if next.children.is_empty() {
            break;
        }
        prefix.push(next_index);
        current = &next.children;
        depth += 1;
    }

    columns.into_any_element()
}

fn labels_for_path(options: &[CascadeOption], path: &[usize]) -> Vec<SharedString> {
    let mut labels = Vec::new();
    let mut current = options;
    for index in path {
        let Some(option) = current.get(*index) else {
            break;
        };
        labels.push(option.item.label.clone());
        current = &option.children;
    }
    labels
}

fn path_to_key(path: &[usize]) -> String {
    path.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::{CascadeOption, CascadeSelect};

    #[test]
    fn cascade_select_reports_path_labels() {
        let select = CascadeSelect::new("region")
            .options(vec![
                CascadeOption::new("americas", "Americas")
                    .children(vec![CascadeOption::new("us", "United States")]),
            ])
            .path(vec![0, 0]);
        assert_eq!(
            select
                .selected_labels()
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
            vec!["Americas", "United States"]
        );
    }
}
