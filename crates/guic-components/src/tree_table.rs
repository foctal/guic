use crate::Label;
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A column in a [`TreeTable`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeTableColumn {
    id: SharedString,
    title: SharedString,
    width: Option<u32>,
}

impl TreeTableColumn {
    /// Creates a tree-table column.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            width: None,
        }
    }

    /// Sets an explicit column width in logical pixels.
    #[must_use]
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width.max(48));
        self
    }
}

/// A hierarchical row in a [`TreeTable`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeTableRow {
    id: SharedString,
    cells: Vec<SharedString>,
    children: Vec<TreeTableRow>,
    expanded: bool,
    selected: bool,
}

impl TreeTableRow {
    /// Creates a row from cell values.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, cells: Vec<impl Into<SharedString>>) -> Self {
        Self {
            id: id.into(),
            cells: cells.into_iter().map(Into::into).collect(),
            children: Vec::new(),
            expanded: false,
            selected: false,
        }
    }

    /// Replaces child rows.
    #[must_use]
    pub fn children(mut self, children: Vec<TreeTableRow>) -> Self {
        self.children = children;
        self
    }

    /// Sets whether the row is expanded.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets whether the row is selected.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

/// A controlled tree table combining hierarchical rows and table columns.
#[derive(gpui::IntoElement)]
pub struct TreeTable {
    id: SharedString,
    columns: Vec<TreeTableColumn>,
    rows: Vec<TreeTableRow>,
    empty_message: SharedString,
    on_select: Option<IdHandler>,
    on_toggle: Option<IdHandler>,
}

impl TreeTable {
    /// Creates an empty tree table.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            empty_message: SharedString::from("No rows available"),
            on_select: None,
            on_toggle: None,
        }
    }

    /// Replaces columns.
    #[must_use]
    pub fn columns(mut self, columns: Vec<TreeTableColumn>) -> Self {
        self.columns = columns;
        self
    }

    /// Replaces rows.
    #[must_use]
    pub fn rows(mut self, rows: Vec<TreeTableRow>) -> Self {
        self.rows = rows;
        self
    }

    /// Sets empty-state text.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Registers a row selection handler.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Registers a branch toggle handler.
    #[must_use]
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Returns visible row identifiers in render order.
    #[must_use]
    pub fn visible_row_ids(&self) -> Vec<SharedString> {
        let mut ids = Vec::new();
        collect_visible_tree_table_ids(&self.rows, &mut ids);
        ids
    }
}

impl RenderOnce for TreeTable {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let root_id = self.id.clone();
        let mut root = div()
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .overflow_hidden()
            .flex()
            .flex_col();

        let mut header = div()
            .w_full()
            .flex()
            .items_center()
            .bg(theme.secondary().opacity(0.24))
            .border_b_1()
            .border_color(theme.border());
        for column in &self.columns {
            let mut cell = div()
                .px_3()
                .py_2()
                .text_color(theme.muted_foreground())
                .child(column.title.clone());
            cell = if let Some(width) = column.width {
                cell.w(px(width as f32))
            } else {
                cell.flex_1()
            };
            header = header.child(cell);
        }
        root = root.child(header);

        if self.rows.is_empty() {
            return div()
                .id(root_id)
                .accessibility(AccessibilityProps::new(Role::Table))
                .child(
                    root.child(
                        div()
                            .p_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Label::new(self.empty_message).muted(true)),
                    ),
                )
                .into_any_element();
        }

        for row in self.rows {
            root = render_tree_table_row(
                root,
                row,
                0,
                &self.columns,
                self.on_select.clone(),
                self.on_toggle.clone(),
                theme,
            );
        }

        div()
            .id(root_id)
            .accessibility(AccessibilityProps::new(Role::Table))
            .child(root)
            .into_any_element()
    }
}

