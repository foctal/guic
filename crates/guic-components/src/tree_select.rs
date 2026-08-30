use crate::{Label, Tag, TagVariant};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// A selectable tree option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeSelectNode {
    id: SharedString,
    label: SharedString,
    children: Vec<TreeSelectNode>,
    expanded: bool,
    disabled: bool,
}

impl TreeSelectNode {
    /// Creates a leaf tree-select node.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            expanded: false,
            disabled: false,
        }
    }

    /// Replaces child nodes.
    #[must_use]
    pub fn children(mut self, children: Vec<TreeSelectNode>) -> Self {
        self.children = children;
        self
    }

    /// Sets whether the branch is expanded.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets whether the node is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A controlled select trigger backed by a tree option list.
#[derive(gpui::IntoElement)]
pub struct TreeSelect {
    id: SharedString,
    nodes: Vec<TreeSelectNode>,
    selected_id: Option<SharedString>,
    placeholder: SharedString,
    expanded: bool,
    disabled: bool,
    on_toggle: Option<ToggleHandler>,
    on_select: Option<IdHandler>,
}

impl TreeSelect {
    /// Creates an empty tree select.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            nodes: Vec::new(),
            selected_id: None,
            placeholder: SharedString::from("Select an item"),
            expanded: false,
            disabled: false,
            on_toggle: None,
            on_select: None,
        }
    }

    /// Replaces option nodes.
    #[must_use]
    pub fn nodes(mut self, nodes: Vec<TreeSelectNode>) -> Self {
        self.nodes = nodes;
        self
    }

    /// Sets the selected node id.
    #[must_use]
    pub fn selected(mut self, selected_id: impl Into<SharedString>) -> Self {
        self.selected_id = Some(selected_id.into());
        self
    }

    /// Sets placeholder text.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets whether the dropdown is expanded.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets whether the select is disabled.
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

    /// Registers a node selection handler.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns the selected node label, if the id exists.
    #[must_use]
    pub fn selected_label(&self) -> Option<SharedString> {
        self.selected_id
            .as_ref()
            .and_then(|id| find_tree_select_label(&self.nodes, id))
    }
}

impl RenderOnce for TreeSelect {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let selected_label = self
            .selected_id
            .as_ref()
            .and_then(|id| find_tree_select_label(&self.nodes, id));
        let trigger_label = selected_label
            .clone()
            .unwrap_or_else(|| self.placeholder.clone());
        let mut root = div().w_full().flex().flex_col().gap_2();
        let trigger = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(self.placeholder.clone())
                    .expanded(self.expanded)
                    .disabled(self.disabled),
            )
            .debug_selector(|| format!("guic-tree-select-trigger-{}", self.id))
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
            .text_color(if selected_label.is_some() {
                theme.foreground()
            } else {
                theme.muted_foreground()
            })
            .child(trigger_label)
            .child(Icon::new(IconName::ChevronDown).color(theme.muted_foreground()));

        root = if self.disabled {
            root.child(trigger.opacity(0.55))
        } else if let Some(on_toggle) = self.on_toggle.clone() {
            let next = !self.expanded;
            let keyboard_handler = on_toggle.clone();
            root.child(
                trigger
                    .tab_index(0)
                    .key_context("GuicTreeSelect")
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
            let mut roving_position = (0, visible_enabled_node_count(&self.nodes));
            let mut panel = div()
                .w_full()
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .bg(theme.background())
                .shadow_lg()
                .p_1()
                .flex()
                .flex_col();
            for node in self.nodes {
                panel = render_tree_select_node(
                    panel,
                    node,
                    0,
                    self.selected_id.as_ref(),
                    self.on_select.clone(),
                    &mut roving_position,
                    theme,
                );
            }
            root = root.child(
                div()
                    .id(format!("{}-tree", self.id))
                    .accessibility(
                        AccessibilityProps::new(Role::Tree).label(self.placeholder.clone()),
                    )
                    .child(panel),
            );
        }

        root
    }
}

fn render_tree_select_node(
    mut panel: gpui::Div,
    node: TreeSelectNode,
    depth: usize,
    selected_id: Option<&SharedString>,
    on_select: Option<IdHandler>,
    roving_position: &mut (usize, usize),
    theme: &Theme,
) -> gpui::Div {
    let selected = selected_id == Some(&node.id);
    let has_children = !node.children.is_empty();
    let id = node.id.clone();
    let label = node.label.clone();
    let mut row = div()
        .id(node.id.clone())
        .accessibility(
            AccessibilityProps::new(Role::TreeItem)
                .label(label)
                .selected(selected)
                .expanded(node.expanded)
                .disabled(node.disabled),
        )
        .debug_selector({
            let id = node.id.clone();
            move || format!("guic-tree-select-node-{id}")
        })
        .flex()
        .items_center()
        .gap_2()
        .px(px(theme.spacing.x3 + depth as f32 * theme.spacing.x3))
        .py_2()
        .rounded(px(theme.radius.sm))
        .text_color(if node.disabled {
            theme.muted_foreground()
        } else if selected {
            theme.primary()
        } else {
            theme.foreground()
        })
        .bg(if selected {
            theme.secondary().opacity(0.3)
        } else {
            theme.background()
        })
        .child(if has_children {
            Icon::new(if node.expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .size(14.0)
            .color(theme.muted_foreground())
            .into_any_element()
        } else {
            div().w(px(14.0)).into_any_element()
        })
        .child(Label::new(node.label.clone()));

    if selected {
        row = row.child(Tag::new("Selected").variant(TagVariant::Info));
    }

    panel = if node.disabled {
        panel.child(row.opacity(0.55))
    } else if let Some(handler) = on_select.clone() {
        let position = roving_position.0;
        roving_position.0 += 1;
        let enabled_count = roving_position.1;
        let keyboard_handler = handler.clone();
        let keyboard_id = id.clone();
        panel.child(
            row.tab_index(0)
                .key_context("GuicTreeSelectItem")
                .cursor_pointer()
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let handled = if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        keyboard_handler(&keyboard_id, window, cx);
                        true
                    } else {
                        crate::handle_roving_focus_key(event, position, enabled_count, window, cx)
                    };
                    if handled {
                        cx.stop_propagation();
                    }
                })
                .on_click(move |_event: &ClickEvent, window, cx| {
                    handler(&id, window, cx);
                }),
        )
    } else {
        panel.child(row)
    };

    if node.expanded {
        for child in node.children {
            panel = render_tree_select_node(
                panel,
                child,
                depth + 1,
                selected_id,
                on_select.clone(),
                roving_position,
                theme,
            );
        }
    }
    panel
}

fn visible_enabled_node_count(nodes: &[TreeSelectNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            usize::from(!node.disabled)
                + if node.expanded {
                    visible_enabled_node_count(&node.children)
                } else {
                    0
                }
        })
        .sum()
}

fn find_tree_select_label(nodes: &[TreeSelectNode], id: &SharedString) -> Option<SharedString> {
    for node in nodes {
        if &node.id == id {
            return Some(node.label.clone());
        }
        if let Some(label) = find_tree_select_label(&node.children, id) {
            return Some(label);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{TreeSelect, TreeSelectNode};

    #[test]
    fn tree_select_finds_selected_label() {
        let select = TreeSelect::new("project")
            .nodes(vec![
                TreeSelectNode::new("src", "src")
                    .children(vec![TreeSelectNode::new("main", "main.rs")]),
            ])
            .selected("main");
        assert_eq!(select.selected_label().as_deref(), Some("main.rs"));
    }
}