fn render_tree_table_row(
    mut root: gpui::Div,
    row: TreeTableRow,
    depth: usize,
    columns: &[TreeTableColumn],
    on_select: Option<IdHandler>,
    on_toggle: Option<IdHandler>,
    theme: &Theme,
) -> gpui::Div {
    let has_children = !row.children.is_empty();
    let row_id = row.id.clone();
    let row_label = row.cells.first().cloned().unwrap_or_else(|| row.id.clone());
    let mut line = div()
        .id(row.id.clone())
        .accessibility(
            AccessibilityProps::new(Role::Row)
                .label(row_label)
                .selected(row.selected)
                .expanded(row.expanded),
        )
        .debug_selector({
            let id = row.id.clone();
            move || format!("guic-tree-table-row-{id}")
        })
        .w_full()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(theme.border())
        .bg(if row.selected {
            theme.primary().opacity(0.08)
        } else {
            theme.background()
        });

    for (index, column) in columns.iter().enumerate() {
        let mut cell = div().px_3().py_2().text_color(theme.foreground());
        cell = if let Some(width) = column.width {
            cell.w(px(width as f32))
        } else {
            cell.flex_1()
        };
        if index == 0 {
            let disclosure = if has_children {
                let icon = Icon::new(if row.expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(14.0)
                .color(theme.muted_foreground());
                if let Some(on_toggle) = on_toggle.clone() {
                    let toggle_id = row.id.clone();
                    let keyboard_id = toggle_id.clone();
                    let keyboard_handler = on_toggle.clone();
                    div()
                        .id(format!("guic-tree-table-toggle-{}", toggle_id))
                        .accessibility(
                            AccessibilityProps::new(Role::Button)
                                .label(format!("Toggle {}", toggle_id))
                                .expanded(row.expanded),
                        )
                        .tab_index(0)
                        .key_context("GuicTreeTableToggle")
                        .cursor_pointer()
                        .child(icon)
                        .on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                keyboard_handler(&keyboard_id, window, cx);
                                cx.stop_propagation();
                            }
                        })
                        .on_click(move |_event: &ClickEvent, window, cx| {
                            on_toggle(&toggle_id, window, cx);
                            cx.stop_propagation();
                        })
                        .into_any_element()
                } else {
                    icon.into_any_element()
                }
            } else {
                div().w(px(14.0)).into_any_element()
            };
            cell = cell.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl(px(depth as f32 * theme.spacing.x4))
                    .child(disclosure)
                    .child(row.cells.get(index).cloned().unwrap_or_default()),
            );
        } else {
            cell = cell.child(row.cells.get(index).cloned().unwrap_or_default());
        }
        line = line.child(cell);
    }

    if on_select.is_some() || (has_children && on_toggle.is_some()) {
        let keyboard_select = on_select.clone();
        let keyboard_toggle = on_toggle.clone();
        let keyboard_id = row_id.clone();
        line = line
            .tab_index(0)
            .key_context("GuicTreeTableRow")
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let handled = match event.keystroke.key.as_str() {
                    "enter" | "space" => {
                        if let Some(handler) = keyboard_select.as_ref() {
                            handler(&keyboard_id, window, cx);
                            true
                        } else {
                            false
                        }
                    }
                    "left" | "right" if has_children => {
                        if let Some(handler) = keyboard_toggle.as_ref() {
                            handler(&keyboard_id, window, cx);
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
            });
    }

    root = if let Some(on_select) = on_select.clone() {
        root.child(
            line.cursor_pointer()
                .on_click(move |_event: &ClickEvent, window, cx| {
                    on_select(&row_id, window, cx);
                }),
        )
    } else {
        root.child(line)
    };

    if row.expanded {
        for child in row.children {
            root = render_tree_table_row(
                root,
                child,
                depth + 1,
                columns,
                on_select.clone(),
                on_toggle.clone(),
                theme,
            );
        }
    }
    root
}

fn collect_visible_tree_table_ids(rows: &[TreeTableRow], ids: &mut Vec<SharedString>) {
    for row in rows {
        ids.push(row.id.clone());
        if row.expanded {
            collect_visible_tree_table_ids(&row.children, ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TreeTable, TreeTableColumn, TreeTableRow};

    #[test]
    fn tree_table_reports_visible_rows() {
        let table = TreeTable::new("files")
            .columns(vec![TreeTableColumn::new("name", "Name")])
            .rows(vec![
                TreeTableRow::new("src", vec!["src"])
                    .expanded(true)
                    .children(vec![TreeTableRow::new("main", vec!["main.rs"])]),
            ]);

        assert_eq!(
            table
                .visible_row_ids()
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
            vec!["src", "main"]
        );
    }
}
